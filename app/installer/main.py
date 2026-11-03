"""Transactional, explicit panorama-only app installation."""
import argparse
import fcntl
import json
import os
import shutil
import secrets
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

from check import check_mods
from fs import Home, digest
from keys import bindings, decoded, merge, render, undo
from manifest import BASE_FILES, BINARY, MANIFEST, MODS, MOD_DATA_DIRS, parents, validate
import runtime
import frontend
import preferences


def inventory(package, binary):
    files = {}
    for name in sorted(BASE_FILES):
        source = package / name
        # Do not install symlinks or unexpectedly executable mod sources.
        info = source.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            raise ValueError(f'Package source is not a private regular file: {source}')
        files[name] = (source.read_bytes(), 0o644)
    for name in MODS:
        if json.loads(files[f'mods/{name}/mod.json'][0]).get('place') != 'none':
            raise ValueError(f'{name} must declare place none')
    if binary:
        source = Path(binary)
        if not source.is_absolute() or not stat.S_ISREG(source.lstat().st_mode):
            raise ValueError('--binary must name an absolute regular artifact, not a symlink')
        if not source.stat().st_mode & 0o111:
            raise ValueError('--binary artifact must be executable')
        files[BINARY] = (source.read_bytes(), 0o755)
    return files


def preflight(home, files):
    for mod in MODS:
        if home.exists(f'mods/{mod}'):
            raise ValueError(f'Name collision: mods/{mod}')
    for path in [*files, MANIFEST, *(MOD_DATA_DIRS - preferences.DIRECTORIES)]:
        if home.exists(path):
            raise ValueError(f'Name collision: {path}')
    preferences.preflight(home)
    # Walk every existing ancestor now: reject links before any writes.
    for directory in sorted({p for f in [*files, MANIFEST] for p in parents(f)}, key=lambda p: (p.count('/'), p)):
        if home.exists(directory):
            with home.parent(directory + '/.probe'):
                pass


def remove_files(home, manifest, removed=None):
    leftovers = []
    for path, info in reversed(list(manifest['files'].items())):
        try:
            current = home.read(path)
            if current is None:
                continue
            if digest(current[0]) != info['sha256'] or current[1] != info['mode']:
                leftovers.append(path + ' (edited; preserved)')
            elif not home.remove(path, current):
                leftovers.append(path + ' (changed; preserved)')
            elif removed is not None:
                removed[path] = current
        except (OSError, ValueError):
            leftovers.append(path + ' (unsafe or changed; preserved)')
    return leftovers


def remove_dirs(home, dirs):
    leftovers = []
    for path in sorted(dirs, key=lambda p: (p.count('/'), p), reverse=True):
        try:
            home.rmdir(path)
        except FileNotFoundError:
            pass
        except OSError:
            leftovers.append(path + ' (not empty or changed; preserved)')
    return leftovers


def install(home, args):
    package = Path(args.package)
    files = inventory(package, args.binary)
    additions = bindings((package / 'keys.json').read_bytes())
    original_keys = home.read('keys.json')
    change = merge(original_keys, additions)
    preflight(home, files)
    checker = args.checker or shutil.which('herdr-app')
    if not checker:
        raise ValueError('herdr-app is required for native mod checks')
    # Native loading writes deterministic SDK/editor metadata. Check the exact
    # planned bytes and journal the two known generated filenames before copying.
    with tempfile.TemporaryDirectory(prefix='panorama-planned-') as planned:
        planned = Path(planned)
        for path, value in files.items():
            if path.startswith('mods/'):
                destination = planned / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(value[0])
        files.update(check_mods([planned / 'mods' / name for name in MODS], checker))
    preflight(home, files)
    manifest = {'v': 2, 'install_id': secrets.token_hex(16), 'files': {p: {'sha256': digest(v[0]), 'mode': v[1]} for p, v in files.items()},
                'dirs': [], 'keys': change}
    written = {}
    runtime_created = {}
    published = {'manifest': False, 'keys': False}
    manifest_written = None
    try:
        for directory in sorted({p for f in [*files, MANIFEST] for p in parents(f)}, key=lambda p: (p.count('/'), p)):
            if home.mkdir(directory):
                manifest['dirs'].append(directory)
        for directory in sorted(MOD_DATA_DIRS):
            if home.mkdir(directory):
                manifest['dirs'].append(directory)
            elif directory in preferences.DIRECTORIES:
                preferences.inspect(home, directory)
            else:
                raise ValueError(f'Name collision: {directory}')
        runtime.reserve(home, manifest, runtime_created)
        # Journal before payload changes, so interrupted installs can be uninstalled.
        manifest_written = (render(manifest), 0o600)
        if len(manifest_written[0]) > 1048576:
            raise ValueError('Install receipt exceeds 1 MiB')
        home.write(MANIFEST, *manifest_written, on_publish=lambda: published.update(manifest=True))
        for path, value in files.items():
            home.write(path, *value, on_publish=lambda p=path: written.update({p: manifest['files'][p]}))
        if change:
            home.write('keys.json', decoded(change['after']), change['mode'], expected=original_keys,
                       on_publish=lambda: published.update(keys=True))
        print('Installed panorama. Current layout unchanged.')
        print('Switch when ready: ctrl-shift-l or herdr-app do layout.switch \'{"name":"panorama"}\'')
    except BaseException:
        leftovers = remove_files(home, {'files': written})
        if published['keys']:
            try:
                note = undo(home, change)
                if note:
                    leftovers.append(note)
            except (OSError, ValueError) as error:
                leftovers.append('keys.json (rollback incomplete: ' + str(error) + ')')
        if leftovers and published['manifest']:
            try:
                runtime.disable(home, manifest)
            except (OSError, ValueError) as error:
                leftovers.append('runtime control preserved: ' + str(error))
        else:
            for path, current in reversed(list(runtime_created.items())):
                try:
                    if not home.remove(path, current):
                        leftovers.append(path + ' (changed; preserved)')
                except (OSError, ValueError):
                    leftovers.append(path + ' (unsafe or changed; preserved)')
        if published['manifest']:
            if leftovers:
                leftovers.append(MANIFEST + ' (recovery journal retained)')
            elif not home.remove(MANIFEST, manifest_written):
                leftovers.append(MANIFEST + ' (changed; preserved)')
        leftovers.extend(remove_dirs(home, manifest['dirs']))
        for path in leftovers:
            print('Preserved: ' + path, file=sys.stderr)
        raise


def uninstall(home, package):
    stored = home.read(MANIFEST, 1048576)
    if stored is None:
        raise ValueError('No panorama install manifest; nothing removed')
    if stored[1] != 0o600:
        raise ValueError('Install receipt must have mode 0600')
    definitions = bindings((Path(package) / 'keys.json').read_bytes())
    manifest = validate(stored[0], definitions)
    # Validate keys and every ancestor before destructive work. A substituted link
    # in one subtree must not cause partial removal elsewhere.
    for path in [*manifest['files'], 'keys.json']:
        home.read(path)
    if manifest['v'] == 2:
        preferences.preflight(home)
        with runtime.quiesce(home, manifest):
            runtime_plan = runtime.preflight_cleanup(home, manifest)
            frontend.wait(home, manifest)
            finish_uninstall(home, manifest, stored, runtime_plan)
    else:
        finish_uninstall(home, manifest, stored)


def finish_uninstall(home, manifest, stored, runtime_plan=None):
    removed = {}
    leftovers = remove_files(home, manifest, removed)
    if runtime_plan is not None:
        try:
            if not frontend.acknowledged(home, manifest):
                raise ValueError('Frontend published after its stop acknowledgment')
        except (OSError, ValueError, TypeError) as error:
            # Only this invocation's proven unchanged removed bytes are recovery
            # authority. Never overwrite a reappeared/personal destination.
            conflicts = []
            for path, current in reversed(list(removed.items())):
                try:
                    home.write(path, *current)
                except (OSError, ValueError) as restore_error:
                    conflicts.append(path + ': ' + str(restore_error))
            detail = (' Static recovery conflicts (preserved): ' + '; '.join(conflicts)) if conflicts else ' Removed static payload restored for retry.'
            raise ValueError(str(error) + '; disabled receipt retained.' + detail) from error
    note = undo(home, manifest['keys'])
    if note:
        leftovers.append(note)
    if runtime_plan is not None:
        runtime.cleanup(home, runtime_plan)
    if not home.remove(MANIFEST, stored):
        raise ValueError('Install manifest changed during uninstall; preserved')
    if manifest['v'] == 2:
        leftovers.extend(preferences.preserved(home, manifest['dirs']))
    leftovers.extend(remove_dirs(home, manifest['dirs']))
    for path in leftovers:
        print('Preserved: ' + path)
    print('Uninstalled panorama.' if not leftovers else 'Uninstalled unchanged panorama files; personal leftovers listed above.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('install', 'uninstall'))
    parser.add_argument('--app-home', required=True, help='absolute existing app home; never switches its layout')
    parser.add_argument('--package', default=str(Path(__file__).resolve().parents[1]), help='app package directory')
    parser.add_argument('--binary', help='absolute reviewed panorama executable to copy privately (install only)')
    parser.add_argument('--checker', help='absolute native herdr-app checker (test fixtures may supply a substitute)')
    args = parser.parse_args()
    try:
        if args.action == 'uninstall' and (args.binary or args.checker):
            raise ValueError('--binary and --checker apply only to install')
        home = Home(args.app_home)
        try:
            fcntl.flock(home.fd, fcntl.LOCK_EX)
            if args.action == 'install':
                install(home, args)
            else:
                uninstall(home, args.package)
        finally:
            os.close(home.fd)
        return 0
    except (OSError, ValueError, TypeError, KeyError, subprocess.TimeoutExpired) as error:
        print(f'panorama app {args.action}: {error}', file=sys.stderr)
        return 1

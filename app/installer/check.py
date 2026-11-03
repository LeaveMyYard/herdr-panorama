"""Run native checks and stop only their verified private core before cleanup."""
import json
import os
import re
import select
import shutil
import stat
import subprocess
import tempfile
import time
import uuid
from pathlib import Path


def private_sockets(scratch):
    return [path for path in Path(scratch).rglob('*') if path.is_socket()]


def process_identity(pid, env):
    """Read selected identity facts; never print/copy the full process env."""
    expected = {name: env[name] for name in ('HC_SESSION', 'HERDR_APP_HOME', 'XDG_CONFIG_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR')}
    proc = Path('/proc') / str(pid)
    if proc.exists():
        if Path(os.readlink(proc / 'exe')).name != 'herdr-core':
            raise ValueError('Private checker endpoint is not herdr-core')
        values = {}
        for entry in (proc / 'environ').read_bytes().split(b'\0'):
            key, _, value = entry.partition(b'=')
            if key.decode(errors='replace') in expected:
                values[key.decode()] = value.decode()
        if values != expected or (proc / 'task' / str(pid) / 'children').read_text().strip():
            raise ValueError('Private checker core environment/zero-child proof failed')
        # Field 22 starts after the parenthesized comm (which may contain spaces).
        return ('proc', (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19])
    command = subprocess.run(['ps', '-p', str(pid), '-o', 'comm='], capture_output=True, text=True, timeout=2)
    if command.returncode or Path(command.stdout.strip()).name != 'herdr-core':
        raise ValueError('Private checker endpoint process is not herdr-core')
    # BSD/macOS has no /proc. Read privately, check only our five exact values,
    # and discard the rest; it must never be included in diagnostics.
    raw = subprocess.run(['ps', 'eww', '-p', str(pid), '-o', 'command='], capture_output=True, text=True, timeout=2)
    if raw.returncode or any(not re.search(r'(?:^| )' + re.escape(key + '=' + value) + r'(?: |$)', raw.stdout) for key, value in expected.items()):
        raise ValueError('Private checker core environment proof failed')
    children = subprocess.run(['ps', '-axo', 'ppid=,pid='], capture_output=True, text=True, timeout=2)
    if children.returncode or any(line.split() and line.split()[0] == str(pid) for line in children.stdout.splitlines()):
        raise ValueError('Private checker core zero-child proof failed')
    birth = subprocess.run(['ps', '-p', str(pid), '-o', 'lstart='], capture_output=True, text=True, timeout=2)
    if birth.returncode or not birth.stdout.strip():
        raise ValueError('Private checker process identity unavailable')
    return ('ps', birth.stdout.strip())


def exited(pid, identity):
    if identity[0] == 'proc':
        path = Path('/proc') / str(pid) / 'stat'
        try:
            fields = path.read_text().rsplit(')', 1)[1].split()
            return fields[0] == 'Z' or fields[19] != identity[1]
        except FileNotFoundError:
            return True
    result = subprocess.run(['ps', '-p', str(pid), '-o', 'lstart='], capture_output=True, text=True, timeout=2)
    return result.returncode != 0 or result.stdout.strip() != identity[1]


def stop_checker_core(scratch, env):
    # IMPORTANT: `hc server status` AUTOSTARTS a missing core. Do not call it
    # without a preexisting private endpoint, or after stop to test absence.
    if not private_sockets(scratch):
        return
    hc = shutil.which('hc', path=env.get('PATH'))
    if not hc:
        raise ValueError('Private checker endpoint exists but hc is unavailable')
    status = subprocess.run([hc, 'server', 'status', '--json'], env=env, capture_output=True, text=True, timeout=3)
    if status.returncode:
        raise ValueError('Cannot identify private checker core endpoint')
    value = json.loads(status.stdout)
    pid = value.get('pid')
    if (type(pid) is not int or pid < 2 or not isinstance(value.get('core', {}).get('id'), str)
            or type(value.get('live')) is not int or value['live'] != 0 or not value['core']['id'] or not private_sockets(scratch)):
        raise ValueError('Private checker core ownership/empty-state proof failed')
    handle = os.pidfd_open(pid) if hasattr(os, 'pidfd_open') else None
    try:
        identity = process_identity(pid, env)
        stopped = subprocess.run([hc, 'server', 'stop', '--json'], env=env, capture_output=True, text=True, timeout=3)
        if handle is not None:
            poll = select.poll(); poll.register(handle, select.POLLIN)
            gone = bool(poll.poll(3000))
        else:
            deadline = time.monotonic() + 3
            while not exited(pid, identity) and time.monotonic() < deadline:
                time.sleep(.025)
            gone = exited(pid, identity)
        if not gone:
            raise ValueError('Captured private checker core did not exit after scoped stop')
        # A CLI can lose its connection while shutdown succeeds. Captured exit,
        # not a new status call (which would restart it), is the success proof.
        if stopped.returncode and private_sockets(scratch):
            raise ValueError('Scoped private checker stop failed; endpoint preserved')
    finally:
        if handle is not None:
            os.close(handle)


def check_mods(mods, checker):
    if not os.path.isabs(checker) or not Path(checker).is_file():
        raise ValueError('--checker must name an absolute herdr-app executable')
    metadata = {}
    # Unix socket paths are limited (104 bytes on macOS). Its default TMPDIR
    # can already be too deep before hc adds its endpoint name. Keep the whole
    # exclusively created, mode-0700 checker tree under a short Unix temp path.
    scratch = tempfile.mkdtemp(prefix='pc-', dir='/tmp')
    env = {k: v for k, v in os.environ.items() if not k.startswith(('HERDR_', 'HC_'))}
    env.update(HERDR_APP_HOME=f'{scratch}/app', HC_SESSION=f'panorama-check-{uuid.uuid4().hex[:12]}',
               XDG_CONFIG_HOME=f'{scratch}/config', XDG_STATE_HOME=f'{scratch}/state',
               XDG_RUNTIME_DIR=f'{scratch}/runtime', HERDR_TELEMETRY='0', DO_NOT_TRACK='1')
    failure = None
    try:
        for name in ('app', 'config', 'state', 'runtime'):
            Path(scratch, name).mkdir(mode=0o700)
        for mod in mods:
            checked = Path(scratch, 'mods', mod.name)
            shutil.copytree(mod, checked, ignore=shutil.ignore_patterns('gpui-kit.d.ts', 'jsconfig.json'))
            result = subprocess.run([checker, 'check', str(checked)], env=env, capture_output=True, timeout=30)
            if result.returncode:
                raise ValueError(f'Native check failed for {mod.name}: '
                                 + (result.stderr or result.stdout).decode(errors='replace').strip())
            if not stat.S_ISDIR(checked.lstat().st_mode):
                raise ValueError(f'Checker replaced its source directory: {checked}')
            for name in ('gpui-kit.d.ts', 'jsconfig.json'):
                generated = checked / name
                if generated.exists() or generated.is_symlink():
                    info = generated.lstat()
                    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                        raise ValueError(f'Unsafe checker metadata: {generated}')
                    metadata[f'mods/{mod.name}/{name}'] = (generated.read_bytes(), 0o644)
    except BaseException as error:
        failure = error
        raise
    finally:
        try:
            stop_checker_core(scratch, env)
        except BaseException as error:
            # Do not unlink an uncertain live core's socket/config state.
            detail = f'; original check failure: {failure}' if failure else ''
            raise ValueError(f'Native checker cleanup refused; private state retained at {scratch}: {error}{detail}') from error
        shutil.rmtree(scratch)
    return metadata

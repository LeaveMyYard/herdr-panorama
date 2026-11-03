"""The uninstall authority is a bounded inventory, never arbitrary paths."""
import json
import re
from keys import bindings, decoded, loads, merge

MODS = ('panorama', 'panorama-spaces', 'panorama-graph', 'panorama-peek')
MANIFEST = 'data/panorama/install.json'
BINARY = 'data/panorama/bin/panorama'
BASE_FILES = {f'mods/{name}/{file}' for name in MODS for file in ('main.js', 'mod.json')}
BASE_FILES.update(('layouts/panorama.json', 'mods/panorama/LICENSE.herdr-app', 'mods/panorama/NOTICE'))


def parents(path):
    components = path.split('/')
    return ['/'.join(components[:i]) for i in range(1, len(components))]


def valid_mode(value):
    return type(value) is int and 0 <= value <= 0o777


def validate(data):
    manifest = loads(data)
    if not isinstance(manifest, dict) or set(manifest) != {'v', 'files', 'dirs', 'keys'} or type(manifest['v']) is not int or manifest['v'] != 1:
        raise ValueError('Invalid panorama install manifest')
    files, dirs = manifest['files'], manifest['dirs']
    if not isinstance(files, dict) or set(files) not in (BASE_FILES, BASE_FILES | {BINARY}):
        raise ValueError('Manifest file inventory escapes the panorama package')
    for info in files.values():
        if (not isinstance(info, dict) or set(info) != {'sha256', 'mode'}
                or not isinstance(info['sha256'], str) or not re.fullmatch('[a-f0-9]{64}', info['sha256'])
                or not valid_mode(info['mode'])):
            raise ValueError('Invalid manifest file fingerprint')
    allowed_dirs = {p for f in [*files, MANIFEST] for p in parents(f)}
    if (not isinstance(dirs, list) or any(not isinstance(p, str) for p in dirs)
            or len(set(dirs)) != len(dirs) or not set(dirs) <= allowed_dirs):
        raise ValueError('Invalid manifest directory inventory')
    change = manifest['keys']
    if change is not None:
        if not isinstance(change, dict) or set(change) != {'before', 'before_mode', 'after', 'mode', 'added', 'sections'}:
            raise ValueError('Invalid manifest key change')
        original = None
        if change['before'] is not None:
            if not valid_mode(change['before_mode']):
                raise ValueError('Invalid original key mode')
            original = (decoded(change['before']), change['before_mode'])
        elif change['before_mode'] is not None:
            raise ValueError('Invalid absent original keys')
        if not valid_mode(change['mode']):
            raise ValueError('Invalid installed key mode')
        # Recompute the recorded merge, rather than trust arbitrary key-removal data.
        added = bindings(json.dumps(change['added']).encode())
        if not added or merge(original, added) != change:
            raise ValueError('Manifest key change does not match its recorded merge')
    return manifest

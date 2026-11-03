"""The uninstall authority is a bounded inventory, never arbitrary paths."""
import math
import re
from keys import decoded, loads, merge

MODS = ('panorama', 'panorama-spaces', 'panorama-graph', 'panorama-peek')
MOD_DATA_DIRS = frozenset(('data/panorama-spaces', 'data/panorama-graph', 'data/panorama-peek'))
MANIFEST = 'data/panorama/install.json'
BINARY = 'data/panorama/bin/panorama'
BASE_FILES = {f'mods/{name}/{file}' for name in MODS for file in ('main.js', 'mod.json')}
BASE_FILES.update(('layouts/panorama.json', 'mods/panorama/LICENSE.herdr-app', 'mods/panorama/NOTICE'))
LEGACY_FILES = frozenset(BASE_FILES)
BASE_FILES.update(f'mods/panorama/{name}.js' for name in ('contract', 'tree', 'model', 'feed-client', 'marks', 'routing', 'binary', 'viewport', 'viewport-math'))
METADATA_FILES = {f'mods/{mod}/{name}' for mod in MODS for name in ('gpui-kit.d.ts', 'jsconfig.json')}


def parents(path):
    components = path.split('/')
    return ['/'.join(components[:i]) for i in range(1, len(components))]


def safe_integer(value, lower, upper):
    return type(value) in (int, float) and lower <= value <= upper and math.isfinite(value) and value == int(value)


def valid_mode(value):
    return safe_integer(value, 0, 0o777)


def validate(data, definitions):
    manifest = loads(data)
    if not isinstance(manifest, dict) or not safe_integer(manifest.get('v'), 1, 2) or manifest['v'] not in (1, 2):
        raise ValueError('Invalid panorama install manifest')
    version = manifest['v'] = int(manifest['v'])
    if set(manifest) != ({'v', 'files', 'dirs', 'keys'} | ({'install_id'} if version == 2 else set())):
        raise ValueError('Invalid panorama install manifest fields')
    if version == 2 and (not isinstance(manifest['install_id'], str) or not re.fullmatch('[a-f0-9]{32}', manifest['install_id'])):
        raise ValueError('Invalid installation generation')
    files, dirs = manifest['files'], manifest['dirs']
    expected_files = LEGACY_FILES if version == 1 else BASE_FILES
    allowed_files = expected_files | {BINARY} | (METADATA_FILES if version == 2 else set())
    if not isinstance(files, dict) or not expected_files <= set(files) or not set(files) <= allowed_files:
        raise ValueError('Manifest file inventory escapes the panorama package')
    for path, info in files.items():
        if (not isinstance(info, dict) or set(info) != {'sha256', 'mode'}
                or not isinstance(info['sha256'], str) or not re.fullmatch('[a-f0-9]{64}', info['sha256'])
                or not valid_mode(info['mode']) or info['mode'] != (0o755 if path == BINARY else 0o644)):
            raise ValueError('Invalid manifest file fingerprint')
        info['mode'] = int(info['mode'])
    allowed_dirs = {p for f in [*files, MANIFEST] for p in parents(f)}
    if version == 2:
        allowed_dirs.update(MOD_DATA_DIRS)
        allowed_dirs.update(('data/panorama/runtime', f"data/panorama/runtime/{manifest['install_id']}"))
    if (not isinstance(dirs, list) or any(not isinstance(p, str) for p in dirs)
            or len(set(dirs)) != len(dirs) or not set(dirs) <= allowed_dirs):
        raise ValueError('Invalid manifest directory inventory')
    change = manifest['keys']
    expected_additions = {section: keys for section, keys in definitions.items() if keys}
    if bool(change) != bool(expected_additions):
        raise ValueError('Manifest key authority differs from this package definitions')
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
        # The explicit package, not the writable manifest, grants removal
        # authority. Every defined binding was added, or install refused.
        if change['added'] != expected_additions or merge(original, expected_additions) != change:
            raise ValueError('Manifest key change does not match its recorded merge')
        change['mode'] = int(change['mode'])
        if change['before_mode'] is not None:
            change['before_mode'] = int(change['before_mode'])
    return manifest

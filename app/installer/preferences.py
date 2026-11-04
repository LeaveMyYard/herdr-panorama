"""Read-only validation of the two operator-retained stock preference stores."""
import math
from keys import loads

DIRECTORIES = frozenset(('data/panorama-spaces', 'data/panorama-tabs'))
LIMIT = 1024 * 1024


def record(value):
    return isinstance(value, dict)


def integer(value):
    return type(value) in (int, float) and 0 <= value <= 9007199254740991 and math.isfinite(value) and value == int(value)


def machine(value, name):
    required = {'name', 'target', 'id', 'host', 'version', 'state'}
    optional = {'session', 'since_ms', 'temporary', 'via', 'why'}
    if not record(value) or not required <= set(value) or not set(value) <= required | optional:
        return False
    if value['name'] != name or any(not isinstance(value[key], str) for key in required):
        return False
    if any(key in value and value[key] is not None and not isinstance(value[key], str) for key in ('session', 'why')):
        return False
    if 'since_ms' in value and value['since_ms'] is not None and not integer(value['since_ms']):
        return False
    return all(key not in value or type(value[key]) is bool for key in ('temporary', 'via'))


def valid(key, value):
    if key == 'agents.scope':
        return value == 'all'
    if key == 'panels':
        return record(value) and all(item is None or isinstance(item, str) for item in value.values())
    if key == 'machines.folded':
        return (isinstance(value, list) and all(item is None or isinstance(item, str) for item in value)
                and len(set(value)) == len(value))
    if not record(value):
        return False
    for name, state in value.items():
        if (not record(state) or set(state) != {'machine', 'removed', 'busy', 'error'}
                or type(state['removed']) is not bool or type(state['busy']) is not bool
                or (state['error'] is not None and not isinstance(state['error'], str))
                or not machine(state['machine'], name)):
            return False
    return True


def inspect(home, directory):
    """Reuse is not adoption: never rewrite/chmod, journal, or delete this input."""
    if directory not in DIRECTORIES:
        raise ValueError('Not a retained preference directory')
    # Open the directory itself through the no-follow Home walker, even if the
    # known store is absent. Unknown children are personal and remain untouched.
    with home.parent(directory + '/.probe'):
        pass
    path = directory + '/store.json'
    current = home.read(path, LIMIT)
    if current is None:
        return False
    if current[1] != 0o600:
        raise ValueError(f'Retained preference store must have mode 0600: {path}')
    store = loads(current[0].decode('utf8'))
    if not record(store) or any(not isinstance(value, str) for value in store.values()):
        raise ValueError(f'Retained preference store must be a JSON string-map: {path}')
    known = ('panels',) if directory.endswith('panorama-tabs') else ('machines.folded', 'machines.reconnecting', 'agents.scope')
    for key in known:
        if key in store and not valid(key, loads(store[key])):
            raise ValueError(f'Unsupported retained preference shape: {path}/{key}')
    return True


def preflight(home):
    for directory in sorted(DIRECTORIES):
        if home.exists(directory):
            inspect(home, directory)


def preserved(home, created_dirs):
    result = []
    for directory in sorted(DIRECTORIES):
        try:
            if not home.exists(directory):
                continue
            if home.exists(directory + '/store.json'):
                result.append(directory + '/store.json (retained preferences; no deletion authority)')
            elif directory not in created_dirs:
                result.append(directory + ' (preexisting preference directory; no deletion authority)')
        except (OSError, ValueError):
            result.append(directory + ' (unsafe or changed; preserved)')
    return result

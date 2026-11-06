"""Conservative key merge/removal, preserving exact bytes when possible."""
import base64
import json


def object_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f'Duplicate JSON key: {key}')
        result[key] = value
    return result


def loads(data):
    return json.loads(data, object_pairs_hook=object_pairs)


def bindings(data):
    value = loads(data)
    if not isinstance(value, dict) or any(not isinstance(section, dict) for section in value.values()):
        raise ValueError('keys.json must contain binding sections')
    if any(not isinstance(action, str) for section in value.values() for action in section.values()):
        raise ValueError('Key actions must be strings')
    return value


def encoded(data):
    return base64.b64encode(data).decode('ascii')


def decoded(text):
    return base64.b64decode(text, validate=True)


def render(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode()


def merge(original, additions):
    current = bindings(original[0]) if original else {}
    added = {}
    sections = []
    for section, keys in additions.items():
        if not keys:
            continue
        if section not in current:
            sections.append(section)
            current[section] = {}
        for key, action in keys.items():
            if key in current[section]:
                raise ValueError(f'Key collision requires an operator decision: {section}/{key}')
            current[section][key] = action
            added.setdefault(section, {})[key] = action
    if not added:
        return None
    return {'before': encoded(original[0]) if original else None,
            'before_mode': original[1] if original else None,
            'after': encoded(render(current)), 'mode': original[1] if original else 0o644,
            'added': added, 'sections': sections}


def undo(home, change):
    if change is None:
        return None
    original = (decoded(change['before']), change['before_mode']) if change['before'] is not None else None
    installed = (decoded(change['after']), change['mode'])
    current = home.read('keys.json')
    if current is None:
        return None
    if current == installed:
        if original is None:
            home.remove('keys.json', current)
        else:
            home.write('keys.json', *original, expected=current)
        return None
    # The person changed keys after install. Remove only unchanged owned entries.
    try:
        value = bindings(current[0])
    except (ValueError, UnicodeError):
        return 'keys.json (edited; preserved)'
    changed = False
    retained = False
    for section, keys in change['added'].items():
        for key, action in keys.items():
            if section in value and value[section].get(key) == action:
                del value[section][key]
                changed = True
            elif section in value and key in value[section]:
                retained = True
        if section in change['sections'] and value.get(section) == {}:
            del value[section]
            changed = True
    if changed:
        home.write('keys.json', render(value), current[1], expected=current)
    return 'keys.json (personal edits preserved)' if retained else None

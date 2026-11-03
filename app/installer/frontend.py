"""Observe supported frontend cleanup without editing host-owned shared state."""
import json
import math
import time
from keys import object_pairs

LIMIT = 8 * 1024 * 1024


def finite(value):
    return type(value) in (int, float) and abs(value) <= 9007199254740991 and math.isfinite(value)


def constant(value):
    raise ValueError('Non-JSON constant in shared.json')


def acknowledged(home, receipt):
    current = home.read('shared.json', LIMIT)
    if current is None:
        return True
    value = json.loads(current[0], object_pairs_hook=object_pairs, parse_constant=constant)
    if not isinstance(value, dict):
        raise ValueError('Ambiguous frontend shared.json; preserved')
    packet, owner = value.get('panorama.feed'), value.get('panorama.feed.owner')
    if packet is not None:
        if (not isinstance(packet, dict) or packet.get('install_id') != receipt['install_id']
                or set(packet) != {'feed', 'stale', 'errors', 'generated_at', 'writerError', 'install_id'}
                or (packet['feed'] is not None and not isinstance(packet['feed'], dict))
                or type(packet['stale']) is not bool or not isinstance(packet['errors'], list)
                or any(not isinstance(error, str) for error in packet['errors'])
                or (packet['writerError'] is not None and not isinstance(packet['writerError'], str))
                or (packet['generated_at'] is not None and not finite(packet['generated_at']))):
            raise ValueError('Foreign or ambiguous panorama.feed; preserved')
    if owner is not None:
        if (not isinstance(owner, dict) or set(owner) != {'v', 'token', 'role', 'seen_at'}
                or type(owner['v']) not in (int, float) or owner['v'] != 1
                or not isinstance(owner['token'], str) or not 1 <= len(owner['token'].encode('utf8')) <= 256
                or owner['role'] not in ('sidebar', 'consumer')
                or not finite(owner['seen_at']) or not 0 <= owner['seen_at'] <= time.time() * 1000):
            raise ValueError('Foreign or ambiguous panorama.feed.owner; preserved')
    return packet is None and owner is None


def wait(home, receipt):
    deadline = time.monotonic() + 2
    while not acknowledged(home, receipt):
        if time.monotonic() >= deadline:
            raise ValueError('Frontend stop timed out; disabled receipt and static payload retained. Reopen the current mod to clear its disabled generation, then retry.')
        time.sleep(.025)

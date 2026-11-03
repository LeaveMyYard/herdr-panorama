"""Generation-owned, cooperative writer stop and exact runtime cleanup."""
import fcntl
import math
import os
import re
import stat
import time
from contextlib import contextmanager

from fs import digest
from keys import loads, render
from manifest import BINARY

ROOT = 'data/panorama'
RUNTIME = ROOT + '/runtime'
FEED = ROOT + '/feed.json'
FEED_LIMIT = 32 * 1024 * 1024


def generation(receipt):
    return RUNTIME + '/' + receipt['install_id']


def integer(value, lower, upper):
    return (type(value) in (int, float) and lower <= value <= upper and math.isfinite(value) and value == int(value))


def managed(home, path, limit=4096):
    current = home.read(path, limit)
    if current is not None and current[1] != 0o600:
        raise ValueError(f'Runtime file must have mode 0600: {path}')
    return current


def control(home, receipt):
    path = generation(receipt) + '/control.json'
    current = managed(home, path)
    if current is None:
        raise ValueError(f'Missing runtime control: {path}')
    value = loads(current[0])
    if (not isinstance(value, dict) or set(value) != {'v', 'install_id', 'disabled'}
            or not integer(value['v'], 1, 1) or value['install_id'] != receipt['install_id']
            or type(value['disabled']) is not bool):
        raise ValueError(f'Ambiguous runtime control: {path}')
    return current, value


def reserve(home, receipt, created):
    existing = set(home.list(ROOT)) - {'bin', 'runtime'}
    if existing:
        raise ValueError(f'Existing panorama runtime artifact is not adoptable: {ROOT}/{sorted(existing)[0]}')
    if home.exists(FEED):
        raise ValueError('Existing feed is not owned by this installation')
    if home.exists(RUNTIME) and home.list(RUNTIME):
        raise ValueError('Existing runtime generation is not adoptable')
    for directory in (RUNTIME, generation(receipt)):
        if home.mkdir(directory):
            receipt['dirs'].append(directory)
    for path, data in ((generation(receipt) + '/control.json', render({'v': 1, 'install_id': receipt['install_id'], 'disabled': False})),
                       (generation(receipt) + '/writer.lock', b'')):
        home.write(path, data, 0o600, on_publish=lambda p=path, d=data: created.update({p: (d, 0o600)}))


def disable(home, receipt):
    current, value = control(home, receipt)
    temp = generation(receipt) + '/.control.tmp'
    if home.exists(temp):
        raise ValueError(f'Ambiguous runtime staging; preserved: {temp}')
    if not value['disabled']:
        value['disabled'] = True
        home.write(temp, render(value), 0o600)
        home.replace_temp(generation(receipt) + '/control.json', temp, current)


def token(value):
    return (isinstance(value, str) and 1 <= len(value.encode('utf8')) <= 256
            and not any(char in value for char in ('\0', '\r', '\n')))


def ownership(home, receipt):
    path = generation(receipt) + '/ownership.json'
    current = managed(home, path)
    if current is None:
        return None
    value = loads(current[0])
    fields = {'v', 'install_id', 'binary_sha256', 'writer_pid', 'writer_start', 'writer_pidns', 'feed_sha256', 'pending_feed_sha256'}
    binary = receipt['files'].get(BINARY)
    if (not isinstance(value, dict) or set(value) != fields or not integer(value['v'], 1, 1)
            or value['install_id'] != receipt['install_id'] or binary is None
            or value['binary_sha256'] != binary['sha256'] or not integer(value['writer_pid'], 2, 4294967295)
            or not token(value['writer_start']) or (value['writer_pidns'] is not None and not token(value['writer_pidns']))
            or any(d is not None and (not isinstance(d, str) or not re.fullmatch('[a-f0-9]{64}', d))
                   for d in (value['feed_sha256'], value['pending_feed_sha256']))):
        raise ValueError(f'Ambiguous runtime ownership: {path}')
    return current, value


@contextmanager
def quiesce(home, receipt):
    disable(home, receipt)
    path = generation(receipt) + '/writer.lock'
    current = managed(home, path, 0)
    if current is None or current[0] != b'':
        raise ValueError(f'Missing or changed writer lock: {path}')
    with home.parent(path) as (parent, name):
        fd = os.open(name, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
        expected = os.stat(name, dir_fd=parent, follow_symlinks=False)
    try:
        actual = os.fstat(fd)
        if (not stat.S_ISREG(actual.st_mode) or actual.st_nlink != 1 or stat.S_IMODE(actual.st_mode) != 0o600
                or actual.st_size != 0 or (actual.st_dev, actual.st_ino) != (expected.st_dev, expected.st_ino)):
            raise ValueError(f'Unsafe writer lock: {path}')
        deadline = time.monotonic() + 2
        while True:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise ValueError(f'Writer stop timed out; receipt and disabled control retained: {path}')
                time.sleep(0.025)
        with home.parent(path) as (parent, name):
            current_stat = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if (current_stat.st_dev, current_stat.st_ino) != (actual.st_dev, actual.st_ino):
            raise ValueError(f'Writer lock was replaced: {path}')
        yield
    finally:
        os.close(fd)


def preflight_cleanup(home, receipt):
    directory = generation(receipt)
    if set(home.list(RUNTIME)) != {receipt['install_id']}:
        raise ValueError(f'Unknown runtime generation; preserved: {RUNTIME}')
    expected = {'control.json', 'writer.lock', 'ownership.json'}
    unknown = set(home.list(directory)) - expected
    if unknown:
        raise ValueError(f'Unknown or staged runtime artifact; preserved: {directory}/{sorted(unknown)[0]}')
    parent_unknown = set(home.list(ROOT)) - {'install.json', 'bin', 'runtime', 'feed.json', f".feed-{receipt['install_id']}.tmp"}
    if parent_unknown:
        raise ValueError(f'Unknown panorama runtime artifact; preserved: {ROOT}/{sorted(parent_unknown)[0]}')
    record = ownership(home, receipt)
    feed = managed(home, FEED, FEED_LIMIT)
    temp_path = f"{ROOT}/.feed-{receipt['install_id']}.tmp"
    temp = managed(home, temp_path, FEED_LIMIT)
    allowed = set() if record is None else {record[1]['feed_sha256'], record[1]['pending_feed_sha256']} - {None}
    if feed is not None and digest(feed[0]) not in allowed:
        raise ValueError(f'Feed digest is not owned; preserved: {FEED}')
    if temp is not None and (record is None or digest(temp[0]) != record[1]['pending_feed_sha256']):
        raise ValueError(f'Incomplete or foreign feed staging; preserved: {temp_path}')
    # Preflight all authority before any deletion, holding the lifetime lock.
    removals = [(temp_path, temp), (FEED, feed)]
    if record:
        removals.append((directory + '/ownership.json', record[0]))
    removals.extend(((directory + '/control.json', control(home, receipt)[0]),
                     (directory + '/writer.lock', managed(home, directory + '/writer.lock', 0))))
    return removals


def cleanup(home, removals):
    for path, current in removals:
        if current is not None and not home.remove(path, current):
            raise ValueError(f'Runtime changed during cleanup; receipt retained: {path}')

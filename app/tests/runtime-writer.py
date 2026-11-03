"""Owned cooperative test writer; public installer is the code under test."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import sys
import time

home, marker, mode = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
receipt = json.loads((home / 'data/panorama/install.json').read_text())
root = home / 'data/panorama'
generation = root / 'runtime' / receipt['install_id']
with (generation / 'writer.lock').open('r+b') as lifetime:
    fcntl.flock(lifetime.fileno(), fcntl.LOCK_EX)
    data = b'{"v":1,"synthetic":"owned writer"}\n'
    (root / 'feed.json').write_bytes(data)
    (root / 'feed.json').chmod(0o600)
    ownership = {'v': 1, 'install_id': receipt['install_id'],
                 'binary_sha256': receipt['files']['data/panorama/bin/panorama']['sha256'],
                 'writer_pid': os.getpid(), 'writer_start': 'fixture-start', 'writer_pidns': None,
                 'feed_sha256': hashlib.sha256(data).hexdigest(), 'pending_feed_sha256': None}
    (generation / 'ownership.json').write_text(json.dumps(ownership))
    (generation / 'ownership.json').chmod(0o600)
    if mode == 'transient':
        (generation / '.ownership.tmp').write_text('{in-progress')
        (generation / '.ownership.tmp').chmod(0o600)
    print('ready', flush=True)
    while True:
        disabled = json.loads((generation / 'control.json').read_text())['disabled']
        if disabled and mode != 'ignore':
            if mode == 'delay':
                time.sleep(.35)
            if mode == 'transient':
                (generation / '.ownership.tmp').unlink()
            marker.write_text('cooperatively stopped')
            break
        time.sleep(.02)

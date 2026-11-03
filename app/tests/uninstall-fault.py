"""Deterministically publish a host packet after a real owned static removal."""
import errno
import json
import os
from pathlib import Path
import sys

sys.dont_write_bytecode = True
repo = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(repo / 'app/installer'))
import main
from fs import Home

home = Path(sys.argv[sys.argv.index('--app-home') + 1])
receipt = json.loads((home / 'data/panorama/install.json').read_text())
original_remove, original_write = Home.remove, Home.write
fired = False
TARGET = 'mods/panorama/main.js'


def remove(self, path, expected):
    global fired
    result = original_remove(self, path, expected)
    if result and path == TARGET and not fired:
        fired = True
        value = json.loads((home / 'shared.json').read_text())
        value['panorama.feed'] = {'feed': None, 'stale': True, 'errors': [], 'generated_at': None, 'writerError': None, 'install_id': receipt['install_id']}
        (home / 'shared.json').write_text(json.dumps(value))
        if os.environ.get('FRONTEND_FAULT') == 'collision':
            (home / TARGET).write_text('personal replacement')
    return result


def write(self, path, *args, **kwargs):
    if fired and path == TARGET and os.environ.get('FRONTEND_FAULT') == 'restore-eio':
        raise OSError(errno.EIO, 'injected exclusive restore failure')
    return original_write(self, path, *args, **kwargs)


Home.remove, Home.write = remove, write
raise SystemExit(main.main())

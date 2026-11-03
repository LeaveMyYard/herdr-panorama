"""Node-invoked fault probe: raise EIO after destination publication."""
import os
from pathlib import Path
import stat
import sys

sys.dont_write_bytecode = True
root, home_path, package, checker, scenario = sys.argv[1:]
sys.path.insert(0, str(Path(root) / 'app/installer'))
import main as installer
from fs import Home

home = Path(home_path)
parent = {'journal': 'data/panorama', 'payload': 'layouts', 'keys': '.', 'rollback': 'layouts', 'crash-journal': 'data/panorama', 'crash-payload': 'layouts'}[scenario]
original_fsync = os.fsync
original_remove = Home.remove
tripped = False
removal_failed = False


def fail_after_publication(fd):
    global tripped
    info = os.fstat(fd)
    target = home / parent
    if not tripped and stat.S_ISDIR(info.st_mode) and target.exists():
        expected = target.stat()
        if (info.st_dev, info.st_ino) == (expected.st_dev, expected.st_ino):
            tripped = True
            if scenario.startswith('crash-'):
                os._exit(91)  # Deliberately bypass finally and normal rollback.
            raise OSError(5, 'injected directory fsync after publication')
    return original_fsync(fd)


def fail_one_rollback(self, path, expected):
    global removal_failed
    if scenario == 'rollback' and path == 'layouts/panorama.json' and not removal_failed:
        removal_failed = True
        return False
    return original_remove(self, path, expected)


os.fsync = fail_after_publication
Home.remove = fail_one_rollback
sys.argv = ['app-install', 'install', '--app-home', home_path, '--package', package, '--checker', checker]
result = installer.main()
assert tripped and result == 1, 'fault did not exercise the intended published write'
assert scenario != 'rollback' or removal_failed, 'incomplete rollback was not exercised'
print('Fault injected and handled at ' + scenario)

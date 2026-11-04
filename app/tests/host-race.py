"""Node-invoked probe: a running host writes editor metadata into new mod dirs."""
import os
from pathlib import Path
import sys

sys.dont_write_bytecode = True
root, home_path, package, checker, scenario = sys.argv[1:]
sys.path.insert(0, str(Path(root) / 'app/installer'))
import main as installer
import fs
from fs import Home

# Same bytes the fixture checker generates, unless the host's copy differs.
GENERATED = {'gpui-kit.d.ts': '// generated native types', 'jsconfig.json': '{}' if scenario != 'different' else '{"host":"other"}'}
original_mkdir, original_rename_new = Home.mkdir, fs.rename_new
raced = []


def host_writes_at_mkdir(self, path):
    created = original_mkdir(self, path)
    if created and path.startswith('mods/panorama'):
        for name, text in GENERATED.items():
            target = Path(home_path, path, name)
            target.write_text(text)
            os.chmod(target, 0o664)
        raced.append(path)
    return created


def host_writes_before_rename(fd, source, destination):
    # The host's identical write lands between the installer's read and its
    # create-only rename, so the first attempt fails and must be retried.
    if destination == 'gpui-kit.d.ts' and len(raced) < 6:
        with open(os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o664, dir_fd=fd), 'w') as stream:
            stream.write(GENERATED[destination])
        raced.append(destination)
    return original_rename_new(fd, source, destination)


if scenario == 'retry':
    fs.rename_new = host_writes_before_rename
else:
    Home.mkdir = host_writes_at_mkdir
sys.argv = ['app-install', 'install', '--app-home', home_path, '--package', package, '--checker', checker, '--checker-core', 'none']
result = installer.main()
assert len(raced) == 6, 'host metadata race was not exercised for every mod'
print(f'Host metadata race {scenario}: exit {result}')
sys.exit(result)

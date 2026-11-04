"""Node-invoked probe: a running host writes editor metadata into new mod dirs."""
import os
from pathlib import Path
import sys

sys.dont_write_bytecode = True
root, home_path, package, checker, scenario = sys.argv[1:]
sys.path.insert(0, str(Path(root) / 'app/installer'))
import main as installer
from fs import Home

# Same bytes the fixture checker generates, unless the host's copy differs.
GENERATED = {'gpui-kit.d.ts': '// generated native types', 'jsconfig.json': '{}' if scenario == 'identical' else '{"host":"other"}'}
original_mkdir = Home.mkdir
raced = []


def host_writes_metadata(self, path):
    created = original_mkdir(self, path)
    if created and path.startswith('mods/panorama'):
        for name, text in GENERATED.items():
            target = Path(home_path, path, name)
            target.write_text(text)
            os.chmod(target, 0o664)
        raced.append(path)
    return created


Home.mkdir = host_writes_metadata
sys.argv = ['app-install', 'install', '--app-home', home_path, '--package', package, '--checker', checker, '--checker-core', 'none']
result = installer.main()
assert len(raced) == 6, 'host metadata race was not exercised for every mod'
print(f'Host metadata race {scenario}: exit {result}')
sys.exit(result)

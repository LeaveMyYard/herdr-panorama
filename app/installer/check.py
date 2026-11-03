"""Run native checks without inherited app/core endpoints or personal state."""
import os
import shutil
import subprocess
import tempfile
import uuid
from pathlib import Path


def check_mods(mods, checker):
    if not os.path.isabs(checker) or not Path(checker).is_file():
        raise ValueError('--checker must name an absolute herdr-app executable')
    with tempfile.TemporaryDirectory(prefix='panorama-check-') as scratch:
        env = {k: v for k, v in os.environ.items() if not k.startswith(('HERDR_', 'HC_'))}
        env.update(HERDR_APP_HOME=f'{scratch}/app', HC_SESSION=f'panorama-check-{uuid.uuid4().hex}',
                   XDG_CONFIG_HOME=f'{scratch}/config', XDG_STATE_HOME=f'{scratch}/state',
                   XDG_RUNTIME_DIR=f'{scratch}/runtime', HERDR_TELEMETRY='0', DO_NOT_TRACK='1')
        for name in ('app', 'config', 'state', 'runtime'):
            Path(scratch, name).mkdir(mode=0o700)
        for mod in mods:
            # The checker generates type/config files beside its input. Check an
            # identical private copy so installation and source remain untouched.
            checked = Path(scratch, 'mods', mod.name)
            shutil.copytree(mod, checked, ignore=shutil.ignore_patterns('gpui-kit.d.ts', 'jsconfig.json'))
            result = subprocess.run([checker, 'check', str(checked)], env=env, capture_output=True, timeout=30)
            if result.returncode:
                raise ValueError(f'Native check failed for {mod.name}: '
                                 + (result.stderr or result.stdout).decode(errors='replace').strip())

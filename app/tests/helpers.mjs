import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, lstatSync, readlinkSync, realpathSync, cpSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

export const root = fileURLToPath(new URL('../..', import.meta.url));
export function sandbox(t, additions = {}) {
  const temp = realpathSync(mkdtempSync(join(tmpdir(), 'panorama-install-test-')));
  t.after(() => rmSync(temp, { recursive: true, force: true }));
  const home = join(temp, 'home');
  const packageDir = join(temp, 'package');
  const checker = join(temp, 'checker');
  mkdirSync(home);
  cpSync(join(root, 'app'), packageDir, { recursive: true });
  writeFileSync(join(packageDir, 'keys.json'), JSON.stringify(additions));
  writeFileSync(checker, `#!/usr/bin/env python3
import os, sys, time
from pathlib import Path
assert len(sys.argv) == 3 and sys.argv[1] == 'check'
assert os.environ['HC_SESSION'].startswith('panorama-check-')
assert 'HC_SOCKET' not in os.environ and 'HERDR_SOCKET' not in os.environ
assert os.environ['HERDR_APP_HOME'] != os.environ['TEST_APP_HOME']
assert all(os.path.dirname(os.environ[k]) == os.path.dirname(os.environ['HERDR_APP_HOME']) for k in ['XDG_CONFIG_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR'])
with open(os.environ['TEST_LOG'], 'a') as f: f.write(sys.argv[2] + '\\n')
Path(sys.argv[2], 'gpui-kit.d.ts').write_text('// generated native types')
Path(sys.argv[2], 'jsconfig.json').write_text('{}')
time.sleep(float(os.environ.get('TEST_DELAY', '0')))
if os.environ.get('TEST_FAIL') == os.path.basename(sys.argv[2]):
 print('intentional checker failure', file=sys.stderr)
 sys.exit(9)
`, { mode: 0o755 });
  const env = { ...process.env, TEST_APP_HOME: home, TEST_LOG: join(temp, 'checks.log'), HC_SOCKET: '/live/core', HERDR_SOCKET: '/live/tui' };
  const args = action => [join(root, 'scripts/app-install'), action, '--app-home', home, '--package', packageDir, ...(action === 'install' ? ['--checker', checker] : [])];
  const run = (action, extra = [], extraEnv = {}) => spawnSync('python3', [...args(action), ...extra], { env: { ...env, ...extraEnv }, encoding: 'utf8' });
  return { temp, home, packageDir, checker, env, args, run, manifest: join(home, 'data/panorama/install.json') };
}
export function tree(directory) {
  const result = {};
  function walk(dir, prefix = '') {
    for (const name of readdirSync(dir).sort()) {
      const path = join(dir, name), key = prefix + name, info = lstatSync(path);
      if (info.isSymbolicLink()) result[key] = { link: readlinkSync(path) };
      else if (info.isDirectory()) { result[key + '/'] = { mode: info.mode & 0o777 }; walk(path, key + '/'); }
      else result[key] = { data: readFileSync(path).toString('base64'), mode: info.mode & 0o777 };
    }
  }
  walk(resolve(directory));
  return result;
}

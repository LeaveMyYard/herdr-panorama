import test from 'node:test';
import assert from 'node:assert/strict';
import { copyFileSync, cpSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { sandbox, root } from './helpers.mjs';

test('bundle check rejects stale runtime copies and rebuilding restores canonical source', t => {
  const s = sandbox(t), app = join(s.temp, 'app');
  cpSync(s.packageDir, app, { recursive: true }); mkdirSync(join(s.temp, 'scripts'));
  const script = join(s.temp, 'scripts/app-build'); copyFileSync(join(root, 'scripts/app-build'), script);
  const run = (...args) => spawnSync('python3', [script, ...args], { encoding: 'utf8' });
  assert.equal(run('--check').status, 0);
  const generated = join(app, 'mods/panorama/model.js'), source = readFileSync(join(app, 'shared/model.js'), 'utf8');
  writeFileSync(generated, '// stale version\n');
  const stale = run('--check'); assert.equal(stale.status, 1); assert.match(stale.stderr, /mods\/panorama\/model.js/);
  assert.equal(run().status, 0); assert.equal(run('--check').status, 0);
  assert.equal(readFileSync(generated, 'utf8'), '// Generated from app/shared/model.js by scripts/app-build.\n' + source);
  const manifest = join(app, 'mods/panorama/bundle.json'), data = JSON.parse(readFileSync(manifest));
  data.shared = data.shared.filter(name => name !== 'contract.js'); writeFileSync(manifest, JSON.stringify(data));
  const incomplete = run('--check'); assert.equal(incomplete.status, 1); assert.match(incomplete.stderr, /Missing shared dependencies/);
});

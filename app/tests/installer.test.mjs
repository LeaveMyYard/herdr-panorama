import test from 'node:test';
import assert from 'node:assert/strict';
import { writeFileSync, readFileSync, mkdirSync, chmodSync, symlinkSync, rmSync, existsSync, linkSync, lstatSync } from 'node:fs';
import { join } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { root, sandbox, tree } from './helpers.mjs';

const ok = result => assert.equal(result.status, 0, result.stderr || result.stdout);
const fail = result => assert.notEqual(result.status, 0, result.stdout);
function personal(s) {
  mkdirSync(join(s.home, 'mods/personal'), { recursive: true });
  writeFileSync(join(s.home, 'mods/personal/main.js'), 'personal source\n');
  writeFileSync(join(s.home, 'view.json'), '{"layout":"personal"}\n');
  writeFileSync(join(s.home, 'keys.json'), '{ "global": { "alt-p": "personal.command" } }\n', { mode: 0o640 });
}
function artifact(s) {
  const path = join(s.temp, 'reviewed-panorama');
  writeFileSync(path, '#!/bin/sh\nexit 0\n', { mode: 0o751 });
  return path;
}

test('empty keys plus optional binary roundtrip restores exact content and modes', t => {
  const s = sandbox(t); personal(s);
  const before = tree(s.home), binary = artifact(s);
  ok(s.run('install', ['--binary', binary]));
  assert.equal(readFileSync(join(s.home, 'data/panorama/bin/panorama'), 'utf8'), readFileSync(binary, 'utf8'));
  const manifest = JSON.parse(readFileSync(s.manifest));
  assert.equal(manifest.files['data/panorama/bin/panorama'].mode, 0o755);
  assert.equal(readFileSync(join(s.temp, 'checks.log'), 'utf8').trim().split('\n').length, 4);
  assert.equal(readFileSync(join(s.home, 'view.json'), 'utf8'), '{"layout":"personal"}\n');
  ok(s.run('uninstall'));
  assert.deepEqual(tree(s.home), before);
});

test('nonempty key merge restores original exact bytes and permissions', t => {
  const s = sandbox(t, { global: { 'alt-s': 'panorama.open' }, 'mod:panorama': { 'alt-f': 'panorama.fold' } });
  personal(s); const before = tree(s.home);
  ok(s.run('install'));
  const merged = JSON.parse(readFileSync(join(s.home, 'keys.json')));
  assert.equal(merged.global['alt-p'], 'personal.command');
  assert.equal(merged.global['alt-s'], 'panorama.open');
  ok(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
});

test('new keys file and every owned directory disappear on uninstall', t => {
  const s = sandbox(t, { 'mod:panorama': { 'alt-f': 'panorama.fold' } });
  ok(s.run('install')); ok(s.run('uninstall')); assert.deepEqual(tree(s.home), {});
});

for (const collision of ['mods/panorama', 'mods/panorama-spaces', 'layouts/panorama.json', 'data/panorama/install.json', 'data/panorama/bin/panorama']) {
  test(`name collision ${collision} refuses without mutation`, t => {
    const s = sandbox(t); personal(s);
    mkdirSync(join(s.home, collision, '..'), { recursive: true });
    if (collision.startsWith('mods/')) mkdirSync(join(s.home, collision));
    else writeFileSync(join(s.home, collision), 'personal');
    const before = tree(s.home);
    fail(s.run('install', ['--binary', artifact(s)])); assert.deepEqual(tree(s.home), before);
  });
}

test('existing binding even with identical action requires a decision, with no mutation', t => {
  const s = sandbox(t, { global: { 'alt-p': 'personal.command' } }); personal(s);
  const before = tree(s.home); const result = s.run('install');
  fail(result); assert.match(result.stderr, /collision requires an operator decision/);
  assert.deepEqual(tree(s.home), before);
});

test('last staged native check failure leaves files, binary destinations, dirs and original keys untouched', t => {
  const s = sandbox(t, { global: { 'alt-s': 'panorama.open' } }); personal(s);
  const before = tree(s.home);
  fail(s.run('install', ['--binary', artifact(s)], { TEST_FAIL: 'panorama-peek' }));
  assert.deepEqual(tree(s.home), before);
  assert.equal(readFileSync(join(s.temp, 'checks.log'), 'utf8').trim().split('\n').length, 4);
});

test('personal source edits and later files survive uninstall and are listed', t => {
  const s = sandbox(t); ok(s.run('install'));
  writeFileSync(join(s.home, 'mods/panorama/main.js'), '// personal edit');
  writeFileSync(join(s.home, 'mods/panorama/notes.txt'), 'my notes');
  const result = s.run('uninstall'); ok(result);
  assert.match(result.stdout, /mods\/panorama\/main.js \(edited; preserved\)/);
  assert.equal(readFileSync(join(s.home, 'mods/panorama/main.js'), 'utf8'), '// personal edit');
  assert.equal(readFileSync(join(s.home, 'mods/panorama/notes.txt'), 'utf8'), 'my notes');
  assert.equal(existsSync(s.manifest), false);
});

test('binary edits and chmod survive uninstall', t => {
  const s = sandbox(t); ok(s.run('install', ['--binary', artifact(s)]));
  const binary = join(s.home, 'data/panorama/bin/panorama'); chmodSync(binary, 0o700);
  const result = s.run('uninstall'); ok(result); assert.match(result.stdout, /bin\/panorama \(edited; preserved\)/);
  assert.equal(existsSync(binary), true);
});

test('later key edits survive, unchanged owned keys are removed', t => {
  const s = sandbox(t, { global: { 'alt-s': 'panorama.open' }, 'mod:panorama': { 'alt-f': 'panorama.fold' } });
  personal(s); ok(s.run('install'));
  const path = join(s.home, 'keys.json'), value = JSON.parse(readFileSync(path));
  value.global['alt-s'] = 'personal.replacement'; value.global['alt-x'] = 'personal.new';
  writeFileSync(path, JSON.stringify(value));
  const result = s.run('uninstall'); ok(result); assert.match(result.stdout, /personal edits preserved/);
  assert.deepEqual(JSON.parse(readFileSync(path)), { global: { 'alt-p': 'personal.command', 'alt-s': 'personal.replacement', 'alt-x': 'personal.new' } });
});

test('malformed personal keys after install remain byte-for-byte intact', t => {
  const s = sandbox(t, { global: { 'alt-s': 'panorama.open' } }); ok(s.run('install'));
  writeFileSync(join(s.home, 'keys.json'), '{unfinished');
  const result = s.run('uninstall'); ok(result); assert.match(result.stdout, /keys.json \(edited; preserved\)/);
  assert.equal(readFileSync(join(s.home, 'keys.json'), 'utf8'), '{unfinished');
});

for (const parent of ['mods', 'layouts', 'data', 'data/panorama']) {
  test(`install refuses symlink ancestor ${parent}`, t => {
    const s = sandbox(t); const external = join(s.temp, 'external'); mkdirSync(external);
    mkdirSync(join(s.home, parent, '..'), { recursive: true }); symlinkSync(external, join(s.home, parent));
    const before = tree(s.home); fail(s.run('install'));
    assert.deepEqual(tree(s.home), before); assert.deepEqual(tree(external), {});
  });
}

test('symlink app home and hardlinked keys are refused', t => {
  const s = sandbox(t); const alias = join(s.temp, 'alias'); symlinkSync(s.home, alias);
  fail(s.run('install', ['--app-home', alias]));
  writeFileSync(join(s.temp, 'keys'), '{}'); linkSync(join(s.temp, 'keys'), join(s.home, 'keys.json'));
  const before = tree(s.home); fail(s.run('install')); assert.deepEqual(tree(s.home), before);
});

for (const mutation of ['traversal', 'absolute', 'extra-directory', 'wrong-key-delta', 'duplicate-json', 'bad-hash']) {
  test(`tampered manifest ${mutation} refuses all removal`, t => {
    const s = sandbox(t, { global: { 'alt-s': 'panorama.open' } }); ok(s.run('install'));
    const manifest = JSON.parse(readFileSync(s.manifest));
    if (mutation === 'traversal') manifest.files['../../personal'] = { sha256: '0'.repeat(64), mode: 420 };
    if (mutation === 'absolute') manifest.files['/tmp/personal'] = { sha256: '0'.repeat(64), mode: 420 };
    if (mutation === 'extra-directory') manifest.dirs.push('mods/personal');
    if (mutation === 'wrong-key-delta') manifest.keys.added.global['alt-p'] = 'personal.command';
    if (mutation === 'bad-hash') manifest.files['mods/panorama/main.js'].sha256 = [];
    writeFileSync(s.manifest, mutation === 'duplicate-json' ? '{"v":1,"v":1}' : JSON.stringify(manifest));
    const before = tree(s.home); fail(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
  });
}

test('substituted mod directory symlink prevents partial uninstall or external deletion', t => {
  const s = sandbox(t); ok(s.run('install'));
  const path = join(s.home, 'mods/panorama-graph'), external = join(s.temp, 'external');
  mkdirSync(external); writeFileSync(join(external, 'main.js'), 'external personal');
  rmSync(path, { recursive: true }); symlinkSync(external, path);
  const before = tree(s.home), outside = tree(external); fail(s.run('uninstall'));
  assert.deepEqual(tree(s.home), before); assert.deepEqual(tree(external), outside);
});

test('missing/relative/nonexecutable/symlink binary fails without mutation', t => {
  const s = sandbox(t); const binary = artifact(s); const alias = join(s.temp, 'alias'); symlinkSync(binary, alias);
  const before = tree(s.home);
  for (const path of ['relative', join(s.temp, 'absent'), alias]) { fail(s.run('install', ['--binary', path])); assert.deepEqual(tree(s.home), before); }
  chmodSync(binary, 0o644); fail(s.run('install', ['--binary', binary])); assert.deepEqual(tree(s.home), before);
});

test('directory lock serializes competing installers; one succeeds and one refuses', async t => {
  const s = sandbox(t);
  const start = () => new Promise((resolve, reject) => {
    const child = spawn('python3', s.args('install'), { env: { ...s.env, TEST_DELAY: '0.08' } });
    let stderr = ''; child.stderr.on('data', chunk => { stderr += chunk; });
    child.on('error', reject); child.on('exit', code => resolve({ code, stderr }));
  });
  const results = await Promise.all([start(), start()]);
  assert.deepEqual(results.map(x => x.code).sort(), [0, 1], JSON.stringify(results));
  assert.equal(readFileSync(join(s.temp, 'checks.log'), 'utf8').trim().split('\n').length, 4);
  ok(s.run('uninstall')); assert.deepEqual(tree(s.home), {});
});

test('post-install binary content edits survive and are explicitly reported', t => {
  const s = sandbox(t); ok(s.run('install', ['--binary', artifact(s)]));
  const binary = join(s.home, 'data/panorama/bin/panorama'); writeFileSync(binary, 'personal binary');
  const result = s.run('uninstall'); ok(result); assert.match(result.stdout, /bin\/panorama \(edited; preserved\)/);
  assert.equal(readFileSync(binary, 'utf8'), 'personal binary');
});

test('key mutation during native checking refuses commit and preserves the personal edit', t => {
  const s = sandbox(t, { global: { 'alt-s': 'panorama.open' } }); personal(s);
  writeFileSync(s.checker, '#!/usr/bin/env python3\nimport os\nfrom pathlib import Path\nPath(os.environ["TEST_APP_HOME"], "keys.json").write_text(\'{"global":{"alt-x":"personal.new"}}\')\n', { mode: 0o755 });
  const result = s.run('install'); fail(result);
  assert.deepEqual(JSON.parse(readFileSync(join(s.home, 'keys.json'))), { global: { 'alt-x': 'personal.new' } });
  assert.deepEqual(Object.keys(tree(s.home)).filter(path => path.includes('panorama')), []);
  assert.equal(readFileSync(join(s.home, 'view.json'), 'utf8'), '{"layout":"personal"}\n');
});

test('killed staged native checking precedes the journal and leaves the original home untouched', async t => {
  const s = sandbox(t, { global: { 'alt-s': 'panorama.open' } }); personal(s);
  const before = tree(s.home);
  const child = spawn('python3', s.args('install'), { detached: true, env: { ...s.env, TMPDIR: s.temp, TEST_DELAY: '20' } });
  const exited = new Promise(resolve => child.on('exit', resolve));
  t.after(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; } });
  const deadline = Date.now() + 5000;
  while (!existsSync(join(s.temp, 'checks.log')) && Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 10));
  }
  assert.equal(existsSync(join(s.temp, 'checks.log')), true, 'staged checker was reached before journal or payload writes');
  process.kill(-child.pid, 'SIGKILL'); await exited;
  assert.equal(existsSync(s.manifest), false); assert.deepEqual(tree(s.home), before);
  fail(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
});

test('symlinked manifest and key file are refused without touching their targets', t => {
  const s = sandbox(t); ok(s.run('install'));
  const outside = join(s.temp, 'personal'); writeFileSync(outside, 'personal bytes');
  const manifest = readFileSync(s.manifest); rmSync(s.manifest); symlinkSync(outside, s.manifest);
  let before = tree(s.home); fail(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
  rmSync(s.manifest); writeFileSync(s.manifest, manifest, { mode: 0o600 });
  symlinkSync(outside, join(s.home, 'keys.json'));
  before = tree(s.home); fail(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
  assert.equal(readFileSync(outside, 'utf8'), 'personal bytes');
});

test('duplicate keys JSON and unsafe mod placement fail without mutation', t => {
  const s = sandbox(t); writeFileSync(join(s.home, 'keys.json'), '{"global":{},"global":{}}');
  let before = tree(s.home); fail(s.run('install')); assert.deepEqual(tree(s.home), before);
  rmSync(join(s.home, 'keys.json')); writeFileSync(join(s.packageDir, 'mods/panorama/mod.json'), '{"place":"left"}');
  before = tree(s.home); fail(s.run('install')); assert.deepEqual(tree(s.home), before);
});

test('coherent forged manifest delta cannot claim a personal binding as installer-owned', t => {
  const s = sandbox(t); personal(s); ok(s.run('install'));
  const manifest = JSON.parse(readFileSync(s.manifest));
  assert.equal(manifest.keys, null);
  const render = value => JSON.stringify(value, null, 2) + '\n';
  const personalKeys = JSON.parse(readFileSync(join(s.home, 'keys.json')));
  // Recompute every dependent field coherently: self-consistency is insufficient.
  writeFileSync(join(s.home, 'keys.json'), render(personalKeys));
  manifest.keys = {
    before: Buffer.from(render({ global: {} })).toString('base64'), before_mode: 0o640,
    after: Buffer.from(render(personalKeys)).toString('base64'), mode: 0o640,
    added: personalKeys, sections: [],
  };
  writeFileSync(s.manifest, JSON.stringify(manifest));
  const before = tree(s.home); const result = s.run('uninstall'); fail(result);
  assert.match(result.stderr, /key authority differs/); assert.deepEqual(tree(s.home), before);
});

test('coherent forged extra key is rejected even when the package defines valid own keys', t => {
  const s = sandbox(t, { 'mod:panorama': { 'alt-f': 'panorama.fold' } }); personal(s); ok(s.run('install'));
  const manifest = JSON.parse(readFileSync(s.manifest));
  const original = JSON.parse(Buffer.from(manifest.keys.before, 'base64'));
  delete original.global['alt-p'];
  manifest.keys.before = Buffer.from(JSON.stringify(original, null, 2) + '\n').toString('base64');
  manifest.keys.added.global = { 'alt-p': 'personal.command' };
  writeFileSync(s.manifest, JSON.stringify(manifest));
  const before = tree(s.home); fail(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
});

for (const scenario of ['journal', 'payload', 'keys', 'rollback']) {
  test(`post-publication ${scenario} failure preserves ownership and rollback/recovery`, t => {
    const s = sandbox(t, { global: { 'alt-s': 'panorama.open' } }); personal(s);
    const before = tree(s.home);
    const result = spawnSync('python3', [join(root, 'app/tests/write-fault.py'), root, s.home, s.packageDir, s.checker, scenario], { env: s.env, encoding: 'utf8' });
    ok(result); assert.match(result.stdout, /Fault injected and handled/);
    if (scenario === 'rollback') {
      assert.equal(existsSync(s.manifest), true, 'incomplete rollback keeps its ownership journal');
      assert.equal(existsSync(join(s.home, 'layouts/panorama.json')), true);
      assert.match(result.stderr, /recovery journal retained/);
      ok(s.run('uninstall'));
    } else assert.equal(existsSync(s.manifest), false, 'complete rollback leaves no journal');
    assert.deepEqual(tree(s.home), before);
  });
}

for (const scenario of ['crash-journal', 'crash-payload']) {
  test(`abrupt ${scenario} publication exit is recoverable by public uninstall`, t => {
    const s = sandbox(t); personal(s);
    const before = tree(s.home);
    const result = spawnSync('python3', [join(root, 'app/tests/write-fault.py'), root, s.home, s.packageDir, s.checker, scenario], { env: s.env, encoding: 'utf8' });
    assert.equal(result.status, 91, result.stderr);
    assert.equal(lstatSync(s.manifest).nlink, 1, 'published recovery journal remains a private regular file');
    if (scenario === 'crash-payload') assert.equal(lstatSync(join(s.home, 'layouts/panorama.json')).nlink, 1);
    assert.equal(Object.keys(tree(s.home)).some(path => path.includes('.panorama-')), false, 'publication leaves no second inode name');
    ok(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
  });
}

test('unrelated hardlink to an installed payload still refuses uninstall without mutation', t => {
  const s = sandbox(t); personal(s); ok(s.run('install'));
  const path = join(s.home, 'layouts/panorama.json'), outside = join(s.temp, 'external-link');
  linkSync(path, outside);
  const before = tree(s.home), sentinel = readFileSync(outside);
  const result = s.run('uninstall'); fail(result); assert.match(result.stderr, /Not a private regular file/);
  assert.deepEqual(tree(s.home), before); assert.deepEqual(readFileSync(outside), sentinel);
});

test('staged native metadata is explicitly fingerprinted; changed host-update metadata is preserved', t => {
  const s = sandbox(t); ok(s.run('install'));
  const manifest = JSON.parse(readFileSync(s.manifest));
  const metadata = Object.keys(manifest.files).filter(path => path.endsWith('/gpui-kit.d.ts') || path.endsWith('/jsconfig.json'));
  assert.equal(metadata.length, 8); assert.ok(metadata.every(path => manifest.files[path].mode === 0o644));
  const updated = join(s.home, 'mods/panorama/gpui-kit.d.ts'); writeFileSync(updated, '// next host version');
  const result = s.run('uninstall'); ok(result); assert.match(result.stdout, /gpui-kit.d.ts \(edited; preserved\)/);
  assert.equal(readFileSync(updated, 'utf8'), '// next host version'); assert.equal(existsSync(s.manifest), false);
  assert.equal(existsSync(join(s.home, 'mods/panorama/jsconfig.json')), false);
});

test('metadata post-publication fsync failure rolls back all declared generated payload', t => {
  const s = sandbox(t); personal(s); const before = tree(s.home);
  const result = spawnSync('python3', [join(root, 'app/tests/write-fault.py'), root, s.home, s.packageDir, s.checker, 'metadata'], { env: s.env, encoding: 'utf8' });
  ok(result); assert.match(result.stdout, /Fault injected and handled/); assert.deepEqual(tree(s.home), before);
});

test('receipt size cap refuses a large key merge without leaving an uninstallable install', t => {
  const s = sandbox(t, { global: { 'alt-s': 'panorama.open' } });
  writeFileSync(join(s.home, 'keys.json'), JSON.stringify({ global: { 'alt-p': 'p'.repeat(450000) } }));
  const before = tree(s.home), result = s.run('install'); fail(result); assert.match(result.stderr, /receipt exceeds 1 MiB/);
  assert.deepEqual(tree(s.home), before);
});

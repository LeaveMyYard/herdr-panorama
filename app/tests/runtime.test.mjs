import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdirSync, rmSync, chmodSync, existsSync, symlinkSync } from 'node:fs';
import { join } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { sandbox, tree, root as repo } from './helpers.mjs';
const ok = result => assert.equal(result.status, 0, result.stderr || result.stdout);
const fail = result => assert.notEqual(result.status, 0, result.stdout);
const sha = data => createHash('sha256').update(data).digest('hex');
const writeManaged = (path, value) => { writeFileSync(path, typeof value === 'string' || Buffer.isBuffer(value) ? value : JSON.stringify(value), { mode: 0o600 }); chmodSync(path, 0o600); };
function setup(t, binary = true, additions = {}) {
  const s = sandbox(t, additions);
  writeFileSync(join(s.home, 'keys.json'), '{ "global": { "alt-p": "personal" } }\n');
  const artifact = join(s.temp, 'artifact'); writeFileSync(artifact, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  ok(s.run('install', binary ? ['--binary', artifact] : []));
  s.receipt = JSON.parse(readFileSync(s.manifest));
  s.runtime = join(s.home, 'data/panorama/runtime');
  s.generation = join(s.runtime, s.receipt.install_id);
  s.control = join(s.generation, 'control.json');
  s.feed = join(s.home, 'data/panorama/feed.json');
  s.ownership = join(s.generation, 'ownership.json');
  return s;
}
function used(s, { committed = 'committed bytes\n', pending = null, feed = committed, temp = null } = {}) {
  const record = { v: 1, install_id: s.receipt.install_id, binary_sha256: s.receipt.files['data/panorama/bin/panorama'].sha256,
    writer_pid: 4242, writer_start: 'fixture-start', writer_pidns: null, feed_sha256: committed === null ? null : sha(committed), pending_feed_sha256: pending === null ? null : sha(pending) };
  writeManaged(s.ownership, record);
  if (feed !== null) writeManaged(s.feed, feed);
  if (temp !== null) writeManaged(join(s.home, `data/panorama/.feed-${s.receipt.install_id}.tmp`), temp);
  return record;
}
const withoutControl = (snapshot, s) => { const copy = { ...snapshot }; delete copy[`data/panorama/runtime/${s.receipt.install_id}/control.json`]; return copy; };

test('v2 installs reserve fresh generation/control/lock; reinstall never reuses identity', t => {
  const s = setup(t, false); assert.equal(s.receipt.v, 2); assert.match(s.receipt.install_id, /^[a-f0-9]{32}$/);
  assert.deepEqual(JSON.parse(readFileSync(s.control)), { v: 1, install_id: s.receipt.install_id, disabled: false });
  assert.equal(readFileSync(join(s.generation, 'writer.lock')).length, 0);
  assert.equal(Object.keys(s.receipt.files).some(path => path.includes('/runtime/')), false);
  ok(s.run('uninstall')); ok(s.run('install'));
  assert.notEqual(JSON.parse(readFileSync(s.manifest)).install_id, s.receipt.install_id);
  ok(s.run('uninstall')); assert.deepEqual(Object.keys(tree(s.home)), ['keys.json']);
});

const ledger = (s, failures = 2) => writeManaged(join(s.generation, 'start.json'), { v: 1, install_id: s.receipt.install_id, failures, retry_at_ms: 1793717546321 });
for (const state of ['committed', 'after-rename', 'pending-temp', 'start-ledger']) {
  test(`used runtime ${state} cleans only recorded committed/pending bytes`, t => {
    const s = setup(t);
    if (state === 'committed') used(s);
    if (state === 'after-rename') used(s, { pending: 'new bytes\n', feed: 'new bytes\n' });
    if (state === 'pending-temp') used(s, { pending: 'new bytes\n', temp: 'new bytes\n' });
    if (state === 'start-ledger') { used(s); ledger(s); }
    ok(s.run('uninstall')); assert.deepEqual(Object.keys(tree(s.home)), ['keys.json']);
  });
}

test('edited private binary is preserved after original-receipt runtime cleanup without execution', t => {
  const s = setup(t); used(s);
  const binary = join(s.home, 'data/panorama/bin/panorama');
  writeFileSync(binary, '#!/bin/sh\necho SHOULD_NEVER_EXECUTE\nexit 33\n');
  const result = s.run('uninstall'); ok(result); assert.match(result.stdout, /bin\/panorama \(edited; preserved\)/);
  assert.equal(existsSync(s.feed), false); assert.equal(existsSync(s.manifest), false);
  assert.match(readFileSync(binary, 'utf8'), /SHOULD_NEVER_EXECUTE/); assert.doesNotMatch(result.stdout, /^SHOULD_NEVER_EXECUTE$/m);
});

for (const mutation of ['feed-edited', 'ownership-foreign', 'ownership-invalid-pid', 'ownership-oversize', 'ownership-wrong-mode', 'ownership-temp', 'control-temp', 'generation-note', 'other-generation', 'foreign-feed-temp', 'partial-feed-temp', 'start-ledger-invalid', 'start-ledger-float', 'start-temp']) {
  test(`runtime ambiguity ${mutation} preserves all payloads and retains disabled receipt`, t => {
    const s = setup(t), record = used(s);
    if (mutation === 'feed-edited') writeManaged(s.feed, 'personal feed');
    if (mutation === 'ownership-foreign') { record.binary_sha256 = '0'.repeat(64); writeManaged(s.ownership, record); }
    if (mutation === 'ownership-invalid-pid') { record.writer_pid = true; writeManaged(s.ownership, record); }
    if (mutation === 'ownership-oversize') writeManaged(s.ownership, ' '.repeat(4097));
    if (mutation === 'ownership-wrong-mode') chmodSync(s.ownership, 0o644);
    if (mutation === 'ownership-temp') writeManaged(join(s.generation, '.ownership.tmp'), '{partial');
    if (mutation === 'control-temp') writeManaged(join(s.generation, '.control.tmp'), '{partial');
    if (mutation === 'generation-note') writeManaged(join(s.generation, 'personal.txt'), 'personal');
    if (mutation === 'other-generation') mkdirSync(join(s.runtime, 'other'));
    if (mutation === 'foreign-feed-temp') writeManaged(join(s.home, 'data/panorama/.feed-foreign.tmp'), 'personal');
    if (mutation === 'start-ledger-invalid') ledger(s, 8);
    if (mutation === 'start-ledger-float') writeManaged(join(s.generation, 'start.json'), `{"v":1,"install_id":"${s.receipt.install_id}","failures":2.0,"retry_at_ms":0}`);
    if (mutation === 'start-temp') writeManaged(join(s.generation, '.start.tmp'), '{partial');
    if (mutation === 'partial-feed-temp') writeManaged(join(s.home, `data/panorama/.feed-${s.receipt.install_id}.tmp`), '{partial');
    const before = tree(s.home), receiptBytes = readFileSync(s.manifest); const result = s.run('uninstall'); fail(result);
    assert.deepEqual(withoutControl(tree(s.home), s), withoutControl(before, s));
    assert.deepEqual(readFileSync(s.manifest), receiptBytes);
    assert.equal(JSON.parse(readFileSync(s.control)).disabled, mutation !== 'control-temp');
  });
}

test('runtime symlink never deletes outside the generation', t => {
  const s = setup(t); used(s);
  const outside = join(s.temp, 'personal'); writeFileSync(outside, 'personal'); rmSync(s.feed); symlinkSync(outside, s.feed);
  const before = tree(s.home); fail(s.run('uninstall'));
  assert.deepEqual(withoutControl(tree(s.home), s), withoutControl(before, s)); assert.equal(readFileSync(outside, 'utf8'), 'personal');
});

test('existing feed or foreign generation refuses new install without adoption or mutation', t => {
  for (const kind of ['feed', 'generation']) {
    const s = sandbox(t); mkdirSync(join(s.home, 'data/panorama/runtime'), { recursive: true });
    if (kind === 'feed') writeFileSync(join(s.home, 'data/panorama/feed.json'), 'personal');
    else mkdirSync(join(s.home, 'data/panorama/runtime/foreign'));
    const before = tree(s.home); fail(s.run('install')); assert.deepEqual(tree(s.home), before);
  }
});

function writer(t, s, mode = 'stop') {
  const marker = join(s.temp, 'stopped');
  const child = spawn('python3', [join(repo, 'app/tests/runtime-writer.py'), s.home, marker, mode], { stdio: ['ignore', 'pipe', 'pipe'] });
  const exited = new Promise(resolve => child.on('exit', resolve));
  t.after(async () => { if (child.exitCode === null) child.kill(); await exited; });
  return new Promise((resolve, reject) => {
    let error = ''; child.stderr.on('data', data => { error += data; });
    child.on('error', reject); child.on('exit', code => { if (code) reject(Error(error)); });
    child.stdout.once('data', data => { assert.match(String(data), /ready/); resolve({ child, exited, marker }); });
  });
}

test('public uninstall cooperatively stops an owned writer and restores the original home', async t => {
  const s = setup(t); const active = await writer(t, s);
  ok(s.run('uninstall')); assert.equal(await active.exited, 0); assert.equal(readFileSync(active.marker, 'utf8'), 'cooperatively stopped');
  assert.deepEqual(Object.keys(tree(s.home)), ['keys.json']);
});

test('busy writer timeout retains disabled control/receipt and never signals the process', async t => {
  const s = setup(t); const active = await writer(t, s, 'ignore');
  const before = readFileSync(s.manifest), start = Date.now(); const result = s.run('uninstall'); fail(result);
  assert.match(result.stderr, /timed out/); assert.ok(Date.now() - start >= 1900);
  assert.equal(active.child.exitCode, null); assert.equal(JSON.parse(readFileSync(s.control)).disabled, true);
  assert.deepEqual(readFileSync(s.manifest), before); assert.equal(existsSync(s.feed), true);
});

test('uninstall holds the lifecycle lock until receipt removal, preventing a waiting start from observing it', async t => {
  const s = setup(t); const active = await writer(t, s, 'delay');
  const uninstall = spawn('python3', s.args('uninstall'), { env: s.env });
  const finished = new Promise(resolve => uninstall.on('exit', resolve));
  const deadline = Date.now() + 3000;
  while (!JSON.parse(readFileSync(s.control)).disabled && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 5));
  const observer = spawnSync('python3', ['-c', 'import fcntl,os,pathlib,sys; fd=os.open(sys.argv[1],os.O_RDONLY|os.O_DIRECTORY); fcntl.flock(fd,fcntl.LOCK_EX); print(pathlib.Path(sys.argv[1],"data/panorama/install.json").exists())', s.home], { encoding: 'utf8' });
  ok(observer); assert.equal(observer.stdout.trim(), 'False'); assert.equal(await finished, 0); assert.equal(await active.exited, 0);
});

test('a frozen B1 v1 static receipt remains uninstallable by the v2 package without runtime authority', t => {
  const s = setup(t, false);
  const legacy = new Set(['layouts/panorama.json', 'mods/panorama/LICENSE.herdr-app', 'mods/panorama/NOTICE', ...['panorama', 'panorama-spaces', 'panorama-graph', 'panorama-peek'].flatMap(mod => [`mods/${mod}/main.js`, `mods/${mod}/mod.json`])]);
  for (const path of Object.keys(s.receipt.files)) if (!legacy.has(path)) { rmSync(join(s.home, path)); delete s.receipt.files[path]; }
  rmSync(s.runtime, { recursive: true });
  const v2OnlyDirs = new Set(['data/panorama-spaces','data/panorama-graph','data/panorama-peek','data/panorama-tabs','data/panorama-pane','mods/panorama-tabs','mods/panorama-pane']);
  for (const path of v2OnlyDirs) rmSync(join(s.home,path),{recursive:true});
  s.receipt.v = 1; delete s.receipt.install_id; s.receipt.dirs = s.receipt.dirs.filter(path => !path.startsWith('data/panorama/runtime') && !v2OnlyDirs.has(path));
  writeManaged(s.manifest, s.receipt);
  const foreign = join(s.home, 'data/panorama/runtime/unowned'); mkdirSync(foreign, { recursive: true });
  const control = join(foreign, 'control.json'); writeManaged(control, { disabled: false, personal: true });
  const original = readFileSync(control); ok(s.run('uninstall'));
  assert.deepEqual(readFileSync(control), original, 'v1 must not disable/adopt/remove runtime');
  assert.equal(existsSync(s.manifest), false); assert.equal(existsSync(join(s.home, 'mods/panorama')), false);
  assert.equal(readFileSync(join(s.home, 'keys.json'), 'utf8'), '{ "global": { "alt-p": "personal" } }\n');
});

test('v2 numeric integer spellings normalize safely, while booleans and oversized receipts refuse', t => {
  const s = setup(t, false);
  const text = readFileSync(s.manifest, 'utf8').replace('"v": 2', '"v": 2e0').replaceAll('"mode": 420', '"mode": 420.0');
  writeManaged(s.manifest, text); ok(s.run('uninstall'));
  ok(s.run('install'));
  const manifest = JSON.parse(readFileSync(s.manifest)); manifest.v = true; writeManaged(s.manifest, manifest);
  const before = tree(s.home); fail(s.run('uninstall')); assert.deepEqual(tree(s.home), before);
  writeManaged(s.manifest, ' '.repeat(1048577)); fail(s.run('uninstall'));
  assert.equal(existsSync(join(s.home, 'mods/panorama/main.js')), true);
});


test('busy transient ownership staging is evaluated only after the writer quiesces', async t => {
  const s = setup(t); const active = await writer(t, s, 'transient');
  assert.equal(existsSync(join(s.generation, '.ownership.tmp')), true);
  ok(s.run('uninstall')); assert.equal(await active.exited, 0);
  assert.deepEqual(Object.keys(tree(s.home)), ['keys.json']);
});

test('failed key restoration keeps the quiescent runtime journal recoverable until public retry', t => {
  const s = setup(t, true, { global: { 'alt-s': 'panorama.open' } }); used(s);
  const result = spawnSync('python3', [join(repo, 'app/tests/write-fault.py'), repo, s.home, s.packageDir, s.checker, 'uninstall-keys'], { env: s.env, encoding: 'utf8' });
  ok(result); assert.match(result.stdout, /Fault injected and handled/);
  assert.equal(existsSync(s.manifest), true); assert.equal(existsSync(join(s.generation, 'writer.lock')), true);
  assert.equal(JSON.parse(readFileSync(s.control)).disabled, true); assert.equal(existsSync(s.feed), true);
  ok(s.run('uninstall')); assert.deepEqual(Object.keys(tree(s.home)), ['keys.json']);
  assert.equal(readFileSync(join(s.home, 'keys.json'), 'utf8'), '{ "global": { "alt-p": "personal" } }\n');
});


for (const name of ['panorama-spaces','panorama-graph','panorama-peek']) {
  test(`exact mod-data reservation ${name} ${name === 'panorama-spaces' ? 'reuses its retained preference parent without deletion authority' : 'refuses preexisting personal directories'}`, t => {
    const s = sandbox(t); mkdirSync(join(s.home,'data',name),{recursive:true});
    writeFileSync(join(s.home,'data',name,'personal.json'),'personal');
    const before = tree(s.home); const result = s.run('install');
    if (name === 'panorama-spaces') {
      ok(result);
      assert.equal(JSON.parse(readFileSync(s.manifest)).dirs.includes(`data/${name}`),false);
      ok(s.run('uninstall'));
    } else {
      fail(result); assert.match(result.stderr,/Name collision/);
    }
    assert.deepEqual(tree(s.home),before);
  });
}
test('a retained spaces store with the sidebar scope is reused; an unsupported scope value refuses before mutation', t => {
  for (const [value, accepted] of [['"all"', true], ['"everything"', false]]) {
    const s = sandbox(t); mkdirSync(join(s.home,'data/panorama-spaces'),{recursive:true});
    writeManaged(join(s.home,'data/panorama-spaces/store.json'), { 'agents.scope': value, 'machines.folded': '[]' });
    const before = tree(s.home), result = s.run('install');
    if (accepted) { ok(result); ok(s.run('uninstall')); }
    else { fail(result); assert.match(result.stderr,/Unsupported retained preference shape: data\/panorama-spaces\/store.json\/agents.scope/); }
    assert.deepEqual(tree(s.home),before);
  }
});
test('the resume toggle in a retained spaces store is reused; an unsupported value refuses before mutation', t => {
  for (const [key, value, accepted] of [['agents.resume', '"on"', true], ['agents.resume', '"yes"', false], ['agents.resume.clicks', '{"t5":{"loss":1,"token":null,"at":2}}', true], ['agents.resume.clicks', '{"t5":1793971728356}', false]]) {
    const s = sandbox(t); mkdirSync(join(s.home,'data/panorama-spaces'),{recursive:true});
    writeManaged(join(s.home,'data/panorama-spaces/store.json'), { [key]: value });
    const before = tree(s.home), result = s.run('install');
    if (accepted) { ok(result); ok(s.run('uninstall')); }
    else { fail(result); assert.match(result.stderr,/Unsupported retained preference shape: data\/panorama-spaces\/store.json\/agents.resume/); }
    assert.deepEqual(tree(s.home),before);
  }
});
test('uninstall removes the generation\'s agent resume record and a crashed staging file', t => {
  const s = setup(t); used(s);
  writeManaged(join(s.generation, 'resume.json'), { v: 1, install_id: s.receipt.install_id, records: {} });
  writeManaged(join(s.generation, '.resume.tmp'), 'partial');
  ok(s.run('uninstall')); assert.deepEqual(Object.keys(tree(s.home)), ['keys.json']);
});
test('a foreign agent resume record is preserved and refuses uninstall', t => {
  for (const record of [{ v: 1, install_id: 'f'.repeat(32) }, { v: 2 }, []]) {
    const s = setup(t); used(s);
    const path = join(s.generation, 'resume.json');
    writeManaged(path, JSON.stringify(record));
    const result = s.run('uninstall'); fail(result);
    assert.match(result.stderr, /Ambiguous agent resume record/); assert.equal(existsSync(path), true);
  }
});
test('owned empty mod-data dirs are removed; added mod-data files survive without recursive cleanup', t => {
  const s = setup(t,false);
  for (const name of ['panorama-spaces','panorama-graph','panorama-peek']) assert.ok(s.receipt.dirs.includes(`data/${name}`));
  writeFileSync(join(s.home,'data/panorama-spaces/personal.json'),'personal');
  const result = s.run('uninstall');ok(result);
  assert.match(result.stdout,/data\/panorama-spaces \(not empty or changed; preserved\)/);
  assert.equal(readFileSync(join(s.home,'data/panorama-spaces/personal.json'),'utf8'),'personal');
  assert.equal(existsSync(join(s.home,'data/panorama-graph')),false);
  assert.equal(existsSync(join(s.home,'data/panorama-peek')),false);
  assert.equal(existsSync(s.manifest),false);
});

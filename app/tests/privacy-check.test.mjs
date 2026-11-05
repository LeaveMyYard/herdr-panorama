import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join, resolve } from 'node:path';
import test from 'node:test';

const script = resolve(import.meta.dirname, '../../scripts/privacy-check');
const TERM = 'plantedterm';

function repo(t, files) {
  const dir = mkdtempSync(join(tmpdir(), 'privacy-check-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const root = join(dir, 'repo');
  mkdirSync(root);
  for (const [name, text] of Object.entries(files)) writeFileSync(join(root, name), text);
  spawnSync('git', ['init', '-q'], { cwd: root });
  spawnSync('git', ['add', '-A'], { cwd: root });
  return { dir, root };
}

function list(dir, name, text) {
  const path = join(dir, name);
  writeFileSync(path, text);
  return path;
}

function run(root, args, env = process.env) {
  const out = spawnSync(script, args, { cwd: root, env, encoding: 'utf8' });
  return { status: out.status, text: out.stdout + out.stderr };
}

test('a clean tree passes and a content hit fails, printing counts only', (t) => {
  const { dir, root } = repo(t, { 'a.txt': `one ${TERM.toUpperCase()} two\n${TERM}\n` });
  const clean = run(root, [list(dir, 'clean', 'absent\n')]);
  assert.equal(clean.status, 0);
  assert.match(clean.text, /content hits: 0, file-name hits: 0/);
  const hit = run(root, [list(dir, 'hit', `${TERM}\n`)]);
  assert.equal(hit.status, 1);
  assert.match(hit.text, /content hits: 2, file-name hits: 0/);
  assert.doesNotMatch(hit.text, new RegExp(TERM, 'i'));
});

test('a terms file with no final newline keeps the next file\'s first term', (t) => {
  const { dir, root } = repo(t, { 'a.txt': `${TERM}\n` });
  const first = list(dir, 'first', 'absent');
  const second = list(dir, 'second', `${TERM}\n`);
  const out = run(root, [first, second]);
  assert.equal(out.status, 1);
  assert.match(out.text, /terms: 2, content hits: 1/);
});

test('a missing or unusable terms file fails even beside a good one', (t) => {
  const { dir, root } = repo(t, { 'a.txt': 'text\n' });
  const good = list(dir, 'good', 'absent\n');
  const nope = join(dir, 'nope');
  for (const args of [[nope], [good, nope], [nope, good], [dir]]) {
    assert.equal(run(root, args).status, 2, args.join(' '));
  }
  assert.equal(run(root, [list(dir, 'blank', '\n  \r\n')]).status, 2);
  assert.equal(run(root, []).status, 2);
});

test('a failing git grep is a failed check, not a clean tree', (t) => {
  const { dir, root } = repo(t, { 'a.txt': `${TERM}\n` });
  const real = spawnSync('sh', ['-c', 'command -v git'], { encoding: 'utf8' }).stdout.trim();
  const bin = join(dir, 'bin');
  mkdirSync(bin);
  const shim = join(bin, 'git');
  writeFileSync(shim, `#!/bin/sh\n[ "$1" = grep ] && exit 128\nexec "${real}" "$@"\n`);
  chmodSync(shim, 0o755);
  const env = { ...process.env, PATH: `${bin}${delimiter}${process.env.PATH}` };
  const out = run(root, [list(dir, 'terms', `${TERM}\n`)], env);
  assert.equal(out.status, 2);
  assert.doesNotMatch(out.text, /content hits/);
});

test('file names match with non-ASCII paths and CRLF term lines', (t) => {
  const { dir, root } = repo(t, { 'naïve-tèrm.txt': 'text\n', 'plain-crlfname.txt': 'text\n' });
  const utf8 = run(root, [list(dir, 'utf8', 'tèrm\n')]);
  assert.equal(utf8.status, 1);
  assert.match(utf8.text, /file-name hits: 1/);
  const crlf = run(root, [list(dir, 'crlf', 'crlfname\r\n')]);
  assert.equal(crlf.status, 1);
  assert.match(crlf.text, /file-name hits: 1/);
});

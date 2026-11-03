import test from 'node:test';
import assert from 'node:assert/strict';
import { Pages, parsePage } from '../mods/panorama-peek/paging.js';
const result = (text, first, last, lines) => ({ code: 0, stdout: JSON.stringify({ text, first, last, lines }), stderr: '' });
const scope = { key: 'G/root/agent/path', path: "/literal ' ; $(never).jsonl" };
test('page validation preserves UTF8/CRLF/NUL and rejects false counters or structures', () => {
  const text = '雪\r\nbefore\0AFTER\n';
  assert.equal(parsePage(result(text, 1, 2, 3), 1).page.text, text);
  assert.equal(parsePage(result('', 1, 0, 0), 1).page.lines, 0);
  for (const reply of [result(text, 2, 3, 3), result(text, 1, 1, 3), { code: 0, stdout: '[]' }, result('x', 1, 1, Number.MAX_SAFE_INTEGER + 1)]) assert.ok(parsePage(reply, 1).error);
});
test('next errors retain exact prior page and cursor with scoped raw error; no skip or fallback', async () => {
  const calls = [];
  const pages = new Pages(async args => { calls.push(args); return calls.length === 1 ? result('first\n', 1, 1, 2) : { code: 6, stdout: '', stderr: 'line 2 is over 8 MB' }; }, () => true);
  assert.equal(await pages.load(scope), true);
  const before = pages.page;
  assert.equal(await pages.next(scope), false);
  assert.equal(pages.page, before); assert.deepEqual(pages.history, []);
  assert.deepEqual(pages.error, { raw: 'line 2 is over 8 MB', scope: scope.key, path: scope.path, offset: 2 });
  assert.equal(calls.length, 2); assert.equal(calls[0].at(-1), scope.path);
  assert.deepEqual(calls[0].slice(0, -1), ['file', 'read', '--json', '--offset', '1', '--limit', '100']);
});
test('old async reads and errors never publish after generation/intent/close changes', async () => {
  const pending = []; let current = true;
  const pages = new Pages(() => new Promise(resolve => pending.push(resolve)), () => current);
  const old = pages.load(scope); current = false; pending.shift()(result('old\n', 1, 1, 1));
  assert.equal(await old, false); assert.equal(pages.page, null); assert.equal(pages.error, null);
  current = true; const closed = pages.load(scope); pages.close(); pending.shift()({ code: 1, stdout: '', stderr: 'late failure' });
  assert.equal(await closed, false); assert.equal(pages.page, null); assert.equal(pages.error, null);
});
test('adjacent pages use actual returned range and only successful reads change history', async () => {
  const replies = [result('one\ntwo\n', 1, 2, 3), result('three', 3, 3, 3), result('one\ntwo\n', 1, 2, 3)];
  const calls = []; const pages = new Pages(async args => { calls.push(args); return replies.shift(); }, () => true);
  await pages.load(scope); await pages.next(scope); assert.equal(pages.page.first, 3); assert.deepEqual(pages.history, [1]);
  await pages.previous(scope); assert.equal(pages.page.last, 2); assert.deepEqual(pages.history, []);
  assert.deepEqual(calls.map(args => args[4]), ['1', '3', '1']);
});

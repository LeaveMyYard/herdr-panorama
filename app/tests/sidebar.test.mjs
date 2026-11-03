import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { root as repo } from './helpers.mjs';
import { parseFeed, readResult, rootKey, threadKey, agentKey } from '../shared/contract.js';
import { rootTarget, detailTarget } from '../shared/routing.js';
import { aggregate, forest, annotate, rows, isOpen, threadMark } from '../shared/tree.js';
import { joinRoots, harnessOf, iconProgram, nativeMembership, spaceTotals } from '../shared/model.js';
const fixture = () => JSON.parse(readFileSync(join(repo, 'tests/fixtures/app-feed/swarm.json')));
const root = () => fixture().orchestrators[0];
const agent = (id, status = 'run', parent = null) => ({ ...root().threads[0].agents[0], id, status, parent, question_ids: [], questions: [], harness: 'codex' });
const thread = agents => ({ ...root().threads[0], id: 'thread', agents, status: 'active' });
const minimal = (agents = []) => ({ ...root(), inbox: [], inbox_texts: [], threads: [thread(agents)], herdr_status: 'working' });

test('A1 fixtures parse and invalid read/version retains the last valid snapshot with provenance', () => {
  const first = readResult(null, JSON.stringify(fixture())); assert.equal(first.stale, false);
  for (const text of [null, '{broken', '{"v":2}']) {
    const next = readResult(first, text); assert.equal(next.feed, first.feed); assert.equal(next.stale, true); assert.equal(next.generated_at, first.generated_at);
  }
  assert.equal(readResult(null, null).feed, null);
});

test('required fields, safe integer values and exact ownership reject malformed enrichment', () => {
  for (const mutate of [
    feed => { delete feed.orchestrators[0].host; },
    feed => { feed.v = true; },
    feed => { feed.orchestrators[0].threads[0].agents[0].seq = Number.MAX_SAFE_INTEGER + 1; },
    feed => { feed.orchestrators[0].threads[0].agents[0].seq = 0.5; },
    feed => { feed.orchestrators.push(structuredClone(feed.orchestrators[0])); },
    feed => { feed.orchestrators[0].threads[0].agents[0].question_ids = [999]; },
    feed => { feed.orchestrators[0].workspace_id = 'fabricated-space'; },
    feed => { feed.inbox_ok = false; },
  ]) { const feed = fixture(); mutate(feed); assert.throws(() => parseFeed(JSON.stringify(feed))); }
  const text = JSON.stringify(fixture()).replace('"v":1,', '"v":1e0,');
  assert.equal(parseFeed(text).v, 1);
  const additive = fixture(); additive.future = { any: [] }; additive.counts.future = -123;
  assert.equal(parseFeed(JSON.stringify(additive)).counts.future, -123);
});

test('forest keeps dangling, self-parent and cycles exactly once without depth guessing', () => {
  const input = [agent('a', 'run', 'b'), agent('b', 'wait', 'a'), agent('self', 'q', 'self'), agent('missing', 'q', 'absent')];
  input[0].depth = 99;
  const roots = forest(input), seen = [], stack = roots.slice();
  while (stack.length) { const node = stack.pop(); seen.push(node.agent.id); stack.push(...node.children); }
  assert.deepEqual(seen.sort(), input.map(agent => agent.id).sort());
  assert.equal(new Set(seen).size, 4);
});

test('arbitrary-depth needs paths are reconstructed from parents and remain visible', () => {
  const agents = Array.from({ length: 150 }, (_, i) => agent(String(i), i === 149 ? 'fail' : 'done', i ? String(i - 1) : null));
  const r = minimal(agents); const visible = rows(r, new Map(), true).filter(row => row.kind === 'agent');
  assert.equal(visible.length, 150); assert.equal(visible.at(-1).lines.length, 151);
  assert.equal(visible[0].agent.id, '0'); assert.equal(visible.at(-1).agent.id, '149');
});

test('default news folding preserves ancestor path and a final completed-sibling summary', () => {
  const r = minimal([agent('parent', 'run'), agent('failed', 'fail', 'parent'), agent('done', 'done', 'parent'), agent('hidden', 'run')]);
  const visible = rows(r, new Map(), true);
  assert.deepEqual(visible.map(row => row.kind === 'agent' ? row.agent.id : row.kind), ['thread', 'parent', 'failed', 'summary']);
  assert.equal(visible.at(-1).count, 1);
  const folds = new Map(), summary = visible.at(-1); folds.set(summary.key, { open: true, signature: summary.signature });
  assert.deepEqual(rows(r, folds, true).filter(row => row.kind === 'agent').map(row => row.agent.id), ['parent', 'failed', 'done']);
});

test('manual folds override defaults until a subtree status/question change', () => {
  const r = minimal([agent('a', 'wait')]); const folds = new Map();
  const initial = rows(r, folds, true), entry = initial[0];
  folds.set(entry.key, { open: false, signature: entry.signature });
  assert.deepEqual(rows(r, folds, true).map(row => row.kind), ['thread']);
  r.threads[0].agents[0].status = 'fail';
  assert.deepEqual(rows(r, folds, true).map(row => row.kind), ['thread', 'agent']);
  assert.equal(folds.has(entry.key), false);
});

test('finished threads fold into tasks summary and explicit expansion shows them', () => {
  const r = minimal([]); r.threads[0].status = 'done';
  const folds = new Map(), initial = rows(r, folds, true);
  assert.equal(initial.length, 1); assert.equal(initial[0].tasks, true);
  folds.set(initial[0].key, { open: true, signature: initial[0].signature });
  assert.deepEqual(rows(r, folds, true).map(row => row.kind), ['thread']);
});

test('needs and question counts are distinct; repeated attribution does not double-add', () => {
  const r = minimal([agent('a', 'wait'), agent('b', 'blk'), agent('c', 'fail')]);
  r.inbox = [14]; r.inbox_texts = ['question'];
  for (const a of r.threads[0].agents) { a.question_ids = [14]; a.questions = ['question']; }
  const result = aggregate(r);
  assert.equal(result.questions, 1); assert.equal(result.needsCount, 3); assert.deepEqual(result.kinds, ['fail', 'blk', 'wait']);
  assert.equal(threadMark(r.threads[0]).questions, 1);
  r.threads[0].agents = []; assert.equal(aggregate(r).needsCount, 1);
});

test('isolated empty failed thread has fail/Needs/+1 fallback while keeping own working state', () => {
  const r = parseFeed(readFileSync(join(repo, 'tests/fixtures/app-feed/empty-failed.json'), 'utf8')).orchestrators[0];
  const result = aggregate(r);
  assert.deepEqual(result.kinds, ['fail']); assert.equal(result.priority, 4); assert.equal(result.needsCount, 1);
  assert.equal(result.working, true); assert.equal(r.herdr_status, 'working'); assert.equal(result.sweep, false);
  assert.equal(threadMark(r.threads[0]).status, 'fail');
});

test('exact qualified core join enriches once; parity, unknown roots and missing live targets survive', () => {
  const r = root(); r.terminal = 'vps/t7'; r.tab_label = 'feed metadata';
  const f = { orchestrators: [r] }, stock = [{ address: 't7', space: 's1', tab: 'tb1', space_name: 'Local', status: 'idle', seq: 1, program: { key: 'amp', icon: 'icons/amp.svg' } }, { address: 'vps/t7', space: 's2', tab: 'tb2', space_name: 'Remote', status: 'working', seq: 2, program: { key: 'codex', icon: 'icons/codex.svg' } }, { address: 't9', space: 's1', tab: 'tb1', space_name: 'Parity', status: 'done', seq: 3 }];
  const known = [{ address: 'vps/t7', state: 'live' }];
  const joined = joinRoots(f, stock, known, [], []);
  assert.equal(joined.length, 3); assert.equal(joined.filter(item => item.root === r).length, 1);
  assert.equal(joined.find(item => item.root === r).stock.address, 'vps/t7');
  assert.equal(joined.find(item => item.stock.address === 't7').root.threads.length, 0);
  assert.equal(joined.find(item => item.stock.address === 't7').terminal, null);
  assert.equal(joinRoots(f, stock, [], [], []).find(item => item.root === r).terminal, null);
  r.host = 'herdr-tui';
  const unmatched = joinRoots(f, stock, known, [], []).find(item => !item.stock);
  assert.equal(unmatched.name, 'feed metadata'); assert.equal(unmatched.space, null); assert.equal(unmatched.terminal, null);
  assert.equal(joinRoots(f, stock, known, [], []).length, 4);
});

test('legacy cwd/session/title matches never claim membership and unsupported harnesses stay spelled', () => {
  const r = root(); r.host = 'unknown'; r.terminal = 't1'; r.tab_label = '';
  const joined = joinRoots({ orchestrators: [r] }, [], [{ address: 't1', state: 'live' }], [{ terminal: 't1', tab: 'tb1' }], [{ id: 's1', tabs: [{ id: 'tb1' }] }]);
  assert.equal(joined[0].name, r.pane_id); assert.equal(joined[0].terminal, null); assert.equal(joined[0].space, null);
  const a = agent('a'); a.harness = 'future-harness';
  assert.equal(harnessOf(a, r, thread([a])), 'future-harness'); assert.equal(iconProgram('future-harness').icon, 'icons/agent.svg');
  const p = agent('parent'); p.harness = 'gemini'; a.harness = 'unknown'; a.parent = p.id;
  assert.equal(harnessOf(a, r, thread([p, a])), 'gemini');
});

test('pane-less native descendants preserve urgency and exact child enrichment appears once', () => {
  const r = root(); r.terminal = 'vps/t7';
  const stock = [{ address: 't1', space: 's1', tab: 'tb1', space_name: 'Native', status: 'idle', seq: 1 }];
  const terminals = [{ address: 't1', state: 'live', status: 'idle' }, { address: 'vps/t7', parent: 't1', state: 'live', status: 'blocked', program: { key: 'future-native', icon: 'icons/agent.svg' } }, { address: 't9', parent: 'vps/t7', state: 'live', status: 'working' }];
  const joined = joinRoots({ orchestrators: [r] }, stock, terminals, [], []);
  assert.equal(joined.length, 1); assert.equal(joined[0].nativeChildren.length, 2);
  assert.equal(joined[0].nativeChildren.filter(node => node.root === r).length, 1);
  assert.equal(joined[0].aggregate.priority, 4); assert.equal(joined[0].aggregate.working, true);
  assert.equal(joined[0].nativeChildren[0].terminal, 'vps/t7'); assert.equal(joined[0].nativeChildren[0].program.key, 'future-native');
});

test('every listed stock descendant remains top-level and owns its own unlisted subtree', () => {
  const stock = [{ address: 't1', space: 's1', tab: 'tb1', status: 'idle', seq: 1 }, { address: 't2', space: 's2', tab: 'tb2', status: 'working', seq: 2 }];
  const terminals = [{ address: 't1', state: 'live' }, { address: 't2', state: 'live', parent: 't1' }, { address: 't3', state: 'live', parent: 't2', status: 'blocked' }];
  const joined = joinRoots(null, stock, terminals, [], []);
  assert.equal(joined.length, 2); assert.equal(joined.find(item => item.stock.address === 't1').nativeChildren.length, 0);
  assert.deepEqual(joined.find(item => item.stock.address === 't2').nativeChildren.map(item => item.terminal), ['t3']);
});

test('native membership uses direct live joins before verified parent ancestry and rejects cycles/gaps', () => {
  const stock = [{ address: 't1', space: 'parent-space', tab: 'parent-tab', status: 'idle' }];
  const terminals = [{ address: 't1', state: 'live' }, { address: 't2', parent: 't1', state: 'live' }, { address: 't3', parent: 't2', state: 'live' }];
  const direct = [{ terminal: 't2', tab: 'child-tab' }], spaces = [{ id: 'direct-space', tabs: [{ id: 'child-tab' }] }];
  let joined = joinRoots(null, stock, terminals, direct, spaces);
  assert.equal(joined[0].nativeChildren.find(item => item.terminal === 't2').space, 'direct-space');
  assert.equal(joined[0].nativeChildren.find(item => item.terminal === 't3').space, 'direct-space');
  joined = joinRoots(null, stock, terminals, [], []);
  assert.equal(joined[0].nativeChildren.find(item => item.terminal === 't3').space, 'parent-space');
  assert.equal(nativeMembership('x', stock, [{ address: 'x', parent: 'y', state: 'live' }, { address: 'y', parent: 'x', state: 'live' }], [], []).space, null);
  assert.equal(nativeMembership('x', stock, [{ address: 'x', parent: 'missing', state: 'live' }], [], []).space, null);
  assert.equal(nativeMembership('t2', stock, [{ address: 't2', parent: 't1', state: 'live' }, { address: 't1', state: 'finished' }], [], []).space, null);
});


test('routing focuses verified stock roots, peeks native children and never treats detailed IDs as addresses', () => {
  const r = root(), item = { root: r, terminal: 'vps/t7' };
  assert.deepEqual(rootTarget(item), { name: 'agent.focus', args: { terminal: 'vps/t7' } });
  assert.deepEqual(rootTarget({ ...item, isNative: true }), { name: 'terminal.peek', args: { terminal: 'vps/t7' } });
  assert.equal(rootTarget({ ...item, terminal: null }), null);
  const row = { thread: r.threads[0], agent: agent('t99') };
  const route = detailTarget({ ...item, terminal: null }, row);
  assert.equal(route.mod, 'panorama-peek'); assert.equal(route.props.agent.id, 't99'); assert.equal(route.props.terminal, null);
  assert.equal(route.name, undefined);
});

test('healthy threads retain their mark without inventing a completed-summary row in news folding', () => {
  const r = minimal([agent('done', 'done'), agent('reviewing', 'rev')]);
  assert.deepEqual(rows(r, new Map(), true).map(row => row.kind), ['thread']);
});


test('opaque root/thread/agent IDs cannot collide with each other or finished summaries', () => {
  const r = minimal([agent('thread', 'fail')]);
  assert.notEqual(threadKey(r, r.threads[0]), agentKey(r, r.threads[0].agents[0]));
  const other = { ...thread([]), id: '$finished', status: 'done' };
  r.threads.push(other);
  const visible = rows(r, new Map(), true);
  assert.equal(new Set(visible.map(row => row.key)).size, visible.length);
  assert.notEqual(visible.at(-1).key, threadKey(r, other));
});

test('joined nativeChildren arrays cannot change a stock root click into terminal peek', () => {
  const stock = [{ address: 't1', status: 'working', space: 's', tab: 'tb' }];
  const joined = joinRoots(null, stock, [{ address: 't1', state: 'live' }], [], []);
  assert.deepEqual(joined[0].nativeChildren, []);
  assert.equal(joined[0].isNative, false);
  assert.equal(rootTarget(joined[0]).name, 'agent.focus');
});


test('listed descendants keep top-level ownership while their needs project onto ancestors', () => {
  const stock = [{ address: 't1', status: 'idle', space: 's', tab: 'tb' }, { address: 't2', status: 'blocked', space: 's', tab: 'tb2' }];
  const live = [{ address: 't1', state: 'live' }, { address: 't2', parent: 't1', state: 'live', status: 'blocked' }];
  const joined = joinRoots(null, stock, live, [], []);
  assert.equal(joined.length, 2); assert.equal(joined.find(item => item.stock.address === 't1').nativeChildren.length, 0);
  assert.equal(joined.find(item => item.stock.address === 't1').aggregate.priority, 4);
  assert.equal(spaceTotals(joined, 's').needsCount, 1, 'projected parent does not double-add listed child');
});

test('space totals use direct child membership and scoped questions without projected double-counting', () => {
  const source = root(); source.terminal = 't2';
  const stock = [{ address: 't1', status: 'working', space: 'parent', tab: 'p' }];
  const live = [{ address: 't1', state: 'live' }, { address: 't2', parent: 't1', state: 'live', status: 'blocked' }];
  const joined = joinRoots({ orchestrators: [source] }, stock, live, [{ terminal: 't2', tab: 'direct' }], [{ id: 'child', tabs: [{ id: 'direct' }] }]);
  assert.equal(spaceTotals(joined, 'parent').needsCount, 0);
  assert.equal(spaceTotals(joined, 'child').needsCount, aggregate(source, 'blocked').needsCount);
  assert.equal(spaceTotals(joined, 'child').questions, 2);
  assert.equal(spaceTotals([...joined, joined[0]], 'child').needsCount, spaceTotals(joined, 'child').needsCount);
  assert.equal(spaceTotals(joined, 'missing').needsCount, 0);
});

test('idle roots with working descendants promote Working; own done remains Done and child done never promotes', () => {
  const r = minimal([agent('run', 'run')]); r.herdr_status = 'idle'; assert.equal(aggregate(r).priority, 2);
  const stock = [{ address: 't1', status: 'idle', space: 's', tab: 'tb' }];
  const live = [{ address: 't1', state: 'live' }, { address: 't2', parent: 't1', state: 'live', status: 'working' }];
  assert.equal(joinRoots(null, stock, live, [], [])[0].aggregate.priority, 2);
  stock[0].status = 'done'; assert.equal(joinRoots(null, stock, live, [], [])[0].aggregate.priority, 3);
  stock[0].status = 'idle'; live[1].status = 'done';
  assert.equal(joinRoots(null, stock, live, [], [])[0].aggregate.priority, 1);
});

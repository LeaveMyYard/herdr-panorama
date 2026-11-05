import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { root as repo } from './helpers.mjs';
import { parseFeed, readResult, rootKey, agentKey, probeMatches, requireProbe, aged } from '../shared/contract.js';
import { rootTarget, detailTarget } from '../shared/routing.js';
import { aggregate, forest, rows, directTiles, agentTitle } from '../shared/tree.js';
import { joinRoots, harnessOf, iconProgram, nativeMembership, spaceTotals, rootTitle, listed, groups, headerTotals, storedScope, cardPlace } from '../shared/model.js';
const read = name => readFileSync(join(repo, `tests/fixtures/app-feed/${name}.json`), 'utf8');
const fixture = () => JSON.parse(read('swarm'));
const root = () => fixture().roots[0];
const agent = (id, status = 'run', parent = null) => ({ ...root().agents[0], id, title: `T ${id}`, status, parent, question_ids: [], questions: [], harness: 'codex' });
const minimal = (agents = []) => ({ ...root(), inbox: [], inbox_texts: [], agents, hidden_finished: 0, herdr_status: 'working' });
const ids = list => list.map(row => row.kind === 'agent' ? row.agent.id : row.kind);

test('v2 fixtures parse; invalid, v1 and unknown versions retain the last valid snapshot with provenance', () => {
  for (const name of ['swarm', 'degraded', 'own-blocked']) assert.equal(parseFeed(read(name)).v, 2);
  const first = readResult(null, read('swarm')); assert.equal(first.stale, false);
  for (const text of [null, '{broken', '{"v":2}', JSON.stringify({ ...fixture(), v: 1 }), JSON.stringify({ ...fixture(), v: 3 })]) {
    const next = readResult(first, text); assert.equal(next.feed, first.feed); assert.equal(next.stale, true); assert.equal(next.generated_at, first.generated_at);
  }
  assert.equal(readResult(null, null).feed, null);
});

test('the capability probe is exactly the first stdout line `panorama app-feed v2`', async () => {
  for (const answer of [{ code: 0, stdout: 'panorama app-feed v1\n' }, { code: 1, stdout: 'panorama app-feed v2\n' }, null]) await assert.rejects(requireProbe(async () => answer));
  assert.equal((await requireProbe(async () => ({ code: 0, stdout: 'panorama app-feed v2\n' }))).code, 0);
  assert.equal(probeMatches('panorama app-feed v2\nusage'), true);
  for (const stdout of ['panorama app-feed v1\n', ' panorama app-feed v2', 'x\npanorama app-feed v2', 'panorama app-feed v20', null]) assert.equal(probeMatches(stdout), false);
});

test('required fields, safe integer values and exact ownership reject malformed enrichment', () => {
  for (const mutate of [
    feed => { delete feed.roots[0].host; },
    feed => { delete feed.roots[0].title; },
    feed => { delete feed.roots[0].agents[0].title_source; },
    feed => { feed.orchestrators = feed.roots; delete feed.roots; },
    feed => { feed.v = true; },
    feed => { feed.roots[0].agents[0].seq = Number.MAX_SAFE_INTEGER + 1; },
    feed => { feed.roots[0].agents[0].seq = 0.5; },
    feed => { feed.roots[0].hidden_finished = -1; },
    feed => { feed.roots.push(structuredClone(feed.roots[0])); },
    feed => { feed.roots[0].agents.push(structuredClone(feed.roots[0].agents[0])); },
    feed => { feed.roots[0].agents[0].question_ids = [999]; },
    feed => { feed.roots[0].workspace_id = 'fabricated-space'; },
    feed => { feed.inbox_ok = false; },
  ]) { const feed = fixture(); mutate(feed); assert.throws(() => parseFeed(JSON.stringify(feed))); }
  const text = JSON.stringify(fixture()).replace('"v":2,', '"v":2e0,');
  assert.equal(parseFeed(text).v, 2);
  const additive = fixture(); additive.future = { any: [] }; additive.counts.future = -123; additive.roots[0].agents[0].parent = 'absent';
  assert.equal(parseFeed(JSON.stringify(additive)).counts.future, -123, 'unknown fields and dangling parents are valid');
});

test('forest keeps dangling, self-parent and cycles exactly once without depth guessing', () => {
  const input = [agent('a', 'run', 'b'), agent('b', 'wait', 'a'), agent('self', 'q', 'self'), agent('missing', 'q', 'absent')];
  input[0].depth = 99;
  const roots = forest(input), seen = [], stack = roots.slice();
  while (stack.length) { const node = stack.pop(); seen.push(node.agent.id); stack.push(...node.children); }
  assert.deepEqual(seen.sort(), input.map(agent => agent.id).sort());
  assert.equal(new Set(seen).size, 4);
});

test('two row layers: deeper descendants at any depth fold into the layer-2 state run', () => {
  const agents = Array.from({ length: 150 }, (_, i) => agent(String(i), i === 149 ? 'fail' : 'done', i ? String(i - 1) : null));
  const visible = rows(minimal(agents), new Map(), true);
  assert.deepEqual(ids(visible), ['0', '1']);
  assert.deepEqual(visible.map(row => row.layer), [1, 2]);
  assert.equal(visible[1].run.statuses.length, 148); assert.ok(visible[1].run.statuses.includes('fail'));
  assert.deepEqual(visible[1].lines, ['', '└']);
});

test('the fixture root lists layer 1 and 2 with V11 runs and a +N older fold', () => {
  const r = root(), visible = rows(r, new Map(), true);
  assert.deepEqual(ids(visible), ['agent-0', 'agent-1', 'agent-6', 'agent-8', 'agent-10', 'agent-11', 'agent-12', 'summary']);
  const [author, reviewer] = visible;
  assert.equal(author.open, true); assert.equal(author.run.finished, 3, 'done, cxl and inf children count only as +N✓');
  assert.deepEqual(reviewer.run.statuses.slice().sort(), ['blk', 'fail', 'wait']);
  const fold = visible.at(-1); assert.equal(fold.count, 0); assert.equal(fold.older, 2); assert.deepEqual(fold.lines, ['└']);
  assert.equal(agentTitle(author.agent), 'Implement the migration');
});

test('finished direct subagents fold into one N finished · titles row that expands on demand', () => {
  const r = minimal([agent('parent', 'run'), agent('failed', 'fail', 'parent'), agent('done', 'done', 'parent'), agent('hidden', 'run'), agent('old', 'done'), agent('older', 'cxl')]);
  const visible = rows(r, new Map(), true);
  assert.deepEqual(ids(visible), ['parent', 'failed', 'hidden', 'summary']);
  assert.equal(visible[0].run.finished, 1);
  const summary = visible.at(-1); assert.equal(summary.count, 2); assert.deepEqual(summary.names, ['T old', 'T older']);
  const folds = new Map([[summary.key, { open: true, signature: summary.signature }]]);
  assert.deepEqual(ids(rows(r, folds, true)), ['parent', 'failed', 'hidden', 'old', 'older', 'summary']);
});

test('manual folds override defaults until a subtree status/question change', () => {
  const r = minimal([agent('a', 'run'), agent('b', 'wait', 'a')]), folds = new Map();
  const entry = rows(r, folds, true)[0];
  folds.set(entry.key, { open: false, signature: entry.signature });
  const folded = rows(r, folds, true);
  assert.deepEqual(ids(folded), ['a']); assert.deepEqual(folded[0].run.statuses, ['wait']);
  r.agents[1].status = 'fail';
  assert.deepEqual(ids(rows(r, folds, true)), ['a', 'b']);
  assert.equal(folds.has(entry.key), false);
});

test('an empty root has no rows: no duplicate title row and no tasks-done fold', () => {
  assert.deepEqual(rows(minimal([]), new Map(), true), []);
  assert.deepEqual(rows(minimal([agent('a')]), new Map(), false), []);
  const aged = { ...minimal([]), hidden_finished: 3 }, only = rows(aged, new Map(), true);
  assert.deepEqual(ids(only), ['summary']); assert.equal(only[0].count, 0); assert.equal(only[0].older, 3);
  assert.deepEqual(directTiles(minimal([])), { tiles: [], done: 0, active: 0, live: 0 });
});

test('line-2 tiles are per direct subagent: its subtree\'s most urgent state, distinct questions, ✓n', () => {
  const line = directTiles(root(), 2);
  assert.equal(line.active, 6); assert.equal(line.done, 0); assert.equal(line.live, 10 + 2);
  assert.deepEqual(line.tiles[0], { status: 'fail', questions: 1, key: 'agent-0' });
  const finished = directTiles(minimal([agent('a', 'done'), agent('b', 'cxl'), agent('c', 'done', 'a')]));
  assert.deepEqual([finished.active, finished.done, finished.live], [0, 2, 0]);
});

test('needs and question counts are distinct; repeated attribution does not double-add', () => {
  const r = minimal([agent('a', 'wait'), agent('b', 'blk', 'a'), agent('c', 'fail', 'b')]);
  r.inbox = [14]; r.inbox_texts = ['question'];
  for (const a of r.agents) { a.question_ids = [14]; a.questions = ['question']; }
  const result = aggregate(r);
  assert.equal(result.questions, 1); assert.equal(result.needsCount, 3); assert.deepEqual(result.kinds, ['fail', 'blk', 'wait']);
  assert.equal(directTiles(r).tiles[0].questions, 1);
  r.agents = []; assert.equal(aggregate(r).needsCount, 1);
});

test('a root whose only problem is its own blocked status needs the operator once', () => {
  const r = parseFeed(read('own-blocked')).roots[0], result = aggregate(r);
  assert.deepEqual(result.kinds, ['blk']); assert.equal(result.priority, 4); assert.equal(result.needsCount, 1);
  assert.equal(result.working, true); assert.equal(r.agents.filter(a => a.status === 'fail').length, 0);
  const empty = parseFeed(read('swarm')).roots.find(root => !root.agents.length && root.herdr_status === 'blocked');
  assert.deepEqual(aggregate(empty).kinds, ['blk']); assert.deepEqual(rows(empty, new Map(), true), []);
});

test('root titles: operator-named terminal, then a named tab holding one agent, then feed title; space name never wins (D3, D6)', () => {
  const program = { key: 'claude', name: 'Claude Code' }, feed = { title: 'Billing API migration', title_source: 'session' };
  const tab = { id: 'tb', name: 'Release', named: true };
  assert.deepEqual(rootTitle({ address: 't2', program, terminal: { name: 'reviewer pane' }, tab, alone: true, feed }), { name: 'reviewer pane', source: 'pane' });
  assert.deepEqual(rootTitle({ address: 't2', program, terminal: { name: 't2' }, tab, alone: true, feed }), { name: 'Release', source: 'tab' });
  assert.deepEqual(rootTitle({ address: 't2', program, terminal: { name: 'Claude Code' }, tab, alone: false, feed }), { name: 'Billing API migration', source: 'session' });
  assert.deepEqual(rootTitle({ address: 't2', program, tab: { ...tab, named: false, name: '2' }, alone: true, feed: null, pane: { title: 'Fix login flake', program } }), { name: 'Fix login flake', source: 'live' });
  assert.deepEqual(rootTitle({ address: 't2', program, pane: { title: '~/Code/x', program } }), { name: 'Claude Code · t2', source: 'program' });
  const r = root(); r.terminal = 't2';
  const stock = [{ address: 't2', space: 's', space_name: 'Payments space', tab: 'tb', status: 'working', program }, { address: 't3', space: 's', space_name: 'Payments space', tab: 'tb', status: 'idle', program }];
  const joined = joinRoots({ roots: [r] }, stock, [{ address: 't2', state: 'live' }], [], [{ id: 's', tabs: [tab] }]);
  assert.equal(joined.find(item => item.root === r).name, 'Billing API migration', 'two agents share the tab, so the tab names neither');
  assert.equal(joinRoots({ roots: [r] }, stock.slice(0, 1), [{ address: 't2', state: 'live' }], [], [{ id: 's', tabs: [tab] }])[0].name, 'Release');
});

test('exact qualified core join enriches once; parity, unknown roots and missing live targets survive', () => {
  const r = root(); r.terminal = 'vps/t7'; r.title = ''; r.tab_label = 'feed metadata';
  const f = { roots: [r] }, stock = [{ address: 't7', space: 's1', tab: 'tb1', space_name: 'Local', status: 'idle', seq: 1, program: { key: 'amp', icon: 'icons/amp.svg' } }, { address: 'vps/t7', space: 's2', tab: 'tb2', space_name: 'Remote', status: 'working', seq: 2, program: { key: 'codex', icon: 'icons/codex.svg' } }, { address: 't9', space: 's1', tab: 'tb1', space_name: 'Parity', status: 'done', seq: 3 }];
  const known = [{ address: 'vps/t7', state: 'live' }];
  const joined = joinRoots(f, stock, known, [], []);
  assert.equal(joined.length, 3); assert.equal(joined.filter(item => item.root === r).length, 1);
  assert.equal(joined.find(item => item.root === r).stock.address, 'vps/t7');
  assert.equal(joined.find(item => item.stock.address === 't7').root.agents.length, 0);
  assert.equal(joined.find(item => item.stock.address === 't7').terminal, null);
  assert.equal(joinRoots(f, stock, [], [], []).find(item => item.root === r).terminal, null);
  r.host = 'herdr-tui';
  const unmatched = joinRoots(f, stock, known, [], []).find(item => !item.stock);
  assert.equal(unmatched.name, 'feed metadata'); assert.equal(unmatched.space, null); assert.equal(unmatched.terminal, null);
  assert.equal(unmatched.title_source, 'tab');
  r.title = '·'; r.tab_label = ''; assert.equal(joinRoots(f, stock, known, [], []).find(item => !item.stock).title_source, 'id');
  r.title = 'Watch the deploy';
  assert.equal(joinRoots(f, stock, known, [], []).find(item => !item.stock).name, 'Watch the deploy');
  assert.equal(joinRoots(f, stock, known, [], []).length, 4);
});

test('legacy cwd/session/title matches never claim membership and unsupported harnesses stay spelled', () => {
  const r = root(); r.host = 'unknown'; r.terminal = 't1'; r.title = ''; r.tab_label = '';
  const joined = joinRoots({ roots: [r] }, [], [{ address: 't1', state: 'live' }], [{ terminal: 't1', tab: 'tb1' }], [{ id: 's1', tabs: [{ id: 'tb1' }] }]);
  assert.equal(joined[0].name, r.pane_id); assert.equal(joined[0].terminal, null); assert.equal(joined[0].space, null);
  const a = agent('a'); a.harness = 'future-harness';
  assert.equal(harnessOf(a, { ...r, agents: [a] }), 'future-harness'); assert.equal(iconProgram('future-harness').icon, 'icons/agent.svg');
  const p = agent('parent'); p.harness = 'gemini'; a.harness = 'unknown'; a.parent = p.id;
  assert.equal(harnessOf(a, { ...r, agents: [p, a] }), 'gemini');
  assert.equal(harnessOf(null, r), r.harness);
});

test('pane-less native descendants preserve urgency and exact child enrichment appears once', () => {
  const r = root(); r.terminal = 'vps/t7';
  const stock = [{ address: 't1', space: 's1', tab: 'tb1', space_name: 'Native', status: 'idle', seq: 1 }];
  const terminals = [{ address: 't1', state: 'live', status: 'idle' }, { address: 'vps/t7', parent: 't1', state: 'live', status: 'blocked', program: { key: 'future-native', icon: 'icons/agent.svg' } }, { address: 't9', parent: 'vps/t7', state: 'live', status: 'working' }];
  const joined = joinRoots({ roots: [r] }, stock, terminals, [], []);
  assert.equal(joined.length, 1); assert.equal(joined[0].nativeChildren.length, 2);
  assert.equal(joined[0].nativeChildren.filter(node => node.root === r).length, 1);
  assert.equal(joined[0].nativeChildren[0].name, r.title, 'an enriched native child takes the feed title');
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
  const route = detailTarget({ ...item, terminal: null }, { agent: agent('t99') });
  assert.equal(route.mod, 'panorama-peek'); assert.equal(route.props.agent.id, 't99'); assert.equal(route.props.terminal, null);
  assert.equal(route.name, undefined); assert.equal('thread' in route.props, false);
});

test('healthy live subagents stay listed; only finished ones fold', () => {
  const r = minimal([agent('done', 'done'), agent('reviewing', 'rev'), agent('queued', 'q')]);
  assert.deepEqual(ids(rows(r, new Map(), true)), ['reviewing', 'queued', 'summary']);
});

test('opaque root/agent IDs cannot collide with each other or the finished fold', () => {
  const r = minimal([agent('summary', 'fail'), agent('$finished', 'done'), agent('agents', 'done')]);
  const folds = new Map(), first = rows(r, folds, true);
  folds.set(first.at(-1).key, { open: true, signature: first.at(-1).signature });
  const visible = rows(r, folds, true);
  assert.equal(visible.length, 4); assert.equal(new Set(visible.map(row => row.key)).size, visible.length);
  assert.ok(visible.every(row => row.kind === 'summary' || row.key === agentKey(r, row.agent)));
  assert.notEqual(rootKey(r), visible.at(-1).key);
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
  const joined = joinRoots({ roots: [source] }, stock, live, [{ terminal: 't2', tab: 'direct' }], [{ id: 'child', tabs: [{ id: 'direct' }] }]);
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

test('feed-only roots stay listed in the ordinary sections; a stale feed shows them Stale with no needs', () => {
  const blocked = root(); blocked.host = 'herdr-tui'; blocked.terminal = null; blocked.herdr_status = 'blocked';
  const stock = [{ address: 't1', space: 'a', tab: 'tb', status: 'working' }, { address: 't9', space: 'b', tab: 'tb2', status: 'idle' }];
  const all = joinRoots({ roots: [blocked] }, stock, [{ address: 't1', state: 'live' }], [], []);
  const fresh = listed(all, 'a', false);
  assert.deepEqual(fresh.map(item => item.stock?.address ?? item.root.pane_id), [blocked.pane_id, 't1']);
  assert.deepEqual(groups(fresh).map(group => group.label), ['Needs you', 'Working']);
  assert.deepEqual(groups(listed(all, 'a', true)).map(group => group.label), ['Working', 'Idle']);
  const stale = listed(all, 'a', true).find(item => !item.stock);
  assert.equal(stale.status, 'stale'); assert.equal(stale.stale, true); assert.deepEqual(stale.aggregate.kinds, []); assert.equal(stale.aggregate.priority, 1);
  assert.equal(all.find(item => !item.stock).aggregate.priority, 4, 'the joined entity itself is not mutated');
});

test('an aged collection joins no live terminal: rows show stock data alone and nothing is counted from it', () => {
  // The previous core's t3 and t4, against a new core that reused both addresses.
  const old = { ...minimal([agent('a', 'wait')]), terminal: 't3', title: 'Old conversation', inbox: [7], inbox_texts: ['Old question'] };
  old.agents[0].question_ids = [7]; old.agents[0].questions = ['Old question'];
  const child = { ...minimal([agent('b', 'fail')]), terminal: 't4', pane_id: 'p4', title: 'Old child' };
  const elsewhere = { ...minimal([]), terminal: 't8', pane_id: 'p8', title: 'No terminal now' };
  const feed = { roots: [old, child, elsewhere] };
  const stock = [{ address: 't3', space: 's', tab: 'tb', status: 'idle', title: 'plain', program: { key: 'claude', name: 'Claude' } }];
  const live = [{ address: 't3', state: 'live' }, { address: 't4', state: 'live', parent: 't3', program: { key: 'codex', name: 'Codex' } }];
  const fresh = joinRoots(feed, stock, live, [], [], {});
  const enriched = fresh.find(item => item.stock);
  assert.equal(enriched.name, 'Old conversation'); assert.equal(enriched.nativeChildren[0].name, 'Old child');
  const joined = joinRoots(feed, stock, live, [], [], { aged: true });
  const row = joined.find(item => item.stock), others = joined.filter(item => !item.stock);
  assert.equal(row.stock.address, 't3'); assert.equal(row.terminal, 't3'); assert.equal(row.name, 'plain');
  assert.deepEqual(row.root.agents, []); assert.deepEqual(row.root.inbox, []);
  assert.deepEqual(row.aggregate.kinds, []); assert.equal(row.aggregate.questions, 0); assert.equal(row.aggregate.needsCount, 0);
  assert.deepEqual(row.nativeChildren[0].root.agents, []); assert.equal(row.nativeChildren[0].name, 'Codex · t4');
  assert.deepEqual(spaceTotals(joined, 's'), { kinds: [], needsCount: 0, questions: 0, working: false, done: false });
  assert.deepEqual(others.map(item => item.root), [elsewhere], 'a matched root is not listed again, an unmatched one stays');
  assert.equal(others[0].terminal, null); assert.equal(listed(joined, 's', true).find(item => !item.stock).status, 'stale');
});

test('collection age: only a finite, past timestamp within 15 s is current', () => {
  const now = 1_000_000_000;
  for (const generated_at of [now / 1000, now / 1000 - 15]) assert.equal(aged({ generated_at }, now), false);
  for (const generated_at of [now / 1000 - 15.001, now / 1000 + 1, null, NaN, Infinity, undefined]) assert.equal(aged({ generated_at }, now), true);
  assert.equal(aged(null, now), true);
});

test('All spaces lists every space in the same priority sections; header totals follow the scope', () => {
  const asking = minimal([agent('q', 'blk')]); asking.terminal = 't2';
  const stock = [{ address: 't1', space: 'a', space_name: 'Alpha', tab: 'ta', tab_index: 1, status: 'working' },
    { address: 't2', space: 'b', space_name: 'Beta', tab: 'tb', tab_index: 1, status: 'idle' }];
  const joined = joinRoots({ roots: [asking] }, stock, stock.map(({ address }) => ({ address, state: 'live' })), [], []);
  assert.deepEqual(listed(joined, 'a', false).map(item => item.stock.address), ['t1']);
  const every = listed(joined, 'a', false, 'all');
  assert.deepEqual(every.map(item => item.stock.address), ['t2', 't1']);
  assert.deepEqual(groups(every).map(group => group.label), ['Needs you', 'Working']);
  assert.deepEqual(headerTotals(listed(joined, 'a', false), 'a'), { wait: 0, blk: 0, fail: 0 });
  assert.deepEqual(headerTotals(every, 'a', 'all'), { wait: 0, blk: 1, fail: 0 });
  // Both cards are tab 1 of their space: line 2 names the space so the ids stay distinct.
  assert.deepEqual(every.map(item => cardPlace(item, 'all')), ['Beta · tab 1', 'Alpha · tab 1']);
  assert.equal(cardPlace(every[0], 'space'), '');
  assert.equal(cardPlace({ stock: null }, 'all'), '', 'a feed-only root has no app space');
});

test('the stored scope is All spaces only for an exact JSON "all"; anything else is This space', () => {
  assert.equal(storedScope(JSON.stringify({ 'agents.scope': JSON.stringify('all'), 'machines.folded': '[]' })), 'all');
  for (const text of [null, '', '{broken', '{}', JSON.stringify({ 'agents.scope': 'all' }), JSON.stringify({ 'agents.scope': JSON.stringify('space') }), '[]'])
    assert.equal(storedScope(text), 'space');
});

test('every view that joins a packet passes its collection age, so none can show an aged join', () => {
  for (const mod of ['panorama', 'panorama-graph', 'panorama-spaces', 'panorama-tabs', 'panorama-pane']) {
    const calls = readFileSync(join(repo, `app/mods/${mod}/main.js`), 'utf8').split('\n').filter(line => line.includes('joinRoots('));
    assert.equal(calls.length, 1, mod); assert.match(calls[0], /aged: [\w.?]+\.aged \}\)/, mod);
  }
});

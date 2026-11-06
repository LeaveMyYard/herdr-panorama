import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { register } from 'node:module';

register('./herdr-loader.mjs', import.meta.url);
const { default: Panorama } = await import('../mods/panorama/main.js');
const { joinRoots, listed, groups, spaceTotals } = await import('../mods/panorama/model.js');
const { STATES, NEEDS, stateCode, stateOf, cardColors, tile, badge } = await import('../mods/panorama/state.js');
const { state: host } = await import('./herdr-fixture.mjs');

const feed = JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json', import.meta.url)));
const source = feed.roots[0].agents[0];
const agent = (id, status = 'run', parent = null) => ({ ...source, id, title: `Question Title ${id}`, status, parent, question_ids: [], questions: [], error: null });
const task = { id: 'b1', kind: 'shell', description: 'Synthetic wait', started: 1 };
const rootOf = (agents, extra = {}) => ({ ...structuredClone(feed.roots[0]), pane_id: 'core:t1', terminal: 't1', inbox: [], inbox_texts: [], pending: [], agents, hidden_finished: 0, bg: [], ...extra });
const stockOf = (status, focused = false) => ({ address: 't1', pane: 'p1', tab: 'tab', space: 's', status, focused, title: 'Project Alpha', program: { key: 'claude' } });
const live = [{ address: 't1', state: 'live' }];
const joined = (root, status, terminals = live, stock = [stockOf(status)]) => joinRoots({ ...feed, roots: [root] }, stock, terminals, [], [])[0];

const EPOCH = 2400 * 750000000; // The sweep's cycle starts here, and the feed is this fresh.
// One real render of the sidebar; the feed and native lists are synthetic.
function sidebar(root, { status = 'working', focused = true, height = 2000, offset = 0, now = 1200, rail = false } = {}) {
  host.agents = [stockOf(status, focused)]; host.terminals = live; host.spaces = [{ id: 's', active: true, tabs: [] }];
  host.layout = rail ? { id: 'sidebar', size: { px: 100 } } : { content: { mod: 'panorama' } };
  const view = new Panorama();
  Object.assign(view, { folds: new Map(), changed: {}, scope: 'space', hovered: null, held: null, viewportPhysical: { width: 320, height }, scrollOffset: offset, contentHeight: 0,
    scrollState: { value: () => 1, set_value() {} }, scrollFocus: {}, peekCaller: { sync() {}, rowKey: null, error: null },
    feed: { feed: { ...feed, roots: [root] }, generated_at: EPOCH / 1000, errors: [], stale: false } });
  const clock = Date.now; Date.now = () => EPOCH + now;
  try { return { view, tree: view.render({}) }; } finally { Date.now = clock; }
}
const walk = (node, out = []) => { if (node && typeof node === 'object' && node.calls) { out.push(node); for (const kid of node.kids) walk(kid, out); } return out; };
const byId = (tree, id) => walk(tree).find(node => node.calls.id?.[0] === id);
const sweeps = el => walk(el).filter(node => node.calls.bottom?.[0] === 0 && node.calls.h?.[0] === 8);
const card = tree => walk(tree).find(node => String(node.calls.id?.[0]).startsWith('root-'));
const holder = (tree, el) => walk(tree).find(node => node.kids.includes(el));
const closing = tree => walk(tree).find(node => node.calls.border_b && node.calls.rounded_b?.[0] === 8 && node.calls.relative);
const rowsOf = tree => walk(tree).filter(node => node.calls.pl?.[0] === 29);
const isTile = (node, size) => node.calls.border_1 && node.calls.h?.[0] === size && node.calls.w?.[0] === size;
const text = el => walk(el).flatMap(node => node.kids.filter(kid => typeof kid === 'string'));
const percent = value => Number.parseFloat(value) / 100;

test('a closed running card sweeps its bottom border, flush to both outer edges, above the border', () => {
  const { tree } = sidebar(rootOf([agent('a')]), { focused: false });
  const outer = holder(tree, card(tree)), [bar] = sweeps(outer);
  assert.ok(bar, 'the sweep is on the card');
  assert.equal(sweeps(card(tree)).length, 0, 'not a child: gpui paints an element\'s border over its children');
  assert.deepEqual([bar.calls.left[0], bar.calls.right[0], bar.calls.bottom[0]], [0, 0, 0], 'the card\'s outer edges, no side margins');
  assert.deepEqual(outer.kids, [card(tree), bar], 'painted after the card');
  assert.ok(outer.calls.relative);
  assert.equal(card(tree).calls.h[0] + outer.calls.pt[0], outer.calls.h[0], 'the holder ends where the card does');
});

test('an open card carries the sweep over its last row, below every tree row, and not on the head', () => {
  const { tree } = sidebar(rootOf([agent('a'), agent('b'), agent('a1', 'run', 'a')]));
  assert.equal(sweeps(holder(tree, card(tree))).length, 0, 'none between the meta line and the tree');
  assert.equal(sweeps(tree).length, 1, 'one per card');
  const outer = holder(tree, closing(tree));
  assert.deepEqual(outer.kids, [closing(tree), ...sweeps(tree)], 'over the row that closes the card');
  assert.ok(outer.calls.relative);
});

test('the sweep keeps its meaning: a 28% window of the full width, on the card outline', () => {
  for (const now of [0, 300, 1200, 2000, 2399]) {
    const { tree } = sidebar(rootOf([agent('a')]), { focused: false, now }), [bar] = sweeps(tree);
    const [window] = bar.kids, [line] = window.kids, p = now / 2400, eased = p < .5 ? 2 * p * p : 1 - (-2 * p + 2) ** 2 / 2;
    assert.equal(window.calls.w[0], '28%');
    assert.ok(window.calls.overflow_hidden, 'the window clips, the card does not');
    assert.ok(Math.abs(percent(window.calls.left[0]) - (-.28 + eased * 1.28)) < 1e-9, 'unchanged travel');
    // The outline stays pinned to the card however far the window has travelled.
    assert.ok(Math.abs(percent(window.calls.left[0]) + percent(line.calls.left[0]) * .28) < 1e-9);
    assert.ok(Math.abs(percent(line.calls.w[0]) * .28 - 1) < 1e-9, 'as wide as the card');
    assert.equal(line.calls.opacity[0], .85);
    assert.equal(bar.calls.h[0], 8, 'the visible part stops where the corner does');
    assert.deepEqual([line.calls.top[0], line.calls.h[0]], [-8, 16], 'tall enough to keep the full radius, with its bottom on the card\'s');
    assert.equal(line.calls.rounded_b[0], 8, 'bends with the card corner');
    for (const side of ['border_l', 'border_r', 'border_b']) assert.equal(line.calls[side][0], 1, 'border weight');
    assert.equal(line.calls.border_color[0], stateOf('run').hue);
  }
});

test('selected and idle cards: the selected border stays, and only a running subagent sweeps', () => {
  const open = sidebar(rootOf([agent('a')])).tree;
  assert.equal(card(open).calls.border_color[0], cardColors('run', true).border);
  assert.equal(closing(open).calls.border_color[0], cardColors('run', true).border);
  assert.equal(sweeps(sidebar(rootOf([agent('a', 'done')]), { status: 'idle', focused: false }).tree).length, 0);
  assert.equal(sweeps(sidebar(rootOf([agent('a', 'done')]), { status: 'idle' }).tree).length, 0);
});

test('the frame timer follows the row that draws the sweep, even when the head has scrolled away', () => {
  const root = rootOf([agent('a'), agent('b'), agent('c')]);
  assert.equal(sidebar(root, { height: 40, offset: 84 }).view.animate, true, 'head gone, last row visible');
  assert.equal(sidebar(root, { height: 40, offset: 0 }).view.animate, false, 'head visible, last row below the fold');
  assert.equal(sidebar(rootOf([agent('a')]), { focused: false, height: 100 }).view.animate, true, 'closed card');
});

test('level 1 rows box their state, level 2 and deeper stay bare, and the connectors meet the tile', () => {
  const { tree } = sidebar(rootOf([agent('a'), agent('a1', 'run', 'a'), agent('b', 'wait')]));
  const [first, second, third] = rowsOf(tree);
  const mark = (row, node) => walk(row).find(el => el.calls.id?.[0]?.startsWith?.('icon-')).kids[0];
  assert.ok(isTile(mark(first), 16), 'level 1');
  assert.equal(mark(second).calls.size[0], 11, 'level 2 is the bare glyph'); assert.equal(mark(second).calls.border_1, undefined);
  assert.ok(isTile(mark(third), 16), 'a needs-you level 1 row keeps a square tile');
  // x positions in the row: pl 29, one 13px segment per level, the first tile's -1 margin.
  const wrapper = walk(first).find(el => el.calls.id?.[0]?.startsWith?.('icon-'));
  const gaps = first.kids.filter(kid => kid.calls.w?.[0] === 4);
  const left = 29 + 13 + gaps.length * 4 + wrapper.calls.ml[0];
  assert.equal(gaps.length, 0, 'no gap between the connector and the tile');
  assert.ok(left <= 29 + 13, 'the tick reaches the tile');
  assert.equal(left + 8, 29 + 13 + 7, 'the tile is centred on its children\'s line');
  assert.equal(left + 16 + wrapper.calls.mr[0], 29 + 13 + 4 + 11 + 7, 'the title has not moved');
  assert.equal(second.kids.filter(kid => kid.calls.w?.[0] === 4).length, 1, 'deeper rows keep their gap');
});

test('the finished fold is boxed like the level 1 rows it stands for, and stays square', () => {
  const { tree } = sidebar(rootOf([agent('a'), agent('b', 'done'), agent('c', 'done')]));
  const fold = rowsOf(tree).at(-1), holder = fold.kids.find(kid => kid.calls.mr?.[0] === 7);
  assert.ok(isTile(holder.kids[0], 16)); assert.equal(holder.calls.ml[0], -1);
});

test('live background work: an idle or done root is Waiting, in its own section', () => {
  for (const status of ['idle', 'done']) {
    const item = joined(rootOf([agent('a', 'done')], { bg: [task], herdr_status: status }), status);
    assert.equal(item.status, 'bg'); assert.equal(item.aggregate.sweep, false); assert.deepEqual(item.aggregate.kinds, []);
    const [group] = groups([item]);
    assert.equal(group.label, 'Waiting');
    assert.equal(spaceTotals([item], 's').done, false, 'not counted as done');
  }
  const idle = joined(rootOf([], { herdr_status: 'idle' }), 'idle'), waiting = joined(rootOf([], { bg: [task] }), 'idle');
  const working = joined(rootOf([agent('a')]), 'working');
  assert.deepEqual([idle, waiting, working].sort((a, b) => b.aggregate.priority - a.aggregate.priority).map(item => item.status), ['working', 'bg', 'idle']);
});

test('without live background work nothing changes', () => {
  assert.equal(joined(rootOf([], { bg: [], herdr_status: 'idle' }), 'idle').status, 'idle');
  assert.equal(joined(rootOf([], { bg: [], herdr_status: 'done' }), 'done').status, 'done');
  const old = rootOf([]); delete old.bg;
  assert.equal(joined(old, 'idle').status, 'idle', 'a root with no bg field');
  assert.equal(joined(old, 'idle').aggregate.priority, 1);
});

test('everything more urgent than waiting still wins', () => {
  const bg = { bg: [task] };
  const blocked = joined(rootOf([], { ...bg, herdr_status: 'blocked' }), 'blocked');
  assert.equal(blocked.status, 'blocked'); assert.equal(blocked.aggregate.priority, 4);
  for (const [status, kind] of [['wait', 'wait'], ['blk', 'blk'], ['fail', 'fail']]) {
    const item = joined(rootOf([agent('a', status)], bg), 'idle');
    assert.notEqual(item.status, 'bg'); assert.deepEqual(item.aggregate.kinds, [kind]); assert.equal(item.aggregate.priority, 4);
  }
  assert.notEqual(joined(rootOf([], { ...bg, inbox: [7], inbox_texts: ['Question Title 123'] }), 'idle').status, 'bg', 'its own question');
  assert.equal(joined(rootOf([], bg), 'working').status, 'working');
  const subagent = joined(rootOf([agent('a')], bg), 'idle');
  assert.equal(subagent.status, 'idle'); assert.equal(subagent.aggregate.priority, 2);
  const family = [...live, { address: 't2', state: 'live', parent: 't1', status: 'error' }];
  const parent = joined(rootOf([], bg), 'idle', family);
  assert.notEqual(parent.status, 'bg', 'a native child\'s need'); assert.equal(parent.aggregate.priority, 4);
});

test('a stale feed-only root reads Stale, however much background work it reported', () => {
  const item = joinRoots({ ...feed, roots: [rootOf([], { bg: [task], herdr_status: 'idle' })] }, [], [], [], [])[0];
  assert.equal(item.status, 'bg');
  const [quiet] = listed([item], null, true);
  assert.equal(quiet.status, 'stale'); assert.equal(quiet.aggregate.priority, 1);
});

test('the waiting code cannot read as Needs answer', () => {
  assert.equal(stateOf('bg').label, 'Waiting'); assert.equal(stateOf('bg').title, 'Waiting');
  assert.equal(stateOf('bg').hue, '#b39ddb', 'lavender');
  assert.equal(stateCode('waiting'), 'wait', 'the word stays the Needs answer alias, so roots use the code');
  assert.equal(STATES.wait.title, 'Needs answer'); assert.equal(STATES.wait.label, 'Answer');
  assert.notEqual(stateOf('bg').hue, STATES.wait.hue); assert.equal(NEEDS.includes('bg'), false);
  assert.equal(tile('bg', 16).calls.loop, undefined, 'steady: no breathing'); assert.ok(tile('wait', 16).calls.loop);
  assert.deepEqual(text(badge('bg')), ['Waiting']);
});

test('a waiting card renders the lavender Waiting badge with no sweep or needs-you', () => {
  const { tree } = sidebar(rootOf([agent('a', 'done')], { bg: [task], herdr_status: 'done' }), { status: 'done', focused: false });
  const root = card(tree);
  assert.equal(root.calls.bg[0], stateOf('bg').hue, 'the left accent');
  assert.equal(root.calls.border_color[0], cardColors('bg').border);
  assert.ok(text(root).includes('Waiting')); assert.equal(text(root).includes('Done'), false); assert.equal(text(root).includes('Idle'), false);
  assert.equal(sweeps(tree).length, 0);
  assert.ok(text(tree).includes('Waiting'), 'the section heading');
  const rail = sidebar(rootOf([], { bg: [task] }), { status: 'idle', focused: false, rail: true }).tree;
  assert.ok(walk(rail).some(node => node.calls.size?.[0] === 12 && walk(node).some(el => String(el.args?.[0]).includes('state-orbit'))), 'the lavender mark on the rail');
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { register } from 'node:module';

register('./herdr-loader.mjs', import.meta.url);
const { graphModel, recordCardHeight, WIDTH } = await import('../mods/panorama-graph/graph.js');
const { agentKey } = await import('../mods/panorama-graph/contract.js');
const { card } = await import('../mods/panorama-graph/cards.js');
const { default: Graph } = await import('../mods/panorama-graph/main.js');
const { div } = await import('./gpui-fixture.mjs');

const feed = JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json', import.meta.url)));
const source = feed.roots.flatMap(root => root.agents)[0];
const LONG_ID = 'a1b2c3d4e5f60718a';
const agent = (id, extra = {}) => ({ ...source, id, parent: null, role: 'general-purpose', title: `Task ${id}`, status: 'run', model: 'opus', tool_calls: 31, questions: [], question_ids: [], activity: 'Bash(Wait for it)', error: null, ...extra });
function scene(agents) {
  const root = structuredClone(feed.roots[0]); root.agents = agents; root.hidden_finished = 0;
  return { key: 'root', name: 'Synthetic root', root, aggregate: { kinds: [], sweep: false }, status: 'working', space: 's', nativeChildren: [] };
}
const stateFor = (item, selected, heights) => ({ root: item.key, agent: agentKey(item.root, selected), focus: 1, expanded: new Set(), remember: new Map(), scroll: {}, heights });
const find = (model, id) => model.nodes.find(node => node.agent?.id === id);
const scale = 1.5, inner = (WIDTH - 2) * scale; // The host's report at zoom 1.5.
const report = height => ({ width: inner, height: height * scale });
const theme = { foreground: '#eeeeef', muted: '#aaaaaa', dim: '#777777' };
function drawn(item, node, measure = () => {}, current = node) {
  const view = { current, canAnswer: () => true, measureCard: measure, actionButton: (_t, action) => div().h(24).id(action) };
  return card(view, null, theme, node, null, { hi: 9 }, new Set(), 0);
}
const body = frame => frame.kids.find(child => child.calls.on_size_change);
const number = (el, name) => el.calls[name]?.[0] ?? 0;
// Lays a card body out by its described sizes, wrapping text at the estimate's 34 characters.
function contentHeight(el) {
  if (el.calls.h) return number(el, 'h');
  if (el.calls.line_height) return 17 * Math.max(1, Math.ceil([...String(el.kids[0] ?? '')].length / 34));
  const parts = el.kids.filter(child => typeof child === 'object').map(contentHeight);
  const inside = el.kind === 'h_flex' ? Math.max(0, ...parts) : parts.reduce((sum, part) => sum + part, 0) + number(el, 'gap') * Math.max(0, parts.length - 1);
  return number(el, 'pt') + number(el, 'pb') + inside;
}

test('the first frame of a selected card counts the agent line and its wrap', () => {
  const short = agent('x', { role: '', tool_calls: 0, model: '' }), long = agent(LONG_ID);
  for (const [picked, lines] of [[short, 2], [long, 3]]) {
    const item = scene([picked]), model = graphModel([item], stateFor(item, picked, new Map()));
    assert.equal(find(model, picked.id).height, 68 + lines * 17 + 2 * 3);
  }
});
test('every variable line of a selected card fits the card, on the first frame too', () => {
  const words = 'a long synthetic line that keeps going well past one row of the card';
  const cases = [
    agent(LONG_ID),
    agent(LONG_ID, { activity: '' }),
    agent(LONG_ID, { activity: words }),
    agent(LONG_ID, { status: 'wait', questions: [words, 'Short?'], question_ids: [1, 2] }),
    agent(LONG_ID, { status: 'fail', error: words }),
    agent(LONG_ID, { status: 'fail', activity: words, error: words, questions: [words, words, words], question_ids: [1, 2, 3] }),
  ];
  for (const picked of cases) {
    const item = scene([picked]), model = graphModel([item], stateFor(item, picked, new Map())), node = find(model, picked.id);
    assert.equal(contentHeight(body(drawn(item, node))) + 2, node.height);
  }
});
test('the height the host reports sizes the card, whatever the estimate said', () => {
  const picked = agent(LONG_ID), item = scene([picked, agent('next'), agent('last')]), heights = new Map();
  const key = agentKey(item.root, picked);
  assert.equal(recordCardHeight(heights, key, report(160)), true);
  const model = graphModel([item], stateFor(item, picked, heights));
  assert.equal(find(model, picked.id).height, 162);
  assert.equal(find(model, 'next').y, 162 + 10);
  assert.equal(find(model, 'last').y, 162 + 10 + 54 + 10);
});
test('the column below and the tree lines follow a measured card', () => {
  const picked = agent(LONG_ID), kid = agent('kid', { parent: 'next' }), lower = agent('lower', { parent: 'next' });
  const item = scene([picked, agent('next'), kid, lower, agent('last')]), key = agentKey(item.root, picked), heights = new Map();
  const before = graphModel([item], stateFor(item, picked, heights));
  recordCardHeight(heights, key, report(200));
  const model = graphModel([item], stateFor(item, picked, heights)), shift = find(model, 'next').y - find(before, 'next').y;
  assert.ok(shift > 0);
  for (const id of ['kid', 'lower', 'last']) assert.equal(find(model, id).y, find(before, id).y + shift);
  for (const column of model.columns.slice(1)) {
    for (let index = 1; index < column.length; index++) assert.ok(column[index].y >= column[index - 1].y + column[index - 1].height + 10);
  }
  for (const group of model.tree.groups) {
    const ys = model.columns[group.depth].filter(node => node.parent === group.parent).map(node => node.y + node.mid);
    assert.deepEqual(group.ys, ys);
    if (group.parent) assert.ok(group.ys.every(y => y >= group.parent.y + group.parent.mid));
  }
});
test('a report is read at the scale of the body, not at an assumed zoom', () => {
  for (const zoom of [1, 1.2, 1.5, 1.59, 2]) {
    const heights = new Map();
    recordCardHeight(heights, 'k', { width: (WIDTH - 2) * zoom, height: 123 * zoom });
    assert.equal(heights.get('k'), 125);
  }
});
test('only a real change of height moves the layout', () => {
  const heights = new Map([['k', 125]]);
  assert.equal(recordCardHeight(heights, 'k', report(123)), false);
  assert.equal(recordCardHeight(heights, 'k', report(122)), false, 'pixel rounding must not move the card');
  assert.equal(heights.get('k'), 125);
  assert.equal(recordCardHeight(heights, 'k', report(106)), true, 'a line that stopped wrapping shrinks the card');
  assert.equal(heights.get('k'), 108);
  for (const size of [{ width: 0, height: 150 }, { width: inner, height: 0 }, { width: NaN, height: 150 }]) assert.equal(recordCardHeight(heights, 'k', size), false);
  assert.equal(heights.get('k'), 108);
});
test('a selected agent card reports its body, which holds the actions and ignores the card height', () => {
  const picked = agent(LONG_ID), item = scene([picked]), seen = [];
  const node = find(graphModel([item], stateFor(item, picked, new Map())), picked.id);
  const measured = body(drawn(item, node, (key, size) => seen.push([key, size])));
  assert.deepEqual([measured.calls.absolute, measured.calls.left, measured.calls.right, measured.calls.top], [[], [0], [0], [0]]);
  assert.equal(measured.calls.h, undefined);
  assert.equal(measured.calls.id[0], `graph-card-body-${node.key}`);
  assert.deepEqual(measured.kids.at(-1).kids.map(child => child.calls.id[0]), ['open', 'transcript', 'answer']);
  measured.calls.on_size_change[0]({ width: 405, height: 200 }, null);
  assert.deepEqual(seen, [[node.key, { width: 405, height: 200 }]]);
});
test('an unselected card stays fixed and is not measured', () => {
  const picked = agent(LONG_ID), other = agent('other'), item = scene([picked, other]);
  const model = graphModel([item], stateFor(item, picked, new Map([[agentKey(item.root, other), 300]])));
  const node = find(model, 'other');
  assert.equal(node.height, 54);
  assert.equal(body(drawn(item, node, () => assert.fail('measured'), find(model, picked.id))), undefined);
});
test('the view keeps the report and redraws only when the height changed', () => {
  const view = new Graph(); let notified = 0;
  view.state = { heights: new Map() };
  const cx = { notify: () => notified++ };
  view.measureCard('k', report(150), cx); view.measureCard('k', report(150), cx);
  assert.equal(view.state.heights.get('k'), 152);
  assert.equal(notified, 1);
});

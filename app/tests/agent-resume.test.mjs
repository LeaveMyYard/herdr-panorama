import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { register } from 'node:module';

register('./herdr-loader.mjs', import.meta.url);
const { default: Panorama } = await import('../mods/panorama/main.js');
const { storedResume, resumeSummary, RESUME_KEY, RESUME_CLICKS_KEY } = await import('../mods/panorama/model.js');
const { parseFeed } = await import('../shared/contract.js');
const { state: host } = await import('./herdr-fixture.mjs');

const fixture = JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json', import.meta.url)));
const entry = (terminal, state, reason = null) => ({ terminal, harness: 'claude', session_id: 'id', cwd: '/fixture/project-alpha', state, reason, command: 'claude --resume id' });
const report = entries => ({ tracked: 0, loss: { id: 5, detected_at: 5, entries }, error: null });
const walk = (node, out = []) => { if (node && typeof node === 'object' && node.calls) { out.push(node); for (const kid of node.kids) walk(kid, out); } return out; };
const byId = (tree, id) => walk(tree).find(node => node.calls.id?.[0] === id);
const text = el => walk(el).flatMap(node => node.kids.filter(kid => typeof kid === 'string'));
const click = (node, view) => node.calls.on_click[0]({}, { notify() {}, stop_propagation() {} }, view);

function sidebar(resume, extra = {}) {
  host.agents = []; host.terminals = []; host.spaces = [{ id: 's', active: true, tabs: [] }];
  host.layout = { content: { mod: 'panorama' } };
  const view = new Panorama();
  Object.assign(view, { folds: new Map(), changed: {}, scope: 'space', hovered: null, held: null, viewportPhysical: { width: 320, height: 2000 }, scrollOffset: 0, contentHeight: 0,
    scrollState: { value: () => 1, set_value() {} }, scrollFocus: {}, peekCaller: { sync() {}, rowKey: null, error: null },
    feed: { feed: { ...fixture, roots: [], ...(resume === undefined ? {} : { resume }) }, generated_at: Date.now() / 1000, errors: [], stale: false }, ...extra });
  return { view, tree: view.render({}) };
}

test('the toggle is on only for an exact stored JSON "on"', () => {
  assert.equal(storedResume(JSON.stringify({ [RESUME_KEY]: JSON.stringify('on') })), true);
  for (const value of [null, '', '{broken', '{}', '[]', JSON.stringify({ [RESUME_KEY]: 'on' }), JSON.stringify({ [RESUME_KEY]: JSON.stringify('off') })])
    assert.equal(storedResume(value), false, String(value));
});

test('the sort menu holds the toggle, and clicking it asks panorama-spaces to store the choice', () => {
  host.runs = [];
  const view = new Panorama(); Object.assign(view, { resume: false });
  const row = byId(view.sortMenu({}), 'resume-toggle');
  assert.match(text(row).join(' '), /Resume agents/);
  assert.match(row.calls.tooltip[0], /Only until herdr-app does this itself/);
  assert.equal(walk(row).some(node => node.kind === 'svg'), false, 'off shows no check');
  click(row, view);
  assert.deepEqual(host.runs, [['panorama-spaces.agent-resume-on']]);
  assert.equal(view.resume, true);
  const on = byId(view.sortMenu({}), 'resume-toggle');
  assert.equal(walk(on).some(node => node.kind === 'svg'), true);
  click(on, view);
  assert.deepEqual(host.runs.at(-1), ['panorama-spaces.agent-resume-off']);
});

test('panorama-spaces stores the toggle under the key the writer reads', () => {
  const spaces = readFileSync(new URL('../mods/panorama-spaces/main.js', import.meta.url), 'utf8');
  assert.match(spaces, /"agent-resume-on"\) localStorage\.setItem\(RESUME_KEY, JSON\.stringify\("on"\)\)/);
  assert.match(spaces, /"agent-resume-off"\) \{ localStorage\.removeItem\(RESUME_KEY\); localStorage\.removeItem\(RESUME_CLICKS_KEY\); \}/);
  assert.match(spaces, /startsWith\("agent-resume-press-"\)/);
  assert.match(spaces, /\[terminal\]: \{ loss: Number\(loss\), token, at: Date\.now\(\) \}/);
  const rust = readFileSync(new URL('../../src/app_feed/resume/toggle.rs', import.meta.url), 'utf8');
  assert.ok(rust.includes(`const TOGGLE_KEY: &str = "${RESUME_KEY}";`));
  assert.ok(rust.includes(`const CLICKS_KEY: &str = "${RESUME_CLICKS_KEY}";`));
});

test('the summary counts each outcome and keeps every reason and command', () => {
  const summary = resumeSummary(report([entry('t1', 'resumed'), entry('t2', 'skipped', 'the shell is in a different folder'), entry('t3', 'failed', 'no sign it arrived; not retried'), entry('t4', 'skipped', 'something is running in the terminal')]));
  assert.equal(summary.text, '1 resumed · 2 skipped · 1 failed');
  assert.equal(summary.open, false);
  assert.deepEqual(summary.rows.map(row => [row.terminal, row.word, row.reason]), [['t1', 'resumed', ''], ['t2', 'skipped', 'the shell is in a different folder'], ['t3', 'failed', 'no sign it arrived; not retried'], ['t4', 'skipped', 'something is running in the terminal']]);
  assert.equal(summary.rows[1].command, 'claude --resume id');
  assert.equal(resumeSummary(report([entry('t1', 'waiting')])).open, true);
  assert.equal(resumeSummary(report([{ terminal: 't1', state: 'new-word' }])).rows[0].word, 'unknown');
});

test('nothing to report draws nothing', () => {
  for (const value of [undefined, null, {}, { loss: null }, report([]), { loss: { entries: 'x' } }]) assert.equal(resumeSummary(value), null);
  assert.equal(byId(sidebar(undefined).tree, 'resume-result'), undefined, 'the feature off adds no row');
  assert.equal(byId(sidebar(report([])).tree, 'resume-result'), undefined);
});

test('the sidebar shows the result, opens to each pane with why, and can be hidden', () => {
  const resume = report([entry('t1', 'resumed'), entry('t2', 'skipped', 'something is running in the terminal')]);
  const { view, tree } = sidebar(resume);
  const line = byId(tree, 'resume-result');
  assert.match(text(line).join(' '), /1 resumed · 1 skipped/);
  assert.equal(byId(tree, 'resume-t2'), undefined, 'folded at first');
  click(line, view);
  const open = view.render({});
  const row = byId(open, 'resume-t2');
  assert.match(text(row).join(' '), /t2 claude/);
  assert.match(text(row).join(' '), /skipped: something is running/);
  assert.match(row.calls.tooltip[0], /claude --resume id/, 'the command to run by hand');
  assert.equal(byId(open, 'resume-t1').calls.tooltip, undefined, 'a resumed row has nothing to run');
  click(byId(open, 'resume-hide'), view);
  assert.equal(byId(view.render({}), 'resume-result'), undefined);
});

test('the first press only looks; the row then shows the line, and only that offers a send', () => {
  host.runs = [];
  const shown = { ...entry('t7', 'skipped', 'stopped without a reboot, maybe on purpose'), click: true, note: 'started with options that aren\'t repeated' };
  const resume = report([entry('t1', 'resumed'), shown]);
  const { view, tree } = sidebar(resume);
  assert.match(byId(tree, 'resume-t7').calls.tooltip[0], /options that aren't repeated/);
  const go = byId(tree, 'resume-t7-go');
  assert.match(go.calls.tooltip[0], /Nothing is typed yet/);
  assert.equal(byId(tree, 'resume-t1-go'), undefined, 'a resumed pane has none');
  assert.equal(byId(tree, 'resume-t7-send'), undefined, 'no send before a look');
  click(go, view);
  assert.deepEqual(host.runs, [['panorama-spaces.agent-resume-press-t7-5']]);
  const looked = report([entry('t1', 'resumed'), { ...shown, look: { lines: ['rig$ touch x'], text: 'all of it', token: 'abc123' } }]);
  const next = sidebar(looked);
  assert.equal(byId(next.tree, 'resume-t7-go'), undefined);
  assert.match(text(byId(next.tree, 'resume-t7-look-0')).join(' '), /rig\$ touch x/);
  const send = byId(next.tree, 'resume-t7-send');
  assert.match(send.calls.tooltip[0], /claude --resume id/);
  assert.match(send.calls.tooltip[0], /all of it/);
  assert.match(send.calls.tooltip[0], /show no prompt, don't press/);
  click(send, next.view);
  assert.deepEqual(host.runs.at(-1), ['panorama-spaces.agent-resume-press-t7-5-abc123']);
});

test('a long line keeps its end on the row, where anything typed is', () => {
  const long = `operator@workstation ~/Code/some-project-name/worktrees/feature-branch $ touch PWNED_LONG`;
  const shown = { ...entry('t7', 'skipped', 'why'), click: true, look: { lines: ['first line', long], text: long, token: 'abc' } };
  const { tree } = sidebar(report([shown]));
  const drawn = text(byId(tree, 'resume-t7-look-1')).join('');
  assert.match(drawn, /^….*touch PWNED_LONG$/);
  assert.ok(drawn.length <= 40);
  assert.equal(text(byId(tree, 'resume-t7-look-0')).join(''), 'first line');
});

test('a row saying someone is typing offers no send', () => {
  const typing = { ...entry('t7', 'skipped', 'someone is typing in the terminal'), click: true };
  const { tree } = sidebar(report([typing]));
  assert.equal(byId(tree, 'resume-t7-send'), undefined);
});

test('a row whose terminal can never work has no button', () => {
  const { tree } = sidebar(report([{ ...entry('t7', 'skipped', 'the terminal has finished'), click: false }]));
  assert.equal(byId(tree, 'resume-t7-go'), undefined);
});

test('the toggle says what it does not protect against', () => {
  const view = new Panorama(); Object.assign(view, { resume: false });
  const tip = byId(view.sortMenu({}), 'resume-toggle').calls.tooltip[0];
  assert.match(tip, /can't check and type in one step/);
  assert.match(tip, /within 30 minutes/);
  assert.match(tip, /startup file that asks for input/);
  assert.match(tip, /only covers sessions started after Panorama was shown/);
});

test('a result still being worked through cannot be hidden, and a rail draws none', () => {
  assert.equal(byId(sidebar(report([entry('t1', 'waiting')])).tree, 'resume-hide'), undefined);
  host.layout = { id: 'sidebar', size: { px: 100 } };
  const { view } = sidebar(report([entry('t1', 'resumed')]));
  host.layout = { id: 'sidebar', size: { px: 100 } };
  assert.equal(byId(view.render({}), 'resume-result'), undefined);
});

test('the additive report passes feed validation and survives it', () => {
  const resume = report([entry('t1', 'resumed')]);
  assert.deepEqual(parseFeed(JSON.stringify({ ...fixture, resume })).resume, resume);
});

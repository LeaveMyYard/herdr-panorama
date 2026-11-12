import test from 'node:test';
import assert from 'node:assert/strict';
import { register } from 'node:module';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

register('./herdr-loader.mjs', import.meta.url);
globalThis.localStorage = { getItem: () => null, setItem() {} };
const { default: Tabs } = await import('../mods/panorama-tabs/main.js');
const { state: host } = await import('./herdr-fixture.mjs');

const walk = (node, out = []) => { if (node && typeof node === 'object' && node.calls) { out.push(node); for (const kid of node.kids) walk(kid, out); } return out; };
const child = id => ({ id, type: 'block', content: { mod: id } });
const layoutOf = (ids, active, hidden = false) => ({ id: 'root', children: [{ id: 'panel', hidden }, { id: 'panel-body', type: 'stack', active, children: ids.map(child) }] });
const buttons = tree => walk(tree).filter(n => String(n.calls.id?.[0]).startsWith('panel-')).map(n => n.calls.id[0]);

function bar({ ids, installed, active = 'files', hidden = false }) {
  host.spaces = [{ id: 's', active: true, tabs: [] }]; host.runs = [];
  const view = new Tabs();
  Object.assign(view, { installed, panels: {}, pressed: null, feed: { feed: null, generated_at: null, errors: [], stale: true } });
  host.layout = layoutOf(ids, active, hidden);
  return { view, tree: view.render() };
}
const all = ['files', 'changes', 'usage'];

test('Remote sits right of Usage, only with the mod installed and a remote child in panel-body', () => {
  const both = { usage: true, remote: true };
  assert.deepEqual(buttons(bar({ ids: [...all, 'remote'], installed: both }).tree), ['panel-files', 'panel-changes', 'panel-usage', 'panel-remote']);
  assert.deepEqual(buttons(bar({ ids: all, installed: both }).tree), ['panel-files', 'panel-changes', 'panel-usage'], 'plain panorama layout has no remote child');
  assert.deepEqual(buttons(bar({ ids: [...all, 'remote'], installed: { usage: true, remote: false } }).tree), ['panel-files', 'panel-changes', 'panel-usage'], 'mod not installed');
  assert.deepEqual(buttons(bar({ ids: [...all, 'remote'], installed: { usage: false, remote: true } }).tree), ['panel-files', 'panel-changes', 'panel-remote'], 'Usage unaffected by Remote');
});

test('Remote button label and icon, and its click selects the child and shows the panel', () => {
  const { tree } = bar({ ids: [...all, 'remote'], installed: { usage: false, remote: true } });
  const el = walk(tree).find(n => n.calls.id?.[0] === 'panel-remote');
  assert.deepEqual(el.kids.map(k => typeof k === 'string' ? k : k.args[0]), ['mods/herdr-app-remote/remote.svg', 'Remote']);
  host.runs = []; el.calls.on_click[0]();
  assert.deepEqual(host.runs, [['layout.select', { stack: 'panel-body', child: 'remote' }], ['layout.show', { id: 'panel' }]]);
});

test('a shown remote panel closes once its mod is known absent, but not while availability is unknown', () => {
  const gone = bar({ ids: [...all, 'remote'], installed: { usage: true, remote: false }, active: 'remote' });
  assert.deepEqual(host.runs, [['layout.hide', { id: 'panel' }]]);
  const unknown = bar({ ids: [...all, 'remote'], installed: undefined, active: 'remote' });
  assert.deepEqual(host.runs, []);
  assert.deepEqual(buttons(unknown.tree), ['panel-files', 'panel-changes']);
});

test('the per-space restore keeps a remembered remote panel only where it is offered', () => {
  const kept = bar({ ids: [...all, 'remote'], installed: { usage: true, remote: true }, active: 'files' });
  kept.view.panels = { s: 'remote' }; kept.view.space = 'other'; host.runs = []; kept.view.render();
  assert.deepEqual(host.runs, [['layout.select', { stack: 'panel-body', child: 'remote' }], ['layout.show', { id: 'panel' }]]);
  const dropped = bar({ ids: all, installed: { usage: true, remote: true }, active: 'files', hidden: true });
  dropped.view.panels = { s: 'remote' }; dropped.view.space = 'other'; host.runs = []; dropped.view.render();
  assert.deepEqual(host.runs, [], 'no remote child in this layout: stays closed');
});

test('Usage hides through the same generic path once its mod is known absent', () => {
  bar({ ids: [...all, 'remote'], installed: { usage: false, remote: true }, active: 'usage' });
  assert.deepEqual(host.runs, [['layout.hide', { id: 'panel' }]]);
});

const settle = () => new Promise(resolve => setImmediate(resolve));
async function started(files, layoutIds) {
  const home = mkdtempSync(join(tmpdir(), 'panel-buttons-'));
  for (const name of files) { mkdirSync(join(home, 'mods', name), { recursive: true }); writeFileSync(join(home, 'mods', name, 'mod.json'), '{}'); }
  host.home = home; host.runs = []; host.layout = layoutOf(layoutIds, 'files');
  const view = new Tabs(), notes = [];
  const cx = { spawn: fn => { Promise.resolve(fn(cx)).catch(() => {}); }, sleep: () => new Promise(() => {}), notify: () => notes.push(1), timer: { every: () => ({ cancel() {} }) } };
  view.init({}, cx);
  await settle(); await settle(); await settle();
  host.sendCommand('open-usage'); await settle();
  rmSync(home, { recursive: true, force: true });
  return { view, notes };
}

test('the probe reads each optional mod\'s mod.json and the open-usage command follows the same gate as the button', async () => {
  const only = await started(['herdr-app-remote'], [...all, 'remote']);
  assert.deepEqual(only.view.installed, { usage: false, remote: true });
  assert.deepEqual(host.runs, [], 'open-usage ignored: agent-usage is not installed');
  const both = await started(['agent-usage'], [...all, 'remote']);
  assert.deepEqual(both.view.installed, { usage: true, remote: false });
  assert.deepEqual(host.runs, [['layout.select', { stack: 'panel-body', child: 'usage' }], ['layout.show', { id: 'panel' }]]);
  await started(['agent-usage'], ['files', 'changes']);
  assert.deepEqual(host.runs, [], 'open-usage ignored: this layout has no usage child');
});

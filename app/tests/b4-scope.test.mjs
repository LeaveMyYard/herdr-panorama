import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { subject, transcriptScope, filesystemRef, focusTarget } from '../mods/panorama-peek/subject.js';
import { Pages } from '../mods/panorama-peek/paging.js';
const G = '0123456789abcdef0123456789abcdef', NEW = 'fedcba9876543210fedcba9876543210';
function fixture() {
  const feed = JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json', import.meta.url)));
  const root = feed.orchestrators[0], thread = root.threads[0], agent = thread.agents[0];
  return { packet: { feed, install_id: G, stale: false }, client: { install_id: G, stopped: false }, intent: { peekInstallId: G, root, thread, agent, terminal: root.terminal, terminalAction: 'focus' } };
}
test('resolves data only from the current pinned feed, never raw props', () => {
  const { packet, client, intent } = fixture();
  const raw = structuredClone(intent); raw.agent.last_message = 'untrusted props'; raw.agent.transcript = '/untrusted';
  const value = subject(packet, client, raw);
  assert.equal(value.agent, packet.feed.orchestrators[0].threads[0].agents[0]);
  assert.notEqual(transcriptScope(value, 'linux').path, '/untrusted');
  assert.equal(subject(packet, client, { ...intent, thread: { id: 'other' } }), null);
  assert.equal(subject(packet, client, { ...intent, agent: { id: 'other' } }), null);
});
test('oldG/newG with identical root and agent IDs cannot read or focus', async () => {
  const { packet, client, intent } = fixture();
  packet.install_id = NEW; client.install_id = NEW;
  const calls = [];
  const reader = new Pages(async args => { calls.push(args); }, scope => Boolean(scope));
  const value = subject(packet, client, intent);
  assert.equal(value, null);
  assert.equal(await reader.load(transcriptScope(value, 'linux')), false);
  assert.equal(focusTarget(value, intent, {}), null);
  assert.deepEqual(calls, []);
  for (const id of [undefined, '', 'bad']) assert.equal(subject(packet, client, { ...intent, peekInstallId: id }), null);
});
test('producer-local refs support TUI/unmatched/mirrors and literal paths without machine guesses', () => {
  const { packet, client, intent } = fixture();
  const root = packet.feed.orchestrators[0]; root.host = 'herdr-tui'; root.terminal = null;
  root.transcript = "/local mirror/' ; $(literal).jsonl"; root.threads[0].agents[0].transcript = null;
  const value = subject(packet, client, intent);
  assert.equal(transcriptScope(value, 'linux').path, root.transcript);
  assert.equal(focusTarget(value, intent, { feed: packet.feed, agents: [], terminals: [], panes: [], spaces: [] }), null);
  for (const ref of ['relative.jsonl', '~/.claude/log', 'file:///tmp/log', 'ssh://host/log', '/tmp/nul\0path']) assert.equal(filesystemRef(ref, 'linux'), null);
  assert.equal(filesystemRef('C:\\local\\mirror.jsonl', 'windows'), 'C:\\local\\mirror.jsonl');
  assert.equal(filesystemRef('C:\\local\\mirror.jsonl', 'linux'), null);
});
test('agent transcript wins; nonfilesystem ref has no hidden root fallback', () => {
  const { packet, client, intent } = fixture();
  packet.feed.orchestrators[0].transcript = '/root.jsonl';
  packet.feed.orchestrators[0].threads[0].agents[0].transcript = '/agent.jsonl';
  assert.equal(transcriptScope(subject(packet, client, intent), 'linux').path, '/agent.jsonl');
  packet.feed.orchestrators[0].threads[0].agents[0].transcript = 'file:///agent.jsonl';
  assert.equal(transcriptScope(subject(packet, client, intent), 'linux'), null);
  packet.stale = true;
  assert.equal(transcriptScope(subject(packet, client, intent), 'linux'), null);
  client.stopped = true; assert.equal(subject(packet, client, intent), null);
});

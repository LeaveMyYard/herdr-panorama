// This establishes the shared fixture entry point for later UI logic tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { root } from './helpers.mjs';

export const fixture = name => JSON.parse(readFileSync(join(root, 'tests/fixtures/app-feed', `${name}.json`), 'utf8'));

test('feed v2 swarm supplies one nested, mixed-harness, needs-you agent tree per root', () => {
  const feed = fixture('swarm');
  assert.equal(feed.v, 2);
  const agents = feed.roots.flatMap(root => root.agents);
  assert.ok(Math.max(...agents.map(agent => agent.depth)) >= 4);
  assert.ok(new Set(agents.map(agent => agent.harness)).size >= 2);
  for (const status of ['run', 'rev', 'wait', 'blk', 'fail', 'done', 'q', 'cxl', 'bg']) {
    assert.ok(agents.some(agent => agent.status === status), `fixture must exercise ${status}`);
  }
  assert.ok(agents.some(agent => agent.error && agent.last_message));
  assert.ok(feed.roots.some(root => root.inbox.length));
  assert.ok(feed.roots.some(root => root.hidden_finished > 0));
  assert.ok(feed.roots.some(root => !root.agents.length && root.herdr_status === 'blocked'));
  assert.ok(agents.every(agent => typeof agent.title === 'string' && typeof agent.title_source === 'string'));
});

test('feed v2 degraded feed remains data, alongside collector error strings', () => {
  const feed = fixture('degraded');
  assert.equal(feed.v, 2);
  assert.ok(feed.errors.length > 0);
  assert.deepEqual(feed.roots, []); // A missing collector must not erase stock app agents.
  assert.ok(feed.errors.every(error => typeof error === 'string'));
});

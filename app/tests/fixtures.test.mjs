// This establishes the shared fixture entry point for later UI logic tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { root } from './helpers.mjs';

export const fixture = name => JSON.parse(readFileSync(join(root, 'tests/fixtures/app-feed', `${name}.json`), 'utf8'));

test('merged A1 swarm supplies nested, mixed-harness, needs-you UI scenarios', () => {
  const feed = fixture('swarm');
  assert.equal(feed.v, 1);
  const agents = feed.orchestrators.flatMap(root => root.threads.flatMap(thread => thread.agents));
  assert.ok(Math.max(...agents.map(agent => agent.depth)) >= 4);
  assert.ok(new Set(agents.map(agent => agent.harness)).size >= 2);
  for (const status of ['run', 'rev', 'wait', 'blk', 'fail', 'done', 'q', 'cxl', 'bg']) {
    assert.ok(agents.some(agent => agent.status === status), `fixture must exercise ${status}`);
  }
  assert.ok(agents.some(agent => agent.error && agent.last_message));
  assert.ok(feed.orchestrators.some(root => root.inbox.length));
});

test('merged A1 degraded feed remains data, alongside collector error strings', () => {
  const feed = fixture('degraded');
  assert.equal(feed.v, 1);
  assert.ok(feed.errors.length > 0);
  assert.deepEqual(feed.orchestrators, []); // A missing collector must not erase stock app agents.
  assert.ok(feed.errors.every(error => typeof error === 'string'));
});

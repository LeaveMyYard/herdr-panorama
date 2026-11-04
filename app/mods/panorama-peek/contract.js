// Generated from app/shared/contract.js by scripts/app-build.
// Portable feed v2 validation (docs/protocol/app-feed-v2.md). Unknown additive fields/vocabulary stay intact.
const fields = {
  Feed: { v: 'i', generated_at: 'n', herdr_ok: 'b', inbox_ok: 'b', errors: ['s'], counts: 'Counts', roots: ['Root'] },
  Counts: Object.fromEntries(['roots', 'agents', 'running', 'waiting', 'questions', 'blocked', 'failed'].map(k => [k, 'i'])),
  Root: { pane_id: 's', host: 's', terminal: 's?', harness: 's', tab_id: 's?', tab_label: 's', title: 's', title_source: 's', session_id: 's?', herdr_status: 's', workspace_id: 's?', focused: 'b', cwd: 's?', transcript: 's?', pending: ['s'], inbox: ['i'], inbox_texts: ['s'], agents: ['Agent'], hidden_finished: 'i', tab_no: 'i?', machine: 's?', machine_id: 's?', bg: ['Background'], last_activity: 'n?' },
  Agent: { id: 's', harness: 's', title: 's', title_source: 's', description: 's', role: 's', agent_type: 's?', model: 's?', status: 's', tool_use_id: 's?', started: 'n?', ended: 'n?', last_at: 'n?', tool_calls: 'i', activity: 's', last_message: 's', questions: ['s'], pending: ['s'], question_ids: ['i'], error: 's?', parent: 's?', depth: 'i', resumes: 'i', inferred: 'b', transcript: 's?', seq: 'i', bg: ['Background'] },
  Background: { id: 's', kind: 's', description: 's', started: 'n?' },
};
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
function type(value, spec, path) {
  if (Array.isArray(spec)) {
    if (!Array.isArray(value)) throw Error(path);
    value.forEach((v, i) => type(v, spec[0], `${path}[${i}]`));
  } else if (spec.endsWith('?')) {
    if (value !== null) type(value, spec.slice(0, -1), path);
  } else if (fields[spec]) {
    if (!object(value)) throw Error(path);
    for (const [key, child] of Object.entries(fields[spec])) {
      if (!Object.hasOwn(value, key)) throw Error(`${path}.${key}`);
      type(value[key], child, `${path}.${key}`);
    }
  } else if (!(spec === 's' ? typeof value === 'string' : spec === 'b' ? typeof value === 'boolean' : spec === 'i' ? Number.isSafeInteger(value) : typeof value === 'number' && Number.isFinite(value))) throw Error(path);
}
// The writer's capability probe: exactly this first stdout line of `app-feed --help`.
export const FEED_PROBE = 'panorama app-feed v2';
export const probeMatches = stdout => typeof stdout === 'string' && stdout.split('\n')[0] === FEED_PROBE;
// Peek and its callers re-probe the bound private command before acting; any other writer throws.
export async function requireProbe(callPrivate) {
  const answer = await callPrivate(['app-feed', '--help']);
  if (answer?.code !== 0 || !probeMatches(answer.stdout)) throw Error(answer?.stderr || answer?.stdout || 'app-feed probe');
  return answer;
}
export const rootKey = root => JSON.stringify(['root', root.host, root.host === 'herdr-core' && root.terminal !== null ? root.terminal : root.pane_id]);
export const agentKey = (root, agent) => JSON.stringify(['agent', rootKey(root), agent.id]);
function unique(values, label) {
  if (values.some(value => value === '') || new Set(values).size !== values.length) throw Error(label);
}
export function parseFeed(text) {
  const feed = JSON.parse(text);
  type(feed, 'Feed', 'feed');
  if (feed.v !== 2) throw Error('feed.v');
  for (const field of Object.keys(fields.Counts)) if (feed.counts[field] < 0) throw Error('feed.counts');
  unique(feed.roots.map(rootKey), 'duplicate root');
  for (const root of feed.roots) {
    if (!root.pane_id || root.terminal === '' || !root.harness || !['herdr-core', 'herdr-tui', 'unknown'].includes(root.host) || root.hidden_finished < 0) throw Error('root identity/count');
    if (root.host === 'herdr-core' && (root.tab_id !== null || root.workspace_id !== null || root.tab_no !== null || root.focused !== false)) throw Error('core membership');
    if (!feed.inbox_ok && root.inbox.length) throw Error('unavailable inbox inventory');
    // Dangling, self and cyclic parents are valid: tree.js renders them as root children.
    unique(root.agents.map(agent => agent.id), 'agent identity');
    unique(root.inbox, 'inbox identity');
    if (root.inbox.length !== root.inbox_texts.length) throw Error('inbox alignment');
    const inbox = new Map(root.inbox.map((id, i) => [id, root.inbox_texts[i]]));
    for (const agent of root.agents) {
      if ([agent.tool_calls, agent.resumes].some(n => n < 0)) throw Error('agent counters');
      if (!agent.harness) throw Error('agent harness');
      unique(agent.question_ids, 'question identity');
      if (agent.question_ids.length !== agent.questions.length || agent.question_ids.some((id, i) => !inbox.has(id) || inbox.get(id) !== agent.questions[i])) throw Error('question ownership');
    }
  }
  return feed;
}
export function readResult(previous, text, error = null) {
  try {
    if (error || text === null) throw error || Error('feed.json');
    const feed = parseFeed(text);
    return { feed, stale: false, errors: feed.errors, generated_at: feed.generated_at };
  } catch (failure) {
    return { feed: previous?.feed ?? null, stale: true, errors: [String(failure.message ?? failure)], generated_at: previous?.generated_at ?? null };
  }
}

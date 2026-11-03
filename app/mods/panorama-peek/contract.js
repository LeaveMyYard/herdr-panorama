// Portable v1 validation. Unknown additive fields/vocabulary stay intact.
const fields = {
  Feed: { v: 'i', generated_at: 'n', herdr_ok: 'b', inbox_ok: 'b', errors: ['s'], counts: 'Counts', orchestrators: ['Root'] },
  Counts: Object.fromEntries(['orchestrators', 'threads', 'active_threads', 'agents', 'running', 'waiting', 'questions', 'blocked', 'failed'].map(k => [k, 'i'])),
  Root: { pane_id: 's', host: 's', terminal: 's?', harness: 's', tab_id: 's?', tab_label: 's', session_id: 's?', herdr_status: 's', workspace_id: 's?', focused: 'b', cwd: 's?', flavour: 's', transcript: 's?', pending: ['s'], inbox: ['i'], inbox_texts: ['s'], threads: ['Thread'], hidden_done: 'i', tab_no: 'i?', machine: 's?', machine_id: 's?', bg: ['Background'], last_activity: 'n?' },
  Thread: { id: 's', name: 's', source: 's', phases: ['Phase'], agents: ['Agent'], status: 's', started: 'n?', ended: 'n?', worktree: 's?', pct: 'i?', flavour: 's?' },
  Agent: { id: 's', harness: 's', description: 's', role: 's', agent_type: 's?', model: 's?', status: 's', tool_use_id: 's?', started: 'n?', ended: 'n?', last_at: 'n?', tool_calls: 'i', activity: 's', last_message: 's', questions: ['s'], pending: ['s'], question_ids: ['i'], error: 's?', phases: ['i'], parent: 's?', depth: 'i', resumes: 'i', inferred: 'b', transcript: 's?', seq: 'i', bg: ['Background'] },
  Phase: { name: 's', state: 's', start: 'n?', end: 'n?', note: 's?' },
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
export const rootKey = root => JSON.stringify(['root', root.host, root.host === 'herdr-core' && root.terminal !== null ? root.terminal : root.pane_id]);
export const threadKey = (root, thread) => JSON.stringify(['thread', rootKey(root), thread.id]);
export const agentKey = (root, agent) => JSON.stringify(['agent', rootKey(root), agent.id]);
function unique(values, label) {
  if (values.some(value => value === '') || new Set(values).size !== values.length) throw Error(label);
}
export function parseFeed(text) {
  const feed = JSON.parse(text);
  type(feed, 'Feed', 'feed');
  if (feed.v !== 1) throw Error('feed.v');
  for (const field of Object.keys(fields.Counts)) if (feed.counts[field] < 0) throw Error('feed.counts');
  unique(feed.orchestrators.map(rootKey), 'duplicate root');
  for (const root of feed.orchestrators) {
    if (!root.pane_id || root.terminal === '' || !root.harness || !['herdr-core', 'herdr-tui', 'unknown'].includes(root.host) || root.hidden_done < 0) throw Error('root identity/count');
    if (root.host === 'herdr-core' && (root.tab_id !== null || root.workspace_id !== null || root.tab_no !== null || root.focused !== false)) throw Error('core membership');
    if (!feed.inbox_ok && root.inbox.length) throw Error('unavailable inbox inventory');
    unique(root.threads.map(thread => thread.id), 'thread identity');
    unique(root.threads.flatMap(thread => thread.agents.map(agent => agent.id)), 'agent identity');
    unique(root.inbox, 'inbox identity');
    if (root.inbox.length !== root.inbox_texts.length) throw Error('inbox alignment');
    const inbox = new Map(root.inbox.map((id, i) => [id, root.inbox_texts[i]]));
    for (const thread of root.threads) for (const agent of thread.agents) {
      if ([agent.tool_calls, agent.resumes].some(n => n < 0)) throw Error('agent counters');
      if (!agent.harness) throw Error('agent harness');
      unique(agent.question_ids, 'question identity');
      if (agent.question_ids.length !== agent.questions.length || agent.question_ids.some((id, i) => !inbox.has(id) || inbox.get(id) !== agent.questions[i])) throw Error('question ownership');
      if (agent.phases.some(index => index < 0 || index >= thread.phases.length)) throw Error('phase index');
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

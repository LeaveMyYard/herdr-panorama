// Generated from app/shared/tree.js by scripts/app-build.
import { agentKey, threadKey } from './contract.js';
export const needs = status => ['fail', 'blk', 'wait'].includes(status);
export const finished = status => ['done', 'cxl', 'inf'].includes(status);
export const urgency = status => ({ fail: 12, blk: 11, wait: 10, rev: 9, run: 8, bg: 7, q: 6, lost: 5, inf: 4, done: 3, cxl: 2 }[status] ?? 0);
export function forest(agents) {
  const nodes = agents.map(agent => ({ agent, children: [] }));
  const index = new Map(nodes.map(node => [node.agent.id, node]));
  const roots = [], seen = new Set();
  for (const node of nodes) {
    const parent = index.get(node.agent.parent);
    if (!parent || parent === node) roots.push(node);
    else parent.children.push(node);
  }
  // Cut unreachable cycle components into deterministic thread roots. Traverse
  // iteratively so arbitrary producer depth cannot overflow the JS stack.
  const visit = start => {
    const stack = [start];
    while (stack.length) {
      const node = stack.pop();
      if (seen.has(node)) continue;
      seen.add(node);
      node.children = node.children.filter(child => !seen.has(child));
      stack.push(...node.children.slice().reverse());
    }
  };
  roots.forEach(visit);
  for (const node of nodes) if (!seen.has(node)) { roots.push(node); visit(node); }
  return roots;
}
export function aggregate(root, ownStatus = root.herdr_status) {
  const agents = root.threads.flatMap(thread => thread.agents);
  const statuses = new Set(agents.map(agent => agent.status));
  const emptyFailure = root.threads.some(thread => !thread.agents.length && thread.status === 'failed');
  const kinds = ['fail', 'blk', 'wait'].filter(status => statuses.has(status) || (status === 'fail' && emptyFailure) || (status === 'blk' && ownStatus === 'blocked') || (status === 'wait' && root.inbox.length));
  const detailed = agents.filter(agent => needs(agent.status)).length;
  const needsCount = detailed || (ownStatus === 'blocked' || root.inbox.length > 0 || emptyFailure ? 1 : 0);
  return { kinds, needsCount, questions: root.inbox.length, working: ownStatus === 'working' || statuses.has('run') || statuses.has('rev'), sweep: statuses.has('run') || statuses.has('rev'), priority: kinds.length ? 4 : ownStatus === 'done' ? 3 : ownStatus === 'working' || statuses.has('run') || statuses.has('rev') ? 2 : 1, urgency: Math.max(0, ...kinds.map(urgency)) };
}
export function threadMark(thread) {
  if (!thread.agents.length) return { status: { done: 'done', cancelled: 'cxl', failed: 'fail' }[thread.status] ?? 'q', questions: 0 };
  return { status: thread.agents.reduce((top, a) => urgency(a.status) > urgency(top) ? a.status : top, ''), questions: new Set(thread.agents.flatMap(agent => agent.question_ids)).size };
}
const signature = node => JSON.stringify([node.agent.id, node.agent.status, node.agent.question_ids]) + '|' + node.children.map(child => child.signature).join('|');
export function annotate(nodes) {
  const order = [], stack = nodes.slice();
  while (stack.length) { const node = stack.pop(); order.push(node); stack.push(...node.children); }
  for (const node of order.reverse()) {
    node.hasNews = needs(node.agent.status) || node.children.some(child => child.hasNews);
    node.signature = signature(node);
  }
  return nodes;
}
// Explicit user folds expire only when that subtree changes status/questions.
export function isOpen(folds, key, statusSignature, fallback) {
  const entry = folds.get(key);
  if (entry && entry.signature === statusSignature) return entry.open;
  if (entry) folds.delete(key);
  return fallback;
}
export function rows(root, folds, rootOpen) {
  if (!rootOpen) return [];
  const output = [], threads = root.threads;
  const active = threads.filter(thread => !['done', 'cancelled'].includes(thread.status));
  const done = threads.filter(thread => ['done', 'cancelled'].includes(thread.status));
  const key = JSON.stringify(['summary', 'threads', root.host, root.terminal, root.pane_id]);
  const doneSignature = JSON.stringify(done.map(thread => [thread.id, thread.status]));
  const expandDone = isOpen(folds, key, doneSignature, false);
  const visibleThreads = expandDone ? threads : active;
  for (let i = 0; i < visibleThreads.length; i++) {
    const thread = visibleThreads[i], key = threadKey(root, thread);
    const roots = annotate(forest(thread.agents));
    const sig = JSON.stringify([thread.status, roots.map(node => node.signature)]);
    const lines = [i === visibleThreads.length - 1 && (!done.length || expandDone) ? '└' : '├'];
    output.push({ kind: 'thread', key, signature: sig, thread, lines, mark: threadMark(thread) });
    if (!isOpen(folds, key, sig, true)) continue;
    const stack = [{ nodes: roots, lines: [lines[0] === '└' ? '' : '│'], parentKey: key, parentSig: sig, force: folds.get(key)?.open === true }];
    while (stack.length) {
      const group = stack.pop();
      if (group.row || group.summary) { output.push(group.row ?? group.summary); continue; }
      const completed = group.nodes.filter(node => finished(node.agent.status) && !node.hasNews);
      const summaryKey = JSON.stringify(['summary', 'agents', group.parentKey]), summarySig = JSON.stringify(completed.map(node => [node.agent.id, node.signature]));
      const all = group.force || isOpen(folds, summaryKey, summarySig, false);
      if (!all && !group.nodes.some(node => node.hasNews)) continue;
      const shown = group.nodes.filter(node => all || node.hasNews);
      if (!all && completed.length) stack.push({ summary: { kind: 'summary', key: summaryKey, signature: summarySig, count: completed.length, names: completed.map(node => node.agent.role), lines: [...group.lines, '└'] } });
      for (let n = shown.length - 1; n >= 0; n--) {
        const node = shown[n], key = agentKey(root, node.agent), last = n === shown.length - 1 && (all || !completed.length);
        const lines = [...group.lines, last ? '└' : '├'];
        const open = isOpen(folds, key, node.signature, node.children.some(child => child.hasNews));
        if (open && node.children.length) stack.push({ nodes: node.children, lines: [...group.lines, last ? '' : '│'], parentKey: key, parentSig: node.signature, force: folds.get(key)?.open === true });
        stack.push({ row: { kind: 'agent', key, signature: node.signature, agent: node.agent, thread, lines, children: node.children.length, open } });
      }

    }
  }
  if (done.length && !expandDone) output.push({ kind: 'summary', key, signature: doneSignature, count: done.length, names: done.map(thread => thread.name), tasks: true, lines: ['└'] });
  return output;
}

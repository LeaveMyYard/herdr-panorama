import { agentKey, rootKey } from './contract.js';
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
  // Cut unreachable cycle components into deterministic root children. Traverse
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
// Feed v2: one uniform agent tree per root. A node's own state is its status (for a
// root: herdr status, its inbox and own blocked); needs marks cover every descendant.
export function aggregate(root, ownStatus = root.herdr_status) {
  const agents = root.agents ?? [], inbox = root.inbox ?? [];
  const statuses = new Set(agents.map(agent => agent.status));
  const kinds = ['fail', 'blk', 'wait'].filter(status => statuses.has(status) || (status === 'blk' && ownStatus === 'blocked') || (status === 'wait' && inbox.length));
  const detailed = agents.filter(agent => needs(agent.status)).length;
  const needsCount = detailed || (ownStatus === 'blocked' || inbox.length > 0 ? 1 : 0);
  const running = statuses.has('run') || statuses.has('rev');
  return { kinds, needsCount, questions: inbox.length, working: ownStatus === 'working' || running, sweep: running, priority: kinds.length ? 4 : ownStatus === 'done' ? 3 : ownStatus === 'working' || running ? 2 : 1, urgency: Math.max(0, ...kinds.map(urgency)) };
}
// What every view calls an agent: the writer's cleaned title, else role, else id (feed v2 "Titles").
export const agentTitle = agent => agent.title || agent.role || agent.id;
export function subtree(node) {
  const result = [], stack = [node];
  while (stack.length) { const next = stack.pop(); result.push(next); stack.push(...next.children); }
  return result;
}
const top = nodes => nodes.reduce((best, node) => urgency(node.agent.status) > urgency(best) ? node.agent.status : best, '');
// A subtree stays listed while anything in it is unfinished (failed and lost included).
export const live = node => subtree(node).some(item => !finished(item.agent.status));
// Line 2 of a root card: one tile per active direct subagent (its subtree's most urgent
// state, ?N with its distinct questions), ✓n for finished ones, then the counts.
export function directTiles(root, natives = 0) {
  const heads = forest(root.agents ?? []), tiles = [];
  for (const head of heads) if (live(head)) {
    const nodes = subtree(head);
    tiles.push({ status: top(nodes), questions: new Set(nodes.flatMap(node => node.agent.question_ids)).size, key: head.agent.id });
  }
  return { tiles, done: heads.length - tiles.length, active: tiles.length, live: (root.agents ?? []).filter(agent => !finished(agent.status)).length + natives };
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
// The docked root's list (V11 sidebar): layer 1 is the root's subagents, layer 2 theirs.
// Every row is the same agent row; deeper agents fold into a layer-2 row's state run.
// Finished direct subagents fold into one `N finished · titles` row with the writer's
// `+N older`; an open layer-1 row's run counts only its finished children.
export function rows(root, folds, rootOpen) {
  if (!rootOpen) return [];
  const output = [], heads = annotate(forest(root.agents ?? []));
  const done = heads.filter(node => !live(node)), older = root.hidden_finished ?? 0;
  const summaryKey = JSON.stringify(['summary', 'agents', rootKey(root)]);
  const summarySig = JSON.stringify([done.map(node => node.signature), older]);
  const expand = done.length > 0 && isOpen(folds, summaryKey, summarySig, false);
  const shown = expand ? heads : heads.filter(live), fold = done.length > 0 || older > 0;
  const agentRow = (node, layer, last, lines) => {
    const key = agentKey(root, node.agent), branch = [...lines, last ? '└' : '├'];
    const kids = node.children.filter(live), open = layer < 2 && kids.length > 0 && isOpen(folds, key, node.signature, true);
    const below = open ? [] : layer < 2 && !kids.length ? node.children : subtree(node).slice(1);
    const finishedKids = open ? node.children.length - kids.length : 0;
    output.push({ kind: 'agent', key, signature: node.signature, agent: node.agent, layer, lines: branch, open,
      children: layer < 2 ? kids.length : 0, run: { statuses: below.map(item => item.agent.status), finished: finishedKids } });
    if (open) kids.forEach((kid, n) => agentRow(kid, layer + 1, n === kids.length - 1, [...lines, last ? '' : '│']));
  };
  shown.forEach((node, n) => agentRow(node, 1, n === shown.length - 1 && !fold, []));
  if (fold) output.push({ kind: 'summary', key: summaryKey, signature: summarySig, count: done.length, names: done.map(node => agentTitle(node.agent)), older, open: expand, lines: ['└'] });
  return output;
}

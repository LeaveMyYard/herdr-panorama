// Provisional adapter; replace with shared-owner packet before frozen gates.
import { aggregate } from './tree.js';
export function spaceItems(roots, space) {
  return roots.flatMap(root => [root, ...(root.nativeChildren ?? [])]).filter(item => item.space === space);
}
export function spaceTotals(roots, space) {
  const items = spaceItems(roots, space), seen = new Set();
  const total = { needsCount: 0, questions: 0, working: false, done: false, kinds: [], sweep: false, orchestrators: 0, threads: 0, agents: 0 };
  for (const item of items) {
    if (seen.has(item.key)) continue; seen.add(item.key);
    const own = aggregate(item.root, item.status);
    total.needsCount += own.needsCount; total.questions += own.questions;
    total.working ||= own.working; total.sweep ||= own.sweep; total.done ||= item.status === 'done';
    total.kinds = ['fail','blk','wait'].filter(kind => total.kinds.includes(kind) || own.kinds.includes(kind));
    total.orchestrators += !item.isNative; total.threads += item.root.threads.length;
    total.agents += item.root.threads.reduce((count, thread) => count + thread.agents.length, 0) + Number(item.isNative);
  }
  return total;
}
export function graphTab(space, panes) {
  const ids = new Set(panes.filter(pane => pane.mod === 'panorama-graph').map(pane => pane.tab));
  return space?.tabs.find(tab => ids.has(tab.id)) ?? null;
}

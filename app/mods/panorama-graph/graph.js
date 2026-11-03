import { forest, finished, needs, threadMark } from './tree.js';
import { threadKey, agentKey } from './contract.js';
export const GAP = 36;
export const columnWidth = column => column === 0 ? 236 : column === 1 ? 300 : 272;
export const span = (lo, hi) => {
  let size = 0;
  for (let column = lo; column <= hi; column++) size += columnWidth(column) + (column > lo ? GAP : 0);
  return size;
};
export function columnWindow(count, focus, width, manual = null) {
  if (!count) return { lo: 0, hi: -1, columns: [], total: 0, left: 0, shown: 0 };
  focus = Math.max(0, Math.min(focus, count - 1));
  let lo = Math.max(0, focus - 1), hi = focus;
  const fits = (a, b) => span(a, b) <= Math.max(0, width - 24);
  // The focused column and its parent are retained even at narrow widths.
  if (hi + 1 < count && fits(lo, hi + 1)) hi++;
  while (hi + 1 < count && fits(lo, hi + 1)) hi++;
  while (lo > 0 && fits(lo - 1, hi)) lo--;
  if (manual !== null) {
    lo = Math.max(0, Math.min(manual, count - 1)); hi = lo;
    while (hi + 1 < count && fits(lo, hi + 1)) hi++;
  }
  let x = lo > 0 ? 24 : 0;
  const columns = [];
  for (let column = lo; column <= hi; column++) {
    columns.push({ column, x, width: columnWidth(column) }); x += columnWidth(column) + GAP;
  }
  return { lo, hi, columns, total: span(0, count - 1), left: lo ? span(0, lo - 1) + GAP : 0, shown: span(lo, hi) };
}
export function agentHeight(agent, selected) {
  if (!selected) return 54;
  const wrap = text => Math.max(1, Math.ceil([...String(text ?? '')].length / 34));
  const details = [agent.activity ?? '', ...(agent.questions ?? []), ...(agent.error ? [agent.error] : [])];
  return 68 + details.reduce((height, text) => height + wrap(text) * 17 + 3, 0);
}
function shownTree(agents) {
  const roots = forest(agents), order = [], stack = [...roots];
  while (stack.length) { const node = stack.pop(); order.push(node); stack.push(...node.children); }
  for (const node of order.reverse()) node.shown = !finished(node.agent.status) || node.children.some(child => child.shown);
  return roots;
}
export function treeLayout(item, thread, selected, expanded = new Set()) {
  if (!thread) return { nodes: [], groups: [], depth: 0 };
  const roots = shownTree(thread.agents), nodes = [], groups = [], cur = [], trunk = [];
  // Explicit stack supports producer trees deeper than the JavaScript call stack.
  const work = [{ kind: 'group', list: roots, depth: 1, parent: null, parentY: 0, key: threadKey(item.root, thread) }];
  while (work.length) {
    const task = work.pop();
    if (task.kind === 'end') { trunk[task.group.depth] = Math.max(trunk[task.group.depth] ?? 0, ...task.group.ys); continue; }
    if (task.kind === 'node') {
      const { group, source } = task, depth = group.depth;
      const fold = Array.isArray(source), agent = fold ? null : source.agent;
      const key = fold ? JSON.stringify(['finished', group.key]) : agentKey(item.root, agent);
      const children = !fold && source.children.length > 0;
      const height = fold ? 40 : agentHeight(agent, selected === key);
      const y = Math.max(cur[depth] ?? 0, group.parentY, children ? (trunk[depth + 1] ?? -100) - 27 + 16 : 0);
      const node = { key, agent, fold: fold ? source.map(node => node.agent) : null, depth, column: depth + 1, parent: group.parent, y, height, mid: fold ? 20 : 27 };
      nodes.push(node); cur[depth] = y + height + 10; group.ys.push(y + node.mid);
      if (children) work.push({ kind: 'group', list: source.children, depth: depth + 1, parent: node, parentY: y, key });
      continue;
    }
    const group = { depth: task.depth, parent: task.parent, parentY: task.parentY, key: task.key, ys: [] };
    groups.push(group);
    const done = task.list.filter(node => !node.shown), show = task.list.filter(node => node.shown || expanded.has(group.key));
    work.push({ kind: 'end', group });
    if (done.length && !expanded.has(group.key)) work.push({ kind: 'node', source: done, group });
    for (const source of show.slice().reverse()) work.push({ kind: 'node', source, group });
  }
  return { nodes, groups, depth: nodes.reduce((depth, node) => Math.max(depth, node.depth), 0) };
}
export function nativeLayout(item, selected) {
  // A separate fallback projection: native ancestry never acquires a transcript thread.
  const items = (item.nativeChildren ?? []).filter(child => child.space === item.space);
  const terminals = new Map(items.map(child => [child.terminal, child]));
  const roots = items.filter(child => !terminals.has(child.nativeParent) || child.nativeParent === child.terminal);
  const nodes = [], groups = [], seen = new Set(), cur = [], trunk = [];
  const work = roots.slice().reverse().map(child => ({ child, parent: null, depth: 1 }));
  for (;;) {
    if (!work.length) {
      const disconnected = items.find(child => !seen.has(child.key));
      if (!disconnected) break;
      work.push({ child: disconnected, parent: null, depth: 1 });
    }
    const task = work.pop();
    if (task.end) { trunk[task.node.depth] = Math.max(trunk[task.node.depth] ?? 0, task.node.y + task.node.mid); continue; }
    if (seen.has(task.child.key)) continue;
    seen.add(task.child.key);
    const children = items.filter(child => child.nativeParent === task.child.terminal && !seen.has(child.key));
    const y = Math.max(cur[task.depth] ?? 0, task.parent?.y ?? 0, children.length ? (trunk[task.depth + 1] ?? -100) - 27 + 16 : 0);
    const node = { kind: 'native', key: task.child.key, item: task.child, parent: task.parent, depth: task.depth, column: task.depth, y, mid: 27, height: task.child.key === selected ? 100 : 54 };
    nodes.push(node); cur[task.depth] = y + node.height + 10;
    let group = groups.find(group => group.parent === task.parent && group.depth === task.depth);
    if (!group) { group = { parent: task.parent, depth: task.depth, ys: [] }; groups.push(group); }
    group.ys.push(y + node.mid);
    work.push({ end: true, node });
    work.push(...children.slice().reverse().map(child => ({ child, parent: node, depth: task.depth + 1 })));
  }
  return { nodes, groups, depth: nodes.reduce((depth, node) => Math.max(depth, node.depth), 0) };
}
export function graphModel(roots, state) {
  const item = roots.find(item => item.key === state.orch) ?? roots[0] ?? null;
  const threads = item?.root.threads ?? [];
  const thread = threads.find(thread => threadKey(item.root, thread) === state.thread) ?? threads[0] ?? null;
  const selected = state.focus >= (thread ? 2 : 1) ? state.agent : null;
  const tree = item ? (thread ? treeLayout(item, thread, selected, state.expanded) : nativeLayout(item, selected)) : { nodes: [], groups: [], depth: 0 };
  const orchs = roots.map((root, index) => ({ key: root.key, kind: 'orch', item: root, column: 0, y: index * 64, height: 54, mid: 27 }));
  let y = 0;
  const threadNodes = threads.map(entry => {
    const key = threadKey(item.root, entry), height = entry === thread && state.focus === 1 ? 64 + entry.phases.length * 20 : 54;
    const node = { key, kind: 'thread', thread: entry, column: 1, y, height, mid: 27, mark: threadMark(entry) }; y += height + 10; return node;
  });
  const nodes = [...orchs, ...threadNodes, ...tree.nodes.map(node => ({ ...node, kind: node.kind ?? (node.fold ? 'fold' : 'agent') }))];
  const columns = Array.from({ length: (thread ? 2 : 1) + tree.depth }, (_, column) => nodes.filter(node => node.column === column));
  return { item, thread, tree, nodes, columns, orchs, threadNodes, nativeFallback: !thread };
}
export function currentNode(model, state) {
  const key = state.focus === 0 ? state.orch : model.columns[state.focus]?.[0]?.kind === 'thread' ? state.thread : state.agent;
  return model.columns[state.focus]?.find(node => node.key === key) ?? model.columns[state.focus]?.[0] ?? null;
}
export function selectNode(state, model, node) {
  if (!node) return;
  if (node.kind === 'orch') {
    if (state.orch !== node.key) { state.orch = node.key; state.thread = null; state.agent = null; }
  } else if (node.kind === 'thread') {
    if (state.thread !== node.key) { state.thread = node.key; state.agent = null; }
  } else {
    state.agent = node.key;
    if (node.parent) state.remember.set(node.parent.key, node.key);
    else if (node.kind === 'native') state.remember.set(model.item.key, node.key);
  }
  state.focus = node.column; state.manual = null; state.scroll = {}; state.follow = true;
}
export function navigate(state, model, direction) {
  const current = currentNode(model, state);
  if (!current) return false;
  if (direction === 'up' || direction === 'down') {
    const column = model.columns[state.focus], index = column.indexOf(current);
    const next = column[Math.max(0, Math.min(column.length - 1, index + (direction === 'down' ? 1 : -1)))];
    if (next.key === current.key) return false;
    selectNode(state, model, next); return true;
  }
  if (direction === 'left') {
    if (!state.focus) return false;
    if (current.kind === 'native') {
      if (current.parent) selectNode(state, model, current.parent);
      else { state.focus = 0; state.manual = null; state.scroll = {}; }
    }
    else if (state.focus >= 3 && current.parent) selectNode(state, model, current.parent);
    else { state.focus--; state.manual = null; state.scroll = {}; }
    return true;
  }
  const next = model.columns[state.focus + 1];
  if (!next?.length || current.kind === 'fold') return false;
  let choices = (current.kind === 'native' || state.focus >= 2) ? next.filter(node => node.parent?.key === current.key) : next;
  if (!choices.length) return false;
  const remembered = state.remember.get(current.key);
  let target = choices.find(node => node.key === remembered);
  if (!target && state.focus === 1 && state.agent) {
    let selected = model.nodes.find(node => node.key === state.agent);
    while (selected?.parent) selected = selected.parent;
    target = choices.find(node => node.key === selected?.key);
  }
  selectNode(state, model, target ?? choices[0]); return true;
}
export function selectionPath(model, state) {
  const path = new Set([model.item?.key, model.threadNodes.find(node => node.thread === model.thread)?.key]);
  let node = model.nodes.find(node => node.key === state.agent);
  while (node) { path.add(node.key); node = node.parent; }
  return path;
}
export class ClickGuard {
  constructor() { this.positions = new Map(); this.moved = new Map(); this.scrolled = -Infinity; }
  scroll(now) { this.scrolled = now; }
  refresh(nodes, now, arm = true) {
    const positions = new Map();
    for (const node of nodes) {
      const position = JSON.stringify([node.column, node.y, node.height]); positions.set(node.key, position);
      if (arm && this.positions.has(node.key) && this.positions.get(node.key) !== position) this.moved.set(node.key, now);
      else if (arm && this.positions.size && !this.positions.has(node.key)) this.moved.set(node.key, now);
    }
    this.positions = positions;
    for (const key of this.moved.keys()) if (!positions.has(key)) this.moved.delete(key);
  }
  allows(key, now) { return now - this.scrolled >= 300 && now - (this.moved.get(key) ?? -Infinity) >= 1000; }
}
export function cardMotion(node) {
  if (node.kind === 'orch') return { edge: node.item.aggregate.kinds.length > 0, sweep: Boolean(node.item.aggregate.sweep) };
  if (node.kind === 'native') return { edge: node.item.aggregate.kinds.length > 0, sweep: Boolean(node.item.aggregate.working) };
  if (node.kind === 'agent') return { edge: needs(node.agent.status), sweep: ['run', 'rev'].includes(node.agent.status) };
  return { edge: false, sweep: false };
}
const intersects = (start, end, clip) => clip > 0 && end > 0 && start < clip && end > start;
export function animationVisible(node, offset, height, x = 0, width = 272, clipWidth = Infinity) {
  const motion = cardMotion(node), top = node.y - offset;
  // Match the drawn overlays, not the full card: its top/bottom corners do not animate.
  const edge = motion.edge && intersects(x - 1.5, x + .5, clipWidth) && intersects(top + 8, top + node.height - 8, height);
  const sweep = motion.sweep && intersects(x + 10, x + width - 10, clipWidth) && intersects(top + node.height - 1.5, top + node.height + .5, height);
  return edge || sweep;
}
export function viewportAnimates(model, window, offsets, width, height) {
  return window.columns.some(column => model.columns[column.column].some(node => animationVisible(node, offsets[column.column] ?? 0, height, column.x, column.width, width)));
}

// The installed app reports post-zoom bounds; graph card coordinates remain pre-zoom.
export function logicalSize(measured, zoom) {
  const scale = Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
  return { width: measured.width / scale, height: measured.height / scale };
}

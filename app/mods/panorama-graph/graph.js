import { forest, finished, needs, agentTitle } from './tree.js';
import { agentKey } from './contract.js';
// V11 graph (frames D and D2): column 0 holds the space's root sessions, then one column
// per depth of the selected root's agent tree. Every column and card is the same.
export const GAP = 36, WIDTH = 272;
export const span = (lo, hi) => hi < lo ? 0 : (hi - lo + 1) * WIDTH + (hi - lo) * GAP;
export function columnWindow(count, focus, width, manual = null) {
  if (!count) return { lo: 0, hi: -1, columns: [], total: 0, left: 0, shown: 0 };
  focus = Math.max(0, Math.min(focus, count - 1));
  let lo = Math.max(0, focus - 1), hi = focus;
  const fits = (a, b) => span(a, b) <= Math.max(0, width - 24);
  // The focused column and its parent are retained even at narrow widths.
  while (hi + 1 < count && fits(lo, hi + 1)) hi++;
  while (lo > 0 && fits(lo - 1, hi)) lo--;
  if (manual !== null) {
    lo = Math.max(0, Math.min(manual, count - 1)); hi = lo;
    while (hi + 1 < count && fits(lo, hi + 1)) hi++;
  }
  const x0 = lo > 0 ? 24 : 0;
  const columns = Array.from({ length: hi - lo + 1 }, (_, n) => ({ column: lo + n, x: x0 + n * (WIDTH + GAP), width: WIDTH }));
  return { lo, hi, columns, total: span(0, count - 1), left: lo ? span(0, lo - 1) + GAP : 0, shown: span(lo, hi) };
}
const BORDER = 2; // The card's 1px border, on both sides of its body.
// What a selected agent's `agent` line says.
export const agentMeta = (agent, age) => [[agent.role, agent.id].filter(Boolean).join(' '), agent.model, age, agent.tool_calls ? `${agent.tool_calls} calls` : ''].filter(Boolean).join(' · ');
// A selected agent opens with its `agent` line, what it is doing, its questions and error.
// Those lines wrap, and only the host knows where: `measured` is the height of the card as
// laid out. The estimate covers the first frame, before the host has reported it.
export function agentHeight(agent, selected, measured) {
  if (!selected) return 54;
  if (measured) return measured;
  const wrap = text => Math.max(1, Math.ceil([...String(text ?? '')].length / 34));
  const details = [agentMeta(agent, '<1m'), agent.activity ?? '', ...(agent.questions ?? []), ...(agent.error ? [agent.error] : [])];
  return 68 + details.reduce((height, text) => height + wrap(text) * 17 + 3, 0);
}
// Keeps a selected card's height from its reported body size; true when the layout must follow.
// The body spans the card inside its border, so its width gives the host's scale, whatever
// mix of zoom and font size made it. Pixel rounding can move a report by 1px as the card
// moves, and following that could keep a bottom-pinned card moving, so it is ignored.
export function recordCardHeight(heights, key, size) {
  const scale = size.width / (WIDTH - BORDER);
  if (!(scale > 0) || !(size.height > 0)) return false;
  const height = Math.round(size.height / scale) + BORDER;
  if (Math.abs((heights.get(key) ?? -Infinity) - height) <= 1) return false;
  heights.set(key, height); return true;
}
// One tree under a root: its transcript agents, then its verified same-space native terminal
// children, each carrying its own feed agents and native children. A feed root that is also a
// native child joins as that native node only (R8); joinRoots never lists it again at depth 0.
export function rootTree(item) {
  const natives = (item.nativeChildren ?? []).filter(child => child.space === item.space);
  const terminals = new Set(natives.map(child => child.terminal)), under = new Map(), top = [];
  for (const child of natives) {
    const parent = child.nativeParent;
    if (!terminals.has(parent) || parent === child.terminal) top.push(child);
    else under.set(parent, [...(under.get(parent) ?? []), child]);
  }
  const agentNodes = owner => {
    const heads = forest(owner.root.agents ?? []), wrapped = new Map(), stack = heads.slice();
    while (stack.length) { const node = stack.pop(); wrapped.set(node, { kind: 'agent', key: agentKey(owner.root, node.agent), agent: node.agent, owner, children: [] }); stack.push(...node.children); }
    for (const [node, out] of wrapped) out.children = node.children.map(child => wrapped.get(child));
    return heads.map(node => wrapped.get(node));
  };
  const nativeNode = child => ({ kind: 'native', key: child.key, item: child, owner: child, children: agentNodes(child) });
  const seen = new Set(), result = [...agentNodes(item), ...top.map(nativeNode)], stack = result.filter(node => node.kind === 'native').reverse();
  // Iterative and seen-guarded: cyclic or disconnected native ancestry lists each terminal once.
  for (;;) {
    while (stack.length) {
      const node = stack.pop(); if (seen.has(node.key)) continue; seen.add(node.key);
      const kids = (under.get(node.item.terminal) ?? []).filter(child => !seen.has(child.key)).map(nativeNode);
      node.children.push(...kids); stack.push(...kids.slice().reverse());
    }
    const lost = natives.find(child => !seen.has(child.key));
    if (!lost) break;
    const node = nativeNode(lost); result.push(node); stack.push(node);
  }
  return result;
}
// Bottom-up: a node stays listed while anything in its subtree is unfinished.
function annotate(roots) {
  const order = [], stack = roots.slice();
  while (stack.length) { const node = stack.pop(); order.push(node); stack.push(...node.children); }
  for (const node of order.reverse()) node.shown = node.kind === 'native' || !finished(node.agent.status) || node.children.some(child => child.shown);
  return roots;
}
export function treeLayout(item, selected = null, expanded = new Set(), heights = new Map()) {
  if (!item) return { nodes: [], groups: [], depth: 0 };
  const nodes = [], groups = [], cur = [], trunk = [];
  // Explicit stack supports producer trees deeper than the JavaScript call stack.
  const work = [{ kind: 'group', list: annotate(rootTree(item)), depth: 1, parent: null, parentY: 0, key: item.key }];
  while (work.length) {
    const task = work.pop();
    if (task.kind === 'end') { trunk[task.group.depth] = Math.max(trunk[task.group.depth] ?? 0, ...task.group.ys); continue; }
    if (task.kind === 'node') {
      const { group, source } = task, depth = group.depth, fold = Array.isArray(source);
      const children = !fold && source.children.length > 0;
      const height = fold ? 40 : source.kind === 'native' ? (selected === source.key ? 100 : 54) : agentHeight(source.agent, selected === source.key, heights.get(source.key));
      const y = Math.max(cur[depth] ?? 0, group.parentY, children ? (trunk[depth + 1] ?? -100) - 27 + 16 : 0);
      const node = fold
        ? { kind: 'fold', key: JSON.stringify(['finished', group.key]), group: group.key, fold: source.map(node => node.agent), owner: source[0]?.owner ?? item, older: group.older }
        : { kind: source.kind, key: source.key, agent: source.agent ?? null, item: source.item, owner: source.owner, children: source.children.length };
      Object.assign(node, { depth, column: depth, parent: group.parent, y, height, mid: fold ? 20 : 27 });
      nodes.push(node); cur[depth] = y + height + 10; group.ys.push(y + node.mid);
      if (children) work.push({ kind: 'group', list: source.children, depth: depth + 1, parent: node, parentY: y, key: node.key });
      continue;
    }
    // The writer's `+N older` (aged-out finished agents) rides on the root-level fold, even alone.
    const group = { depth: task.depth, parent: task.parent, parentY: task.parentY, key: task.key, ys: [], older: task.parent ? 0 : item.root.hidden_finished ?? 0 }, open = expanded.has(group.key);
    groups.push(group);
    // Finished siblings fold per parent into one `N finished · titles` card after the live ones.
    const done = task.list.filter(node => !node.shown), show = task.list.filter(node => node.shown || open);
    work.push({ kind: 'end', group });
    if ((done.length && !open) || group.older) work.push({ kind: 'node', source: open ? [] : done, group });
    for (const source of show.slice().reverse()) work.push({ kind: 'node', source, group });
  }
  return { nodes, groups, depth: nodes.reduce((depth, node) => Math.max(depth, node.depth), 0) };
}
export const nodeTitle = node => node.kind === 'agent' ? agentTitle(node.agent) : node.item?.name ?? '';
export function graphModel(roots, state) {
  const item = roots.find(item => item.key === state.root) ?? roots[0] ?? null;
  const tree = treeLayout(item, state.focus >= 1 ? state.agent : null, state.expanded, state.heights);
  const heads = roots.map((root, index) => ({ key: root.key, kind: 'root', item: root, owner: root, column: 0, y: index * 64, height: 54, mid: 27 }));
  const nodes = [...heads, ...tree.nodes];
  const columns = Array.from({ length: 1 + tree.depth }, (_, column) => nodes.filter(node => node.column === column));
  return { item, tree, nodes, columns, heads };
}
export function currentNode(model, state) {
  const key = state.focus === 0 ? state.root : state.agent;
  return model.columns[state.focus]?.find(node => node.key === key) ?? model.columns[state.focus]?.[0] ?? null;
}
export function selectNode(state, model, node) {
  if (!node) return;
  if (node.kind === 'root') {
    if (state.root !== node.key) { state.root = node.key; state.agent = null; }
  } else {
    state.agent = node.key;
    state.remember.set(node.parent?.key ?? model.item.key, node.key);
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
    if (current.parent) selectNode(state, model, current.parent);
    else { state.focus = 0; state.manual = null; state.scroll = {}; }
    return true;
  }
  const next = model.columns[state.focus + 1];
  if (!next?.length || current.kind === 'fold') return false;
  const choices = state.focus === 0 ? next : next.filter(node => node.parent?.key === current.key);
  if (!choices.length) return false;
  let target = choices.find(node => node.key === state.remember.get(current.key));
  if (!target && state.focus === 0 && state.agent) {
    let selected = model.nodes.find(node => node.key === state.agent && node.kind !== 'root');
    while (selected?.parent) selected = selected.parent;
    target = choices.find(node => node.key === selected?.key);
  }
  selectNode(state, model, target ?? choices[0]); return true;
}
const selectedPath = (model, state) => {
  const path = [];
  for (let node = model.nodes.find(node => node.key === state.agent && node.kind !== 'root'); node; node = node.parent) path.unshift(node);
  return path;
};
export function selectionPath(model, state) {
  return new Set([model.item?.key, ...selectedPath(model, state).map(node => node.key)]);
}
// Breadcrumb names: the root's title, then the selected path's titles.
export const crumbs = (model, state) => [model.item?.name ?? '', ...selectedPath(model, state).map(nodeTitle)];
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
  if (node.kind === 'root') return { edge: node.item.aggregate.kinds.length > 0, sweep: Boolean(node.item.aggregate.sweep) };
  if (node.kind === 'native') return { edge: node.item.aggregate.kinds.length > 0, sweep: Boolean(node.item.aggregate.working) };
  if (node.kind === 'agent') return { edge: needs(node.agent.status), sweep: ['run', 'rev'].includes(node.agent.status) };
  return { edge: false, sweep: false };
}
const intersects = (start, end, clip) => clip > 0 && end > 0 && start < clip && end > start;
export function animationVisible(node, offset, height, x = 0, width = WIDTH, clipWidth = Infinity) {
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

// Generated from app/shared/model.js by scripts/app-build.
import { rootKey } from './contract.js';
import { aggregate, urgency } from './tree.js';
const clean = text => (text ?? '').replace(/^[^\p{L}\p{N}]+/u, '').trim();
// A live terminal/tab name counts as the operator's only when it says something beyond
// the defaults: not empty, not a position, not the address or the program's own name.
function named(value, address, program) {
  const name = clean(value), lower = name.toLowerCase();
  return name && !/^\d+$/.test(name) && name !== address && ![program?.name, program?.key].some(p => p && p.toLowerCase() === lower) ? name : null;
}
// The live pane title, when it is telling: not a path, user@host or the program.
function telling(pane) {
  const program = pane?.program?.name, raw = (pane?.title ?? '').trim(), live = clean(raw);
  return live && live !== program && !/^[~/]|^[A-Za-z]:\\/.test(raw) && !live.includes('@') && live.toLowerCase() !== pane?.program?.key ? live : null;
}
// Root title in the app (plan §2.2, D3, D6): an operator-named terminal, then an operator-
// named tab holding exactly one agent root, then the feed title, then the telling live
// pane title, then the program name and address. The space name is secondary context.
export function rootTitle({ address, program, terminal, tab, alone, feed, pane }) {
  const pick = [[named(terminal?.name, address, program), 'pane'], [tab?.named && alone ? named(tab.name, address, program) : null, 'tab'],
    [clean(feed?.title), feed?.title_source ?? 'feed'], [telling(pane), 'live'], [program?.name && address ? `${program.name} · ${address}` : program?.name, 'program'], [address, 'id']].find(([value]) => value);
  return pick ? { name: pick[0], source: pick[1] } : { name: '', source: 'id' };
}
export function joinRoots(feed, stock, terminals, panes, spaces, changes = {}) {
  const enrichment = new Map((feed?.roots ?? []).filter(root => root.host === 'herdr-core' && root.terminal !== null).map(root => [root.terminal, root]));
  const used = new Set(), tabs = spaces.flatMap(space => space.tabs);
  const live = new Set(terminals.filter(t => t.state === 'live').map(t => t.address));
  const byAddress = new Map(terminals.map(t => [t.address, t]));
  const roots = stock.map((agent, index) => {
    const source = enrichment.get(agent.address);
    if (source) used.add(source);
    const root = source ?? { host: 'herdr-core', terminal: agent.address, pane_id: agent.pane ?? agent.address, harness: agent.program?.key ?? 'unknown', herdr_status: agent.status, tab_label: agent.title ?? '', title: '', title_source: 'id', inbox: [], agents: [], hidden_finished: 0 };
    const tab = tabs.find(tab => tab.id === agent.tab);
    const pane = panes.find(pane => pane.terminal === agent.address) ?? panes.find(pane => pane.pane === agent.pane);
    const title = rootTitle({ address: agent.address, program: agent.program, terminal: byAddress.get(agent.address), tab,
      alone: stock.filter(other => other.tab === agent.tab).length === 1, feed: source, pane: pane ?? { title: agent.title, program: agent.program } });
    return { key: rootKey(root), root, stock: agent, terminal: live.has(agent.address) ? agent.address : null,
      name: title.name, title_source: title.source, space: agent.space ?? null,
      tab: agent.tab ?? null, nativeChildren: [], isNative: false, status: agent.status, program: agent.program, pinned: Boolean(agent.pinned), focused: Boolean(agent.focused),
      seq: agent.seq ?? index, changed: changes[agent.address]?.at ?? 0, aggregate: aggregate(root, agent.status) };
  });
  for (const item of roots) {
    item.nativeChildren = terminalChildren(item.stock.address, terminals, stock).map(node => {
      const terminal = node.terminal, source = enrichment.get(terminal.address);
      if (source) used.add(source);
      const status = terminal.status === 'error' ? 'blocked' : terminal.status === 'done' ? 'idle' : terminal.status ?? 'idle';
      const root = source ?? { host: 'herdr-core', terminal: terminal.address, pane_id: terminal.address, harness: terminal.program?.key ?? 'unknown', herdr_status: status, title: '', title_source: 'id', inbox: [], agents: [], hidden_finished: 0 };
      const title = rootTitle({ address: terminal.address, program: terminal.program, terminal, feed: source });
      const membership = nativeMembership(terminal.address, stock, terminals, panes, spaces);
      return { key: rootKey(root), root, stock: null, terminal: terminal.address, isNative: true, nativeChildren: [], nativeParent: terminal.parent ?? null, pinned: false, focused: false, seq: 0, changed: 0, depth: node.depth, program: terminal.program ?? null, name: title.name, title_source: title.source, status, aggregate: aggregate(root, status), ...membership };
    });
  }
  // Projection follows the full native tree, independently of display ownership.
  // Listed descendants stay top-level but still affect their ancestors' urgency.
  const canonical = new Map();
  for (const item of roots) {
    canonical.set(item.stock.address, item);
    for (const child of item.nativeChildren) canonical.set(child.terminal, child);
  }
  for (const [address, entity] of canonical) {
    const related = [entity, ...terminalChildren(address, terminals, []).map(node => canonical.get(node.terminal.address)).filter(Boolean)];
    const own = aggregate(entity.root, entity.status);
    const contributions = related.map(item => aggregate(item.root, item.status));
    const kinds = ['fail', 'blk', 'wait'].filter(kind => contributions.some(part => part.kinds.includes(kind)));
    entity.aggregate = { ...own, kinds, needsCount: contributions.reduce((n, part) => n + part.needsCount, 0), questions: questions(related).size,
      working: contributions.some(part => part.working), sweep: own.sweep || contributions.slice(1).some(part => part.working),
      priority: kinds.length ? 4 : entity.status === 'done' ? 3 : contributions.some(part => part.working) ? 2 : 1,
      urgency: Math.max(0, ...kinds.map(urgency)) };
  }
  // Feed roots without exact app agent matches keep their opaque feed identity and space
  // null: no app space, tab, focus target or aggregate is invented for them. The sidebar
  // lists them after the active space's roots (feed v2 "Identity, tree and app joins").
  for (const root of feed?.roots ?? []) if (!used.has(root)) {
    const [name, source] = [[clean(root.title), root.title_source], [root.tab_label, 'tab'], [root.pane_id, 'id']].find(([value]) => value);
    roots.push({ key: rootKey(root), root, stock: null, terminal: null, name, title_source: source,
      space: null, tab: null, nativeChildren: [], isNative: false, status: root.herdr_status, program: null, pinned: false, focused: false, seq: 0,
      changed: 0, aggregate: aggregate(root) });
  }
  return roots.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.aggregate.priority - a.aggregate.priority || b.aggregate.urgency - a.aggregate.urgency || b.changed - a.changed || b.seq - a.seq);
}
// A root known only from the feed: no stock agent, no app space.
export const feedOnly = item => !item.stock && !item.isNative && item.space === null;
// The sidebar's list: the active space's roots, then every feed-only root (feed v2 keeps
// them listed under their feed identity, in the ordinary priority sections). A stale feed
// is not fresh evidence, so its feed-only roots then read Stale with no needs or work.
export function listed(roots, active, stale) {
  const quiet = item => ({ ...item, status: 'stale', stale: true, aggregate: { ...item.aggregate, kinds: [], needsCount: 0, questions: 0, working: false, sweep: false, priority: 1, urgency: 0 } });
  return roots.filter(item => (item.space !== null && item.space === active) || feedOnly(item)).map(item => stale && feedOnly(item) ? quiet(item) : item)
    .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.aggregate.priority - a.aggregate.priority); // stable: joinRoots order within a section
}
export function groups(roots) {
  const result = [];
  for (const root of roots) {
    const key = root.pinned ? 'pinned' : String(root.aggregate.priority);
    let group = result.find(group => group.key === key);
    if (!group) {
      group = { key, label: root.pinned ? 'Pinned' : ({ 4: 'Needs you', 3: 'Done', 2: 'Working', 1: 'Idle' }[key]), tone: root.pinned ? 'dim' : ({ 4: 'blocked', 3: 'done', 2: 'working', 1: 'dim' }[key]), roots: [] };
      result.push(group);
    }
    group.roots.push(root);
  }
  return result;
}
const agentStates = root => (root.agents ?? []).map(agent => [agent.id, agent.status, agent.question_ids]);
export const rootSignature = item => JSON.stringify([item.status, item.root.inbox, agentStates(item.root), (item.nativeChildren ?? []).map(child => [child.terminal, child.status, child.root.inbox, agentStates(child.root)])]);
// The first explicit harness on the agent's verified parent chain, else the root's.
export function harnessOf(agent, root) {
  let current = agent, seen = new Set();
  while (current) {
    if (current.harness && current.harness !== 'unknown') return current.harness;
    if (seen.has(current.id)) break;
    seen.add(current.id); current = (root.agents ?? []).find(candidate => candidate.id === current.parent);
  }
  return root.harness || 'unknown';
}
export const iconProgram = harness => ['claude', 'codex', 'gemini', 'cursor', 'amp'].includes(harness) ? { icon: `icons/${harness}.svg`, tint: ['gemini', 'cursor'].includes(harness) } : { icon: 'icons/agent.svg', tint: true };
export function terminalChildren(address, terminals, stock) {
  const listed = new Set(stock.map(agent => agent.address)), seen = new Set([address]), result = [];
  const stack = terminals.filter(t => t.parent === address && t.state === 'live' && !listed.has(t.address)).reverse().map(t => ({ terminal: t, depth: 1 }));
  while (stack.length) {
    const node = stack.pop(); if (seen.has(node.terminal.address)) continue;
    seen.add(node.terminal.address); result.push(node);
    stack.push(...terminals.filter(t => t.parent === node.terminal.address && t.state === 'live' && !listed.has(t.address)).reverse().map(t => ({ terminal: t, depth: node.depth + 1 })));
  }
  return result;
}

export function nativeMembership(address, stock, terminals, panes, spaces) {
  const live = new Map(terminals.filter(terminal => terminal.state === 'live').map(terminal => [terminal.address, terminal]));
  const listed = new Map(stock.map(agent => [agent.address, agent]));
  const seen = new Set();
  let current = address;
  while (current && !seen.has(current) && live.has(current)) {
    seen.add(current);
    const pane = panes.find(pane => pane.terminal === current);
    if (pane) {
      const space = spaces.find(space => space.tabs.some(tab => tab.id === pane.tab));
      if (space) return { space: space.id, tab: pane.tab };
    }
    const agent = listed.get(current);
    if (agent) return { space: agent.space ?? null, tab: agent.tab ?? null };
    current = live.get(current).parent;
  }
  return { space: null, tab: null };
}

function questions(entities) {
  return new Set(entities.flatMap(entity => entity.root.inbox.map(id => JSON.stringify([rootKey(entity.root), entity.root.session_id ?? null, id]))));
}

export function spaceTotals(roots, spaceId) {
  const entities = [], seen = new Set();
  for (const root of roots) for (const entity of [root, ...(root.nativeChildren ?? [])]) {
    const address = entity.stock?.address ?? entity.terminal;
    if (entity.space !== spaceId || address === null || address === undefined || seen.has(address)) continue;
    seen.add(address); entities.push(entity);
  }
  const own = entities.map(entity => aggregate(entity.root, entity.status));
  return { kinds: ['fail', 'blk', 'wait'].filter(kind => own.some(part => part.kinds.includes(kind))),
    needsCount: own.reduce((sum, part) => sum + part.needsCount, 0), questions: questions(entities).size,
    working: own.some(part => part.working), done: entities.some(entity => entity.status === 'done') };
}

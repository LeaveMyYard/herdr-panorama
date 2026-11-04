// Generated from app/shared/model.js by scripts/app-build.
import { rootKey } from './contract.js';
import { aggregate, urgency } from './tree.js';
const clean = text => (text ?? '').replace(/^[^\p{L}\p{N}]+/u, '').trim();
function title(tab, pane, fallback) {
  if (!tab) return fallback ?? '';
  const program = pane?.program?.name, live = clean(pane?.title);
  const telling = live && live !== program && !/^[~/]|^[A-Za-z]:\\/.test(live) && !live.includes('@') && live.toLowerCase() !== pane?.program?.key;
  return (tab.name === program || !tab.named) && telling ? live : tab.name;
}
export function joinRoots(feed, stock, terminals, panes, spaces, changes = {}) {
  const enrichment = new Map((feed?.orchestrators ?? []).filter(root => root.host === 'herdr-core' && root.terminal !== null).map(root => [root.terminal, root]));
  const used = new Set(), tabs = spaces.flatMap(space => space.tabs);
  const live = new Set(terminals.filter(t => t.state === 'live').map(t => t.address));
  const roots = stock.map((agent, index) => {
    const source = enrichment.get(agent.address);
    if (source) used.add(source);
    const root = source ?? { host: 'herdr-core', terminal: agent.address, pane_id: agent.pane ?? agent.address, harness: agent.program?.key ?? 'unknown', herdr_status: agent.status, tab_label: agent.title ?? '', inbox: [], threads: [] };
    const tab = tabs.find(tab => tab.id === agent.tab);
    const mine = panes.filter(pane => pane.tab === agent.tab && pane.terminal);
    const lead = mine.find(pane => pane.program?.agent) ?? mine.find(pane => pane.focused) ?? mine[0];
    const detail = title(tab, lead, agent.title || agent.program?.name);
    return { key: rootKey(root), root, stock: agent, terminal: live.has(agent.address) ? agent.address : null,
      name: agent.space_name || detail || agent.address, detail: /^\d+$/.test(detail) ? null : detail, space: agent.space ?? null,
      tab: agent.tab ?? null, nativeChildren: [], isNative: false, status: agent.status, program: agent.program, pinned: Boolean(agent.pinned), focused: Boolean(agent.focused),
      seq: agent.seq ?? index, changed: changes[agent.address]?.at ?? 0, aggregate: aggregate(root, agent.status) };
  });
  for (const item of roots) {
    item.nativeChildren = terminalChildren(item.stock.address, terminals, stock).map(node => {
      const terminal = node.terminal, source = enrichment.get(terminal.address);
      if (source) used.add(source);
      const status = terminal.status === 'error' ? 'blocked' : terminal.status === 'done' ? 'idle' : terminal.status ?? 'idle';
      const root = source ?? { host: 'herdr-core', terminal: terminal.address, pane_id: terminal.address, harness: terminal.program?.key ?? 'unknown', herdr_status: status, inbox: [], threads: [] };
      const membership = nativeMembership(terminal.address, stock, terminals, panes, spaces);
      return { key: rootKey(root), root, stock: null, terminal: terminal.address, isNative: true, nativeChildren: [], nativeParent: terminal.parent ?? null, detail: null, pinned: false, focused: false, seq: 0, changed: 0, depth: node.depth, program: terminal.program ?? null, name: terminal.name || terminal.program?.name || terminal.address, status, aggregate: aggregate(root, status), ...membership };
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
  // Feed roots without exact app agent matches are returned with space null, without
  // assigning legacy titles, cwd or sessions to an app space or inventing a focus
  // target. The V10 sidebar lists the active space only, so it does not show them.
  for (const root of feed?.orchestrators ?? []) if (!used.has(root)) {
    roots.push({ key: rootKey(root), root, stock: null, terminal: null, name: root.tab_label || root.pane_id, detail: null,
      space: null, tab: null, nativeChildren: [], isNative: false, status: root.herdr_status, program: null, pinned: false, focused: false, seq: 0,
      changed: 0, aggregate: aggregate(root) });
  }
  const seen = new Map();
  for (const root of roots) if (root.stock && root.detail === null) seen.set(root.name, (seen.get(root.name) ?? 0) + 1);
  for (const root of roots) if (root.stock && root.detail === null && seen.get(root.name) > 1) root.detail = title(tabs.find(tab => tab.id === root.stock.tab), null, root.stock.title);
  return roots.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.aggregate.priority - a.aggregate.priority || b.aggregate.urgency - a.aggregate.urgency || b.changed - a.changed || b.seq - a.seq);
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
export const rootSignature = root => JSON.stringify([root.status, root.root.inbox, root.root.threads.map(thread => [thread.id, thread.status, thread.agents.map(agent => [agent.id, agent.status, agent.question_ids])]), (root.nativeChildren ?? []).map(child => [child.terminal, child.status, child.root.inbox, child.root.threads.map(thread => [thread.id, thread.status, thread.agents.map(agent => [agent.id, agent.status, agent.question_ids])])])]);
export function harnessOf(agent, root, thread) {
  let current = agent, seen = new Set();
  while (current) {
    if (current.harness && current.harness !== 'unknown') return current.harness;
    if (seen.has(current.id)) break;
    seen.add(current.id); current = thread.agents.find(candidate => candidate.id === current.parent);
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

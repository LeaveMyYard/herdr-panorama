import { rootKey } from './contract.js';
import { joinRoots } from './model.js';
import { rootTarget } from './routing.js';
const generation = value => typeof value === 'string' && /^[0-9a-f]{32}$/.test(value);
export function subject(packet, client, intent) {
  const id = intent?.peekInstallId;
  if (!generation(id) || packet?.stale || client?.stopped || client?.install_id !== id || packet?.install_id !== id || !packet.feed) return null;
  let key;
  try { key = rootKey(intent.root); } catch { return null; }
  const root = packet.feed.roots.find(candidate => rootKey(candidate) === key);
  if (!root || root.session_id !== intent.root.session_id) return null;
  // Scope is {root, agent}: the agent resolves in its root's one tree, never from props.
  const agent = intent.agent ? root.agents.find(candidate => candidate.id === intent.agent.id) : null;
  if (intent.agent && !agent) return null;
  return { root, agent, generation: id, stale: Boolean(packet.stale), key: JSON.stringify([id, key, root.session_id, agent?.id ?? null]) };
}
export function filesystemRef(value, platform) {
  if (typeof value !== 'string' || !value || value.includes('\0')) return null;
  const absolute = platform === 'windows' ? /^(?:[A-Za-z]:[\\/]|\\\\)/.test(value) : value.startsWith('/');
  return absolute ? value : null;
}
export function transcriptScope(value, platform) {
  if (!value || value.stale) return null;
  const path = filesystemRef(value.agent?.transcript || value.root.transcript, platform);
  return path ? { path, key: JSON.stringify([value.key, path]), subject: value.key } : null;
}
export function focusTarget(value, intent, world) {
  if (!value || value.stale || typeof intent.terminal !== 'string') return null;
  const roots = joinRoots(world.feed, world.agents, world.terminals, world.panes, world.spaces);
  const candidates = roots.flatMap(root => [root, ...root.nativeChildren]);
  const item = candidates.find(candidate => rootKey(candidate.root) === rootKey(value.root) && candidate.terminal === intent.terminal);
  if (!item || (item.isNative ? 'peek' : 'focus') !== intent.terminalAction
    || !world.terminals.some(terminal => terminal.address === item.terminal && terminal.state === 'live')) return null;
  return rootTarget(item);
}

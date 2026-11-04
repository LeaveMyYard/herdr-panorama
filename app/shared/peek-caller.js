import { agents, terminals, panes, spaces, openOverlay, closeOverlay, settings } from 'herdr';
import { rootKey, probeMatches } from './contract.js';
import { joinRoots } from './model.js';
const generation = value => typeof value === 'string' && /^[0-9a-f]{32}$/.test(value);
const token = () => Array.from({ length: 4 }, () => Math.floor(Math.random() * 4294967296).toString(16).padStart(8, '0')).join('');
// Every caller keeps only its own native IDs. Commands carry an opaque token,
// never assumed arguments, transcript paths, or shared persisted state.
export class PeekCaller {
  constructor(name, client, packet) {
    if (!['panorama', 'panorama-graph'].includes(name)) throw Error('Peek caller');
    this.name = name; this.client = client; this.packet = packet; this.current = null;
    this.owned = new Set(); this.closed = false; this.error = null;
  }
  get rowKey() { return this.current?.rowKey ?? null; }
  resolve(scope, id) {
    const packet = this.packet();
    if (this.closed || this.client.stopped || packet.stale || !generation(id)
        || id !== this.client.install_id || id !== packet.install_id) return null;
    const root = packet.feed?.roots.find(root => rootKey(root) === rootKey(scope.root));
    if (!root || root.session_id !== scope.root.session_id) return null;
    const agent = scope.agent ? root.agents.find(agent => agent.id === scope.agent.id) : null;
    if (scope.agent && !agent) return null;
    const items = joinRoots(packet.feed, agents(), terminals(), panes(), spaces());
    const item = items.flatMap(item => [item, ...item.nativeChildren]).find(item => rootKey(item.root) === rootKey(root));
    return { root, agent, terminal: item?.terminal ?? null, terminalAction: item?.isNative ? 'peek' : 'focus' };
  }
  retire(keep = null) {
    for (const id of this.owned) if (id !== keep) { this.owned.delete(id); closeOverlay(id); }
  }
  sync() {
    if (this.current && !this.resolve(this.current.scope, this.current.install_id)) {
      this.current = null; this.retire();
    }
  }
  show(scope, rowKey, transcript, cx) {
    const id = this.packet().install_id;
    if (!cx || !this.resolve(scope, id)) return;
    const next = { scope, rowKey, install_id: id, token: token(), id: null };
    // Fence old close acknowledgements before any await or native open.
    this.current = next; this.error = null;
    cx.spawn(async cx => {
      try {
        const answer = await this.client.callPrivate(['app-feed', '--help']);
        if (this.current !== next || this.closed) return;
        if (answer.code !== 0 || !probeMatches(answer.stdout)) throw Error(answer.stderr || answer.stdout);
        const subject = this.resolve(scope, id);
        if (!subject) { this.current = null; this.retire(); return; }
        const viewport = globalThis.window.viewport_size(), zoom = settings().zoom ?? 1;
        const opened = openOverlay({ mod: 'panorama-peek', frame: 'none', kind: transcript ? 'dialog' : 'popup',
          width: transcript ? Math.max(1, Math.min(900, viewport.width / zoom - 24)) : 284,
          ...(transcript ? { height: Math.max(1, Math.min(650, viewport.height / zoom - 24)) } : {}),
          props: { ...subject, action: transcript ? 'transcript' : undefined,
            peekCaller: this.name, peekToken: next.token, peekInstallId: id } });
        this.owned.add(opened); next.id = opened;
        if (this.current !== next || this.closed) { this.owned.delete(opened); closeOverlay(opened); return; }
        this.retire(opened);
      } catch (error) {
        if (this.current === next) { this.error = String(error); this.current = null; this.retire(); }
      } finally { if (!this.closed) cx.notify(); }
    });
  }
  receive(command, cx) {
    if (typeof command !== 'string') return false;
    const match = /^peek-(closed|transcript)-([0-9a-f]{32})$/.exec(command);
    if (!match) return false;
    const current = this.current;
    if (current?.token !== match[2]) return true;
    if (match[1] === 'closed') { if (current.id) this.owned.delete(current.id); this.current = null; }
    else if (this.resolve(current.scope, current.install_id)) this.show(current.scope, current.rowKey, true, cx);
    cx.notify(); return true;
  }
  close() { this.closed = true; this.current = null; this.retire(); }
}

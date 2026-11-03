// Generated from app/shared/feed-client.js by scripts/app-build.
import { appHome, exec, readTextFile, watchFolder, shared, setShared, layout } from 'herdr';
import { readResult } from './contract.js';
import { installReceipt, verifiedBinary } from './binary.js';
const OWNER = 'panorama.feed.owner', PACKET = 'panorama.feed';
const HEARTBEAT = 5000, TTL = 15000;
let serial = 0;
export function validLease(value, now = Date.now()) {
  return Boolean(value && value.v === 1 && typeof value.token === 'string' && value.token.length > 0
    && ['sidebar', 'consumer'].includes(value.role) && Number.isFinite(value.seen_at)
    && value.seen_at <= now && now - value.seen_at < TTL);
}
function references(node) {
  return Boolean(node && (node.content?.mod === 'panorama' || (node.children ?? []).some(references)));
}
// Caller cannot supply an executable, alternate home/generation, or arbitrary
// command. The closure adds both installation authority arguments itself.
export function privateArgs(args, home, generation) {
  if (!Array.isArray(args) || !args.every(value => typeof value === 'string')) throw Error('Private command');
  const [command, operation, ...rest] = args;
  if (!['app-feed', 'app-inbox'].includes(command)) throw Error('Private command');
  if (operation === '--help' && rest.length === 0) return args.slice();
  let tail = rest;
  if (tail.length >= 2 && tail[tail.length - 2] === '--app-home' && tail[tail.length - 1] === home) tail = tail.slice(0, -2);
  const question = tail.length === 2 && tail[0] === '--question' && /^[1-9][0-9]*$/.test(tail[1])
    && Number.isSafeInteger(Number(tail[1]));
  if (!(command === 'app-feed' && operation === '--ensure' && tail.length === 0)
      && !(command === 'app-inbox' && operation === '--available' && tail.length === 0)
      && !(command === 'app-inbox' && operation === '--open' && (tail.length === 0 || question))) throw Error('Private command');
  return [command, operation, ...tail, '--app-home', home, '--install-id', generation];
}
function start(view, cx, role) {
  if (view.feedClient) return view.feedClient;
  const home = appHome(), directory = `${home}/data/panorama`;
  const token = `${Date.now()}:${++serial}:${Math.random()}`;
  let generation = null, stopped = false, closed = false, reading = null, queued = false, started = false, epoch = 0;
  let resolveIdentity;
  // The deferred promise resolves even on failure; early calls reject in their
  // caller-owned spawn, without an unhandled initialization rejection.
  const identity = new Promise(resolve => { resolveIdentity = resolve; });
  const eligible = () => role === 'sidebar' || !references(layout());
  const owns = () => !closed && !stopped && eligible() && validLease(shared(OWNER)) && shared(OWNER).token === token;
  const active = revision => revision === epoch && owns();
  const packet = () => ({ ...view.feed, install_id: generation, writerError: view.writerError ?? null,
    stale: Boolean(view.feed?.stale || !started || stopped),
    errors: [...(view.feed?.errors ?? []), ...(view.writerError ? [view.writerError] : [])] });
  const publish = context => { if (owns()) setShared(PACKET, packet()); context.notify(); };
  function clearOwned() {
    // A losing owner, new generation, or incomplete initialization grants no
    // authority to clear another client's persisted packet.
    if (generation !== null && shared(OWNER)?.token === token && shared(PACKET)?.install_id === generation) {
      setShared(PACKET, null); setShared(OWNER, null);
    }
  }
  function stop(error, context) {
    stopped = true; epoch++;
    view.writerError = String(error?.message ?? error);
    view.feed = { ...view.feed, stale: true };
    if (error?.receiptStop) clearOwned();
    else if (shared(OWNER)?.token === token && generation !== null) setShared(PACKET, packet());
    context?.notify();
  }
  const receipt = async () => {
    const text = await readTextFile(`${directory}/install.json`);
    if (text === null) throw Object.assign(Error('Install receipt missing'), { receiptStop: true });
    const parsed = installReceipt(text);
    if (generation !== null && parsed.install_id !== generation) throw Object.assign(Error('Installation changed'), { receiptStop: true });
    return { text, parsed };
  };
  async function invoke(args, guard) {
    await identity;
    if (!guard() || closed || stopped || generation === null) throw Error(view.writerError ?? 'Client stopped');
    const argv = privateArgs(args, home, generation);
    try {
      const current = await receipt();
      if (!guard() || closed || stopped) throw Error('Client stopped');
      const binary = await verifiedBinary(home, current.text, generation);
      if (!guard() || closed || stopped) throw Error('Client stopped');
      // Backend --install-id fences the remaining receipt/replacement race.
      const result = await exec(binary, argv);
      if (!guard()) throw Error('Client no longer owns operation');
      return result;
    } catch (error) { if (guard()) stop(error); throw error; }
  }
  const callPrivate = args => invoke(args, () => !closed && !stopped);
  function reload(context) {
    if (!owns()) return;
    if (reading === epoch) { queued = true; return; }
    const revision = epoch; reading = revision;
    context.spawn(async context => {
      try {
        await receipt(); if (!active(revision)) return;
        const text = await readTextFile(`${directory}/feed.json`); if (!active(revision)) return;
        await receipt(); if (!active(revision)) return;
        view.feed = readResult(view.feed, text);
      } catch (error) {
        if (!active(revision)) return;
        // Missing/changed/invalid receipt is terminal, unlike a transient feed read.
        try { await receipt(); if (!active(revision)) return; }
        catch (receiptError) { if (active(revision)) stop(receiptError, context); return; }
        view.feed = readResult(view.feed, null, error);
      } finally {
        if (reading === revision) reading = null;
        if (active(revision)) { publish(context); if (queued) { queued = false; reload(context); } }
      }
    });
  }
  function claim(context) {
    if (closed || stopped || !eligible()) return;
    epoch++; started = false; queued = false;
    setShared(OWNER, { v: 1, token, role, seen_at: Date.now() });
    setShared(PACKET, { ...(shared(PACKET) ?? view.feed), stale: true });
    const revision = epoch;
    context.spawn(async context => {
      await identity; if (!active(revision)) return;
      try {
        const capability = await invoke(['app-feed', '--help'], () => active(revision)); if (!active(revision)) return;
        if (capability.code !== 0 || !capability.stdout.includes('panorama app-feed v1')) throw Error(capability.stderr || capability.stdout);
        const result = await invoke(['app-feed', '--ensure'], () => active(revision)); if (!active(revision)) return;
        if (result.code !== 0) throw Error(result.stderr || result.stdout);
        started = true; view.writerError = null; reload(context);
      } catch (error) { if (active(revision)) { view.writerError = String(error.message ?? error); publish(context); } }
    });
    reload(context);
    context.spawn(async context => {
      while (active(revision)) {
        try { await watchFolder(directory); if (!active(revision)) return; reload(context); }
        catch { if (!active(revision)) return; await context.sleep(HEARTBEAT); if (!active(revision)) return; reload(context); }
      }
    });
  }
  const timer = cx.timer.every(HEARTBEAT, context => {
    if (closed || stopped) return;
    if (owns()) { setShared(OWNER, { v: 1, token, role, seen_at: Date.now() }); reload(context); }
    else if (eligible() && !validLease(shared(OWNER))) claim(context);
    else context.notify();
  });
  const client = { callPrivate, get install_id() { return generation; }, get stopped() { return stopped || closed; },
    close(context) { clearOwned(); closed = true; epoch++; timer.cancel(); context?.notify(); } };
  view.feedClient = client;
  view.reloadFeed = reload;
  cx.spawn(async context => {
    try {
      const current = await receipt(); if (closed) return;
      await verifiedBinary(home, current.text); if (closed) return;
      generation = current.parsed.install_id;
    } catch (error) { if (!closed) stop(error, context); }
    finally { resolveIdentity(generation); }
  });
  // Initial sidebar priority, or ONE initial no-sidebar consumer takeover even
  // over a persisted lease. Subsequent heartbeats only reclaim expired leases.
  if (eligible()) claim(cx);
  return client;
}
export const startSidebar = (view, cx) => start(view, cx, 'sidebar');
export const startConsumer = (view, cx) => start(view, cx, 'consumer');
export function consumerFeed(view) {
  const published = shared(PACKET), client = view.feedClient;
  // Each live view retains only its own matching-generation cache. A handoff
  // or owner stop cannot erase this local stale provenance.
  if (published && (!client?.install_id || published.install_id === client.install_id)) view.feed = published;
  const current = view.feed ?? { feed: null, generated_at: null, errors: [], stale: true };
  return { ...current, writerError: view.writerError ?? current.writerError ?? null,
    errors: [...new Set([...(current.errors ?? []), ...(view.writerError ? [view.writerError] : [])])], stale: Boolean(current.stale
      || !published || !client || client.stopped || !client.install_id || current.install_id !== client.install_id
      || published.install_id !== client.install_id || !validLease(shared(OWNER))) };
}
export const closeConsumer = (view, cx) => view.feedClient?.close(cx);

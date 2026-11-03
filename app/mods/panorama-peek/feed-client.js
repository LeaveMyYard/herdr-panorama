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
function aged(packet, now = Date.now()) {
  const collected = packet?.generated_at;
  return !Number.isFinite(collected) || collected * 1000 > now || now - collected * 1000 > TTL;
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
  let generation = null, stopped = false, closed = false, reading = null, queued = false, started = false, epoch = 0, claiming = false;
  let recovering = false, lastRecovery = -Infinity;
  let resolveIdentity;
  const identity = new Promise(resolve => { resolveIdentity = resolve; });
  const alive = () => !closed && !stopped;
  const eligible = () => role === 'sidebar' || !references(layout());
  const owns = () => alive() && eligible() && validLease(shared(OWNER)) && shared(OWNER).token === token;
  const active = revision => revision === epoch && owns();
  const packet = () => ({ feed: view.feed?.feed ?? null, generated_at: view.feed?.generated_at ?? null,
    install_id: generation, writerError: view.writerError ?? null, stale: Boolean(view.feed?.stale || aged(view.feed) || !started || stopped),
    errors: [...(view.feed?.errors ?? []), ...(view.writerError ? [view.writerError] : [])] });
  const cancelled = () => Object.assign(Error('Client no longer owns operation'), { cancelled: true });
  function clearOwned() {
    if (shared(OWNER)?.token !== token) return;
    if (generation !== null && shared(PACKET)?.install_id === generation) setShared(PACKET, null);
    // Own-token release is independent of packet authority, including early close.
    setShared(OWNER, null);
  }
  function stop(error, context) {
    if (error?.cancelled || closed) return;
    // Recovery of an old crashed generation is synchronous and restricted to
    // its matching cached packet; never claim a foreign/missing packet.
    if (error?.disabled && generation !== null && shared(PACKET)?.install_id === generation) {
      setShared(OWNER, { v: 1, token, role, seen_at: Date.now() });
    }
    stopped = true; view.feedValidated = false; epoch++;
    view.writerError = String(error?.message ?? error);
    view.feed = { ...view.feed, stale: true };
    if (error?.receiptStop || error?.disabled) clearOwned();
    else if (shared(OWNER)?.token === token && generation !== null) setShared(PACKET, packet());
    context?.notify();
  }
  async function authority(guard, pin = false) {
    const text = await readTextFile(`${directory}/install.json`);
    if (!guard()) throw cancelled();
    if (text === null) throw Object.assign(Error('Install receipt missing'), { receiptStop: true });
    const parsed = installReceipt(text);
    if (generation !== null && parsed.install_id !== generation) throw Object.assign(Error('Installation changed'), { receiptStop: true });
    if (pin) generation = parsed.install_id;
    const controlText = await readTextFile(`${directory}/runtime/${parsed.install_id}/control.json`);
    if (!guard()) throw cancelled();
    if (controlText === null || controlText.length > 4096) throw Error('Runtime control missing or oversized');
    const control = JSON.parse(controlText);
    if (!control || Object.keys(control).length !== 3 || control.v !== 1 || control.install_id !== parsed.install_id
        || typeof control.disabled !== 'boolean') throw Error('Runtime control invalid');
    if (control.disabled) throw Object.assign(Error('Installation disabled'), { disabled: true });
    return { text, parsed };
  }
  const publish = context => { if (owns()) setShared(PACKET, packet()); context.notify(); };
  async function invoke(args, guard) {
    await identity; if (!guard() || generation === null) throw cancelled();
    const argv = privateArgs(args, home, generation);
    try {
      const current = await authority(guard); if (!guard()) throw cancelled();
      const binary = await verifiedBinary(home, current.text, generation); if (!guard()) throw cancelled();
      await authority(guard); if (!guard()) throw cancelled();
      const result = await exec(binary, argv); if (!guard()) throw cancelled();
      await authority(guard); if (!guard()) throw cancelled();
      return result;
    } catch (error) { if (guard()) stop(error); throw error; }
  }
  const callPrivate = args => invoke(args, alive);
  function recover(context) {
    const now = Date.now();
    if (!started || !owns() || recovering || !aged(view.feed, now) || now - lastRecovery < HEARTBEAT) return;
    const revision = epoch; recovering = true; lastRecovery = now;
    context.spawn(async context => {
      try {
        const result = await invoke(['app-feed', '--ensure'], () => active(revision)); if (!active(revision)) return;
        if (result.code !== 0) throw Error(result.stderr || result.stdout);
        // Ensure acknowledges startup only. Retained bytes stay aged until a
        // newly collected feed supplies a current collection timestamp.
        view.writerError = null; reload(context);
      } catch (error) {
        if (active(revision) && !error?.cancelled) { view.writerError = String(error.message ?? error); publish(context); }
      } finally { recovering = false; }
    });
  }
  function reload(context) {
    if (!owns()) return;
    if (reading === epoch) { queued = true; return; }
    const revision = epoch; reading = revision;
    context.spawn(async context => {
      try {
        await authority(() => active(revision)); if (!active(revision)) return;
        const text = await readTextFile(`${directory}/feed.json`); if (!active(revision)) return;
        await authority(() => active(revision)); if (!active(revision)) return;
        view.feed = readResult(view.feed, text);
      } catch (error) {
        if (!active(revision)) return;
        if (error?.receiptStop || error?.disabled) { stop(error, context); return; }
        try { await authority(() => active(revision)); if (!active(revision)) return; }
        catch (authorityError) { if (active(revision)) stop(authorityError, context); return; }
        view.feed = readResult(view.feed, null, error);
      } finally {
        if (reading === revision) reading = null;
        if (active(revision)) { publish(context); if (queued) { queued = false; reload(context); } }
      }
    });
  }
  function claim(context, initial = false) {
    if (!alive() || !eligible() || claiming) return;
    claiming = true;
    context.spawn(async context => {
      try {
        await identity; if (!alive() || !eligible()) return;
        await authority(() => alive() && eligible()); if (!alive() || !eligible()) return;
        if (!initial && validLease(shared(OWNER))) return;
        epoch++; started = false; queued = false;
        setShared(OWNER, { v: 1, token, role, seen_at: Date.now() });
        setShared(PACKET, packet());
        const revision = epoch;
        reload(context);
        context.spawn(async context => {
          while (active(revision)) {
            try { await watchFolder(directory); if (!active(revision)) return; reload(context); }
            catch { if (!active(revision)) return; await context.sleep(HEARTBEAT); if (!active(revision)) return; reload(context); }
          }
        });
        const capability = await invoke(['app-feed', '--help'], () => active(revision)); if (!active(revision)) return;
        if (capability.code !== 0 || !capability.stdout.includes('panorama app-feed v1')) throw Error(capability.stderr || capability.stdout);
        const result = await invoke(['app-feed', '--ensure'], () => active(revision)); if (!active(revision)) return;
        if (result.code !== 0) throw Error(result.stderr || result.stdout);
        started = true; view.writerError = null; reload(context);
      } catch (error) {
        if (alive()) {
          if (error?.receiptStop || error?.disabled) stop(error, context);
          else if (!error?.cancelled && owns()) { view.writerError = String(error.message ?? error); publish(context); }
        }
      } finally { claiming = false; }
    });
  }
  const timer = cx.timer.every(HEARTBEAT, context => {
    if (!alive()) return;
    context.spawn(async context => {
      try {
        await identity; if (!alive()) return;
        await authority(alive); if (!alive()) return;
        if (owns()) {
          setShared(OWNER, { v: 1, token, role, seen_at: Date.now() });
          publish(context); reload(context); recover(context);
        }
        else if (eligible() && !validLease(shared(OWNER))) claim(context);
        else context.notify();
      } catch (error) { if (alive()) stop(error, context); }
    });
  });
  const client = { callPrivate, get install_id() { return generation; }, get stopped() { return !alive(); },
    close(context) { clearOwned(); view.feedValidated = false; closed = true; epoch++; timer.cancel(); context?.notify(); } };
  view.feedClient = client; view.reloadFeed = reload; view.feedValidated = false;
  cx.spawn(async context => {
    try {
      const current = await authority(alive, true); if (!alive()) return;
      if (eligible()) {
        const text = await readTextFile(`${directory}/feed.json`); if (!alive()) return;
        await authority(alive); if (!alive()) return;
        view.feed = { ...readResult(view.feed, text), install_id: generation, stale: true };
      }
      await verifiedBinary(home, current.text, generation); if (!alive()) return;
      await authority(alive); if (!alive()) return;
      view.feedValidated = true; context.notify();
    } catch (error) { if (alive()) stop(error, context); }
    finally { resolveIdentity(generation); }
  });
  // Claim is deferred until receipt/control/artifact validation. Initial sidebar
  // priority or initial no-sidebar takeover happens ONCE, never a stealing loop.
  if (eligible()) claim(cx, true);
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
    errors: [...new Set([...(current.errors ?? []), ...(view.writerError ? [view.writerError] : [])])], stale: Boolean(current.stale || aged(current)
      || !published || !client || !view.feedValidated || client.stopped || !client.install_id || current.install_id !== client.install_id
      || published.install_id !== client.install_id || !validLease(shared(OWNER))) };
}
export const closeConsumer = (view, cx) => view.feedClient?.close(cx);

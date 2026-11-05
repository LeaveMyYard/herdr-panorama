import test from 'node:test';
import assert from 'node:assert/strict';
import { register } from 'node:module';
import { readFileSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { sandbox, root } from './helpers.mjs';
import { state } from './herdr-fixture.mjs';
register('./herdr-loader.mjs', import.meta.url);
const { startSidebar: startFeed, startConsumer, consumerFeed, closeConsumer, validLease } = await import('../shared/feed-client.js');
const { PeekCaller } = await import('../shared/peek-caller.js');
function ready(t) {
  const s = sandbox(t), artifact = join(s.temp, 'reviewed');
  writeFileSync(artifact, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  const result = s.run('install', ['--binary', artifact]); assert.equal(result.status, 0, result.stderr);
  const feed = JSON.parse(readFileSync(join(root, 'tests/fixtures/app-feed/swarm.json')));
  feed.generated_at = Date.now() / 1000;
  writeFileSync(join(s.home, 'data/panorama/feed.json'), JSON.stringify(feed));
  Object.assign(state, { home: s.home, execs: [], shared: [], watches: [], read: null, exec: null, values: {}, layout: { content: { mod: 'panorama' } } });
  const timers = [], failures = [];
  const cx = { notify() {}, spawn(callback) { Promise.resolve(callback(cx)).catch(error => failures.push(error)); }, sleep() { return new Promise(() => {}); }, timer: { every(ms, callback) { const timer = { ms, callback, cancelled: false, cancel() { this.cancelled = true; } }; timers.push(timer); return timer; } } };
  return { ...s, cx, timers, failures, view: {} };
}
async function until(condition) {
  const deadline = Date.now() + 2000;
  while (!condition() && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 1));
  assert.ok(condition(), 'asynchronous reader condition was not reached');
}

test('native adapter prehash gates exact private capability/ensure argv with explicit app home', async t => {
  const s = ready(t); startFeed(s.view, s.cx);
  await until(() => state.execs.length === 2);
  assert.deepEqual(state.execs, [
    { command: join(s.home, 'data/panorama/bin/panorama'), args: ['app-feed', '--help'] },
    { command: join(s.home, 'data/panorama/bin/panorama'), args: ['app-feed', '--ensure', '--app-home', s.home, '--install-id', JSON.parse(readFileSync(s.manifest)).install_id] },
  ]);
  await until(() => s.view.feed?.feed);
  assert.equal(s.view.feed.stale, false); assert.equal(s.failures.length, 0);
});

test('modified artifact never executes, while valid feed and stock enrichment data survive', async t => {
  const s = ready(t); writeFileSync(join(s.home, 'data/panorama/bin/panorama'), '#!/bin/sh\nexit 91\n');
  startFeed(s.view, s.cx); await until(() => s.view.writerError && s.view.feed?.feed);
  assert.equal(state.execs.length, 0); assert.equal(s.view.feed.feed.roots.length, 6);
  assert.ok(consumerFeed(s.view).errors.length); assert.equal(s.failures.length, 0);
});

test('absent receipt prevents new startup and a later read retains stale provenance', async t => {
  const s = ready(t); startFeed(s.view, s.cx);
  await until(() => consumerFeed(s.view).stale === false);
  rmSync(s.manifest); s.view.reloadFeed(s.cx);
  await until(() => s.view.writerError && s.view.feed?.feed);
  assert.equal(state.execs.length, 2);
  const last = s.view.feed.feed; rmSync(join(s.home, 'data/panorama/feed.json'));
  s.view.reloadFeed(s.cx); await until(() => s.view.feed?.stale);
  assert.equal(s.view.feed.feed, last); assert.equal(state.execs.length, 2);
});

test('incompatible capability marker prevents ensure rather than falling back to PATH', async t => {
  // A v1 writer (or a v2 line that is not the first) is as foreign as any other executable.
  for (const stdout of ['another executable', 'panorama app-feed v1\n', 'note\npanorama app-feed v2\n']) {
    const s = ready(t); state.exec = () => ({ code: 0, stdout, stderr: '' });
    startFeed(s.view, s.cx); await until(() => s.view.writerError);
    assert.equal(state.execs.length, 1); assert.deepEqual(state.execs[0].args, ['app-feed', '--help']);
  }
});

test('coalesced file events queue a second read so a completed stale read cannot swallow the latest feed', async t => {
  const s = ready(t);startFeed(s.view,s.cx);await until(()=>consumerFeed(s.view).stale===false);
  let resolveFirst, reads = 0;
  const original = readFileSync(join(s.home, 'data/panorama/feed.json'), 'utf8');
  const latest = JSON.parse(original); latest.generated_at += 1;
  state.read = path => {
    if (path.endsWith('/feed.json')) { reads++; return reads === 1 ? new Promise(resolve => { resolveFirst = resolve; }) : JSON.stringify(latest); }
    return readFileSync(path, 'utf8');
  };
  s.view.reloadFeed(s.cx); await until(() => resolveFirst); s.view.reloadFeed(s.cx); resolveFirst(original);
  await until(() => s.view.feed?.generated_at === latest.generated_at);
  assert.ok(reads >= 2); assert.equal(s.failures.length, 0);
});

test('consumer follows sidebar without launching; hidden sidebar reference prevents fallback', async t => {
  const s = ready(t); const sidebar = startFeed(s.view, s.cx);
  await until(() => consumerFeed(s.view).stale === false);
  const follower = {}; const client = startConsumer(follower, s.cx);
  await until(() => follower.feedValidated);
  assert.equal(state.execs.length, 2); assert.equal(consumerFeed(follower).feed, consumerFeed(s.view).feed);
  state.layout = { hidden: true, children: [{ hidden: true, content: { mod: 'panorama' } }] };
  state.values['panorama.feed.owner'] = null;
  for (const timer of s.timers) timer.callback(s.cx);
  await new Promise(resolve => setTimeout(resolve, 15));
  assert.equal(state.values['panorama.feed.owner'].role, 'sidebar');
  closeConsumer(follower, s.cx); sidebar.close(s.cx);
});

test('no-sidebar consumer initial takeover wins once; losing async help cannot start ensure', async t => {
  const s = ready(t); state.layout = {};
  const releases = [];
  state.exec = (_binary, args) => args[1] === '--help' ? new Promise(resolve => { releases.push(resolve); }) : { code: 0, stdout: '', stderr: '' };
  const first = {}; startConsumer(first, s.cx); await until(() => releases.length === 1);
  const second = {}; startConsumer(second, s.cx);
  await until(() => state.execs.length === 2); const winner = state.values['panorama.feed.owner'].token;
  releases[0]({ code: 0, stdout: 'panorama app-feed v2', stderr: '' });
  await new Promise(resolve => setTimeout(resolve, 10));
  assert.equal(state.execs.filter(call => call.args[1] === '--ensure').length, 0);
  releases[1]({ code: 0, stdout: 'panorama app-feed v2', stderr: '' });
  await until(() => state.execs.some(call => call.args[1] === '--ensure'));
  for (const timer of s.timers) timer.callback(s.cx);
  assert.equal(state.values['panorama.feed.owner'].token, winner);
  assert.equal(state.execs.filter(call => call.args[1] === '--ensure').length, 1);
  assert.equal(first.feedClient.stopped, false);
});

test('changed installation permanently fences an early bound one-arg call and retained feed', async t => {
  const s = ready(t), client = startFeed(s.view, s.cx);
  const early = client.callPrivate(['app-inbox', '--available']);
  await early;
  await until(() => consumerFeed(s.view).stale === false);
  const last = consumerFeed(s.view).feed, before = state.execs.length;
  const receipt = JSON.parse(readFileSync(s.manifest)); receipt.install_id = 'f'.repeat(32); writeFileSync(s.manifest, JSON.stringify(receipt));
  await assert.rejects(client.callPrivate(['app-inbox', '--open', '--question', '14']), /Installation changed/);
  assert.equal(state.execs.length, before); assert.equal(client.stopped, true);
  assert.equal(consumerFeed(s.view).feed, last); assert.equal(consumerFeed(s.view).stale, true);
  receipt.install_id = client.install_id; writeFileSync(s.manifest, JSON.stringify(receipt));
  await assert.rejects(client.callPrivate(['app-inbox', '--available']));
  assert.equal(state.execs.length, before);
});

test('each call rehashes artifact and finite argv rejects invented operations, homes, ids, or caller fences', async t => {
  const s = ready(t), client = startFeed(s.view, s.cx);
  await until(() => consumerFeed(s.view).stale === false);
  for (const args of [['sh','--help'], ['app-inbox','--answer'], ['app-inbox','--open','--question','01'], ['app-inbox','--open','--question','9007199254740992'], ['app-feed','--ensure','--app-home','/foreign'], ['app-feed','--ensure','--install-id','f'.repeat(32)]]) {
    await assert.rejects(client.callPrivate(args), /Private command/);
  }
  const before = state.execs.length;
  await client.callPrivate(['app-inbox','--open','--question','14']);
  assert.deepEqual(state.execs.at(-1).args, ['app-inbox','--open','--question','14','--app-home',s.home,'--install-id',client.install_id]);
  writeFileSync(join(s.home,'data/panorama/bin/panorama'),'modified');
  await assert.rejects(client.callPrivate(['app-inbox','--help']));
  assert.equal(state.execs.length,before+1);
});

test('lease timestamp validation and close prevent persisted freshness and rearming', async t => {
  const s = ready(t); const client = startFeed(s.view,s.cx);
  await until(() => consumerFeed(s.view).stale === false);
  const lease = state.values['panorama.feed.owner'];
  for (const seen_at of [NaN,Infinity,Date.now()+100000,Date.now()-15000]) assert.equal(validLease({...lease,seen_at}),false);
  client.close(s.cx); const count = state.execs.length;
  for (const watch of state.watches) watch.resolve([]);
  for (const timer of s.timers) timer.callback(s.cx);
  await new Promise(resolve=>setTimeout(resolve,10));
  assert.equal(state.execs.length,count); assert.equal(s.timers.every(timer=>timer.cancelled),true);
  assert.equal(consumerFeed(s.view).stale,true);
});


test('final owned close nulls only matching-generation shared data; followers keep stale local cache', async t => {
  const s = ready(t), owner = startFeed(s.view,s.cx);
  await until(()=>consumerFeed(s.view).stale===false);
  const follower={};startConsumer(follower,s.cx);await until(()=>follower.feedClient.install_id);
  const last=consumerFeed(follower).feed;owner.close(s.cx);
  assert.equal(state.values['panorama.feed'],null);assert.equal(state.values['panorama.feed.owner'],null);
  assert.equal(consumerFeed(follower).feed,last);assert.equal(consumerFeed(follower).stale,true);
});

for (const replacement of ['token', 'generation']) {
  test(`old-client close independently guards replacement ${replacement}`, async t => {
    const s = ready(t), old = startFeed(s.view,s.cx);
    await until(()=>consumerFeed(s.view).stale===false);
    const follower={};startConsumer(follower,s.cx);await until(()=>follower.feedClient.install_id);
    const last=consumerFeed(follower).feed;
    const newer={...state.values['panorama.feed.owner'],...(replacement==='token'?{token:'different-owner'}:{})};
    const next={...state.values['panorama.feed'],...(replacement==='generation'?{install_id:'f'.repeat(32)}:{}),feed:{newer:true}};
    state.values['panorama.feed.owner']=newer;state.values['panorama.feed']=next;
    old.close(s.cx);
    assert.equal(state.values['panorama.feed'],next);assert.equal(state.values['panorama.feed.owner'],replacement==='token'?newer:null);
    if(replacement==='generation') {
      assert.equal(consumerFeed(follower).feed,last);assert.equal(consumerFeed(follower).stale,true);
    } else assert.equal(consumerFeed(follower).feed,next.feed);
  });
}

test('missing receipt stop nulls only its own packet and permanently stops rearming', async t => {
  const s=ready(t);startFeed(s.view,s.cx);await until(()=>consumerFeed(s.view).stale===false);
  const last=consumerFeed(s.view).feed;rmSync(s.manifest);s.view.reloadFeed(s.cx);
  await until(()=>s.view.feedClient.stopped);
  assert.equal(state.values['panorama.feed'],null);assert.equal(state.values['panorama.feed.owner'],null);
  assert.equal(consumerFeed(s.view).feed,last);assert.equal(consumerFeed(s.view).stale,true);
  const count=state.execs.length;for(const timer of s.timers)timer.callback(s.cx);
  assert.equal(state.execs.length,count);
});

function disabled(s) {
  const receipt=JSON.parse(readFileSync(s.manifest));
  const path=join(s.home,'data/panorama/runtime',receipt.install_id,'control.json');
  writeFileSync(path,JSON.stringify({v:1,install_id:receipt.install_id,disabled:true}));
  return receipt.install_id;
}
test('initial disabled generation clears only matching cached data; no lease for missing/foreign packet', async t=>{
  for(const mode of ['matching','missing','foreign']) {
    const s=ready(t), generation=disabled(s);
    const packet={feed:JSON.parse(readFileSync(join(s.home,'data/panorama/feed.json'))),stale:false,errors:[],generated_at:1,writerError:null,install_id:mode==='foreign'?'f'.repeat(32):generation};
    const owner={v:1,token:'persisted-owner',role:'sidebar',seen_at:Date.now()};
    if(mode!=='missing') Object.assign(state.values,{'panorama.feed':packet,'panorama.feed.owner':owner});
    startFeed(s.view,s.cx);await until(()=>s.view.feedClient.stopped);
    assert.equal(state.execs.length,0);
    if(mode==='matching') {assert.equal(state.values['panorama.feed'],null);assert.equal(state.values['panorama.feed.owner'],null);}
    if(mode==='missing') {assert.equal(state.values['panorama.feed'],undefined);assert.equal(state.values['panorama.feed.owner'],undefined);}
    if(mode==='foreign') {assert.equal(state.values['panorama.feed'],packet);assert.equal(state.values['panorama.feed.owner'],owner);}
  }
});
test('early close during deferred initial receipt cannot leave a lease or publish later', async t=>{
  const s=ready(t);let release;
  state.read=path=>path.endsWith('install.json')?new Promise(resolve=>{release=resolve;}):readFileSync(path,'utf8');
  const client=startFeed(s.view,s.cx);await until(()=>release);client.close(s.cx);release(readFileSync(s.manifest,'utf8'));
  await new Promise(resolve=>setTimeout(resolve,10));
  assert.equal(state.execs.length,0);assert.equal(state.values['panorama.feed.owner'],undefined);assert.equal(state.values['panorama.feed'],undefined);
});
test('delayed feed result cannot republish after disabled stop acknowledgment', async t=>{
  const s=ready(t);startFeed(s.view,s.cx);await until(()=>consumerFeed(s.view).stale===false);
  let release;const original=readFileSync(join(s.home,'data/panorama/feed.json'),'utf8');
  state.read=path=>path.endsWith('/feed.json')?new Promise(resolve=>{release=resolve;}):readFileSync(path,'utf8');
  s.view.reloadFeed(s.cx);await until(()=>release);disabled(s);
  for(const timer of s.timers)timer.callback(s.cx);await until(()=>s.view.feedClient.stopped);
  release(original);await new Promise(resolve=>setTimeout(resolve,10));
  assert.equal(state.values['panorama.feed'],null);assert.equal(state.values['panorama.feed.owner'],null);
  for(const timer of s.timers)timer.callback(s.cx);await new Promise(resolve=>setTimeout(resolve,10));
  assert.equal(state.values['panorama.feed'],null);
});
test('delayed private response cannot restart or republish after disabled generation', async t=>{
  const s=ready(t);let release;
  state.exec=(_binary,args)=>args[1]==='--ensure'?new Promise(resolve=>{release=resolve;}):{code:0,stdout:'panorama app-feed v2',stderr:''};
  startFeed(s.view,s.cx);await until(()=>release);disabled(s);
  for(const timer of s.timers)timer.callback(s.cx);await until(()=>s.view.feedClient.stopped);
  release({code:0,stdout:'',stderr:''});await new Promise(resolve=>setTimeout(resolve,10));
  assert.equal(state.values['panorama.feed'],null);assert.equal(state.values['panorama.feed.owner'],null);
  assert.equal(state.execs.filter(call=>call.args[1]==='--ensure').length,1);
});

test('fresh initialization rechecks a delayed formerly-enabled control before any lease publication', async t=>{
  const s=ready(t);let release,first=true;
  state.read=path=>{if(path.endsWith('/control.json')&&first){first=false;return new Promise(resolve=>{release=resolve;});}return readFileSync(path,'utf8');};
  startFeed(s.view,s.cx);await until(()=>release);const generation=disabled(s);
  release(JSON.stringify({v:1,install_id:generation,disabled:false}));await until(()=>s.view.feedClient.stopped);
  assert.equal(state.execs.length,0);assert.equal(state.values['panorama.feed.owner'],undefined);assert.equal(state.values['panorama.feed'],undefined);
});

test('a pinned receipt and valid persisted lease cannot present fresh data before full initialization proof', async t=>{
  const s=ready(t);let release,first=true;const receipt=JSON.parse(readFileSync(s.manifest));
  const feed=JSON.parse(readFileSync(join(s.home,'data/panorama/feed.json')));
  state.values['panorama.feed']={feed,stale:false,errors:[],generated_at:feed.generated_at,writerError:null,install_id:receipt.install_id};
  state.values['panorama.feed.owner']={v:1,token:'other-live-owner',role:'sidebar',seen_at:Date.now()};
  state.read=path=>{if(path.endsWith('/control.json')&&first){first=false;return new Promise(resolve=>{release=resolve;});}return readFileSync(path,'utf8');};
  startConsumer(s.view,s.cx);await until(()=>release);assert.equal(s.view.feedClient.install_id,receipt.install_id);assert.equal(consumerFeed(s.view).stale,true);
  release(JSON.stringify({v:1,install_id:receipt.install_id,disabled:false}));await until(()=>s.view.feedValidated);
  assert.equal(consumerFeed(s.view).stale,false);assert.equal(state.execs.length,0);
});

test('Peek opens only after the bound writer answers the exact v2 probe', async t => {
  const G = '0123456789abcdef0123456789abcdef', feed = JSON.parse(readFileSync(join(root, 'tests/fixtures/app-feed/swarm.json')));
  const previous = globalThis.window; t.after(() => { globalThis.window = previous; });
  globalThis.window = { viewport_size: () => ({ width: 1200, height: 900 }) };
  for (const [stdout, opens] of [['panorama app-feed v1\n', 0], ['panorama app-feed v2\n', 1]]) {
    state.overlays = []; let done;
    const finished = new Promise(resolve => { done = resolve; });
    const client = { stopped: false, install_id: G, callPrivate: async args => { assert.deepEqual(args, ['app-feed', '--help']); return { code: 0, stdout, stderr: '' }; } };
    const caller = new PeekCaller('panorama', client, () => ({ feed, stale: false, install_id: G }));
    const cx = { spawn: callback => Promise.resolve(callback(cx)), notify: done };
    caller.show({ root: feed.roots[0], agent: feed.roots[0].agents[0] }, 'row', false, cx); await finished;
    assert.equal(state.overlays.length, opens);
    assert.equal(Boolean(caller.error), !opens);
    if (opens) assert.equal(state.overlays[0].props.agent, feed.roots[0].agents[0]);
  }
});

// Startup ensure retry (app-feed-runtime-v1.md "Starting, stopping and timing"): the fake
// host answers each ensure from `codes`, and `beat` is one owner heartbeat at a clock moved
// past the 5 s retry throttle.
const ensures = () => state.execs.filter(call => call.args[1] === '--ensure').length;
const settle = () => new Promise(resolve => setTimeout(resolve, 20));
const staleFeed = s => {
  const path = join(s.home, 'data/panorama/feed.json'), feed = JSON.parse(readFileSync(path));
  writeFileSync(path, JSON.stringify({ ...feed, generated_at: Date.now() / 1000 - 3600 }));
};
function answering(codes) {
  state.exec = (_binary, args) => args[1] === '--ensure'
    ? { code: codes.shift() ?? 0, stdout: '', stderr: 'writer start deferred 5000 ms' } : { code: 0, stdout: 'panorama app-feed v2\n', stderr: '' };
}
function skewing(t) {
  const real = Date.now; let skew = 0; Date.now = () => real() + skew; t.after(() => { Date.now = real; });
  return async s => {
    skew += 6000;
    if (state.values['panorama.feed.owner']) state.values['panorama.feed.owner'] = { ...state.values['panorama.feed.owner'], seen_at: Date.now() };
    for (const timer of s.timers) timer.callback(s.cx);
    await settle();
  };
}
test('a failed startup ensure is retried on an aged feed until one succeeds, and its error then clears', async t => {
  const s = ready(t), beat = skewing(t); staleFeed(s); answering([1, 1, 0]);
  startFeed(s.view, s.cx); await until(() => s.view.writerError);
  assert.equal(ensures(), 1); assert.match(s.view.writerError, /deferred/);
  await beat(s); assert.equal(ensures(), 2); assert.match(s.view.writerError, /deferred/, 'a failed retry keeps reporting');
  await beat(s); assert.equal(ensures(), 3); assert.equal(s.view.writerError, null);
  assert.equal(state.values['panorama.feed'].writerError, null);
  assert.equal(s.view.feedClient.stopped, false); assert.equal(s.failures.length, 0);
});
test('a failed startup ensure is retried on a fresh feed, or a late-starting writer would stay stale', async t => {
  const s = ready(t); answering([1, 0]);
  startFeed(s.view, s.cx); await until(() => s.view.writerError);
  assert.equal(consumerFeed(s.view).stale, true, 'an unproven start is never fresh');
  for (const timer of s.timers) timer.callback(s.cx);
  await until(() => s.view.writerError === null && consumerFeed(s.view).stale === false);
  assert.equal(ensures(), 2);
  for (const timer of s.timers) timer.callback(s.cx);
  await settle(); assert.equal(ensures(), 2, 'a proven start with a fresh feed needs no ensure');
});
test('an ensure retry keeps the 5 s throttle and its one pending attempt', async t => {
  const s = ready(t), beat = skewing(t); staleFeed(s); answering([1, 1]);
  startFeed(s.view, s.cx); await until(() => s.view.writerError);
  for (const timer of s.timers) timer.callback(s.cx);
  await settle(); assert.equal(ensures(), 2);
  for (const timer of s.timers) timer.callback(s.cx);
  await settle(); assert.equal(ensures(), 2, 'inside the 5 s throttle');
  let release; const held = new Promise(resolve => { release = resolve; });
  state.exec = (_binary, args) => args[1] === '--ensure' ? held : { code: 0, stdout: '', stderr: '' };
  await beat(s); assert.equal(ensures(), 3);
  await beat(s); assert.equal(ensures(), 3, 'one pending attempt');
  release({ code: 0, stdout: '', stderr: '' }); await until(() => s.view.writerError === null);
});
test('a failed retry after a proven start keeps retrying on a fresh feed until its error clears', async t => {
  const s = ready(t), beat = skewing(t); answering([0, 1, 0]);
  startFeed(s.view, s.cx); await until(() => consumerFeed(s.view).stale === false);
  await beat(s); await beat(s); await beat(s); // 18 s: aged, and the retry fails
  assert.equal(ensures(), 2); assert.match(s.view.writerError, /deferred/);
  const path = join(s.home, 'data/panorama/feed.json');
  writeFileSync(path, JSON.stringify({ ...JSON.parse(readFileSync(path)), generated_at: Date.now() / 1000 }));
  s.view.reloadFeed(s.cx); await until(() => consumerFeed(s.view).aged === false);
  await beat(s);
  assert.equal(ensures(), 3); assert.equal(s.view.writerError, null);
  assert.deepEqual(consumerFeed(s.view).errors, []); assert.equal(consumerFeed(s.view).stale, false);
});
test('an old retry answering after a new claim cannot prove that claim', async t => {
  const s = ready(t), beat = skewing(t); answering([1]);
  startFeed(s.view, s.cx); await until(() => s.view.writerError);
  let release, calls = 0; const held = new Promise(resolve => { release = resolve; });
  state.exec = (_binary, args) => args[1] !== '--ensure' ? { code: 0, stdout: 'panorama app-feed v2\n', stderr: '' }
    : ++calls === 1 ? held : { code: 1, stdout: '', stderr: 'the new claim failed' };
  await beat(s); assert.equal(ensures(), 2);
  state.values['panorama.feed.owner'] = null; // the lease lapsed while the retry is held
  await beat(s); await until(() => /new claim/.test(s.view.writerError ?? ''));
  release({ code: 0, stdout: '', stderr: '' }); await settle();
  assert.match(s.view.writerError, /new claim/); assert.equal(state.values['panorama.feed'].stale, true);
});
test('a stopped client still renders each heartbeat, so a collection can age out of the joins', async t => {
  const s = ready(t); answering([1]); startFeed(s.view, s.cx); await until(() => s.view.writerError);
  disabled(s); for (const timer of s.timers) timer.callback(s.cx);
  await until(() => s.view.feedClient.stopped);
  let renders = 0; s.cx.notify = () => { renders++; };
  for (const timer of s.timers) timer.callback(s.cx);
  await settle(); assert.equal(renders, 1);
  s.view.feedClient.close(s.cx); renders = 0;
  for (const timer of s.timers) timer.callback(s.cx);
  await settle(); assert.equal(renders, 0, 'a closed client is finished');
});
test('a disabled generation ends the ensure retries for good', async t => {
  const s = ready(t), beat = skewing(t); staleFeed(s); answering([1]);
  startFeed(s.view, s.cx); await until(() => s.view.writerError);
  disabled(s); await beat(s); await beat(s);
  assert.equal(s.view.feedClient.stopped, true); assert.equal(ensures(), 1);
  assert.equal(state.values['panorama.feed.owner'], null);
});
test('an incompatible capability probe stays final: the failed start was no ensure, so none is retried', async t => {
  const s = ready(t), beat = skewing(t); staleFeed(s);
  state.exec = () => ({ code: 0, stdout: 'panorama app-feed v1\n', stderr: '' });
  startFeed(s.view, s.cx); await until(() => s.view.writerError);
  await beat(s); await beat(s);
  assert.equal(ensures(), 0);
});
test('a renewed claim starts over: its startup ensure runs alone, with no retry beside it', async t => {
  const s = ready(t), beat = skewing(t); staleFeed(s); answering([1]);
  startFeed(s.view, s.cx); await until(() => s.view.writerError);
  let release; const held = new Promise(resolve => { release = resolve; });
  state.exec = (_binary, args) => args[1] === '--ensure' ? held : { code: 0, stdout: 'panorama app-feed v2\n', stderr: '' };
  state.values['panorama.feed.owner'] = null; // the lease lapsed: the next heartbeat claims again
  await beat(s); await until(() => ensures() === 2);
  await beat(s); assert.equal(ensures(), 2, 'recover() waits for the new startup ensure');
  release({ code: 0, stdout: '', stderr: '' }); await until(() => s.view.writerError === null);
});
test('the consumer packet reports collection age apart from lease and read staleness', async t => {
  const s = ready(t); startFeed(s.view, s.cx); await until(() => consumerFeed(s.view).stale === false);
  assert.equal(consumerFeed(s.view).aged, false);
  const packet = state.values['panorama.feed'];
  state.values['panorama.feed'] = { ...packet, stale: false, generated_at: Date.now() / 1000 - 16 };
  assert.equal(consumerFeed(s.view).aged, true);
  assert.equal(consumerFeed(s.view).stale, true, 'age alone is stale: Peek joins without `aged` because it refuses stale');
  state.values['panorama.feed'] = packet;
  state.values['panorama.feed.owner'] = null;
  assert.equal(consumerFeed(s.view).stale, true); assert.equal(consumerFeed(s.view).aged, false, 'a lease handoff does not age the collection');
  state.values['panorama.feed'] = { ...state.values['panorama.feed'], generated_at: Date.now() / 1000 - 16 };
  assert.equal(consumerFeed(s.view).aged, true);
});

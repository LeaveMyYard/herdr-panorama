import test from 'node:test';
import assert from 'node:assert/strict';
import { InboxNavigation, parseAvailability, parseOpened, questionArgument } from '../mods/panorama-graph/inbox.js';
const result = (value, code = 0) => ({ code, stdout: JSON.stringify(value), stderr: '' });
function bridge(responses = []) {
  const calls = [];
  const navigation = new InboxNavigation({ appHome: '/owned/app', callPrivate: async args => { calls.push(args); return responses.shift(); } });
  return { navigation, calls };
}
test('availability accepts exact booleans; question capability is never a count or inventory', () => {
  assert.deepEqual(parseAvailability(result({available:true,questions:true})),{available:true,questions:true});
  assert.deepEqual(parseAvailability(result({available:true,questions:false})),{available:true,questions:false});
  for (const output of [result({available:1,questions:true}),result({available:true,questions:2}),result({available:true,questions:[14]}),result({available:true,questions:true,other:0}),result({available:false,questions:true}),result({available:true}),result({available:true,questions:true},1),{code:0,stdout:'not JSON'}]) {
    assert.deepEqual(parseAvailability(output),{available:false,questions:false});
  }
});
test('refresh performs the no-I/O help probe then explicit-home availability through callPrivate', async () => {
  const {navigation,calls}=bridge([{code:0,stdout:'panorama app-inbox v1\n',stderr:''},result({available:true,questions:true})]);
  assert.deepEqual(await navigation.refresh(),{available:true,questions:true});
  assert.deepEqual(calls,[['app-inbox','--help'],['app-inbox','--available','--app-home','/owned/app']]);
});
test('a missing or incompatible help capability never reaches availability or navigation', async () => {
  const {navigation,calls}=bridge([{code:0,stdout:'another capability',stderr:''}]);
  await navigation.refresh();assert.deepEqual(await navigation.open(),{opened:false});assert.equal(calls.length,1);
});
test('plain i works without question deep links; a never falls back to plain inbox', async () => {
  const {navigation,calls}=bridge([result({opened:true})]);navigation.capabilities={available:true,questions:false};
  assert.deepEqual(await navigation.open(14),{opened:false});assert.equal(calls.length,0);
  assert.deepEqual(await navigation.open(),{opened:true});
  assert.deepEqual(calls,[['app-inbox','--open','--app-home','/owned/app']]);
});
test('question navigation preserves canonical positive safe ID and does not interpolate a command', async () => {
  const {navigation,calls}=bridge([result({opened:true})]);navigation.capabilities={available:true,questions:true};
  assert.deepEqual(await navigation.open(14),{opened:true});
  assert.deepEqual(calls,[['app-inbox','--open','--question','14','--app-home','/owned/app']]);
  for (const question of [0,-1,1.5,NaN,Infinity,Number.MAX_SAFE_INTEGER+1,'14','01','Q14','14; echo bad']) assert.throws(()=>navigation.open(question),RangeError);
  assert.equal(calls.length,1);assert.equal(questionArgument(Number.MAX_SAFE_INTEGER),'9007199254740991');
});
test('duplicate user gestures share one pending open until it settles, regardless of requested question', async () => {
  let settle;const calls=[];
  const navigation=new InboxNavigation({appHome:'/owned/app',callPrivate:args=>{calls.push(args);return new Promise(resolve=>{settle=resolve;});}});
  navigation.capabilities={available:true,questions:true};
  const first=navigation.open(14);assert.equal(navigation.busy,true);
  assert.equal(navigation.open(15),first);assert.equal(navigation.open(),first);assert.equal(calls.length,1);
  settle(result({opened:true}));assert.deepEqual(await first,{opened:true});assert.equal(navigation.busy,false);
});
test('failed/ambiguous opens and rejected private verification are explicit failures with no retry', async () => {
  for (const response of [result({opened:true},3),result({opened:true},1),result({opened:false}),result({opened:true,shown:true}),{code:0,stdout:'invalid',stderr:'bad reply'}]) {
    const {navigation,calls}=bridge([response]);navigation.capabilities={available:true,questions:true};
    assert.deepEqual(await navigation.open(14),{opened:false});assert.equal(calls.length,1);assert.equal(navigation.busy,false);
  }
  let calls=0;const navigation=new InboxNavigation({appHome:'/owned/app',callPrivate:async()=>{calls++;throw Error('disabled receipt');}});
  navigation.capabilities={available:true,questions:true};
  assert.deepEqual(await navigation.open(14),{opened:false});assert.equal(calls,1);assert.equal(navigation.error,'disabled receipt');
});
test('concurrent availability refreshes share one probe and do not acquire open authority', async () => {
  let settle;const calls=[];
  const navigation=new InboxNavigation({appHome:'/owned/app',callPrivate:args=>{calls.push(args);if(args.includes('--help'))return new Promise(resolve=>{settle=resolve;});return Promise.resolve(result({available:false,questions:false}));}});
  const first=navigation.refresh();assert.equal(navigation.refresh(),first);settle({code:0,stdout:'panorama app-inbox v1',stderr:''});
  assert.deepEqual(await first,{available:false,questions:false});assert.equal(calls.length,2);
  assert.deepEqual(await navigation.open(),{opened:false});assert.equal(calls.length,2);
});
test('success records RPC acceptance only; shown/closed fields cannot fabricate completion evidence', () => {
  assert.equal(parseOpened(result({opened:true})),true);
  for(const value of [{opened:true,shown:true},{opened:true,closed:true},{shown:true},{opened:1}]) assert.equal(parseOpened(result(value)),false);
});

test('help accepts following usage lines but never a different version token',async()=>{
 const good=bridge([{code:0,stdout:'panorama app-inbox v1\nUsage: app-inbox',stderr:''},result({available:true,questions:false})]);
 assert.equal((await good.navigation.refresh()).available,true);assert.equal(good.calls.length,2);
 const wrong=bridge([{code:0,stdout:'panorama app-inbox v10\nUsage',stderr:''}]);
 assert.equal((await wrong.navigation.refresh()).available,false);assert.equal(wrong.calls.length,1);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { peekCommand, guard } from '../mods/panorama-peek/actions.js';
const G='0123456789abcdef0123456789abcdef';
test('close/transcript commands use exact approved callers and opaque token, with no payload',()=>{
  const intent={peekCaller:'panorama-graph',peekToken:G,peekInstallId:G};
  assert.equal(peekCommand(intent,'closed'),`panorama-graph.peek-closed-${G}`);
  assert.equal(peekCommand(intent,'transcript'),`panorama-graph.peek-transcript-${G}`);
  for(const altered of [{...intent,peekCaller:'files'},{...intent,peekToken:'../x'},{...intent,peekInstallId:null}]) assert.equal(peekCommand(altered,'closed'),null);
  assert.equal(peekCommand(intent,'answer'),null);
});
test('generation changes during validation or native read prevent use/publication',async()=>{
  let valid=true, reads=0, validations=0;
  const scope={key:G};
  const noRead=await guard(scope,()=>valid,async()=>{validations++;valid=false;},async()=>{reads++;return 'bad';});
  assert.equal(noRead,null);assert.equal(reads,0);assert.equal(validations,1);
  valid=true;
  const noPublish=await guard(scope,()=>valid,async()=>{validations++;},async()=>{reads++;valid=false;return 'old-data';});
  assert.equal(noPublish,null);assert.equal(reads,1);
});

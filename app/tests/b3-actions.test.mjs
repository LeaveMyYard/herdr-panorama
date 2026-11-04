import test from 'node:test';
import assert from 'node:assert/strict';
import { answerIntent, transcriptProps } from '../mods/panorama-graph/actions.js';
const root={terminal:'t2',status:'blocked',root:{inbox:[23,14]},isNative:false};
test('a navigates selected questions even when another root permission is blocked',()=>{
 assert.deepEqual(answerIntent(root,{agent:{status:'wait',question_ids:[23,14]}}),{kind:'question',question:14});
 assert.deepEqual(answerIntent(root,{agent:{status:'blk',question_ids:[]}}),{kind:'permission',target:{name:'agent.focus',args:{terminal:'t2'}}});
 assert.equal(answerIntent(root,{agent:{status:'done',question_ids:[]}}),null);
 assert.equal(answerIntent({...root,terminal:null},{agent:{status:'blk',question_ids:[]}}),null);
});
test('native permission routing remains on its verified native terminal',()=>{
 const native={...root,terminal:'t-child',isNative:true};
 assert.deepEqual(answerIntent(root,{kind:'native',item:native}),{kind:'permission',target:{name:'terminal.peek',args:{terminal:'t-child'}}});
});
test('inbox navigation emits positive safe question identities and never agent IDs as addresses',()=>{
 const item={...root,status:'idle',root:{inbox:[]}};
 assert.equal(answerIntent(item,{agent:{status:'wait',question_ids:[0,-1,Number.MAX_SAFE_INTEGER+1]}}),null);
 assert.deepEqual(answerIntent(item,{agent:{id:'t99',status:'wait',question_ids:[9]}}),{kind:'question',question:9});
});
test('Transcript intent keeps source references inert and carries only the {root, agent} scope',()=>{
 const source={...root,root:{...root.root,transcript:'$(never-execute)'}};
 assert.deepEqual(transcriptProps(source,{kind:'root',item:source}),{root:source.root,agent:null,terminal:'t2',terminalAction:'focus',action:'transcript'});
});
test('an agent under a native terminal child answers and opens through its owning terminal',()=>{
 const native={...root,terminal:'t-child',isNative:true,status:'idle',root:{inbox:[31]}}, agent={id:'a',status:'blk',question_ids:[]};
 assert.deepEqual(answerIntent(root,{kind:'agent',agent,owner:native}),{kind:'permission',target:{name:'terminal.peek',args:{terminal:'t-child'}}});
 assert.deepEqual(transcriptProps(root,{kind:'agent',agent,owner:native}),{root:native.root,agent,terminal:'t-child',terminalAction:'peek',action:'transcript'});
});

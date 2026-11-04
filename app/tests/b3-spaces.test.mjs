import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { joinRoots } from '../mods/panorama-graph/model.js';
import { spaceTotals } from '../mods/panorama-graph/model.js';
import { spaceInventory } from '../mods/panorama-pane/space.js';
const feed=JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json',import.meta.url)));
test('space counts use exact app joins, unique scoped needs, and exclude unmatched feed roots',()=>{
 const root=feed.roots.find(root=>root.host==='herdr-core'&&root.terminal);
 const stock=[{address:root.terminal,space:'s',tab:'tab',status:'working',program:{key:'codex'}}];
 const roots=joinRoots(feed,stock,[{address:root.terminal,state:'live'}],[],[{id:'s',tabs:[{id:'tab',name:'Tab',named:true}]}]);
 const total=spaceTotals(roots,'s');
 assert.equal(total.needsCount,root.agents.filter(agent=>['fail','blk','wait'].includes(agent.status)).length);
 assert.equal(total.questions,root.inbox.length);
 assert.equal(spaceInventory(roots,'s').roots,1);
 assert.equal(spaceTotals(roots,'other').needsCount,0);
});
test('native descendant direct space membership wins and does not double count sidebar aggregate',()=>{
 const source=structuredClone(feed.roots[0]);source.agents=[];source.inbox=[];source.herdr_status='idle';
 const own={key:'root',terminal:'t-root',root:source,status:'idle',space:'A',isNative:false,nativeChildren:[],aggregate:{needsCount:99}};
 const child={key:'child',terminal:'t-child',root:{...source,herdr_status:'blocked'},status:'blocked',space:'B',isNative:true,nativeChildren:[]};
 own.nativeChildren=[child];
 assert.equal(spaceTotals([own],'A').needsCount,0);
 assert.equal(spaceTotals([own],'B').needsCount,1);
 assert.equal(spaceTotals([own,child],'B').needsCount,1);
 assert.equal(spaceInventory([own],'B').agents,1);
});
test('a stock agent without a pane id never borrows another pane title',()=>{
 const stock=[{address:'t1',space:'s',tab:'tab',status:'idle',program:{key:'claude',name:'Claude'}}];
 const [root]=joinRoots(null,stock,[{address:'t1',state:'live'}],[{terminal:'other',tab:'tab',title:'Wrong pane title'}],[{id:'s',tabs:[{id:'tab',name:'Tab'}]}]);
 assert.equal(root.name,'Claude · t1');
});

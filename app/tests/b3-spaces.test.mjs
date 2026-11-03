import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { joinRoots } from '../mods/panorama-graph/model.js';
import { spaceTotals } from '../mods/panorama-graph/space.js';
const feed=JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json',import.meta.url)));
test('space counts use exact app joins, unique scoped needs, and exclude unmatched feed roots',()=>{
 const root=feed.orchestrators.find(root=>root.host==='herdr-core'&&root.terminal);
 const stock=[{address:root.terminal,space:'s',tab:'tab',status:'working',program:{key:'codex'}}];
 const roots=joinRoots(feed,stock,[{address:root.terminal,state:'live'}],[],[{id:'s',tabs:[{id:'tab',name:'Tab',named:true}]}]);
 const total=spaceTotals(roots,'s');
 assert.equal(total.needsCount,root.threads.flatMap(thread=>thread.agents).filter(agent=>['fail','blk','wait'].includes(agent.status)).length);
 assert.equal(total.questions,root.inbox.length);
 assert.equal(total.orchestrators,1);
 assert.equal(spaceTotals(roots,'other').needsCount,0);
});
test('native descendant direct space membership wins and does not double count sidebar aggregate',()=>{
 const source=structuredClone(feed.orchestrators[0]);source.threads=[];source.inbox=[];source.herdr_status='idle';
 const own={key:'root',root:source,status:'idle',space:'A',isNative:false,nativeChildren:[],aggregate:{needsCount:99}};
 const child={key:'child',root:{...source,herdr_status:'blocked'},status:'blocked',space:'B',isNative:true,nativeChildren:[]};
 own.nativeChildren=[child];
 assert.equal(spaceTotals([own],'A').needsCount,0);
 assert.equal(spaceTotals([own],'B').needsCount,1);
 assert.equal(spaceTotals([own,child],'B').needsCount,1);
 assert.equal(spaceTotals([own],'B').agents,1);
});

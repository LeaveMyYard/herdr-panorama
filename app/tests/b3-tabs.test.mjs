import test from 'node:test';
import assert from 'node:assert/strict';
import { createSwarmOpener, graphTab, contentHeight } from '../mods/panorama-spaces/tabs.js';
function world() {
 const space={id:'s',active:true,tabs:[{id:'old',name:'terminal'},{id:'fake',name:'Swarm'}]}, ps=[], commands=[];
 let created=0;
 const run=(name,args)=>{commands.push([name,args]);if(name==='tab.new')created++;};
 const sleep=async()=>{if(created&&!ps.length){space.tabs.push({id:'graph',name:'Swarm'});ps.push({tab:'graph',mod:'panorama-graph'});}};
 const open=createSwarmOpener({run,spaces:()=>[space],panes:()=>ps,sleep});
 return {space,ps,commands,open};
}
test('rapid space clicks create Swarm exactly once, discover its actual mod pane, move only new tab to index 1', async()=>{
 const w=world();await Promise.all([w.open('s'),w.open('s'),w.open('s')]);
 assert.equal(w.commands.filter(([name])=>name==='tab.new').length,1);
 assert.deepEqual(w.commands.filter(([name])=>name==='tab.move'),[['tab.move',{id:'graph',index:1}]]);
 assert.equal(w.commands.at(-1)[0],'tab.select');assert.equal(w.commands.at(-1)[1].id,'graph');
});
test('existing graph tab renamed or dragged stays in place on later space clicks',async()=>{
 const w=world();w.space.tabs.push({id:'renamed',name:'mine'});w.ps.push({tab:'renamed',mod:'panorama-graph'});
 await w.open('s');assert.equal(w.commands.filter(([name])=>name==='tab.new'||name==='tab.move').length,0);
 assert.equal(w.commands.at(-1)[1].id,'renamed');
});
test('closed graph is recreated only on next explicit click, not on model read',async()=>{
 const w=world();await w.open('s');w.ps.length=0;w.space.tabs=w.space.tabs.filter(tab=>tab.id!=='graph');
 assert.equal(graphTab(w.space,w.ps),null);assert.equal(w.commands.filter(([name])=>name==='tab.new').length,1);
 await w.open('s');assert.equal(w.commands.filter(([name])=>name==='tab.new').length,2);
});
test('late async creation never steals focus from a later space selection',async()=>{
 const w=world();const first=w.open('s');w.space.active=false;await first;
 assert.equal(w.commands.filter(([name])=>name==='tab.select').length,0);
});
test('content-sized Spaces leaves swarm remainder and follows nested row heights',()=>{
 assert.equal(contentHeight([{},{},{}]),143);assert.equal(contentHeight([]),90);
 assert.equal(contentHeight([{}, {tree:'└─'},{}],1),167);
});

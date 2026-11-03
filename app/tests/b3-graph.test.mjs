import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { graphModel, treeLayout, columnWindow, navigate, selectNode, currentNode, ClickGuard, animationVisible, viewportAnimates, logicalSize } from '../mods/panorama-graph/graph.js';
import { agentKey } from '../mods/panorama-graph/contract.js';
const feed = JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json', import.meta.url)));
const item = { key: 'root', root: feed.orchestrators[0], aggregate: { kinds: [], sweep: false }, status: 'working' };
const state = () => ({ orch: item.key, thread: null, agent: null, focus: 1, expanded: new Set(), remember: new Map(), scroll: {} });
const sourceAgent = feed.orchestrators.flatMap(root => root.threads.flatMap(thread => thread.agents))[0];
const agent = (id, status, parent = null, extra = {}) => ({ ...sourceAgent, id, parent, role: id, status, questions: [], question_ids: [], activity: '', error: null, ...extra });
function scenario() {
  const root = structuredClone(item);
  root.root.threads = [{ ...root.root.threads[0], agents: [
    agent('reviewer','rev'), agent('author','done'), agent('researcher','run','reviewer'),
    agent('batch','run','reviewer'), agent('done-explorer','done','reviewer'),
    agent('checker','wait','researcher'), agent('fixer','fail','researcher'),
    agent('shard','run','batch'), agent('probe','run','checker'), agent('done-probe','done','checker'),
  ] }];
  return root;
}
test('fixed-width window matches final D and D2 and retains focus+parent on narrow viewports', () => {
  assert.deepEqual(columnWindow(7, 1, 1230).columns.map(column => column.column), [0,1,2,3]);
  const scrolled = columnWindow(7, 5, 1230);
  assert.deepEqual(scrolled.columns.map(column => column.column), [3,4,5,6]);
  assert.equal(scrolled.columns[0].x,24);
  assert.equal(scrolled.columns[0].width,272);
  assert.deepEqual(columnWindow(7,5,200).columns.map(column=>column.column),[4,5]);
  assert.equal(columnWindow(7,5,1230,0).lo,0);
  assert.ok(scrolled.left > 0 && scrolled.shown < scrolled.total);
});
test('5d placement aligns spawn groups and reserves trunk room after prior children', () => {
  const root = scenario(), thread = root.root.threads[0], result = treeLayout(root,thread,null);
  const find = id => result.nodes.find(node => node.agent?.id === id);
  assert.equal(find('researcher').y,find('reviewer').y);
  assert.ok(find('batch').y >= find('researcher').y + find('researcher').height + 10);
  assert.ok(find('batch').y + 27 >= find('fixer').y + 27 + 16);
  assert.equal(find('probe').depth,4);
  assert.ok(result.nodes.filter(node => node.fold).length === 3);
  assert.ok(result.nodes.filter(node => node.fold).every(node => node.mid === 20));
});
test('finished ancestors with live descendants stay visible; folding and unfolding preserve identity', () => {
  const root=scenario(), thread=root.root.threads[0]; thread.agents.find(a=>a.id==='researcher').status='done';
  const result=treeLayout(root,thread,null), researcher=result.nodes.find(node=>node.agent?.id==='researcher');
  assert.ok(researcher);
  const reviewer=result.nodes.find(node=>node.agent?.id==='reviewer');
  assert.equal(result.nodes.find(node=>node.fold?.some(a=>a.id==='done-explorer')).parent.key,reviewer.key);
  const opened=treeLayout(root,thread,null,new Set([reviewer.key]));
  assert.ok(opened.nodes.some(node=>node.agent?.id==='done-explorer'));
  assert.equal(new Set(opened.nodes.map(node=>node.key)).size,opened.nodes.length);
});
test('arrow navigation traverses parents and remembers each child selection', () => {
  const root=scenario(), s=state();let model=graphModel([root],s);
  selectNode(s,model,model.threadNodes[0]);
  navigate(s,model,'right'); model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'reviewer');
  navigate(s,model,'right');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'researcher');
  navigate(s,model,'down');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'batch');
  navigate(s,model,'left');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'reviewer');
  navigate(s,model,'right');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'batch');
  navigate(s,model,'right');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'shard');
  navigate(s,model,'left');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'batch');
});
test('arbitrary depth and cyclic producer parent references do not recurse or drop identities', () => {
  const root=scenario(), thread=root.root.threads[0];
  thread.agents=Array.from({length:2500},(_,i)=>agent(`a${i}`,'run',i ? `a${i-1}`:null));
  assert.equal(treeLayout(root,thread,null).depth,2500);
  thread.agents=[agent('a','run','b'),agent('b','run','a'),agent('c','run','c')];
  const nodes=treeLayout(root,thread,null).nodes;
  assert.equal(nodes.length,3);assert.equal(new Set(nodes.map(node=>node.key)).size,3);
});
test('scroll and moved-card refresh guards have exact independent deadlines; selection geometry does not arm', () => {
  const guard=new ClickGuard(), nodes=[{key:'x',column:2,y:0,height:54}];
  guard.refresh(nodes,0);assert.equal(guard.allows('x',0),true);
  guard.scroll(10);assert.equal(guard.allows('x',309),false);assert.equal(guard.allows('x',310),true);
  guard.refresh([{...nodes[0],y:64}],400);assert.equal(guard.allows('x',1399),false);assert.equal(guard.allows('x',1400),true);
  guard.refresh([{...nodes[0],y:100}],1500,false);assert.equal(guard.allows('x',1500),true);
});
test('animation math pauses cards exactly outside the independent column viewport', () => {
  const node={kind:'agent',agent:agent('x','wait'),y:100,height:54};
  assert.ok(animationVisible(node,0,109));assert.equal(animationVisible(node,0,108),false);
  assert.equal(animationVisible(node,146,200),false);assert.ok(animationVisible(node,145,200));
  node.agent.status='done';assert.equal(animationVisible(node,100,200),false);
});
test('fixture agent identity remains scoped to its owning root and does not become a terminal', () => {
  const thread=item.root.threads[0], result=treeLayout(item,thread,null);
  assert.ok(result.nodes.some(node=>node.key===agentKey(item.root,thread.agents[0])));
  assert.ok(result.nodes.every(node=>node.terminal===undefined));
});
test('selected card details collapse when focus returns to thread and expand only in focused depth',()=>{
 const root=scenario(), s=state();const key=agentKey(root.root,root.root.threads[0].agents.find(a=>a.id==='checker'));
 s.agent=key;s.focus=4;let model=graphModel([root],s);
 assert.ok(model.nodes.find(node=>node.key===key).height>54);
 assert.equal(model.threadNodes[0].height,54);
 s.focus=1;model=graphModel([root],s);
 assert.equal(model.nodes.find(node=>node.key===key).height,54);
 assert.ok(model.threadNodes[0].height>54);
});
test('threadless native fallback uses only verified same-space ancestry and never invents a thread',()=>{
 const root=scenario();root.space='s';root.root.threads=[];
 const native=(id,parent,space='s')=>({key:id,terminal:id,nativeParent:parent,space,status:'working',root:{threads:[],inbox:[],herdr_status:'working'},aggregate:{kinds:[],working:true},isNative:true,name:id});
 root.nativeChildren=[native('t-child','t-root'),native('t-grandchild','t-child'),native('other','t-root','other-space')];
 const s=state();let model=graphModel([root],s);
 assert.equal(model.thread,null);assert.equal(model.threadNodes.length,0);assert.equal(model.nativeFallback,true);
 assert.deepEqual(model.tree.nodes.map(n=>[n.key,n.column]),[['t-child',1],['t-grandchild',2]]);
 selectNode(s,model,model.orchs[0]);navigate(s,model,'right');model=graphModel([root],s);
 assert.equal(currentNode(model,s).key,'t-child');
 navigate(s,model,'right');model=graphModel([root],s);assert.equal(currentNode(model,s).key,'t-grandchild');
 navigate(s,model,'left');model=graphModel([root],s);assert.equal(currentNode(model,s).key,'t-child');
 navigate(s,model,'left');assert.equal(s.focus,0);
 root.root.threads=scenario().root.threads;model=graphModel([root],s);
 assert.equal(model.nativeFallback,false);assert.ok(model.tree.nodes.every(n=>n.kind!=='native'));
});

test('an arrow at the column boundary preserves an independently scrolled viewport',()=>{
 const root=scenario(), s=state(), model=graphModel([root],s);
 selectNode(s,model,model.orchs[0]);s.scroll={0:64};s.manual=2;
 assert.equal(navigate(s,model,'up'),false);
 assert.deepEqual(s.scroll,{0:64});assert.equal(s.manual,2);
});

test('vertical motion intersection excludes visible nonanimated corners and clipped bottom sweeps',()=>{
 const waiting={kind:'agent',agent:agent('wait','wait'),y:100,height:54};
 assert.equal(animationVisible(waiting,0,108),false); // Only the rounded top corner is inside.
 assert.equal(animationVisible(waiting,0,109),true);
 assert.equal(animationVisible(waiting,146,100),false); // Only the rounded bottom corner is inside.
 const working={kind:'agent',agent:agent('run','run'),y:100,height:54};
 assert.equal(animationVisible(working,0,150),false); // Spinner/card visible, animated bottom entirely below.
 assert.equal(animationVisible(working,0,153),true);
 assert.equal(animationVisible(working,155,100),false);
 assert.equal(animationVisible(working,154,100),true); // The outside half of the border strip is still visible.
 assert.equal(animationVisible(waiting,100,0),false);
});
test('horizontal clip and independent vertical column offsets jointly govern animation',()=>{
 const card=(id,column,y,status)=>({key:id,column,y,height:54,kind:'agent',agent:agent(id,status)});
 const model={columns:[[card('quiet',0,0,'done')],[card('run',1,0,'run')],[card('wait',2,0,'wait')]]};
 const window={columns:[{column:0,x:0,width:236},{column:1,x:272,width:300}]};
 assert.equal(viewportAnimates(model,window,{},200,500),false); // Included fixed-width column is completely clipped.
 assert.equal(viewportAnimates(model,window,{},290,500),true);
 assert.equal(viewportAnimates(model,window,{1:100},500,500),false); // Working card above its column viewport.
 assert.equal(viewportAnimates(model,window,{1:-600},500,500),false); // Working card below its column viewport.
 assert.equal(viewportAnimates(model,window,{1:0,2:0},500,500),true);
 const onlyQuiet={columns:[window.columns[0]]};
 assert.equal(viewportAnimates(model,onlyQuiet,{},500,500),false); // Horizontally hidden needs edge never animates.
});

test('native zoomed canvas bounds normalize into the same units as graph card placement',()=>{
 const measured={width:1475,height:814}, size=logicalSize(measured,1.2);
 assert.equal(size.width,1475/1.2);assert.equal(size.height,814/1.2);
 const run={kind:'agent',agent:agent('last','run'),y:1856,height:54};
 assert.equal(animationVisible(run,1096,size.height),false); // Initial native evidence: card entirely below actual viewport.
 assert.equal(animationVisible(run,1910-size.height,size.height),true);
 assert.deepEqual(logicalSize(measured,1),measured);assert.deepEqual(logicalSize(measured,NaN),measured);
});

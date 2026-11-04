import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { graphModel, treeLayout, columnWindow, navigate, selectNode, currentNode, crumbs, ClickGuard, animationVisible, viewportAnimates, logicalSize, WIDTH } from '../mods/panorama-graph/graph.js';
import { agentKey, rootKey } from '../mods/panorama-graph/contract.js';
import { joinRoots } from '../mods/panorama-graph/model.js';
const feed = JSON.parse(readFileSync(new URL('../../tests/fixtures/app-feed/swarm.json', import.meta.url)));
const item = { key: 'root', name: 'Billing API migration', root: feed.roots[0], aggregate: { kinds: [], sweep: false }, status: 'working', space: 's', nativeChildren: [] };
const state = () => ({ root: item.key, agent: null, focus: 0, expanded: new Set(), remember: new Map(), scroll: {} });
const sourceAgent = feed.roots.flatMap(root => root.agents)[0];
const agent = (id, status, parent = null, extra = {}) => ({ ...sourceAgent, id, parent, role: id, title: `Task ${id}`, status, questions: [], question_ids: [], activity: '', error: null, ...extra });
function scenario() {
  const root = structuredClone(item);
  root.root.agents = [
    agent('reviewer','rev'), agent('author','done'), agent('researcher','run','reviewer'),
    agent('batch','run','reviewer'), agent('done-explorer','done','reviewer'),
    agent('checker','wait','researcher',{question_ids:[14],questions:['Which migration should be kept?']}), agent('fixer','fail','researcher'),
    agent('shard','run','batch'), agent('probe','run','checker'), agent('done-probe','done','checker'),
  ];
  return root;
}
const native=(id,parent,space='s',agents=[])=>({key:id,terminal:id,nativeParent:parent,space,status:'working',root:{agents,inbox:[],herdr_status:'working'},aggregate:{kinds:[],working:true},isNative:true,name:id});
test('uniform fixed-width depth window matches V11 D and D2 and retains focus+parent on narrow viewports', () => {
  assert.deepEqual(columnWindow(6, 1, 1230).columns.map(column => column.column), [0,1,2,3]);
  const scrolled = columnWindow(6, 4, 1230);
  assert.deepEqual(scrolled.columns.map(column => column.column), [2,3,4,5]);
  assert.equal(scrolled.columns[0].x,24);
  assert.ok(scrolled.columns.every(column => column.width === WIDTH));
  assert.deepEqual(columnWindow(6,4,200).columns.map(column=>column.column),[3,4]);
  assert.equal(columnWindow(6,4,1230,0).lo,0);
  assert.ok(scrolled.left > 0 && scrolled.shown < scrolled.total);
});
test('depth columns hold one card kind per node: roots at depth 0, then the whole agent tree', () => {
  const root=scenario(), model=graphModel([root,{...structuredClone(item),key:'other',name:'Other'}],state());
  assert.deepEqual(model.columns.map(column=>column.length),[2,2,3,3,2]);
  assert.deepEqual(model.columns[0].map(node=>node.kind),['root','root']);
  assert.ok(model.nodes.every(node=>['root','agent','fold','native'].includes(node.kind)));
  assert.ok(model.nodes.filter(node=>node.kind!=='root').every(node=>node.column===node.depth));
});
test('5d placement aligns spawn groups and reserves trunk room after prior children', () => {
  const result = treeLayout(scenario());
  const find = id => result.nodes.find(node => node.agent?.id === id);
  assert.equal(find('researcher').y,find('reviewer').y);
  assert.ok(find('batch').y >= find('researcher').y + find('researcher').height + 10);
  assert.ok(find('batch').y + 27 >= find('fixer').y + 27 + 16);
  assert.equal(find('probe').depth,4);
  assert.ok(result.nodes.filter(node => node.fold).length === 3);
  assert.ok(result.nodes.filter(node => node.fold).every(node => node.mid === 20));
});
test('finished siblings fold per parent with titles; live descendants keep finished ancestors visible', () => {
  const root=scenario(); root.root.agents.find(a=>a.id==='researcher').status='done';
  const result=treeLayout(root), researcher=result.nodes.find(node=>node.agent?.id==='researcher');
  assert.ok(researcher);
  const reviewer=result.nodes.find(node=>node.agent?.id==='reviewer');
  const fold=result.nodes.find(node=>node.fold?.some(a=>a.id==='done-explorer'));
  assert.equal(fold.parent.key,reviewer.key); assert.equal(fold.group,reviewer.key); assert.equal(fold.older,0);
  const top=result.nodes.find(node=>node.fold?.some(a=>a.id==='author'));
  assert.equal(top.parent,null); assert.equal(top.older,root.root.hidden_finished);
  const opened=treeLayout(root,null,new Set([reviewer.key]));
  assert.ok(opened.nodes.some(node=>node.agent?.id==='done-explorer'));
  assert.equal(new Set(opened.nodes.map(node=>node.key)).size,opened.nodes.length);
});
test('a card carries the needs of its subtree below it, distinct per question', () => {
  const nodes=treeLayout(scenario()).nodes, find=id=>nodes.find(node=>node.agent?.id===id);
  assert.deepEqual(find('reviewer').below.kinds,['fail','wait']); assert.equal(find('reviewer').below.questions.size,1);
  assert.deepEqual(find('checker').below.kinds,[]); assert.deepEqual(find('batch').below.kinds,[]);
});
test('arrow navigation traverses parents from depth 0 and remembers each child selection', () => {
  const root=scenario(), s=state();let model=graphModel([root],s);
  selectNode(s,model,model.heads[0]);
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
  assert.deepEqual(crumbs(model,s),['Billing API migration','Task reviewer','Task batch','Task shard']);
  navigate(s,model,'left');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'batch');
  navigate(s,model,'left');navigate(s,model,'left');assert.equal(s.focus,0);
  model=graphModel([root],s);navigate(s,model,'right');model=graphModel([root],s);
  assert.equal(currentNode(model,s).agent.id,'reviewer');
});
test('arbitrary depth and cyclic producer parent references do not recurse or drop identities', () => {
  const root=scenario();
  root.root.agents=Array.from({length:2500},(_,i)=>agent(`a${i}`,'run',i ? `a${i-1}`:null));
  assert.equal(treeLayout(root).depth,2500);
  root.root.agents=[agent('a','run','b'),agent('b','run','a'),agent('c','run','c')];
  const nodes=treeLayout(root).nodes;
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
  const result=treeLayout(item);
  assert.ok(result.nodes.some(node=>node.key===agentKey(item.root,item.root.agents[0])));
  assert.ok(result.nodes.every(node=>node.terminal===undefined && (node.kind==='fold' || node.owner===item)));
});
test('selected card details collapse when focus returns to depth 0 and expand only in agent depths',()=>{
 const root=scenario(), s=state();const key=agentKey(root.root,root.root.agents.find(a=>a.id==='checker'));
 s.agent=key;s.focus=3;let model=graphModel([root],s);
 assert.ok(model.nodes.find(node=>node.key===key).height>54+20);
 assert.equal(model.heads[0].height,54);
 s.focus=0;model=graphModel([root],s);
 assert.equal(model.nodes.find(node=>node.key===key).height,54);
});
test('native terminal children and transcript agents merge into one tree under the root',()=>{
 const root=scenario();root.root.agents=[];
 root.nativeChildren=[native('t-child','t-root'),native('t-grandchild','t-child'),native('other','t-root','other-space')];
 const s=state();let model=graphModel([root],s);
 assert.deepEqual(model.tree.nodes.map(n=>[n.key,n.column]),[['t-child',1],['t-grandchild',2]]);
 navigate(s,model,'right');model=graphModel([root],s);
 assert.equal(currentNode(model,s).key,'t-child');
 navigate(s,model,'right');model=graphModel([root],s);assert.equal(currentNode(model,s).key,'t-grandchild');
 navigate(s,model,'left');model=graphModel([root],s);assert.equal(currentNode(model,s).key,'t-child');
 navigate(s,model,'left');assert.equal(s.focus,0);
 root.root.agents=scenario().root.agents;model=graphModel([root],s);
 assert.deepEqual(model.columns[1].map(n=>n.kind),['agent','native','fold']);
 assert.equal(model.columns[2].filter(n=>n.kind==='native').length,1);
 root.nativeChildren=[native('loop-a','loop-b'),native('loop-b','loop-a')];
 assert.deepEqual(treeLayout(root).nodes.filter(n=>n.kind==='native').map(n=>[n.key,n.depth]),[['loop-a',1],['loop-b',2]]);
});
test('R8: a feed root that is also a native terminal child appears once, carrying its own agents',()=>{
 const own=structuredClone(feed.roots[0]), nested=structuredClone(feed.roots[0]);
 own.terminal='t1';own.pane_id='core:t1';own.agents=[agent('lead','run')];
 nested.terminal='vps/t7';nested.pane_id='core:vps/t7';nested.title='Nested session';nested.agents=[agent('helper','wait',null,{question_ids:[14],questions:[nested.inbox_texts[0]]})];
 const stock=[{address:'t1',space:'s',tab:'tab',status:'working',program:{key:'claude'}}];
 const terminals=[{address:'t1',state:'live'},{address:'vps/t7',parent:'t1',state:'live',status:'working'},{address:'t9',parent:'vps/t7',state:'live',status:'idle'}];
 const roots=joinRoots({roots:[own,nested]},stock,terminals,[],[{id:'s',tabs:[{id:'tab',name:'Tab'}]}]).filter(root=>root.space==='s');
 const model=graphModel(roots,{...state(),root:rootKey(own)});
 assert.deepEqual(model.columns[0].map(n=>n.key),[rootKey(own)]);
 assert.equal(new Set(model.nodes.map(n=>n.key)).size,model.nodes.length);
 assert.equal(model.nodes.filter(n=>n.key===rootKey(nested)).length,1);
 const host=model.nodes.find(n=>n.key===rootKey(nested));
 assert.equal(host.kind,'native');assert.equal(host.depth,1);assert.equal(host.item.name,'Nested session');
 const helper=model.nodes.find(n=>n.agent?.id==='helper');
 assert.equal(helper.parent.key,host.key);assert.equal(helper.key,agentKey(nested,nested.agents[0]));assert.equal(helper.owner,host.item);
 assert.deepEqual(model.columns[2].map(n=>n.kind),['agent','native']);
});

test('an arrow at the column boundary preserves an independently scrolled viewport',()=>{
 const root=scenario(), s=state(), model=graphModel([root],s);
 selectNode(s,model,model.heads[0]);s.scroll={0:64};s.manual=2;
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
 const window={columns:[{column:0,x:0,width:272},{column:1,x:308,width:272}]};
 assert.equal(viewportAnimates(model,window,{},300,500),false); // Included fixed-width column is completely clipped.
 assert.equal(viewportAnimates(model,window,{},330,500),true);
 assert.equal(viewportAnimates(model,window,{1:100},600,500),false); // Working card above its column viewport.
 assert.equal(viewportAnimates(model,window,{1:-600},600,500),false); // Working card below its column viewport.
 assert.equal(viewportAnimates(model,window,{1:0,2:0},600,500),true);
 const onlyQuiet={columns:[window.columns[0]]};
 assert.equal(viewportAnimates(model,onlyQuiet,{},600,500),false); // Horizontally hidden needs edge never animates.
});

test('native zoomed canvas bounds normalize into the same units as graph card placement',()=>{
 const measured={width:1475,height:814}, size=logicalSize(measured,1.2);
 assert.equal(size.width,1475/1.2);assert.equal(size.height,814/1.2);
 const run={kind:'agent',agent:agent('last','run'),y:1856,height:54};
 assert.equal(animationVisible(run,1096,size.height),false); // Initial native evidence: card entirely below actual viewport.
 assert.equal(animationVisible(run,1910-size.height,size.height),true);
 assert.deepEqual(logicalSize(measured,1),measured);assert.deepEqual(logicalSize(measured,NaN),measured);
});

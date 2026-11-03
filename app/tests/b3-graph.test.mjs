import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { graphModel, treeLayout, columnWindow, navigate, selectNode, currentNode, ClickGuard, animationVisible } from '../mods/panorama-graph/graph.js';
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
  assert.ok(animationVisible(node,0,101));assert.equal(animationVisible(node,0,100),false);
  assert.equal(animationVisible(node,154,200),false);assert.ok(animationVisible(node,153,200));
  node.agent.status='done';assert.equal(animationVisible(node,100,200),false);
});
test('fixture agent identity remains scoped to its owning root and does not become a terminal', () => {
  const thread=item.root.threads[0], result=treeLayout(item,thread,null);
  assert.ok(result.nodes.some(node=>node.key===agentKey(item.root,thread.agents[0])));
  assert.ok(result.nodes.every(node=>node.terminal===undefined));
});

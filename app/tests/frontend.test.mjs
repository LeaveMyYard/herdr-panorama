import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, renameSync, existsSync, symlinkSync, linkSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { sandbox, tree, root } from './helpers.mjs';
const ok = result => assert.equal(result.status,0,result.stderr||result.stdout);
function setup(t) {
  const s=sandbox(t);writeFileSync(join(s.home,'keys.json'),'{ "global": { "alt-p": "personal" } }\n');ok(s.run('install'));
  s.receipt=JSON.parse(readFileSync(s.manifest));s.control=join(s.home,'data/panorama/runtime',s.receipt.install_id,'control.json');s.shared=join(s.home,'shared.json');
  s.packet={feed:null,stale:true,errors:[],generated_at:1793717546.3210924,writerError:null,install_id:s.receipt.install_id};
  s.owner={v:1,token:'test-owned',role:'sidebar',seen_at:Date.now()};
  return s;
}
const own = s => ({personal:{opaque:['preserve',7]},'panorama.feed':s.packet,'panorama.feed.owner':s.owner});
const payload = s => Object.fromEntries(Object.keys(s.receipt.files).map(path=>[path,readFileSync(join(s.home,path)).toString('base64')]));
async function until(condition) {const end=Date.now()+4000;while(!condition()&&Date.now()<end)await new Promise(resolve=>setTimeout(resolve,5));assert.ok(condition());}

test('frontend ack waits before static removal and never edits unrelated host state', async t=>{
  const s=setup(t),before=payload(s);writeFileSync(s.shared,JSON.stringify(own(s)));
  const child=spawn('python3',s.args('uninstall'),{env:s.env,stdio:['ignore','pipe','pipe']});let stderr='';child.stderr.on('data',data=>stderr+=data);
  const exited=new Promise(resolve=>child.on('exit',resolve));
  await until(()=>JSON.parse(readFileSync(s.control)).disabled);
  assert.deepEqual(payload(s),before);assert.ok(existsSync(s.manifest));
  const acknowledged=JSON.stringify({personal:{opaque:['preserve',7]},'panorama.feed':null,'panorama.feed.owner':null});
  const complete=s.shared+'.ack';writeFileSync(complete,acknowledged);renameSync(complete,s.shared);
  assert.equal(await exited,0,stderr);assert.equal(readFileSync(s.shared,'utf8'),acknowledged);assert.equal(existsSync(s.manifest),false);
});

test('crashed cached frontend times out with all static/keys intact; explicit supported ack permits retry', t=>{
  const s=setup(t),before=payload(s),keys=readFileSync(join(s.home,'keys.json'));writeFileSync(s.shared,JSON.stringify(own(s)));
  const result=s.run('uninstall');assert.notEqual(result.status,0);assert.match(result.stderr,/Frontend stop timed out/);
  assert.deepEqual(payload(s),before);assert.deepEqual(readFileSync(join(s.home,'keys.json')),keys);assert.ok(existsSync(s.manifest));assert.equal(JSON.parse(readFileSync(s.control)).disabled,true);
  const value=own(s);value['panorama.feed']=null;value['panorama.feed.owner']=null;writeFileSync(s.shared,JSON.stringify(value));ok(s.run('uninstall'));
});

for(const mutation of ['foreign','malformed','duplicate','oversize','symlink','hardlink','invalid-owner','non-json']) {
  test(`unsafe/foreign shared ${mutation} preserves receipt and static without editing host file`, t=>{
    const s=setup(t),before=payload(s);let data=JSON.stringify(own(s));
    if(mutation==='foreign'){const value=own(s);value['panorama.feed'].install_id='f'.repeat(32);data=JSON.stringify(value);}
    if(mutation==='malformed')data='[]';
    if(mutation==='duplicate')data='{"personal":1,"personal":2}';
    if(mutation==='oversize')data=' '.repeat(8*1024*1024+1);
    if(mutation==='invalid-owner'){const value=own(s);value['panorama.feed.owner'].seen_at=true;data=JSON.stringify(value);}
    if(mutation==='non-json')data='{"personal":NaN}';
    if(['symlink','hardlink'].includes(mutation)){
      const outside=join(s.temp,'outside');writeFileSync(outside,data);(mutation==='symlink'?symlinkSync:linkSync)(outside,s.shared);
    } else writeFileSync(s.shared,data);
    const result=s.run('uninstall');assert.notEqual(result.status,0);assert.deepEqual(payload(s),before);assert.ok(existsSync(s.manifest));assert.equal(readFileSync(s.shared,'utf8'),data);assert.equal(JSON.parse(readFileSync(s.control)).disabled,true);
  });
}

for(const fault of ['late','collision','restore-eio']) {
  test(`late frontend publication ${fault} restores only removed owned static or reports exact recovery gap`, t=>{
    const s=setup(t),before=payload(s),keys=readFileSync(join(s.home,'keys.json'));
    writeFileSync(s.shared,JSON.stringify({personal:'unchanged','panorama.feed':null,'panorama.feed.owner':null}));
    const result=spawnSync('python3',[join(root,'app/tests/uninstall-fault.py'),...s.args('uninstall').slice(1)],{env:{...s.env,FRONTEND_FAULT:fault},encoding:'utf8'});
    assert.notEqual(result.status,0);assert.ok(existsSync(s.manifest));assert.equal(JSON.parse(readFileSync(s.control)).disabled,true);assert.ok(existsSync(join(s.home,'data/panorama/runtime',s.receipt.install_id,'writer.lock')));assert.deepEqual(readFileSync(join(s.home,'keys.json')),keys);
    if(fault==='late'){
      assert.match(result.stderr,/static payload restored for retry/i);assert.deepEqual(payload(s),before);
      writeFileSync(s.shared,JSON.stringify({personal:'unchanged','panorama.feed':null,'panorama.feed.owner':null}));ok(s.run('uninstall'));
    } else {
      assert.match(result.stderr,/Static recovery conflicts/);
      if(fault==='collision')assert.equal(readFileSync(join(s.home,'mods/panorama/main.js'),'utf8'),'personal replacement');
      else assert.equal(existsSync(join(s.home,'mods/panorama/main.js')),false);
      for(const [path,data]of Object.entries(before))if(path!=='mods/panorama/main.js')assert.equal(readFileSync(join(s.home,path)).toString('base64'),data);
    }
  });
}

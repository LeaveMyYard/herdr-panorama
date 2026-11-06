import test from 'node:test';
import assert from 'node:assert/strict';
import { writeFileSync, readFileSync, mkdirSync, existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { sandbox, tree } from './helpers.mjs';
function mockHc(s, source) {
  const bin=join(s.temp,'bin');mkdirSync(bin);writeFileSync(join(bin,'hc'),'#!/usr/bin/env python3\n'+source,{mode:0o755});
  return {PATH:bin+':'+process.env.PATH,TMPDIR:s.temp,CHECK_CORE_LOG:join(s.temp,'core.log')};
}

test('checker teardown never calls autostarting core status when no private endpoint exists',t=>{
  const s=sandbox(t),env=mockHc(s,"import os,sys\nwith open(os.environ['CHECK_CORE_LOG'],'a') as out:out.write(repr(sys.argv)+'\\n')\nsys.exit(99)\n");
  assert.equal(s.run('install',[],env).status,0);assert.equal(existsSync(env.CHECK_CORE_LOG),false);
  assert.equal(s.run('uninstall').status,0);
});

test('failed check with unprovable private endpoint preserves diagnostic scratch and never stops another process',t=>{
  const s=sandbox(t),before=tree(s.home);
  const source=readFileSync(s.checker,'utf8').replace("if os.environ.get('TEST_FAIL')", "import socket\nendpoint=socket.socket(socket.AF_UNIX);endpoint.bind(os.environ['XDG_RUNTIME_DIR']+'/unproven.sock')\nif os.environ.get('TEST_FAIL')");writeFileSync(s.checker,source,{mode:0o755});
  const env=mockHc(s,"import os,sys,json\nwith open(os.environ['CHECK_CORE_LOG'],'a') as out:out.write(' '.join(sys.argv[1:])+'\\n')\nassert sys.argv[1:]==['server','status','--json']\nprint(json.dumps({'pid':1,'live':0,'core':{'id':'unproven'}}))\n");
  const result=s.run('install',[],{...env,TEST_FAIL:'panorama'});
  assert.notEqual(result.status,0);assert.match(result.stderr,/Native checker cleanup refused/);
  const scratch=result.stderr.match(/private state retained at ([^:]+):/)[1];assert.ok(existsSync(scratch));
  t.after(()=>rmSync(scratch,{recursive:true,force:true}));
  assert.equal(readFileSync(env.CHECK_CORE_LOG,'utf8'),'server status --json\n');assert.deepEqual(tree(s.home),before);
});

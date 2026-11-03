import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, renameSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { sandbox, root, tree } from './helpers.mjs';
const fixture=JSON.parse(readFileSync(join(root,'tests/fixtures/app-feed-runtime/lifecycle.json')));
const feed=readFileSync(join(root,fixture.feed_fixture));
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');

for(const state of ['initial','prepared','committed']) {
  test(`merged A3 ${state} ownership fixture is accepted by the installer and restores the original tree`,t=>{
    const s=sandbox(t);writeFileSync(join(s.home,'personal.txt'),'personal');const before=tree(s.home);
    const binary=join(s.temp,'fixture-artifact');writeFileSync(binary,fixture.binary_fixture_bytes,{mode:0o755});
    const install=s.run('install',['--binary',binary]);assert.equal(install.status,0,install.stderr);
    const receipt=JSON.parse(readFileSync(s.manifest)),old=receipt.install_id;
    renameSync(join(s.home,'data/panorama/runtime',old),join(s.home,fixture.paths.generation));
    receipt.install_id=fixture.install_id;receipt.dirs=receipt.dirs.map(path=>path.replace(old,fixture.install_id));
    writeFileSync(s.manifest,JSON.stringify(receipt));writeFileSync(join(s.home,fixture.paths.control),JSON.stringify(fixture.enabled_control));
    assert.deepEqual(receipt.files[fixture.paths.binary],fixture.binary_entry);
    const ownership=fixture[`${state}_ownership`];
    writeFileSync(join(s.home,fixture.paths.ownership),JSON.stringify(ownership),{mode:0o600});
    if(state==='prepared') {assert.equal(sha(feed),ownership.pending_feed_sha256);writeFileSync(join(s.home,fixture.paths.feed_temp),feed,{mode:0o600});}
    if(state==='committed') {assert.equal(sha(feed),ownership.feed_sha256);writeFileSync(join(s.home,fixture.paths.feed),feed,{mode:0o600});}
    const result=s.run('uninstall');assert.equal(result.status,0,result.stderr);assert.deepEqual(tree(s.home),before);
    assert.equal(existsSync(join(s.home,fixture.paths.generation)),false);
  });
}

import test from 'node:test';
import assert from 'node:assert/strict';
import { metadata } from '../mods/panorama-peek/metadata.js';
test('FrameC metadata uses provided times/model/tool count without inventing absent duration',()=>{
 const now=100000000;
 const value={root:{herdr_status:'working'},agent:{status:'fail',started:now/1000-600,ended:now/1000-60,model:'sonnet',tool_calls:23}};
 assert.equal(metadata(value,'claude',now),'Failed 1m ago · Claude Code · sonnet · 9m · 23 tools');
 value.agent.ended=null;assert.equal(metadata(value,'future-harness',now),'Failed · future-harness · sonnet · 23 tools');
});

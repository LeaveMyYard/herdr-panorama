#!/usr/bin/env python3
"""Synthetic app-server. Never reads real Codex state or calls a model."""
import json
import os
import pathlib
import sys
import time

home = pathlib.Path(os.environ['CODEX_HOME'])
mode = (home / 'mode').read_text()
(home / 'pid').write_text(str(os.getpid()))
for line in sys.stdin:
    req = json.loads(line)
    with (home / 'calls').open('a') as out:
        out.write(json.dumps(req) + '\n')
    method = req['method']
    if 'id' not in req:
        continue
    params = req['params']
    if mode == 'hang':
        time.sleep(30)
    if mode == 'malformed':
        print('invalid-json', flush=True)
        continue
    if mode == 'huge':
        print('x' * (2 * 1024 * 1024 + 1), flush=True)
        continue
    if mode == 'slow':
        time.sleep(.05)
    result = {}
    error = None
    tid = params.get('threadId', 'root-a')
    if method == 'initialize':
        result = {'userAgent': 'fake/0.166.0'}
    elif method == 'thread/read':
        if (mode == 'hang-child' and tid == 'child-a') or (mode == 'hang-grand' and tid == 'grand-a'):
            time.sleep(30)
        parent = {'child-a': 'root-a', 'grand-a': 'child-a', 'foreign': 'other'}.get(tid)
        result = {'thread': {'id': 'wrong-id' if mode == 'mismatch' else tid,
            'sessionId': 'shared-tree', 'parentThreadId': parent, 'historyMode': 'legacy' if (mode in ('legacy', 'no-items-legacy') or (mode == 'mixed' and tid == 'root-a')) else 'paginated',
            'cwd': '/same/cwd', 'status': {'type': 'active'}, 'model': 'synthetic-model',
            'createdAt': 100, 'updatedAt': 200, 'name': 'Synthetic', 'turns': []}}
        # Spawned threads as codex-cli 0.166.0 describes them (spawn_shapes.json): the child
        # has a spawner-given task name; the grandchild only Codex's nickname and role.
        result['thread'].update({
            'child-a': {'agentNickname': 'Euclid', 'agentRole': 'explorer', 'source': {'subAgent': {'thread_spawn': {
                'parent_thread_id': 'root-a', 'depth': 1, 'agent_path': '/root/review_auth',
                'agent_nickname': 'Euclid', 'agent_role': 'explorer'}}}},
            'grand-a': {'name': None, 'agentNickname': 'Lovelace', 'agentRole': 'worker'},
        }.get(tid, {}))
        if mode == 'bad-parent':
            result['thread']['parentThreadId'] = {'unexpected':'parent'}
        if params.get('includeTurns'):
            result['thread']['turns'] = [{'id': 't1', 'status': 'completed', 'items': [
                {'id':'u1','type':'userMessage','content':[{'type':'text','text':'Legacy prompt'}]},
                {'id':'c1','type':'commandExecution'}, {'id':'a1','type':'agentMessage','text':'Legacy answer'}]}]
    elif method == 'thread/turns/list':
        if mode in ('legacy', 'unsupported'):
            error = {'code': -32601, 'message': 'unsupported synthetic method'}
        else:
            result = {'data': [{'id':'t2','status':'inProgress','startedAt':150,'items':[]},
                               {'id':'t1','status':'completed','startedAt':100,'completedAt':120,'items':[]}], 'nextCursor': None}
            if params.get('itemsView') == 'full':
                result['data'][0]['items'] = [{'id':'full-u','type':'userMessage','content':[{'type':'text','text':'Full turn fallback'}]},
                    {'id':'full-c','type':'commandExecution'},
                    {'id':'full-a','type':'agentMessage','text':'Full answer'}]
                if tid == 'root-a':
                    result['data'][0]['items'].append({'id':'full-spawn','type':'collabAgentToolCall','tool':'spawnAgent','receiverThreadIds':['child-a']})
    elif method == 'thread/items/list':
        if mode in ('no-items', 'no-items-legacy') or (mode == 'mixed' and tid == 'root-a'):
            error = {'code': -32601, 'message':'thread/items/list is not supported yet'}
        elif mode == 'items-fail':
            error = {'code': -32000, 'message':'synthetic error'}
        elif params.get('cursor') is None:
            items = [{'turnId':'t2','startedAtMs':151000,'item':{'id':'c2','type':'commandExecution','aggregatedOutput':'SECRET NEVER RETURNED'}}]
            ids = ['child-a', 'foreign'] if tid == 'root-a' else ['grand-a'] if tid == 'child-a' else []
            if ids:
                items.append({'turnId':'t2','item':{'id':'spawn','type':'collabAgentToolCall','tool':'spawnAgent','receiverThreadIds':ids}})
            result = {'data':items, 'nextCursor':'page-2'}
        else:
            result = {'data':[
                {'turnId':'t2','item':{'id':'c2','type':'commandExecution'}},
                {'turnId':'t2','item':{'id':'u2','type':'userMessage','content':[{'type':'text','text':'Synthetic\u001b prompt'}]}},
                {'turnId':'t1','item':{'id':'a1','type':'agentMessage','text':'Earlier answer'}},
                {'turnId':'t1','item':{'id':'m1','type':'mcpToolCall','tool':'lookup'}}],
                'nextCursor': 'page-2' if mode == 'repeat-cursor' else None}
    else:
        raise AssertionError('Mutation or unexpected RPC: ' + method)
    response = {'id':req['id'], 'error':error} if error else {'id':req['id'], 'result':result}
    print(json.dumps(response), flush=True)

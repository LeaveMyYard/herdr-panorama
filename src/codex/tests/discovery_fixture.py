#!/usr/bin/env python3
"""Bounded synthetic ancestry histories. No user state, network or model calls."""
import json,os,pathlib,sys,time
home=pathlib.Path(os.environ['CODEX_HOME'])
(home/'pid').write_text(str(os.getpid()))
for line in sys.stdin:
 r=json.loads(line)
 with (home/'calls').open('a') as f:f.write(json.dumps(r)+'\n')
 if 'id' not in r:continue
 p=r['params'];method=r['method'];mode=(home/'mode').read_text();tid=p.get('threadId','root-a');result={};error=None
 if method=='initialize':pass
 elif method=='thread/read':
  parent='other-root' if tid.startswith('bad-') else 'root-a' if tid.startswith('child-') else 'child-0' if tid=='grand' else 'other-root' if tid=='foreign' else None
  if mode=='changed-parent' and tid=='child-0':parent='other-root'
  result={'thread':{'id':tid,'parentThreadId':parent,'historyMode':'legacy' if mode=='legacy-progress' else 'paginated','createdAt':1,'updatedAt':2,'turns':[]}}
  if p.get('includeTurns'):
   old=[{'id':f'old-{i}','type':'collabAgentToolCall','receiverThreadIds':[f'child-{i-300}']} if 300<=i<306 else {'id':f'old-{i}','type':'reasoning'} for i in range(512)] if tid=='root-a' else []
   result['thread']['turns']=[{'id':'old','status':'completed','items':old}]+[{'id':f'new-{i}','status':'completed','items':[{'id':f'new-item-{i}','type':'reasoning'}]} for i in range(17)]
 elif method=='thread/turns/list':
  if mode=='legacy-progress':error={'code':-32601,'message':'synthetic unsupported'}
  elif mode in ('full-progress','full-hang'):
   if mode=='full-hang' and p.get('sortDirection')=='asc':time.sleep(30)
   old=[{'id':f'old-{i}','type':'collabAgentToolCall','receiverThreadIds':[f'child-{i-300}']} if 300<=i<306 else {'id':f'old-{i}','type':'reasoning'} for i in range(512)] if tid=='root-a' else []
   turns=[{'id':'old','status':'completed','items':old,'itemsView':'full'}]+[{'id':f'new-{i}','status':'completed','items':[{'id':f'new-item-{i}','type':'reasoning'}],'itemsView':'full'} for i in range(17)]
   if p.get('sortDirection')=='desc':turns=list(reversed(turns))
   start=int(p.get('cursor') or 0);limit=p.get('limit',16);page=turns[start:start+limit];nxt=start+len(page)
   result={'data':page,'nextCursor':str(nxt) if nxt<len(turns) else None}
  else:result={'data':[{'id':'t','status':'completed'}],'nextCursor':None}
 elif method=='thread/items/list':
  if mode in ('full-progress','full-hang'):error={'code':-32601,'message':'synthetic unsupported'}
  else:
   start=int(p.get('cursor') or 0);limit=p.get('limit',64);size=1024 if tid=='root-a' and mode not in ('recent','aged','changed-parent','scope') else 1
   if mode in ('fair','wide'):size=1
   if mode=='backfill-hang' and start>=128:time.sleep(30)
   items=[]
   for i in range(start,min(start+limit,size)):
    item={'id':f'item-{i}','type':'reasoning'}
    if tid=='root-a' and 800<=i<806:item={'id':f'spawn-{i}','type':'subAgentActivity','agentThreadId':f'child-{i-800}'}
    if tid=='root-a' and mode in ('recent','scope'):item={'id':'spawn','type':'collabAgentToolCall','receiverThreadIds':['child-0','foreign','root-a'] if mode=='scope' else ['child-0']}
    if tid=='root-a' and mode=='wide':item={'id':'many-valid','type':'collabAgentToolCall','receiverThreadIds':[f'child-{i}' for i in range(12)]}
    if tid=='root-a' and mode=='fair':item={'id':'many','type':'collabAgentToolCall','receiverThreadIds':[f'bad-{i:02}' for i in range(12)]+['child-0']}
    if tid=='root-a' and mode=='split-backfill' and i==128:item={'id':'late','type':'subAgentActivity','agentThreadId':'child-0'}
    if tid=='child-0' and mode=='scope':item={'id':'grand-ref','type':'subAgentActivity','agentThreadId':'grand'}
    items.append({'turnId':'t','item':item})
   nxt=start+len(items);result={'data':items,'nextCursor':str(nxt) if nxt<size else None}
 else:raise AssertionError('Unexpected/mutating RPC: '+method)
 encoded=json.dumps({'id':r['id'],'error':error} if error else {'id':r['id'],'result':result})
 if mode=='split-backfill' and method=='thread/items/list' and tid=='root-a' and int(p.get('cursor') or 0)>=128:
  split=len(encoded)//2;sys.stdout.write(encoded[:split]);sys.stdout.flush();time.sleep(.25);sys.stdout.write(encoded[split:]+'\n');sys.stdout.flush()
 else:print(encoded,flush=True)

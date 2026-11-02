# Offline end-to-end test: real HTTP/storage/process adapters, deterministic CLI fixtures.
import os, pathlib, tempfile, json, subprocess, time, urllib.request, http.cookiejar, uuid
ROOT=pathlib.Path(__file__).resolve().parents[1]
temporary=tempfile.TemporaryDirectory(prefix='nodo-supervision-e2e-');TMP=pathlib.Path(temporary.name)
BIN=TMP/'bin';BIN.mkdir(exist_ok=True)
FAKE='''#!/usr/bin/env python3
import sys,json,uuid,pathlib,os
prompt=sys.stdin.read(); args=sys.argv[1:]
sid=args[args.index('resume')+1] if 'resume' in args else str(uuid.uuid4())
home=pathlib.Path(os.environ['HOME']); f=home/'tmp'/('mock-'+sid+'.json')
f.parent.mkdir(parents=True,exist_ok=True)
turn=json.loads(f.read_text())+1 if f.exists() else 1
f.write_text(json.dumps(turn))
supervising='Sei il supervisore del run.' in prompt
assert ('read-only' in args)==supervising
if supervising:
 p=json.loads(prompt.split('RICHIESTA: ',1)[1].split('\\nOUTPUT:',1)[0])
 decision='answer' if p['kind'] in ['question','gate'] else 'approve'
 result=dict(status='completed',summary='Recommended from the supplied goal and evidence',details='Use stable defaults and keep the existing public contract.',decision=decision)
else:
 if 'GATE decisionale' in prompt:
  result=dict(status='completed',summary='Gate verified',decision='pass')
 elif turn==1:
  result=dict(status='needs_input',summary='Choose compatibility policy',questions=['Keep the existing public contract?'],decision='')
 else: result=dict(status='completed',summary='Applied recommended policy',decision='')
print(json.dumps(dict(type='thread.started',thread_id=sid)),flush=True)
print(json.dumps(dict(type='item.completed',item=dict(type='agent_message',text='<nodo_result>'+json.dumps(result)+'</nodo_result>'))),flush=True)
'''
(BIN/'codex').write_text(FAKE);(BIN/'codex').chmod(0o755)
ENV=os.environ|{'NODO_DATA':str(TMP/'data'),'NODO_ASSETS':str(ROOT),'NODO_LISTEN':'127.0.0.1:8083','PATH':str(BIN)+':'+os.environ['PATH'],'NODO_SECURE_COOKIE':'0'}
log=open(TMP/'server.log','w')
p=subprocess.Popen([str(ROOT/'target/debug/rightcommit')],env=ENV,stdout=log,stderr=log)
client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
def api(path,data=None,method=None):
 req=urllib.request.Request('http://127.0.0.1:8083/api'+path,data=None if data is None else json.dumps(data).encode(),method=method,headers={'Content-Type':'application/json','X-Nodo-Request':'1'})
 return json.load(client.open(req))
def wait(rid,predicate):
 for _ in range(300):
  d=api('/runs/'+rid);r=d['run']
  if predicate(r):return d
  if r['status']=='failed':raise AssertionError(r['error'])
  time.sleep(.1)
 raise AssertionError('Timeout '+json.dumps(r))
def edge(a,b,kind='control'):return {'from':a,'to':b,'kind':kind,'outcome':''}
def node(id,kind,**kw):return dict(id=id,kind=kind,title=id,x=50,y=50,**kw)
def policy(mode):return dict(mode=mode,runtime='codex',instructions='Keep the public contract stable.',max_decisions=12,max_per_node=4,allow_external_writes=False)
def workflow(mode):
 return dict(id='supervision-'+mode,name='Supervision '+mode,description='Isolated protocol integration test',entry='task',execution=policy(mode),nodes=[node('source','Source',source_type='example'),node('agent','Agent',runtime='codex'),node('task','Task',agent='agent',prompt='Check compatibility'),node('review1','Review',mode='human',prompt='Verify the report'),node('review2','Review',mode='human',prompt='Final independent review')],edges=[edge('source','agent','context'),edge('agent','task','context'),edge('task','review1'),edge('review1','review2')])
try:
 for _ in range(200):
  try:api('/auth/status');break
  except Exception:time.sleep(.1)
 credentials=dict(email='supervision-test@example.invalid',password='Isolated-supervision-2026!')
 if api('/auth/status')['setup_required']:api('/auth/register',credentials)
 else:api('/auth/login',credentials)
 results={}
 for mode in ['interactive','assisted','unattended']:
  w=workflow(mode);api('/workflows',w)
  run=api('/runs',dict(workflow_id=w['id'],input='Preserve compatibility'))
  rid=run['id'];confirmed=0
  if mode=='unattended':d=wait(rid,lambda r:r['status']=='completed')
  else:
   while True:
    d=wait(rid,lambda r:r['status']=='completed' or r['pending'] is not None and (mode=='interactive' or r['pending'].get('proposal') is not None) and r['status']!='running')
    r=d['run']
    if r['status']=='completed':break
    pending=r['pending']
    if mode=='interactive':assert pending['proposal'] is None
    else:
     assert pending['proposal']['answer']=='Use stable defaults and keep the existing public contract.'
     assert pending['proposal']['session']
    # The async worker releases its run reservation immediately after checkpointing.
    for attempt in range(30):
     try:api('/runs/'+rid+'/decide',dict(action='answer' if pending['kind']=='question' else 'approve',answer='Confirmed compatibility policy'));break
     except urllib.error.HTTPError as e:
      message=e.read().decode()
      if 'turno attivo' not in message and 'già attiva' not in message:raise
      time.sleep(.1)
    confirmed+=1
  r=d['run'];ds=d['decisions']
  assert r['sessions']['agent']['turns']==2
  assert confirmed==(0 if mode=='unattended' else 3)
  if mode=='interactive':assert 'supervisor:run' not in r['sessions']
  else:
   assert r['sessions']['supervisor:run']['turns']==3
   assert len([x for x in ds if x['actor']=='ai_recommendation'])==3
  assert len([x for x in ds if x['actor']=='ai'])==(3 if mode=='unattended' else 0)
  results[mode]=dict(run=rid,status=r['status'],confirmations=confirmed,supervisor=r['sessions'].get('supervisor:run'),decisions=len(ds))
 # Child configured Interactive must inherit Unattended, with a distinct session.
 child=workflow('interactive');child['id']='supervision-child';api('/workflows',child)
 parent=workflow('unattended');parent['id']='supervision-parent';parent['nodes'].append(node('call','Workflow',workflow_id=child['id']));parent['edges'].append(edge('review2','call'));api('/workflows',parent)
 run=api('/runs',dict(workflow_id=parent['id'],input='Run parent and child'))
 d=wait(run['id'],lambda r:r['status']=='completed');r=d['run'];c=api('/runs/'+r['children'][0]['run'])['run']
 assert c['execution']==r['execution']
 assert c['execution']['mode']=='unattended'
 assert c['sessions']['supervisor:run']['native_id']!=r['sessions']['supervisor:run']['native_id']
 assert c['workspace']==r['workspace']
 results['inheritance']=dict(parent=r['id'],child=c['id'],mode=c['execution']['mode'],separate_sessions=True)
 # Run override must not modify saved workflow defaults.
 run=api('/runs',dict(workflow_id=child['id'],input='Override',execution=policy('unattended')))
 wait(run['id'],lambda r:r['status']=='completed')
 saved=api('/workflows')
 assert next(w for w in saved if w['id']==child['id'])['execution']['mode']=='interactive'
 results['override']=True
 (ROOT/'test-results').mkdir(exist_ok=True)
 (ROOT/'test-results/supervision-e2e.json').write_text(json.dumps(results,indent=2))
 print(json.dumps(results,indent=2))
finally:
 p.terminate();p.wait(timeout=20);log.close();temporary.cleanup()

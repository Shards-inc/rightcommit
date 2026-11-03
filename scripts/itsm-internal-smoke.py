import sys,os,tempfile,subprocess,time,json,urllib.request,http.cookiejar
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'examples/itsm'))
import setup_internal
root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='itsm-native-') as data:
 env={**os.environ,'NODO_DATA':data,'NODO_LISTEN':'127.0.0.1:18247','NODO_ASSETS':str(root)}
 server=subprocess.Popen([str(root/'target/debug/rightcommit')],cwd=data,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 try:
  for _ in range(100):
   try:
    urllib.request.urlopen('http://127.0.0.1:18247/healthz');break
   except OSError:time.sleep(.1)
  req=urllib.request.Request('http://127.0.0.1:18247/api/auth/register',data=json.dumps({'email':'itsm@demo.local','password':'temporary-itsm-password'}).encode(),headers={'Content-Type':'application/json','X-Nodo-Request':'1'})
  with urllib.request.urlopen(req) as response:
   cookie=response.headers['Set-Cookie'].split(';')[0].split('=',1)[1]
  os.environ.update(RIGHTCOMMIT_URL='http://127.0.0.1:18247',RIGHTCOMMIT_SESSION=cookie)
  result=setup_internal.setup(output=data+'/output')
  assert result['chunks']>=4,result
  hits=json.loads(Path(data+'/output/retrieval-smoke.json').read_text())
  assert any('KB-DEMO-001' in h['path'] for h in hits['results']),hits
  for suffix in ['generic','northstar']:
   w=json.loads(Path(data+f'/output/workflow-{suffix}-bound.json').read_text())
   # Save native graph via the real workflow validator.
   setup_internal.api('POST','/workflows',w)
  print(json.dumps({'pass':True,**result,'bound_graphs_saved':2}))
 finally:
  server.terminate();server.wait(timeout=20)

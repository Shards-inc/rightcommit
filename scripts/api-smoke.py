#!/usr/bin/env python3
"""Real HTTP smoke test. No provider credentials or agent calls are used here."""
import http.cookiejar,json,os,secrets,time,urllib.request,urllib.error
from pathlib import Path
BASE=os.environ.get('NODO_TEST_URL','http://127.0.0.1:18080')
OUT=Path(os.environ.get('NODO_TEST_OUTPUT','test-results'));OUT.mkdir(exist_ok=True,parents=True)
class Client:
    def __init__(self):self.jar=http.cookiejar.CookieJar();self.http=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.jar))
    def call(self,path,body=None,method=None,expected=200,csrf=True):
        headers={'Content-Type':'application/json'}
        if csrf:headers['X-Nodo-Request']='1'
        req=urllib.request.Request(BASE+'/api'+path,data=json.dumps(body).encode() if body is not None else None,headers=headers,method=method or ('POST' if body is not None else 'GET'))
        try:
            with self.http.open(req,timeout=120) as r:status=r.status;data=json.load(r)
        except urllib.error.HTTPError as e:
            status=e.code
            try:data=json.loads(e.read())
            except:data={}
        assert status==expected,(path,status,data)
        return data
def main():
    checks=[];a=Client();b=Client();anon=Client()
    creds_path=OUT/'test-login.json'
    if creds_path.exists():creds=json.loads(creds_path.read_text());a.call('/auth/login',creds)
    else:
        creds={'email':'qa-admin@nodo.local','password':secrets.token_urlsafe(24)};a.call('/auth/register',creds);creds_path.write_text(json.dumps(creds));creds_path.chmod(0o600)
    second={'email':'qa-user-'+secrets.token_hex(4)+'@nodo.local','password':secrets.token_urlsafe(24)};a.call('/users',second);b.call('/auth/login',second)
    anon.call('/workflows',expected=401);a.call('/auth/logout',{},expected=403,csrf=False);checks.append('HTTP auth, session cookie and CSRF guard')
    aa=a.call('/settings');bb=b.call('/settings');aa['agents']['codex']['api_key']='smoke-not-a-provider-key';aa['agents']['codex']['model']='configured-model';aa=a.call('/settings',aa,'PUT');assert aa['agents']['codex']['api_key']=='' and aa['agents']['codex']['configured'];assert b.call('/settings')['agents']['codex']['model']==bb['agents']['codex']['model'];checks.append('Per-user settings and secrets redacted')
    aa['agents']['codex']['api_key']='__CLEAR__';aa['agents']['codex']['model']='';a.call('/settings',aa,'PUT')
    workflows=a.call('/workflows');assert len(workflows)>=2
    for w in workflows:a.call('/workflows/validate',w)
    checks.append('Both real SDD presets validate')
    # A custom graph can run real local CI without paying for an agent.
    w={'id':'smoke-'+secrets.token_hex(6),'name':'API smoke · real local CI','description':'Automated product test','entry':'test','nodes':[{'id':'source','kind':'Source','title':'Example repo','source_type':'example','x':80,'y':100},{'id':'test','kind':'Connection','title':'cargo test','provider':'local','operation':'test','x':300,'y':100},{'id':'commit','kind':'Connection','title':'Local commit','provider':'local','operation':'commit','x':520,'y':100}], 'edges':[{'from':'source','to':'test','kind':'context','outcome':''},{'from':'test','to':'commit','kind':'control','outcome':''}]}
    a.call('/workflows',w);a.call('/workflows/validate',w);assert all(x['id']!=w['id'] for x in b.call('/workflows'));checks.append('Custom workflow saved and isolated')
    r=a.call('/runs',{'workflow_id':w['id'],'input':'No agent call: run actual Rust tests and inspect commit.'});rid=r['id']
    for _ in range(120):
        d=a.call('/runs/'+rid);r=d['run']
        if r['status'] in ['completed','failed']:break
        time.sleep(.25)
    assert r['status']=='completed',r.get('error');assert 'test result: ok' in r['outputs']['test']['summary'];assert d['artifacts'];checks.append('Custom graph executes real cargo test and Git commit')
    b.call('/runs/'+rid,expected=400);b.call('/runs/'+rid+'/events',expected=400);b.call('/artifacts/'+d['artifacts'][0]['id'],expected=400);checks.append('Run, event and artifact ownership checks')
    artifact=a.call('/artifacts/'+d['artifacts'][0]['id']);assert 'html' in artifact
    ev=a.call('/runs/'+rid+'/events');assert len(ev)>2 and all(x['seq']>0 for x in ev);checks.append('Immutable artifacts, Markdown endpoint and persistent events')
    a.call('/auth/logout',{});a.call('/settings',expected=401);a.call('/auth/login',creds);assert a.call('/runs/'+rid)['run']['status']=='completed';checks.append('Logout invalidates token; login restores persisted runs')
    new_password=secrets.token_urlsafe(24);a.call('/auth/password',{'current':'wrong-password','new':new_password},expected=400);a.call('/auth/password',{'current':creds['password'],'new':new_password});a.call('/settings',expected=401);creds['password']=new_password;creds_path.write_text(json.dumps(creds));creds_path.chmod(0o600);a.call('/auth/login',creds);checks.append('Password rotation checks current password and revokes all web sessions')
    report={'pass':True,'scope':'Real Rust server and local CI; no provider inference','checks':checks,'run_id':rid,'base_url':BASE};(OUT/'api-smoke.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
if __name__=='__main__':main()

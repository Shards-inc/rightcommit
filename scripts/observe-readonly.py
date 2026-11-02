#!/usr/bin/env python3
"""Observe an actual running review and probe its OS permissions, no inference."""
import importlib,json,os,sys,subprocess,time
from pathlib import Path
sys.path.insert(0,str(Path(__file__).parent));os.environ['NODO_TEST_URL']='http://127.0.0.1:18081'
m=importlib.import_module('api-smoke');c=m.Client();c.call('/auth/login',json.loads(Path('test-results/container/test-login.json').read_text()))
out=Path('test-results/readonly-review.json');previous=json.loads(out.read_text()) if out.exists() else {'attempts':[]}
deadline=time.time()+1800
while time.time()<deadline:
    try:
        live=json.loads(Path('test-results/live-workflows.json').read_text())
        for item in live.get('runs',[]):
            r=c.call('/runs/'+item['run'])['run'];n=next(n for n in r['workflow']['nodes'] if n['id']==r['current'])
            if r['status']!='running' or n['kind'] not in ['Review','Gate']:continue
            owner=subprocess.run(['docker','exec','nodo-qa','stat','-c','%u',r['workspace']],capture_output=True,text=True).stdout.strip()
            if owner!='0':continue
            probe=subprocess.run(['docker','exec','--user','20001:20001','nodo-qa','sh','-c','test -r "$1/src/lib.rs" && test ! -w "$1/src/lib.rs" && test ! -w "$1" && test ! -r /data/system/nodo.sqlite','_',r['workspace']],capture_output=True)
            if probe.returncode==0:
                previous['attempts'].append({'run':r['id'],'node':n['id'],'observed_unix_time':time.time(),'workspace_owner':0,'tested_uid':20001,'source_readable':True,'workspace_and_source_not_writable':True,'server_database_not_readable':True,'probe_exit_code':0})
                previous.update(pass_=True,status='observed_actual_native_review');previous['pass']=previous.pop('pass_');out.write_text(json.dumps(previous,indent=2));print('ACTUAL_READ_ONLY_REVIEW_PROBE_PASS',n['id'],flush=True);sys.exit(0)
    except Exception:pass
    time.sleep(.5)
previous.update(status='not_observed_before_deadline');out.write_text(json.dumps(previous,indent=2));sys.exit(1)

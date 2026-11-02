#!/usr/bin/env python3
"""Opt-in paid/inference test. Uses an already configured NODO account only.

Never imports host auth. Automatic approvals are labeled as test decisions and
are restricted to the two local sample workflows; external writes are refused.
"""
import argparse,http.cookiejar,json,os,time,urllib.request,urllib.error
from pathlib import Path

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--run',action='store_true',help='Explicitly run both real-agent workflows');p.add_argument('--url',default='http://127.0.0.1:18080');p.add_argument('--credentials',default='test-results/test-login.json');p.add_argument('--model',default='');p.add_argument('--resume-run',default='');p.add_argument('--resume-incident',default='');args=p.parse_args()
    if not args.run:p.error('Use --run only after configuring a real provider for this account.')
    jar=http.cookiejar.CookieJar();op=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    def api(path,body=None,method=None):
        req=urllib.request.Request(args.url+'/api'+path,data=json.dumps(body).encode() if body is not None else None,headers={'Content-Type':'application/json','X-Nodo-Request':'1'},method=method or ('POST' if body is not None else 'GET'))
        try:
            with op.open(req,timeout=180) as r:return json.load(r)
        except urllib.error.HTTPError as e:raise RuntimeError(f'{path}: {e.code} {e.read().decode()[:1000]}')
    api('/auth/login',json.loads(Path(args.credentials).read_text()))
    settings=api('/settings')
    if args.model:
        settings['agents']['codex']['model']=args.model;settings['agents']['codex']['effort']='low';api('/settings',settings,'PUT')
    runtime=settings['default_runtime'];assert settings['agents'][runtime].get('configured'),f'Configure {runtime} credentials in the personal settings first'
    report={'scope':'Real provider execution of two local sample workflows; automated test-account approvals, no production or external approval','started':time.time(),'runs':[]};out=Path('test-results/live-workflows.json')
    policy='''Automated verification on Northstar Helpdesk only. Rust library, no new dependencies or services. One feature directory specs/001-ticket-dedup. V1: exact case-sensitive fingerprint matching, retain oldest/lower ID, sum occurrences and preserve oldest priority. Priorities are integers 1..5 (1 most urgent); invalid input must be documented and tested. Do not add auth, persistence, APIs or a UI to this library. V2 incident is an explicitly requested specification change: minimum numeric priority wins; keep V1 ID and occurrence rules. Add P1-into-P3, equal-priority and existing regression tests. For remaining reversible technical decisions choose the smallest implementation and document it. Report necessary questions with needs_input. Do not create external PRs or deploy.'''
    base=None
    try:
        for wf in ['feature-sdd','incident-sdd']:
            current_policy=policy+(' THIS RUN IS V1 ONLY. V2 is a future unapproved change reserved for the second workflow; do not require or implement V2 tasks in the first run.' if wf=='feature-sdd' else ' THIS RUN IS V2. Update the existing V1 living spec after the business-approval step, then implement the minimum-priority regression.')
            existing=args.resume_run if wf=='feature-sdd' else args.resume_incident
            r=api('/runs/'+existing)['run'] if existing else api('/runs',{'workflow_id':wf,'input':current_policy,'base_run':base});rid=r['id'];
            # Creation already launches the runner; its returned snapshot may still be paused.
            if existing and r['status']=='paused':api('/runs/'+rid+'/start',{'acknowledge':False})
            entry={'workflow':wf,'run':rid,'transitions':[]};report['runs'].append(entry);last=None;deadline=time.time()+5400;question_counts={}
            while time.time()<deadline:
                d=api('/runs/'+rid);r=d['run'];key=(r['current'],r['status'])
                if key!=last:
                    print(wf,rid[:8],r['current'],r['status'],flush=True);entry['transitions'].append({'node':r['current'],'status':r['status'],'time':time.time()});last=key;out.write_text(json.dumps(report,indent=2))
                if r['status']=='completed':break
                if r['status'] in ['failed','interrupted']:raise RuntimeError(r.get('error') or r['status'])
                pending=r.get('pending')
                if pending:
                    question_counts[pending['node']]=question_counts.get(pending['node'],0)+1
                    if question_counts[pending['node']]>3:raise RuntimeError('Repeated question requires a specific operator answer: '+str(pending['questions']))
                    if pending['kind']=='effect':raise RuntimeError('External effect refused in automatic test')
                    action='answer' if pending['kind'] in ['question','gate'] else 'approve'
                    # Explicit test decision, not a human/customer approval.
                    answer='AUTOMATED TEST ACCOUNT DECISION on local fixture only: I explicitly approve the current requirements/checklist review for this fixture. This does not claim unexecuted tests, does not waive real defects and does not approve external writes. '+current_policy
                    time.sleep(.3);api('/runs/'+rid+'/decide',{'action':action,'answer':answer})
                time.sleep(2)
            assert r['status']=='completed','Workflow timeout'
            assert r['sessions']['author']['turns']>=7
            assert 'test result: ok' in r['outputs']['tests']['summary']
            artifacts=d['artifacts'];assert artifacts and all(a['hash'] for a in artifacts)
            entry.update(status=r['status'],sessions=r['sessions'],artifact_count=len(artifacts),test_summary=r['outputs']['tests']['summary'],commit_summary=r['outputs']['commit']['summary'])
            if wf=='incident-sdd':
                assert r['sessions']['triage-agent']['native_id']!=r['sessions']['author']['native_id'];assert 'handoff' in r['outputs']
                early={'spec','plan','checklist','tasks','analyze','design-review','ready'}
                premature=[a for a in artifacts if a['node'] in early and (a['path'].startswith(('src/','tests/')) or a['path'] in ['Cargo.toml','Cargo.lock'])]
                assert not premature,'Implementation changed before the design approval'
                entry['no_source_changes_before_implementation']=True
            base=rid
        report['pass']=True
    except Exception as e:report['pass']=False;report['error']=str(e);raise
    finally:report['finished']=time.time();out.write_text(json.dumps(report,indent=2))
if __name__=='__main__':main()

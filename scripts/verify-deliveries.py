#!/usr/bin/env python3
"""Read-only export and independent behavioral checks of the two completed QA runs."""
import hashlib, importlib, json, os, subprocess, sys
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(__file__).parent))
os.environ['NODO_TEST_URL'] = 'http://127.0.0.1:18081'
api = importlib.import_module('api-smoke')
c = api.Client()
c.call('/auth/login', json.loads(Path('test-results/container/test-login.json').read_text()))
live = json.loads(Path('test-results/live-workflows.json').read_text())
assert live.get('pass'), 'Complete both real workflows before exporting delivery evidence'
result = {'pass': False, 'scope': 'Independent Rust checks, no model inference or provider credentials', 'deliveries': []}
rust = r'''
use northstar_helpdesk::{Ticket, merge_duplicate};
fn t(id:u64, priority:u8, occurrences:u32)->Ticket { Ticket{id, priority, occurrences, fingerprint:"mail-down".into()} }
#[test] fn requested_v1_v2_change_in_both_orders() {
    let (a,b)=(t(10,3,2),t(20,1,4)); let snapshot=(a.clone(),b.clone());
    for (x,y) in [(&a,&b),(&b,&a)] {
        let m=merge_duplicate(x,y); assert_eq!(m.id,10); assert_eq!(m.priority,EXPECTED);
        assert_eq!(m.occurrences,6); assert_eq!(m.fingerprint,"mail-down");
    }
    assert_eq!((a,b),snapshot);
}
#[test] fn equal_priority_regression() {
    let (a,b)=(t(1,2,0),t(2,2,3)); let m=merge_duplicate(&a,&b);
    assert_eq!(m,t(1,2,3)); assert_eq!(merge_duplicate(&b,&a),m);
}
#[test] fn invalid_priority_on_either_argument_is_rejected() {
    for p in [0,6] { assert!(std::panic::catch_unwind(||merge_duplicate(&t(1,p,1),&t(2,3,1))).is_err()); assert!(std::panic::catch_unwind(||merge_duplicate(&t(1,3,1),&t(2,p,1))).is_err()); }
}
#[test] fn exact_fingerprint_and_overflow_regressions() {
    let mut other=t(2,1,1); other.fingerprint="Mail-down".into();
    assert!(std::panic::catch_unwind(||merge_duplicate(&t(1,3,1),&other)).is_err());
    assert!(std::panic::catch_unwind(||merge_duplicate(&t(1,3,u32::MAX),&t(2,1,1))).is_err());
}
'''
runner = '''import json,pathlib,subprocess,sys,os
d=json.load(sys.stdin);p=pathlib.Path('/data/users/1/verification')/d['version'];(p/'src').mkdir(parents=True,exist_ok=True)
(p/'Cargo.toml').write_text('[package]\\nname="nodo-independent-check"\\nversion="0.1.0"\\nedition="2024"\\n[dependencies]\\nnorthstar-helpdesk={path='+json.dumps(d['workspace'])+'}\\n')
(p/'src/lib.rs').write_text(d['rust'])
env=dict(os.environ,HOME='/data/users/1',CARGO_HOME='/data/users/1/.cargo')
r=subprocess.run(['cargo','test','--offline','--manifest-path',str(p/'Cargo.toml')],env=env,capture_output=True,text=True)
print(r.stdout+r.stderr);sys.exit(r.returncode)
'''
for index, item in enumerate(live['runs']):
    d = c.call('/runs/' + item['run']); r = d['run']; assert r['status'] == 'completed'
    destination = Path('test-results/deliveries') / ('v1' if index == 0 else 'v2'); destination.mkdir(parents=True, exist_ok=True)
    latest = {a['path']: a for a in d['artifacts']}
    manifest = []
    for name, meta in latest.items():
        path = PurePosixPath(name)
        assert not path.is_absolute() and '..' not in path.parts
        if not (name.startswith(('src/', 'specs/', '.specify/memory/', '.rightcommit/reports/', '.nodo/reports/')) or name in ['Cargo.toml', 'Cargo.lock']): continue
        a = c.call('/artifacts/' + meta['id'])['artifact']
        assert hashlib.sha256(a['content'].encode()).hexdigest() == a['hash']
        output = destination / name; output.parent.mkdir(parents=True, exist_ok=True); output.write_text(a['content'])
        manifest.append(meta)
    (destination/'manifest.json').write_text(json.dumps(manifest,indent=2))
    (destination/'decisions.json').write_text(json.dumps(d['decisions'],indent=2))
    early = {'spec','clarify','plan','checklist','tasks','analyze','design-review','ready'}
    assert not [a for a in d['artifacts'] if a['node'] in early and (a['path'].startswith(('src/','tests/')) or a['path'] in ['Cargo.toml','Cargo.lock'])], 'Code changed before implementation'
    payload = {'version':f'v{index+1}', 'workspace':r['workspace'], 'rust':f'#![cfg(test)]\nconst EXPECTED:u8={3 if index==0 else 1};\n'+rust}
    checked = subprocess.run(['docker','exec','-i','--user','20001:20001','nodo-qa','python3','-c',runner], input=json.dumps(payload), capture_output=True, text=True)
    log = checked.stdout + checked.stderr; (destination/'independent-tests.log').write_text(log)
    assert checked.returncode == 0, log
    assert '4 passed; 0 failed' in log, log
    result['deliveries'].append({'run':r['id'],'workflow':item['workflow'],'independent_tests_passed':4,'no_source_changes_before_implementation':True,'native_sessions':r['sessions'],'commit':r['outputs']['commit']['summary'],'artifact_manifest':str(destination/'manifest.json')})
result['pass'] = True
Path('test-results/delivery-verification.json').write_text(json.dumps(result,indent=2))
print(json.dumps(result,indent=2))

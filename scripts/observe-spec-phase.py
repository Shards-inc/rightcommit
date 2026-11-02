#!/usr/bin/env python3
"""Probe real Spec Kit phase permissions and compare source bytes with V1."""
import importlib, json, os, subprocess, sys, time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
os.environ['NODO_TEST_URL'] = 'http://127.0.0.1:18081'
api = importlib.import_module('api-smoke')
c = api.Client()
c.call('/auth/login', json.loads(Path('test-results/container/test-login.json').read_text()))
deadline = time.time() + 1800
while time.time() < deadline:
    report = json.loads(Path('test-results/live-workflows.json').read_text())
    if len(report['runs']) == 2:
        r = c.call('/runs/' + report['runs'][1]['run'])['run']
        if r['status'] == 'running' and r['current'] in ['spec', 'plan', 'checklist', 'tasks']:
            workspace = r['workspace']
            probe = subprocess.run(['docker', 'exec', '--user', '20001:20001', 'nodo-qa', 'python3', '-c', '''
import pathlib, sys
p=pathlib.Path(sys.argv[1]); base=pathlib.Path(sys.argv[2]); source=p/'src/lib.rs'
assert source.read_bytes()==(base/'src/lib.rs').read_bytes(), 'Code changed before implementation'
try:
    with source.open('r+b'): pass
except PermissionError: pass
else: raise AssertionError('Source is writable during a specification phase')
q=p/'specs/.nodo-permission-probe'
try: q.write_text('permission probe')
finally: q.unlink(missing_ok=True)
print('PASS')
''', workspace, '/data/users/1/workspaces/' + report['runs'][0]['run']], capture_output=True, text=True)
            after = c.call('/runs/' + r['id'])['run']
            if probe.returncode == 0 and after['current'] == r['current']:
                result = {'pass': True, 'run': r['id'], 'node': r['current'], 'code_matches_completed_v1': True, 'actual_source_open_for_write_denied': True, 'actual_document_create_and_remove_succeeded': True, 'uid': 20001}
                Path('test-results/spec-phase-permissions.json').write_text(json.dumps(result, indent=2))
                print(json.dumps(result), flush=True)
                sys.exit(0)
            if 'Code changed' in probe.stderr:
                raise RuntimeError(probe.stderr)
    time.sleep(1)
raise RuntimeError('No specification phase observed before timeout')

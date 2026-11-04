"""Offline two-user HTTP/process credential isolation checks. No provider requests."""
import concurrent.futures
import http.cookiejar
import json
import os
from pathlib import Path
import runpy
import socket
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = runpy.run_path(str(ROOT / 'scripts/check-agent-environment.py'))['FIXTURE']
FIXTURE = FIXTURE.replace("'CODEX_API_'", "'CODEX_'")
FIXTURE = FIXTURE.replace("if rt=='codex':", "result=result.replace('host-openai-secret', ' '.join(v for k,v in os.environ.items() if k.endswith('_API_KEY')))\nif rt=='codex':")
FIXTURE = FIXTURE.replace("or k=='UNRELATED_SECRET'", "or k in ['UNRELATED_SECRET', 'HOME', 'CODEX_HOME', 'CLAUDE_CONFIG_DIR']")


def check(tokens_only=False):
    with tempfile.TemporaryDirectory(prefix='rightcommit-credentials-') as directory:
        directory = Path(directory)
        bins = directory / 'bin'
        bins.mkdir()
        for runtime in ['claude', 'codex', 'pi']:
            executable = bins / runtime
            executable.write_text(FIXTURE)
            executable.chmod(0o755)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        prefixes = ('ANTHROPIC_', 'CLAUDE_CODE_', 'CODEX_', 'OPENAI_', 'AWS_', 'AZURE_', 'GOOGLE_', 'NODO_')
        env = {key: value for key, value in os.environ.items() if not key.startswith(prefixes)}
        env.update(NODO_DATA=str(directory / 'data'), NODO_ASSETS=str(ROOT),
                   NODO_LISTEN=f'127.0.0.1:{port}', NODO_ISOLATE_USERS='0',
                   PATH=str(bins) + ':' + os.environ['PATH'],
                   CLAUDE_CODE_OAUTH_TOKEN='dummy-admin-claude-session',
                   CODEX_ACCESS_TOKEN='dummy-admin-codex-session',
                   ANTHROPIC_OAUTH_TOKEN='dummy-forbidden-pi-token',
                   ANTHROPIC_BASE_URL='https://dummy-company-gateway.invalid',
                   ANTHROPIC_MODEL='dummy-server-model',
                   AWS_ACCESS_KEY_ID='dummy-company-access', AWS_SECRET_ACCESS_KEY='dummy-company-cloud',
                   CLAUDE_CODE_USE_BEDROCK='1')
        if not tokens_only:
            env.update(ANTHROPIC_API_KEY='dummy-company-anthropic', CODEX_API_KEY='dummy-company-codex')
        base = f'http://127.0.0.1:{port}/api'

        def client():
            opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
            def api(path, body=None, method=None):
                request = urllib.request.Request(base + path,
                    data=json.dumps(body).encode() if body is not None else None, method=method,
                    headers={'Content-Type': 'application/json', 'X-Nodo-Request': '1'})
                return json.load(opener.open(request, timeout=20))
            return api

        admin, member = client(), client()
        log = open(directory / 'server.log', 'w')
        server = subprocess.Popen([str(ROOT / 'target/debug/rightcommit')], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(150):
                if server.poll() is not None:
                    raise AssertionError('Server stopped unexpectedly')
                try:
                    admin('/auth/status')
                    break
                except OSError:
                    time.sleep(.1)
            admin('/auth/register', {'email': 'admin@test.invalid', 'password': 'temporary-password-admin'})
            admin('/users', {'email': 'member@test.invalid', 'password': 'temporary-password-member'})
            member('/auth/login', {'email': 'member@test.invalid', 'password': 'temporary-password-member'})
            assert 'instance_mode' not in admin('/auth/me')
            assert 'instance_mode' not in member('/auth/me')

            def execute(api, uid, runtime, personal, phase, inherited=None):
                settings = api('/settings')
                settings = api('/settings', {'revision': settings['_revision'], 'changes': {'agents': {
                    runtime: {'api_key': personal or '__CLEAR__', 'model': 'offline-fixture-model'}}}}, 'PATCH')
                assert set(settings['agents']) == {'claude', 'codex', 'pi'}
                assert settings['agents'][runtime]['uses_admin_api_key'] == bool(inherited)
                if inherited:
                    assert not settings['agents'][runtime]['api_key_preview']
                    assert inherited not in json.dumps(settings)
                assert personal not in json.dumps(settings) if personal else True
                wid = f'u{uid}-{runtime}-{phase}'
                workflow = dict(id=wid, name=wid, description='Offline credential check', entry='task',
                    nodes=[dict(id='source', kind='Source', source_type='example'),
                           dict(id='agent', kind='Agent', runtime=runtime),
                           dict(id='task', kind='Task', agent='agent', prompt='Offline credential test')],
                    edges=[dict(**{'from': 'source', 'to': 'agent'}, kind='context'),
                           dict(**{'from': 'agent', 'to': 'task'}, kind='context')])
                api('/workflows', workflow)
                run = api('/runs', {'workflow_id': wid, 'input': 'Offline test'})
                for _ in range(200):
                    result = api('/runs/' + run['id'])
                    if result['run']['status'] in ['completed', 'failed']:
                        break
                    time.sleep(.03)
                assert result['run']['status'] == 'completed', result['run'].get('error')
                captured = json.loads((directory / f'data/users/{uid}/tmp/captured-env.json').read_text())
                assert 'dummy-forbidden-pi-token' not in json.dumps(captured)
                assert Path(captured['HOME']).resolve() == (directory / f'data/users/{uid}').resolve()
                assert Path(captured['CODEX_HOME']).resolve() == (directory / f'data/users/{uid}/.codex').resolve()
                assert Path(captured['CLAUDE_CONFIG_DIR']).resolve() == (directory / f'data/users/{uid}/.claude').resolve()
                if personal or inherited:
                    key = 'CODEX_API_KEY' if runtime == 'codex' else 'ANTHROPIC_API_KEY'
                    assert captured[key] == (personal or inherited)
                    assert not any(key in captured for key in ['CLAUDE_CODE_OAUTH_TOKEN', 'CODEX_ACCESS_TOKEN', 'ANTHROPIC_AUTH_TOKEN'])
                elif uid == 2:
                    assert not any(key in captured for key in ['CLAUDE_CODE_OAUTH_TOKEN', 'CODEX_ACCESS_TOKEN'])
                    if not tokens_only and runtime == 'pi':
                        key = 'CODEX_API_KEY' if runtime == 'codex' else 'ANTHROPIC_API_KEY'
                        assert captured[key] == ('dummy-company-codex' if runtime == 'codex' else 'dummy-company-anthropic')
                    if runtime in ['claude', 'codex']:
                        assert not any(key in captured for key in ['ANTHROPIC_API_KEY', 'CODEX_API_KEY', 'OPENAI_API_KEY', 'AWS_SECRET_ACCESS_KEY', 'AWS_ACCESS_KEY_ID', 'CLAUDE_CODE_USE_BEDROCK', 'ANTHROPIC_MODEL', 'ANTHROPIC_BASE_URL', 'OPENAI_BASE_URL'])
                for secret in [inherited, personal, 'dummy-admin-claude-session', 'dummy-admin-codex-session', 'dummy-company-anthropic', 'dummy-company-codex']:
                    if secret:
                        assert secret not in json.dumps(result)
                return captured

            # A private native login is available to each user without distributing sessions.
            for uid in [1, 2]:
                home = directory / f'data/users/{uid}'
                (home / '.claude/.credentials.json').write_text(json.dumps({'owner': uid}))
                (home / '.codex/config.toml').write_text('model_provider = \"openai\"\n')
                (home / '.codex/auth.json').write_text(json.dumps({'tokens': {'access_token': f'dummy-private-u{uid}'}}))
            for runtime in ['claude', 'codex', 'pi']:
                # Save the admin key first; the member's personal key must still override it.
                execute(admin, 1, runtime, f'dummy-private-key-u1-{runtime}', 'admin-setup')
                with concurrent.futures.ThreadPoolExecutor(2) as pool:
                    futures = [pool.submit(execute, api, uid, runtime, f'dummy-private-key-u{uid}-{runtime}', 'personal')
                               for uid, api in [(1, admin), (2, member)]]
                    for future in futures:
                        future.result()
                shared = f'dummy-private-key-u1-{runtime}'
                captured = execute(member, 2, runtime, '', 'shared', inherited=shared)
                admin_settings = admin('/settings')
                admin('/settings', {'revision': admin_settings['_revision'], 'changes': {'agents': {runtime: {'api_key': '__CLEAR__'}}}}, 'PATCH')
                captured = execute(member, 2, runtime, '', 'fallback')
                assert not any('u1-' in value for value in captured.values())
            # Clearing native Pi OAuth is mandatory, even if a usable API key is supplied.
            auth = directory / 'data/users/2/.pi/agent/auth.json'
            oauth = {'anthropic': {'type': 'oauth', 'access': 'dummy-pi-access', 'refresh': 'dummy-pi-refresh', 'expires': 1}}
            auth.write_text(json.dumps(oauth))
            settings = member('/settings')
            settings = member('/settings', {'revision': settings['_revision'], 'changes': {'agents': {
                'pi': {'provider': 'anthropic', 'api_key': 'dummy-pi-api', 'model': 'offline-fixture-model'}}}}, 'PATCH')
            assert not settings['agents']['pi']['configured']
            assert json.loads(auth.read_text()) == oauth
            # Encrypted personal keys are not present in persisted database files.
            for file in (directory / 'data/system').glob('nodo.sqlite*'):
                assert b'dummy-private-key-u' not in file.read_bytes()
            print('PASS: two users, concurrent Claude/Codex processes, personal keys, '
                  + ('private-login fallback and admin-token isolation' if tokens_only else 'organizational API fallback')
                  + ', encrypted storage, no secret API leakage, all runtimes and Pi OAuth guard.')
        finally:
            server.terminate()
            server.wait(timeout=20)
            log.close()


if __name__ == '__main__':
    check()
    check(tokens_only=True)

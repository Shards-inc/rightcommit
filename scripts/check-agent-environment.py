"""Offline provider environment checks using real HTTP/process adapters and fake CLIs."""
import http.cookiejar
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
KEYS = ['ANTHROPIC_AUTH_TOKEN', 'CLAUDE_CODE_OAUTH_TOKEN', 'OPENAI_API_KEY', 'CODEX_API_KEY', 'ANTHROPIC_API_KEY', 'OPENROUTER_API_KEY',
        'AWS_BEARER_TOKEN_BEDROCK', 'AWS_ACCESS_KEY_ID', 'AWS_SECRET_ACCESS_KEY', 'AWS_SESSION_TOKEN']
MODELS = {
    'ANTHROPIC_DEFAULT_OPUS_MODEL': 'eu.anthropic.claude-opus-5[1M]',
    'ANTHROPIC_DEFAULT_SONNET_MODEL': 'eu.anthropic.claude-sonnet-5[1M]',
    'ANTHROPIC_DEFAULT_HAIKU_MODEL': 'eu.anthropic.claude-haiku-4-5-20251001-v1:0',
    'ANTHROPIC_DEFAULT_OPUS_MODEL_NAME': 'Claude Opus 5 - Bedrock EU',
    'ANTHROPIC_DEFAULT_SONNET_MODEL_NAME': 'Claude Sonnet 5 - Bedrock EU',
    'ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME': 'Claude Haiku 4.5 - Bedrock EU',
}
FIXTURE = '''#!/usr/bin/env python3
import json, os, pathlib, sys, uuid
sys.stdin.read()
rt=pathlib.Path(sys.argv[0]).name
home=pathlib.Path(os.environ['HOME'])
captured={k:v for k,v in os.environ.items() if k.startswith(('AWS_', 'ANTHROPIC_', 'OPENAI_', 'OPENROUTER_', 'CODEX_API_', 'CLAUDE_CODE_')) or k=='UNRELATED_SECRET'}
(home/'tmp'/'captured-env.json').write_text(json.dumps(captured))
args=sys.argv[1:]
sid=args[args.index('--session-id')+1] if '--session-id' in args else str(uuid.uuid4())
result='<nodo_result>'+json.dumps(dict(status='completed', summary='host-openai-secret host-bedrock-secret' + (' host-oauth-secret' if os.environ.get('CLAUDE_CODE_OAUTH_TOKEN') else '') + (' host-gateway-secret' if os.environ.get('ANTHROPIC_AUTH_TOKEN') else '') + (' host-header-secret' if os.environ.get('ANTHROPIC_CUSTOM_HEADERS') else ''), artifacts=[], questions=[]))+'</nodo_result>'
if rt=='codex':
 assert '--ignore-user-config' not in args
 print(json.dumps(dict(type='thread.started',thread_id=sid)))
 print(json.dumps(dict(type='item.completed',item=dict(type='agent_message',text=result))))
elif rt=='claude':
 print(json.dumps(dict(type='system',subtype='init',session_id=sid)))
 print(json.dumps(dict(type='result',result=result,is_error=False)))
else:
 print(json.dumps(dict(type='session',id=sid)))
 print(json.dumps(dict(type='message_end',message=dict(role='assistant',content=[dict(type='text',text=result)]))))
'''


def check(bedrock, oauth=False, gateway=False):
    with tempfile.TemporaryDirectory(prefix='nodo-provider-env-') as tmp:
        tmp = Path(tmp)
        bins = tmp / 'bin'
        bins.mkdir()
        for runtime in ['codex', 'claude', 'pi']:
            f = bins / runtime
            f.write_text(FIXTURE)
            f.chmod(0o755)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        env = {k: v for k, v in os.environ.items() if not k.startswith(('ANTHROPIC_', 'CLAUDE_CODE_', 'AWS_', 'AZURE_', 'GOOGLE_', 'OPENAI_', 'OPENROUTER_')) and k not in KEYS}
        env.update(NODO_DATA=str(tmp / 'data'), NODO_ASSETS=str(ROOT),
                   NODO_LISTEN=f'127.0.0.1:{port}', NODO_ISOLATE_USERS='0',
                   PATH=str(bins) + ':' + os.environ['PATH'],
                   OPENAI_API_KEY='host-openai-secret', ANTHROPIC_API_KEY='host-anthropic-secret',
                   OPENROUTER_API_KEY='host-openrouter-secret', CODEX_API_KEY='',
                   CLAUDE_CODE_USE_BEDROCK='1' if bedrock else '0',
                   AWS_BEARER_TOKEN_BEDROCK='host-bedrock-secret',
                   AWS_REGION='eu-central-1', AWS_DEFAULT_REGION='eu-central-1',
                   UNRELATED_SECRET='must-not-inherit')
        env.update(MODELS)
        if oauth:
            env['CLAUDE_CODE_OAUTH_TOKEN'] = 'host-oauth-secret'
        if gateway:
            env.update(ANTHROPIC_AUTH_TOKEN='host-gateway-secret', ANTHROPIC_BASE_URL='https://gateway.example/anthropic', ANTHROPIC_MODEL='gateway-model', ANTHROPIC_CUSTOM_HEADERS='X-Diagnostic: host-header-secret')
        # Also verify the Compose host -> container boundary, without printing its values.
        config = json.loads(subprocess.check_output(
            ['docker', 'compose', 'config', '--format', 'json'], cwd=ROOT, env=env))
        for key in ['OPENAI_API_KEY', 'CLAUDE_CODE_USE_BEDROCK', 'AWS_BEARER_TOKEN_BEDROCK', *MODELS]:
            assert config['services']['nodo']['environment'][key] == env[key], key
        client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

        def api(path, body=None, method=None):
            request = urllib.request.Request(f'http://127.0.0.1:{port}/api{path}',
                data=json.dumps(body).encode() if body is not None else None, method=method,
                headers={'Content-Type': 'application/json', 'X-Nodo-Request': '1'})
            return json.load(client.open(request, timeout=10))

        with open(tmp / 'server.log', 'w') as log:
            server = subprocess.Popen([str(ROOT / 'target/debug/rightcommit')], env=env, stdout=log, stderr=log)
            try:
                for _ in range(100):
                    try:
                        api('/auth/status')
                        break
                    except OSError:
                        time.sleep(.1)
                api('/auth/register', dict(email='provider-env@example.invalid', password='temporary-provider-env-password'))
                settings = api('/settings')
                assert all(settings['agents'][rt]['configured'] for rt in ['codex', 'claude', 'pi'])
                assert not any(secret in json.dumps(settings) for secret in [env[k] for k in KEYS if env.get(k)])
                cases = [('codex', 'anthropic', ''), ('claude', 'anthropic', ''),
                         ('pi', 'anthropic', ''), ('pi', 'openai', ''), ('pi', 'openrouter', ''),
                         ('codex', 'anthropic', 'personal-secret'), ('claude', 'anthropic', 'personal-secret')]
                for i, (runtime, provider, personal) in enumerate(cases):
                    settings['agents'][runtime].update(provider=provider, api_key=personal)
                    settings = api('/settings', settings, 'PUT')
                    wid = f'provider-env-{i}'
                    nodes = [dict(id='source', kind='Source', title='Source', source_type='example'),
                             dict(id='agent', kind='Agent', title='Agent', runtime=runtime),
                             dict(id='task', kind='Task', title='Task', agent='agent', prompt='Offline environment test')]
                    edges = [dict(**{'from': 'source', 'to': 'agent'}, kind='context'),
                             dict(**{'from': 'agent', 'to': 'task'}, kind='context')]
                    api('/workflows', dict(id=wid, name=wid, description='Offline provider environment check', entry='task', nodes=nodes, edges=edges))
                    rid = api('/runs', dict(workflow_id=wid, input='Check environment'))['id']
                    for _ in range(200):
                        result = api('/runs/' + rid)
                        if result['run']['status'] in ['completed', 'failed']:
                            break
                        time.sleep(.05)
                    assert result['run']['status'] == 'completed', result['run'].get('error')
                    captured = json.loads((tmp / 'data/users/1/tmp/captured-env.json').read_text())
                    assert 'UNRELATED_SECRET' not in captured
                    if runtime == 'claude' and gateway and not bedrock:
                        assert captured['ANTHROPIC_AUTH_TOKEN'] == 'host-gateway-secret'
                        assert captured['ANTHROPIC_BASE_URL'] == env['ANTHROPIC_BASE_URL']
                        assert captured['ANTHROPIC_MODEL'] == 'gateway-model'
                        assert not any(key in captured for key in ['ANTHROPIC_API_KEY', 'OPENAI_API_KEY', 'CODEX_API_KEY', 'CLAUDE_CODE_OAUTH_TOKEN'])
                    elif runtime == 'claude' and oauth and not bedrock:
                        assert captured['CLAUDE_CODE_OAUTH_TOKEN'] == 'host-oauth-secret'
                        assert not any(key in captured for key in ['ANTHROPIC_API_KEY', 'OPENAI_API_KEY', 'CODEX_API_KEY'])
                    elif runtime == 'claude' and bedrock:
                        assert captured['AWS_BEARER_TOKEN_BEDROCK'] == env['AWS_BEARER_TOKEN_BEDROCK']
                        assert not any(key in captured for key in ['ANTHROPIC_API_KEY', 'OPENAI_API_KEY', 'CODEX_API_KEY', 'CLAUDE_CODE_OAUTH_TOKEN'])
                        for key, value in MODELS.items():
                            assert captured[key] == value
                    else:
                        key = 'CODEX_API_KEY' if runtime == 'codex' else ('ANTHROPIC_API_KEY' if runtime == 'claude' else {'anthropic': 'ANTHROPIC_API_KEY', 'openai': 'OPENAI_API_KEY', 'openrouter': 'OPENROUTER_API_KEY'}[provider])
                        expected = personal or env['OPENAI_API_KEY' if runtime == 'codex' else key]
                        assert captured[key] == expected, (runtime, provider)
                        assert not any(k.startswith('AWS_') for k in captured)
                        assert len([k for k in captured if k.endswith('_API_KEY')]) == 1
                    for secret in ['host-openai-secret', 'host-bedrock-secret', 'host-oauth-secret', 'host-gateway-secret', 'host-header-secret']:
                        assert secret not in json.dumps(result), 'Unredacted secret in run API'
                print(f'PASS: {"Bedrock" if bedrock else ("gateway" if gateway else ("OAuth" if oauth else "direct"))}: Compose, three runtimes, Pi providers, personal precedence, configured state and log redaction')
            finally:
                server.terminate()
                server.wait(timeout=20)


if __name__ == '__main__':
    check(False)
    check(False, oauth=True)
    check(False, oauth=True, gateway=True)
    check(True, oauth=True)

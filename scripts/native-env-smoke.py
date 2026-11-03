"""Native startup checks use temporary .env files and dummy credentials only."""
import http.cookiejar
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import urllib.request

BINARY = Path('target/debug/rightcommit').resolve()


def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def base_environment():
    return {key: value for key, value in os.environ.items()
            if key not in {'NODO_DATA', 'NODO_LISTEN', 'NODO_SOURCES', 'OPENAI_API_KEY'}}


def run_server(cwd, env, listen, check):
    process = subprocess.Popen([str(BINARY)], cwd=cwd, env=env,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    base = f'http://127.0.0.1:{listen}'
    try:
        for _ in range(150):
            if process.poll() is not None:
                raise AssertionError('Native server stopped before becoming ready')
            try:
                if urllib.request.urlopen(base + '/healthz', timeout=1).status == 200:
                    break
            except OSError:
                time.sleep(.1)
        else:
            raise AssertionError('Native server did not become ready')
        check(base)
    finally:
        process.send_signal(signal.SIGINT)
        try:
            stdout, stderr = process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            stdout, stderr = process.communicate()
        assert 'dotenv-dummy # literal' not in stdout + stderr
        assert 'external-dummy-key' not in stdout + stderr


def provider_check(data, expected):
    def check(base):
        assert (data / 'system/nodo.sqlite').exists()
        tool = data / 'system/tools/bin/pi'
        tool.parent.mkdir(parents=True, exist_ok=True)
        tool.write_text('#!/usr/bin/env python3\nimport os,json\n'
                        f'assert os.environ.get("OPENAI_API_KEY") == {expected!r}\n'
                        'print(json.dumps({"type":"message_end","message":{"role":"assistant",'
                        '"stopReason":"stop","content":[{"type":"text","text":"OK"}]}}))\n')
        tool.chmod(0o700)
        opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

        def api(path, body=None, method=None):
            request = urllib.request.Request(base + '/api' + path,
                data=json.dumps(body).encode() if body is not None else None,
                headers={'Content-Type': 'application/json', 'X-Nodo-Request': '1'}, method=method)
            with opener.open(request, timeout=10) as response:
                return json.load(response)

        api('/auth/register', {'email': 'native-env@test.local', 'password': 'temporary-native-env-password'})
        settings = api('/settings')
        saved = api('/settings', {'revision': settings['_revision'], 'changes': {'agents': {'pi': {
            'provider': 'openai', 'model': 'native-env-model'}}}}, 'PATCH')
        assert saved['agents']['pi']['configured']
        assert api('/agents/pi/status', {'provider': 'openai', 'model': 'native-env-model'})['connected']
    return check


with tempfile.TemporaryDirectory(prefix='rightcommit-native-env-') as temporary:
    root = Path(temporary)
    first = root / 'file-start';first.mkdir();listen = port();data = first / 'dotenv data'
    (first / '.env').write_text(f"NODO_DATA='{data}'\nNODO_LISTEN=127.0.0.1:{listen}\n"
                               "export OPENAI_API_KEY='dotenv-dummy # literal'\n")
    run_server(first, base_environment(), listen, provider_check(data, 'dotenv-dummy # literal'))

    second = root / 'override';second.mkdir();listen = port();data = second / 'process-data'
    (second / '.env').write_text(f'NODO_DATA={second / "unused-data"}\nNODO_LISTEN=invalid-address\n'
                                'OPENAI_API_KEY=dotenv-dummy-key\n')
    env = base_environment() | {'NODO_DATA': str(data), 'NODO_LISTEN': f'127.0.0.1:{listen}',
                               'OPENAI_API_KEY': 'external-dummy-key'}
    run_server(second, env, listen, provider_check(data, 'external-dummy-key'))
    assert not (second / 'unused-data').exists()

    parent = root / 'parent';parent.mkdir();child = parent / 'child';child.mkdir();listen = port()
    (parent / '.env').write_text('OPENAI_API_KEY="parent-secret-unclosed\n')
    run_server(child, base_environment() | {'NODO_DATA': str(child / 'data'),
               'NODO_LISTEN': f'127.0.0.1:{listen}'}, listen, lambda base: None)

    blanks = root / 'empty-placeholders';blanks.mkdir();listen = port()
    (blanks / '.env').write_text('NODO_DATA=\nSSL_CERT_FILE=\nSSL_CERT_DIR=\nOPENAI_API_KEY=\n')
    run_server(blanks, base_environment() | {'NODO_LISTEN': f'127.0.0.1:{listen}'}, listen,
               lambda base: None)
    assert (blanks / 'data/system/nodo.sqlite').exists()

    invalid = root / 'invalid';invalid.mkdir()
    (invalid / '.env').write_text(f'NODO_DATA={invalid / "must-not-exist"}\n'
                                  'OPENAI_API_KEY=private-first-key\nXAI_API_KEY="private-unclosed-key\n')
    failed = subprocess.run([str(BINARY)], cwd=invalid, env=base_environment(),
                            capture_output=True, text=True, timeout=10)
    assert failed.returncode != 0
    assert 'File .env non valido' in failed.stderr
    assert 'private-first-key' not in failed.stdout + failed.stderr
    assert 'private-unclosed-key' not in failed.stdout + failed.stderr
    assert not (invalid / 'must-not-exist').exists()
    assert subprocess.run([str(BINARY), '--version'], cwd=invalid, env=base_environment(),
                          capture_output=True, timeout=10).returncode == 0
    assert subprocess.run([str(BINARY), '--help'], cwd=invalid, env=base_environment(),
                          capture_output=True, timeout=10).returncode == 0

print('PASS: native .env startup, quoted/exported keys reach Pi, existing environment wins, optional exact-directory file, sanitized parse errors before initialization and utility flags remain usable')

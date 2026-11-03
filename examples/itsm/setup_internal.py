#!/usr/bin/env python3
"""Index the bundled KB using rightcommit's own engine and bind its personal ID."""
import argparse
import json
import os
from pathlib import Path
import re
import time
from urllib.parse import urlsplit
from itsm import http, read, save, ROOT


def api(method, path, body=None):
    base = os.environ.get('RIGHTCOMMIT_URL', 'http://127.0.0.1:8080').rstrip('/')
    parsed = urlsplit(base)
    if parsed.username or parsed.password or not (parsed.scheme == 'https' or
            parsed.scheme == 'http' and parsed.hostname in ('localhost', '127.0.0.1')):
        raise ValueError('Use HTTPS or a loopback rightcommit server')
    session = os.environ['RIGHTCOMMIT_SESSION']
    if not re.fullmatch('[0-9a-f]{64}', session):
        raise ValueError('Invalid rightcommit session')
    return http(method, base + '/api' + path, body,
                {'Cookie': 'nodo_session=' + session, 'X-Nodo-Request': '1'})


def bind(workflow, index):
    w = json.loads(json.dumps(workflow))
    w['nodes'] = [n for n in w['nodes'] if n['id'] not in ('rag-setup','kb-rag')]
    w['nodes'].append({'id':'kb-rag', 'kind':'RAG', 'title':'Internal ITSM KB',
                       'rag_index':index['id'], 'rag_mode':index['config']['retrieval'],
                       'rag_top_k':3, 'x':40, 'y':530})
    w['edges'] = [e for e in w['edges'] if e['from'] != 'kb-rag']
    w['edges'].append({'from':'kb-rag','to':'agent','kind':'context','outcome':''})
    return w


def setup(mode='keyword', root='examples', path='itsm/kb', output='output'):
    index = api('POST', '/rag', {'root':root,'path':path,'config': {
        'strategy':'markdown', 'retrieval':mode, 'top_k':3, 'extensions':['md'],
        'max_size':1500, 'model':'multilingual-minilm'}})
    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        index = api('GET', '/rag/' + index['id'])
        state = index['progress']['state']
        if state not in ('queued','running','cancelling'):
            break
        time.sleep(0.5)
    if not index.get('generation') or index['progress']['state'] != 'completed':
        raise RuntimeError('Index not completed: ' + json.dumps(index['progress']))
    save(Path(output)/'internal-index.json', index)
    for suffix in ['generic','northstar']:
        save(Path(output)/f'workflow-{suffix}-bound.json', bind(read(ROOT/f'workflow-{suffix}.json'),index))
    # Real retrieval evidence; classification is left to Pi, not this helper.
    result = api('POST', '/rag/'+index['id']+'/search', {'query':'VPN', 'mode':mode, 'top_k':3})
    save(Path(output)/'retrieval-smoke.json',result)
    return {'index':index['id'], 'generation':index['generation'], 'chunks':index['stats']['chunks'],
            'retrieved':len(result['results']), 'workflows':str(output)}


if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--mode',choices=['keyword','hybrid','semantic'],default='keyword')
    p.add_argument('--root',default='examples',choices=['examples','sources'])
    p.add_argument('--path',default='itsm/kb')
    p.add_argument('--output',default='output')
    args=p.parse_args()
    print(json.dumps(setup(args.mode,args.root,args.path,args.output),indent=2))

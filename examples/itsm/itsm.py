#!/usr/bin/env python3
"""ITSM conversion: stdlib only, Pi classification and rightcommit internal RAG, PDI adapter."""
import argparse
import json
from pathlib import Path
import re
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent


def read(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def http(method, url, body=None, headers=None):
    request = urllib.request.Request(url, data=None if body is None else json.dumps(body).encode(),
                                     headers={'Content-Type': 'application/json', **(headers or {})}, method=method)
    try:
        with urllib.request.build_opener(NoRedirect).open(request, timeout=40) as response:
            return json.load(response)
    except urllib.error.HTTPError as e:
        # Never echo response bodies, which may contain ticket data or credentials.
        raise RuntimeError(f'HTTP {e.code} on {method} {urllib.parse.urlsplit(url).path}') from None


def plan(ticket, analysis):
    text = ticket['chatInput'].strip()
    if not text or len(text) > 12000:
        raise ValueError('chatInput must contain 1..12000 characters')
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,80}', ticket['request_id']):
        raise ValueError('Invalid request_id')
    if analysis.get('request_id') != ticket['request_id']:
        raise ValueError('Pi analysis belongs to another ticket')
    category = analysis['category']
    if category not in ('Incident', 'Request', 'Other'):
        raise ValueError('Invalid Pi classification')
    retrieval = analysis['retrieval']
    if not retrieval.get('index') or not retrieval.get('generation'):
        raise ValueError('Internal RAG index and generation provenance required')
    hits = retrieval['results']
    for hit in hits:
        if not hit.get('id') or not hit.get('path') or not hit.get('text'):
            raise ValueError('Invalid internal RAG chunk')
    answer = '\n\n'.join(f"[{h['path']}#{h['id']}] {h['text']}" for h in hits) or 'Nessuna risposta trovata nella KB; inoltrare al service desk.'
    return {'catalog_route':ticket.get('catalog_route',''),'request_fields':ticket.get('fields',{}),
            'request_id': ticket['request_id'], 'category': category, 'short_description': text[:160],
            'description': text, 'answer': answer, 'citations': [h['path'] + '#' + h['id'] for h in hits],
            'status': 'resolution_proposed' if hits else 'needs_human', 'backend': 'rightcommit-internal',
            'rag_index': retrieval['index'], 'rag_generation': retrieval['generation'],
            'action': 'create_incident' if category == 'Incident' else 'order_catalog' if category == 'Request' else 'answer_only'}


def batch(manifest, analyses, configuration=None):
    namespace=manifest.get('namespace','')
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,80}',namespace):
        raise ValueError('A stable namespace is required')
    if configuration is not None:
        if configuration.get('schema')!='rightcommit.servicenow.catalog.v1' or configuration.get('namespace')!=namespace:
            raise ValueError('Catalog configuration namespace/schema differs from the batch')
    tickets=manifest.get('tickets',[])
    if not isinstance(tickets,list) or not 1<=len(tickets)<=100:
        raise ValueError('A batch requires 1..100 tickets')
    ids=[ticket['request_id'] for ticket in tickets]
    if len(set(ids))!=len(ids):
        raise ValueError('Duplicate request_id in batch')
    if not isinstance(analyses,dict) or set(analyses)!=set(ids):
        raise ValueError('Provide exactly one Pi analysis for every ticket')
    return {'schema':'rightcommit.itsm.batch.v1','namespace':namespace,
            'tickets':[plan(ticket,analyses[ticket['request_id']]) for ticket in tickets]}


def resolution(created, evidence, close_code='Solution provided', request_id=None):
    operation=created.get('operation')
    if operation=='apply_ticket_batch':
        if not request_id:
            raise ValueError('Select the request_id explicitly for batch resolution')
        candidates=[item for item in created['result']['items'] if item['request_id']==request_id
                    and item.get('action')=='create_incident' and item.get('status') in ('applied','reused')
                    and str(item.get('result',{}).get('state','')) not in ('6','7')]
        if len(candidates)!=1 or not created['result'].get('namespace'):
            raise ValueError('Select an applied/reused Incident from this native batch')
        result=candidates[0]['result']
    elif operation in ('create_incident','apply_ticket_plan'):
        result=created['result']
        if not result.get('correlation_id','').startswith('rightcommit:'+created.get('source_run','')+':'):
            raise ValueError('Created incident receipt must belong to this run')
    else:
        raise ValueError('Use the native incident creation receipt, never check_connection')
    if not re.fullmatch(r'[0-9a-f]{32}',result.get('sys_id','')) or not re.fullmatch(r'INC[0-9]+',result.get('number','')):
        raise ValueError('Invalid created incident receipt')
    run=created.get('source_run','')
    if not run or not created.get('creation_node'):
        raise ValueError('Native run and node provenance are required')
    if not evidence.strip() or len(evidence)>24000:
        raise ValueError('Human verification evidence is required')
    value={'source_run':run,'creation_node':created['creation_node'],'sys_id':result['sys_id'],
           'number':result['number'],'verified':True,'close_code':close_code,'close_notes':evidence.strip()}
    if operation=='apply_ticket_batch':
        value.update(request_id=request_id,namespace=created['result']['namespace'])
    return value


def main():
    parser = argparse.ArgumentParser(description='Prepare an ITSM plan from Pi and internal RAG evidence. ServiceNow actions use the app connector.')
    parser.add_argument('command', choices=['plan','batch','resolution'])
    parser.add_argument('--analysis', default='output/pi-analysis.json')
    parser.add_argument('--ticket', default=str(ROOT / 'tickets/incident.json'))
    parser.add_argument('--out')
    parser.add_argument('--created', default='output/servicenow-apply-plan.json')
    parser.add_argument('--evidence-file', default='output/resolution-evidence.txt')
    parser.add_argument('--close-code', default='Solution provided')
    parser.add_argument('--request-id')
    parser.add_argument('--batch', default=str(ROOT/'tickets/batch.json'))
    parser.add_argument('--analyses', default='output/batch-analysis.json')
    parser.add_argument('--config')
    args = parser.parse_args()
    if args.command=='resolution':
        result=resolution(read(args.created),Path(args.evidence_file).read_text(),args.close_code,args.request_id)
    elif args.command=='batch':
        result=batch(read(args.batch),read(args.analyses),read(args.config) if args.config else None)
    else:
        result=plan(read(args.ticket),read(args.analysis))
    destination=Path(args.out or ('output/resolution.json' if args.command=='resolution' else 'output/batch-plan.json' if args.command=='batch' else 'output/plan.json'))
    if args.command=='batch':
        for ticket in result['tickets']:
            save(destination.parent/'plans'/(ticket['request_id']+'.json'),ticket)
    save(destination, result)
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()

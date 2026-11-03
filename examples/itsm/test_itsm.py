import json
import hashlib
import sys
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import itsm


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.index = None

    def analysis(self, name='incident'):
        category = {'incident':'Incident','request':'Request','faq':'Other','unknown':'Other'}[name]
        article = {'incident':'001','request':'002','faq':'003','unknown':None}[name]
        hits = [] if article is None else [{'id':'chunk-'+article,'path':'KB-DEMO-'+article+'.md',
            'text':(itsm.ROOT/'kb'/('KB-DEMO-'+article+'.md')).read_text()}]
        return {'request_id':'demo-'+name+'-001','category':category,
                'retrieval':{'index':'test-index','generation':'test-generation','results':hits}}

    def test_routes_and_grounding(self):
        for name, expected, action in [('incident','Incident','create_incident'),('request','Request','order_catalog'),
                                      ('faq','Other','answer_only'),('unknown','Other','answer_only')]:
            result = itsm.plan(itsm.read(itsm.ROOT/f'tickets/{name}.json'),self.analysis(name))
            self.assertEqual((result['category'],result['action']),(expected,action))
            self.assertEqual(result['backend'],'rightcommit-internal')
            self.assertEqual(bool(result['citations']),name != 'unknown')
            self.assertNotEqual(result['status'],'resolved')

    def test_personal_index_binding(self):
        from setup_internal import bind
        for path in itsm.ROOT.glob('workflow-*.json'):
            w=bind(itsm.read(path),{'id':'personal-id','config':{'retrieval':'keyword'}})
            self.assertEqual(next(n for n in w['nodes'] if n['kind']=='Agent')['runtime'],'pi')
            self.assertEqual(next(n for n in w['nodes'] if n['kind']=='RAG')['rag_index'],'personal-id')
            self.assertIn({'from':'kb-rag','to':'agent','kind':'context','outcome':''},w['edges'])

    def test_invalid_classification_and_provenance(self):
        ticket=itsm.read(itsm.ROOT/'tickets/incident.json')
        for key,value in [('category','execute'),('request_id','wrong')]:
            analysis=self.analysis();analysis[key]=value
            with self.assertRaises(ValueError):itsm.plan(ticket,analysis)
        analysis=self.analysis();del analysis['retrieval']['generation']
        with self.assertRaises(ValueError):itsm.plan(ticket,analysis)

    def test_batch_contract_matches_templates(self):
        for path in itsm.ROOT.glob('workflow-*.json'):
            w=itsm.read(path)
            classify=next(n for n in w['nodes'] if n['id']=='classify')['prompt']
            apply=next(n for n in w['nodes'] if n['id']=='apply-plan')
            self.assertEqual(apply['operation'],'apply_ticket_batch')
            self.assertEqual(apply['location'],'output/batch-plan.json')
            self.assertIn('--out '+apply['location'],classify)
            self.assertIn('EVERY request_id',classify)

    def test_batch_covers_every_ticket_and_rejects_missing_or_duplicate_analyses(self):
        manifest=itsm.read(itsm.ROOT/'tickets/batch.json')
        analyses={self.analysis(name)['request_id']:self.analysis(name) for name in ['incident','request','faq','unknown']}
        result=itsm.batch(manifest,analyses)
        self.assertEqual(len(result['tickets']),4)
        self.assertEqual([p['action'] for p in result['tickets']],['create_incident','order_catalog','answer_only','answer_only'])
        self.assertEqual(result['tickets'][-1]['status'],'needs_human')
        missing=dict(analyses);missing.pop('demo-request-001')
        with self.assertRaises(ValueError):itsm.batch(manifest,missing)
        duplicate=json.loads(json.dumps(manifest));duplicate['tickets'].append(duplicate['tickets'][0])
        with self.assertRaises(ValueError):itsm.batch(duplicate,analyses)
        changed=json.loads(json.dumps(manifest));changed['namespace']='../bad'
        with self.assertRaises(ValueError):itsm.batch(changed,analyses)

    def test_batch_resolution_requires_exact_incident_request_id(self):
        created={'source_run':'current-run','creation_node':'apply-plan','operation':'apply_ticket_batch',
                 'result':{'namespace':'northstar-demo','items':[{'request_id':'vpn','action':'create_incident','status':'reused',
                    'result':{'sys_id':'b'*32,'number':'INC0010002','correlation_id':'rightcommit:old-run:id'}}]}}
        result=itsm.resolution(created,'User verification',request_id='vpn')
        self.assertEqual(result['sys_id'],'b'*32)
        self.assertEqual(result['source_run'],'current-run')
        self.assertEqual(result['request_id'],'vpn')
        for request in [None,'another-request']:
            with self.assertRaises(ValueError):itsm.resolution(created,'Evidence',request_id=request)

    def test_resolution_uses_native_created_incident_and_requires_evidence(self):
        receipt={'source_run':'run-demo','creation_node':'apply-plan','operation':'apply_ticket_plan',
                 'result':{'sys_id':'b'*32,'number':'INC0010002','correlation_id':'rightcommit:run-demo:demo'}}
        result=itsm.resolution(receipt,'PDI simulated test: user confirmed VPN connection succeeds')
        self.assertEqual(result['sys_id'],'b'*32)
        self.assertEqual(result['number'],'INC0010002')
        self.assertEqual(result['creation_node'],'apply-plan')
        for key,value in [('operation','check_connection'),('source_run','other-run')]:
            bad=json.loads(json.dumps(receipt));bad[key]=value
            with self.assertRaises(ValueError):itsm.resolution(bad,'Evidence')
        with self.assertRaises(ValueError):itsm.resolution(receipt,' ')
        for path in itsm.ROOT.glob('workflow-*.json'):
            workflow=itsm.read(path)
            resolve=next(n for n in workflow['nodes'] if n['id']=='resolve-ticket')
            self.assertEqual(resolve['target'],'apply-plan')
            prompt=next(n for n in workflow['nodes'] if n['id']=='prepare-resolution')['prompt']
            self.assertIn('python3 itsm.py resolution --created output/servicenow-apply-plan.json',prompt)

    def test_import_graphs(self):
        for path in itsm.ROOT.glob('workflow-*.json'):
            w = itsm.read(path)
            ids = {n['id'] for n in w['nodes']}
            self.assertEqual(len(ids), len(w['nodes']))
            self.assertIn(w['entry'], ids)
            for edge in w['edges']:
                self.assertIn(edge['from'], ids)
                self.assertIn(edge['to'], ids)


if __name__ == '__main__':
    unittest.main()

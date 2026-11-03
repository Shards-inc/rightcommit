import copy,json,tempfile,unittest
from pathlib import Path
import request_batch,prepare_spec
ROOT=Path(__file__).resolve().parent
class ExamplesTests(unittest.TestCase):
    def test_request_routes_and_dedup_ids_are_preserved(self):
        config=json.loads((ROOT/'config/catalog-routing.json').read_text());rows=json.loads((ROOT/'requests.json').read_text())
        result=request_batch.build(config,rows);self.assertEqual(len(result['tickets']),2)
        self.assertNotEqual(result['tickets'][0]['catalog_route'],result['tickets'][1]['catalog_route'])
        self.assertEqual(result['tickets'][0]['request_fields']['software_name'],'Python')
        duplicate=copy.deepcopy(rows);duplicate['tickets'].append(duplicate['tickets'][0])
        with self.assertRaises(ValueError):request_batch.build(config,duplicate)
        wrong=copy.deepcopy(rows);wrong['tickets'][0]['catalog_route']='unknown'
        with self.assertRaises(ValueError):request_batch.build(config,wrong)
    def test_fixture_provenance_and_source_mapping_are_explicit(self):
        config=json.loads((ROOT/'fixtures/specification-config.json').read_text());snapshot=json.loads((ROOT/'fixtures/native-snapshot.example.json').read_text())
        with tempfile.TemporaryDirectory() as folder:
            output=Path(folder)/'output';doc=prepare_spec.prepare(config,snapshot,output)
            self.assertIn('OFFLINE FIXTURE',(output/'requirement-brief.md').read_text())
            self.assertEqual(doc['provenance']['sys_id'],snapshot['record']['sys_id'])
            self.assertEqual(doc['fields']['business_goal']['source'],'/variables/business_goal')
            self.assertEqual(doc['feature_directory'],'specs/001-software-approval-portal')
    def test_missing_fields_and_path_traversal_are_rejected(self):
        config=json.loads((ROOT/'fixtures/specification-config.json').read_text());bad=copy.deepcopy(config);bad['specification']['feature_slug']='../../outside'
        with self.assertRaises(ValueError):prepare_spec.feature(bad)
        snapshot=json.loads((ROOT/'fixtures/native-snapshot.example.json').read_text());snapshot['record']['description']=''
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(ValueError):prepare_spec.prepare(config,snapshot,Path(folder))
    def test_live_catalog_field_mapping_keeps_provenance_and_missing_requirements(self):
        config=json.loads((ROOT/'fixtures/specification-config.json').read_text())
        config['source']['mode']='servicenow'
        config['specification']['feature_slug']='live-catalog-request'
        config['specification']['mapping'].update(title='/record/cat_item.name',description='/record/cat_item.description')
        snapshot=json.loads((ROOT/'fixtures/native-snapshot.example.json').read_text())
        snapshot['provenance'].update(mode='servicenow',variables_status='read')
        snapshot['record'].update(short_description='',description='')
        snapshot['record']['cat_item.name']={'value':'Standard Laptop','display_value':'Standard Laptop'}
        snapshot['record']['cat_item.description']={'value':'Catalog hardware description','display_value':'Catalog hardware description'}
        snapshot['variables']={}
        with tempfile.TemporaryDirectory() as folder:
            doc=prepare_spec.prepare(config,snapshot,Path(folder))
            self.assertEqual(doc['fields']['title']['value'],'Standard Laptop')
            self.assertEqual(doc['feature_directory'],'specs/001-live-catalog-request')
            self.assertEqual(doc['provenance']['mode'],'servicenow')
            self.assertEqual(set(doc['missing']),{'business_goal','acceptance_criteria','constraints'})
    def test_oauth_workflow_is_readonly_and_requires_native_auth_evidence(self):
        workflow=json.loads((ROOT/'workflow-oauth-check.json').read_text())
        self.assertEqual(workflow['entry'],'check-servicenow')
        self.assertEqual([n.get('operation') for n in workflow['nodes'] if n['kind']=='Connection'],['check_connection'])
        prompt=next(n['prompt'] for n in workflow['nodes'] if n['id']=='report')
        self.assertIn('client_credentials',prompt)
        self.assertIn('blocked',prompt)
        self.assertIn('output/oauth-check.md',prompt)
    def test_workflows_separate_catalog_writes_from_readonly_specification(self):
        req=json.loads((ROOT/'workflow-requests.json').read_text());spec=json.loads((ROOT/'workflow-request-to-speckit.json').read_text())
        self.assertTrue(any(n.get('operation')=='apply_ticket_batch' for n in req['nodes']))
        self.assertEqual([n.get('operation') for n in spec['nodes'] if n['kind']=='Connection'],['read_request'])
        self.assertTrue(any(n.get('command')=='specify' for n in spec['nodes']))
        for w in [req,spec]:
            ids={n['id'] for n in w['nodes']}
            self.assertIn(w['entry'],ids)
            for e in w['edges']:self.assertIn(e['from'],ids);self.assertIn(e['to'],ids)
        provenance=json.loads((ROOT/'SPECKIT_PROVENANCE.json').read_text())
        import hashlib
        for name,digest in provenance['commands'].items():self.assertEqual(hashlib.sha256((ROOT/'.pi/prompts'/name).read_bytes()).hexdigest(),digest)
if __name__=='__main__':unittest.main()

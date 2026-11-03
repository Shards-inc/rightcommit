import json
import tempfile
import unittest
from pathlib import Path
from capture_evidence import capture, verify, NODES


class EvidenceTest(unittest.TestCase):
    def test_exact_snapshots_and_provenance_then_tampering_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for node in NODES:
                source = root / (node + '-native.json')
                raw = json.dumps({'source': node, 'data': [{'path': 'README.md', 'lines_added': 13, 'lines_removed': 38}]}).encode()
                source.write_bytes(raw)
                manifest = capture(source, node, root)
                self.assertEqual((root / manifest['snapshot']).read_bytes(), raw)
                self.assertEqual(manifest['native_file'], str(source.resolve()))
            self.assertEqual(len(verify(root)), 3)
            (root / 'output/atlassian-evidence/jira.json').write_text('{}')
            with self.assertRaises(ValueError):
                verify(root)

    def test_missing_or_invalid_evidence_is_not_synthesized(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(FileNotFoundError):
                capture(root / 'missing.json', 'jira', root)
            invalid = root / 'invalid.json'
            invalid.write_text('not JSON')
            with self.assertRaises(json.JSONDecodeError):
                capture(invalid, 'jira', root)
            self.assertFalse((root / 'output/atlassian-evidence/jira.json').exists())


if __name__ == '__main__':
    unittest.main()

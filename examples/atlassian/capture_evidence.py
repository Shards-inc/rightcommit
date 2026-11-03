#!/usr/bin/env python3
"""Preserve an explicitly supplied native TWG JSON response for this run."""
import argparse
import hashlib
import json
from pathlib import Path

NODES = {"jira": "jira.json", "bitbucket": "bitbucket.json", "diffstat": "diffstat.json"}


def capture(source, node, root=Path('.')):
    source = Path(source).resolve(strict=True)
    raw = source.read_bytes()
    if len(raw) > 2 * 1024 * 1024:
        raise ValueError('Native evidence exceeds 2 MiB')
    json.loads(raw)  # Require JSON, not a compact summary or a guessed record.
    folder = root / 'output' / 'atlassian-evidence'
    folder.mkdir(parents=True, exist_ok=True)
    target = folder / NODES[node]
    target.write_bytes(raw)
    manifest = {'source_node': node, 'native_file': str(source),
                'snapshot': target.relative_to(root).as_posix(),
                'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw)}
    target.with_suffix('.provenance.json').write_text(json.dumps(manifest, indent=2) + '\n')
    return manifest


def verify(root=Path('.')):
    result = []
    for node, filename in NODES.items():
        target = root / 'output' / 'atlassian-evidence' / filename
        raw = target.read_bytes()
        json.loads(raw)
        manifest = json.loads(target.with_suffix('.provenance.json').read_text())
        if (manifest['source_node'] != node or manifest['snapshot'] != target.relative_to(root).as_posix()
                or manifest['sha256'] != hashlib.sha256(raw).hexdigest() or manifest['bytes'] != len(raw)):
            raise ValueError('Evidence integrity mismatch: ' + node)
        result.append(manifest)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    cap = sub.add_parser('capture')
    cap.add_argument('--node', choices=NODES, required=True)
    cap.add_argument('--source', required=True, help='Exact output_files.stdout path from the immediately preceding native node')
    sub.add_parser('verify')
    args = parser.parse_args()
    value = capture(args.source, args.node) if args.action == 'capture' else verify()
    print(json.dumps(value, indent=2))

#!/usr/bin/env python3
"""Snapshot tracked runtime resources; --check detects stale release input."""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
paths = subprocess.check_output(
    ["git", "ls-files", "-z", "examples", "pi-extensions"], cwd=root
).decode().split("\0")
files = {}
for name in sorted(filter(None, paths)):
    path = root / name
    if path.is_symlink():
        raise SystemExit(f"Symlink not supported: {name}")
    files[name] = {"text": path.read_text(), "executable": bool(path.stat().st_mode & 0o111)}
output = json.dumps(files, ensure_ascii=False, indent=2) + "\n"
dest = root / "distribution/assets.json"
if "--check" in sys.argv:
    if not dest.exists() or dest.read_text() != output:
        raise SystemExit("Bundled resources are stale: run python3 scripts/bundle-assets.py")
else:
    dest.parent.mkdir(exist_ok=True)
    dest.write_text(output)

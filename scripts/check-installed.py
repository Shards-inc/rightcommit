#!/usr/bin/env python3
"""Smoke-test an executable away from its source/build tree, without AI calls."""
from pathlib import Path
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="rightcommit-installed-") as tmp:
    work = Path(tmp)
    installed = work / "rightcommit"
    shutil.copy2(binary, installed)
    subprocess.run([installed, "--version"], check=True, cwd=work)
    env = os.environ.copy()
    for key in ("NODO_ASSETS", "NODO_DATA", "NODO_ISOLATE_USERS"):
        env.pop(key, None)
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    env["NODO_LISTEN"] = f"127.0.0.1:{port}"
    with (work / "server.log").open("w+") as log:
        process = subprocess.Popen([installed], cwd=work, env=env, stdout=log, stderr=log)
        try:
            for _ in range(100):
                if process.poll() is not None:
                    log.seek(0)
                    raise RuntimeError(log.read())
                try:
                    urllib.request.urlopen(f"http://127.0.0.1:{port}/healthz", timeout=1)
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise RuntimeError("Server did not start")
            assert urllib.request.urlopen(f"http://127.0.0.1:{port}/").status == 200
            assets = list((work / "data/system/assets").iterdir())
            assert len(assets) == 1
            for name in ("examples/helpdesk/Cargo.toml", "examples/helpdesk/src/lib.rs",
                         "examples/living-spec/.specify/memory/constitution.md",
                         "pi-extensions/package-lock.json"):
                assert (assets[0] / name).is_file(), name
            print("Installed executable: HTTP, UI and bundled resources OK")
        finally:
            process.terminate()
            process.wait(timeout=10)

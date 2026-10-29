#!/usr/bin/env python3
"""Regenerate twice and compare owned output, including recipe/fluid contracts."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CONTENT = ROOT / 'content'


def generate():
    subprocess.run([sys.executable, str(ROOT / 'tools/content-gen/generate.py')], check=True)


def snapshot():
    paths = set()
    for manifest in ('.content-gen-files.json', '.creative-tabs-files.json', '.hazard-gen-files.json'):
        paths.update(json.loads((CONTENT / manifest).read_text()))
    paths.update(('ids.json', 'fluids.json', 'machines_backlog.json'))
    paths.update(str(path.relative_to(CONTENT)) for path in (CONTENT / 'machine-recipes').glob('*.json'))
    return {name: hashlib.sha256((CONTENT / name).read_bytes()).hexdigest() for name in sorted(paths)}


generate(); before = snapshot(); generate(); after = snapshot()
changes = [name for name in sorted(before.keys() | after.keys()) if before.get(name) != after.get(name)]
report = {'status': 'pass' if not changes else 'fail', 'files_compared': len(before), 'changed': changes}
(ROOT / 'content-gen-evidence/m3/regeneration.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report))
if changes: raise SystemExit(1)

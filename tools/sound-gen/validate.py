#!/usr/bin/env python3
"""Validate every packaged Ogg before cargo-horizon-mod builds the content.

Audited bytes are identified by SHA-256. New/changed files are probed again;
conflicting copyright metadata stops the build, even on an unreferenced file.
"""
import hashlib
import json
from pathlib import Path
from generate import ROOT, HERE, audio_tags, classify_tags


def validate_tree(root):
    manifest = json.loads((HERE / 'manifest.json').read_text())
    audited = {entry['sha256'] for entry in manifest['files'].values()}
    files = sorted(root.rglob('*.ogg'))
    for path in files:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest not in audited:
            classify_tags(audio_tags(path))
    print(f'audio license guard: {len(files)} Ogg checked')


if __name__ == '__main__':
    validate_tree(ROOT / 'content/assets')

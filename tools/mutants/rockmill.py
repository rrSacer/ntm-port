#!/usr/bin/env python3
"""Hand-picked mutants of the rock mill's crafter station (the review of lot 4: the auto switch, the tanks sized each
tick, red sand): each must fail its tests.
Usage: tools/mutants/rockmill.py REPO TARGET LOG (docs/onda-2-sublote-2.md)"""
import subprocess, sys
from pathlib import Path
repo, target, log = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
src = repo / 'crates/ntm-core/src'
MUTANTS = [
    ('crafter.rs', '        if r.items_in.first().is_some_and(|i| count(&self.items, &i.ids) > 0) {\n            return None;\n        }\n', ''),
    ('crafter.rs', 'if r.items_in.first().is_some_and(|i| count(&self.items, &i.ids) > 0) {', 'if r.items_in.first().is_some_and(|i| count(&self.items, &i.ids) > 1) {'),
    ('crafter.rs', '        if self.station == Station::RockMill {\n            self.size_tanks(r);\n        }\n        if !self.can_run(r)', '        if !self.can_run(r)'),
    ('crafter.rs', 'let size = |t: &Tank, f: Option<&(String, u32)>| 4_000.max(f.map_or(0, |f| f.1 * 2)).max(t.mb);', 'let size = |_t: &Tank, f: Option<&(String, u32)>| 4_000.max(f.map_or(0, |f| f.1 * 2));'),
    ('crafter.rs', 'let size = |t: &Tank, f: Option<&(String, u32)>| 4_000.max(f.map_or(0, |f| f.1 * 2)).max(t.mb);', 'let size = |t: &Tank, f: Option<&(String, u32)>| 4_000.max(f.map_or(0, |f| f.1)).max(t.mb);'),
    ('crafter.rs', 'let size = |t: &Tank, f: Option<&(String, u32)>| 4_000.max(f.map_or(0, |f| f.1 * 2)).max(t.mb);', 'let size = |t: &Tank, f: Option<&(String, u32)>| 2_000.max(f.map_or(0, |f| f.1 * 2)).max(t.mb);'),
    ('crafter.rs', '        for t in &mut self.tout {\n            t.cap = size(t, r.fluids_out.first());\n        }\n', ''),
    ('crafter.rs', '        if self.station == Station::RockMill {\n            self.size_tanks(r);\n        }\n    }\n\n    /// The rock mill', '    }\n\n    /// The rock mill'),
]
results = []
for file, old, new in MUTANTS:
    path = src / file
    original = path.read_text()
    if old not in original:
        results.append((file, old[:40], 'ANCHOR MISSING')); continue
    path.write_text(original.replace(old, new, 1))
    try:
        r = subprocess.run(['/usr/local/bin/agent-run', 'env', 'QUEUE=cargo', 'CARGO_BUILD_JOBS=4', f'CARGO_TARGET_DIR={target}', 'cargo', 'test', '-p', 'ntm-core', '--lib', '--', 'rockmill', 'crafter', 'purex'],
                           cwd=repo, capture_output=True, text=True, timeout=3000)
        verdict = 'KILLED' if r.returncode != 0 and 'test result: FAILED' in r.stdout else ('SURVIVED' if r.returncode == 0 else 'BUILD ERROR')
    finally:
        path.write_text(original)
    results.append((file, old[:50].replace('\n', ' '), new[:50].replace('\n', ' '), verdict))
    log.write_text('\n'.join(map(str, results)) + '\n')
log.write_text('\n'.join(map(str, results)) + '\nDONE\n')

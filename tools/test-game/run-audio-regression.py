#!/usr/bin/env python3
"""Run the complete HBM audio acceptance serially through the shared game queue.

Each runner records its binary and bundle hashes in report.json. The dims runner
also executes idle and idle-clean in separate worlds. Failed runs are retained.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

SCENARIOS = ['sounds', 'audio-lifecycle', 'items', 'mining', 'bombs', 'zirnox', 'pile',
             'pile-meltdown', 'dims', 'inventory-proxy', 'radioisotopes',
             'solar', 'combustion', 'catalog', 'comparator', 'stations', 'recipes']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    args.scratch_root.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    for key in ['NTM_ONLY', 'NTM_FAMILIES']:
        env.pop(key, None)
    summary = {'ok': False, 'runs': []}
    for scenario in SCENARIOS:
        log = args.scratch_root / f'{scenario}.log'
        command = ['agent-run', 'env', 'QUEUE=game', sys.executable,
                   str(Path(__file__).with_name('run-native-reactors.py')), scenario,
                   '--binary', str(args.binary.resolve()), '--resources', str(args.resources.resolve()),
                   '--scratch-root', str(args.scratch_root.resolve())]
        print(f'BEGIN {scenario}', flush=True)
        with log.open('w') as output:
            result = subprocess.run(command, env=env, stdout=output, stderr=subprocess.STDOUT)
        summary['runs'].append({'scenario': scenario, 'exit_code': result.returncode, 'log': str(log.resolve())})
        print(f'END {scenario}: {result.returncode}', flush=True)
        (args.scratch_root / 'audio-regression.json').write_text(json.dumps(summary, indent=2) + '\n')
    summary['ok'] = all(row['exit_code'] == 0 for row in summary['runs'])
    (args.scratch_root / 'audio-regression.json').write_text(json.dumps(summary, indent=2) + '\n')
    return 0 if summary['ok'] else 1


if __name__ == '__main__':
    raise SystemExit(main())

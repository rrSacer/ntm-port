#!/usr/bin/env python3
"""T04 strict cut (P3(d) of the T04 review), on disposable worlds: native_rbmk_t04_c.py (T03's scenario C with a dial
rule rewritten and the satellite events in play) killed at the points of the save between kv_committed and
chunks_written, with HORIZON_NO_EAGER_SAVE=1, the rods the reopened world holds counted item by item against the
vanilla control cut at the same point (run-rbmk-t01-proof.py's verdicts), and the rule each world reads back reported
beside the control's.

Example (wrap in agent-run, game queue):
  HORIZON_NO_EAGER_SAVE=1 python3 tools/test-game/run-rbmk-t04-c-proof.py --binary PATH --resources PATH --bundle PATH \
      --scratch-root PATH --keep
"""
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('run_rbmk_t01_proof', HERE / 'run-rbmk-t01-proof.py')
t01 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t01)

t01.SCRIPT = 'native_rbmk_t04_c.py'
t01.ITEMS = ('ntm:rbmk_fuel_',)
t01.DIMS = ('overworld',)
t01.TOTAL = 3
t01.POINTS = ['kv_committed', 'region_record:1', 'region_record:2', 'region_record:3', 'chunks_written']


def evidence(folder, mode):
    path = folder / f'native-rbmk-t04c-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


t01.evidence = evidence


def main():
    code = t01.main()
    # The rules and events each world read back, NTM beside its control, by point.
    for report in sorted(Path(sys.argv[sys.argv.index('--scratch-root') + 1]).glob('rbmk-t01-*/report.json')):
        out = {}
        for folder in sorted(report.parent.glob('point-*')):
            ev = [evidence(folder, m) for m in ('act', 'verify', 'again')]
            out[folder.name] = {'before_save': ev[0].get('rule_before_save'), 'first': ev[1].get('rule'), 'second': ev[2].get('rule_again'),
                                'rays_first': (ev[1].get('rays') or {}).get('count'), 'rays_second': (ev[2].get('rays_again') or {}).get('count')}
        (report.parent / 'rules.json').write_text(json.dumps(out, indent=1) + '\n')
        print(json.dumps(out))
    return code


if __name__ == '__main__':
    sys.exit(main())

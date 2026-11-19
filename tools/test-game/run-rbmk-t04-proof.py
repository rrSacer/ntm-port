#!/usr/bin/env python3
"""T04 proof (`rbmk-plano` T04), J, NTM only, on a disposable world: native_rbmk_t04.py `play`, a clean reopen, then
`reopen`. The reactor dumps it leaves in the base folder's `dumps/` are then replayed by the core
(`cargo test -p ntm-core --lib the_games_dumps_replay -- --ignored` with `RBMK_T04_DUMPS` set to that folder).

Example (wrap in agent-run, game queue):
  python3 tools/test-game/run-rbmk-t04-proof.py --binary PATH --resources PATH --bundle PATH --scratch-root PATH
"""
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('run_rbmk_t01_proof', HERE / 'run-rbmk-t01-proof.py')
t01 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t01)


def evidence(folder, mode):
    mode = {'probe': 'reopen', 'prepare': 'play'}.get(mode, mode)
    path = folder / f'native-rbmk-t04-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def main():
    t01.evidence = evidence
    real = t01.World.script

    def script(self, mode, check=True, extra=None, script=None):
        mode = {'prepare': 'play', 'probe': 'reopen'}.get(mode, mode)
        return real(self, mode, check, extra, 'native_rbmk_t04.py')

    t01.World.script = script
    sys.argv.append('--probe')
    return t01.main()


if __name__ == '__main__':
    sys.exit(main())

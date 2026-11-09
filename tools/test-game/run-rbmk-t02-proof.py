#!/usr/bin/env python3
"""T02 proof (`rbmk-plano` T02), on disposable worlds.

--play: the J run (native_rbmk_t02.py `play`, then `reopen` after a clean reopen), NTM only.
Otherwise scenario C: the game killed at every boundary of the save that follows a lid put on by hand, one taken off
with a screwdriver and a column broken with its lid, and the lids the reopened world holds counted item by item against
the vanilla control cut at the same point, with run-rbmk-t01-proof.py's verdicts (R7c).

Example (wrap in agent-run, game queue):
  python3 tools/test-game/run-rbmk-t02-proof.py --binary PATH --resources PATH --bundle PATH --scratch-root PATH [--play]
"""
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('run_rbmk_t01_proof', HERE / 'run-rbmk-t01-proof.py')
t01 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t01)

t01.SCRIPT = 'native_rbmk_t02.py'
t01.ITEMS = ('ntm:rbmk_lid',)
t01.DIMS = ('overworld',)
t01.TOTAL = 3
t01.POINTS = ['kill', 'kv_committed', 'players_saved', 'live_chunk:2,0']


def evidence(folder, mode):
    # The J run's second launch goes by the T01 runner's probe path (`--play` below).
    mode = {'probe': 'reopen'}.get(mode, mode)
    path = folder / f'native-rbmk-t02-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


t01.evidence = evidence


def main():
    if '--play' not in sys.argv:
        return t01.main()
    sys.argv.remove('--play')
    # The J run takes the T01 runner's probe path: a base launched for `play`, then again for `reopen`.
    t01.SCRIPT = 'native_rbmk_t02.py'
    real = t01.World.script

    def script(self, mode, check=True, extra=None, script=None):
        mode = {'prepare': 'play', 'probe': 'reopen'}.get(mode, mode)
        return real(self, mode, check, extra, 'native_rbmk_t02.py')

    t01.World.script = script
    sys.argv.append('--probe')
    # Its first launch's checks are in the base folder's native-rbmk-t02-play.json, the second's printed.
    return t01.main()


if __name__ == '__main__':
    sys.exit(main())

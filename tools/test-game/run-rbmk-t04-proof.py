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


def again(base):
    """`--again BASE`: one more opening of a proof's world (mode `again`), then quit."""
    import argparse, os
    parser = argparse.ArgumentParser()
    parser.add_argument('--again', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path)
    parser.add_argument('--mode', default='again', help='again (an opening of a proof world) or rad (a fresh world)')
    args = parser.parse_args()
    if not args.again.exists():
        args.again.mkdir(parents=True)
        (args.again / 'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nmaxFps:30\nenableVsync:false\nguiScale:2\ntutorialStep:none\n')
        (args.again / 'horizon-controller.txt').write_text('enabled:false\n')
        (args.again / 'horizon-graphics.txt').write_text('workers:2\n')
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='',
               NTM_TEST_HOOKS='1', NTM_CRASH_VANILLA='0')
    for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'), ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
        env[key] = str(args.again / sub)
    import subprocess
    reader, writer = os.pipe()
    xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0', '1280x720x24', '-nolisten', 'tcp'], pass_fds=(writer,))
    os.close(writer)
    with os.fdopen(reader) as pipe:
        env['DISPLAY'] = ':' + pipe.readline().strip()
    try:
        world = t01.World(args, args.again, env)
        world.launch()
        code = subprocess.run([sys.executable, str(HERE / 'native_rbmk_t04.py'), str(args.again), args.mode], cwd=args.again, env=env, timeout=1200).returncode
        world.quit()
        print(json.dumps(evidence(args.again, args.mode).get('checks')))
        return code
    finally:
        xvfb.kill()


def main():
    if '--again' in sys.argv:
        return again(None)
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

#!/usr/bin/env python3
"""The scenarios of native_crash_more.py, each killed at the boundaries of a save (see run-crash-proof.py).

  python3 tools/test-game/run-crash-more.py --binary B --resources R --scratch-root S SCENARIO [POINT ...]

SCENARIO: coupled-a coupled-b coupled-ea coupled-eb crossing mining break replace. Points default to the ones that
matter for the scenario. A world that passed is deleted; the report keeps the numbers.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('crash_proof', here / 'run-crash-proof.py')
crash_proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crash_proof)

DEFAULT_POINTS = {
    'coupled-a': ['none', 'save_begin', 'region_record:1', 'region_record:2'],
    'coupled-b': ['none', 'save_begin', 'region_record:1', 'region_record:2'],
    'coupled-ea': ['none', 'save_begin', 'region_record:1', 'region_record:2'],
    'coupled-eb': ['none', 'save_begin', 'region_record:1', 'region_record:2'],
    'crossing': ['none'],
    'mining': ['none', 'save_begin', 'region_record:1', 'chunks_written', 'entities_written'],
    'break': ['none', 'save_begin', 'region_record:1', 'chunks_written', 'entities_written'],
    'replace': ['none', 'save_begin', 'region_record:1', 'chunks_written', 'entities_written'],
}


class More(crash_proof.World):
    def __init__(self, args, folder, env, scenario):
        super().__init__(args, folder, env)
        self.scenario = scenario

    def script(self, mode, check=True):
        env = dict(self.env, NTM_CRASH_SCENARIO=self.scenario)
        return subprocess.run([sys.executable, str(here / 'native_crash_more.py'), str(self.folder), mode],
                              cwd=self.folder, env=env, check=check, timeout=1200).returncode


def evidence(folder, mode):
    path = folder / f'native-crash-more-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def play(args, base, scratch, env, scenario, point):
    folder = scratch / ('point-' + point.replace(':', '-'))
    shutil.copytree(base, folder, ignore=shutil.ignore_patterns('control.sock', 'native-crash-more-act.json',
                                                                 'native-crash-more-verify.json', 'native-crash-more-again.json'))
    world = More(args, folder, env, scenario)
    result = {'scenario': scenario, 'point': point, 'folder': str(folder), 'ok': False}
    try:
        world.launch(point)
        world.script('act', check=False)
        if point == 'none':
            result['exit'] = 'survived'
            world.quit()
        else:
            code = world.died()
            result['exit'] = code
            result['killed_at_the_point'] = 'HORIZON_FAILPOINT' in world.log.read_text(errors='replace')
            if code is None:
                world.client.kill()
                world.client.wait()
                raise AssertionError('the game was not killed at ' + point)
            (folder / 'control.sock').unlink(missing_ok=True)
        result['act'] = evidence(folder, 'act').get('after_act') or evidence(folder, 'act').get('after_undo')
        world.launch()
        world.script('verify')
        result['first_reopen'] = evidence(folder, 'verify').get('counted')
        world.quit()
        world.launch()
        world.script('again')
        result['second_reopen'] = evidence(folder, 'again').get('counted_again')
        world.quit()
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('scenario')
    parser.add_argument('points', nargs='*')
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    work = here.parents[1]
    args.binary = args.binary.resolve()
    args.bundle = (args.bundle or work / 'dist/ntm.hmod').resolve()
    scratch = Path(tempfile.mkdtemp(prefix='more-' + args.scenario + '-', dir=args.scratch_root))
    print(scratch, flush=True)
    base = scratch / 'base'
    base.mkdir()
    env = os.environ.copy()
    for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                     ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
        env[key] = str(base / sub)
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='')
    (base / 'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nmaxFps:30\nenableVsync:false\nguiScale:2\ntutorialStep:none\n')
    (base / 'horizon-controller.txt').write_text('enabled:false\n')
    (base / 'horizon-graphics.txt').write_text('workers:2\n')
    reader, writer = os.pipe()
    with (scratch / 'xvfb.log').open('w') as log:
        xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0', '1280x720x24', '-nolisten', 'tcp'],
                                pass_fds=(writer,), stdout=log, stderr=subprocess.STDOUT)
    os.close(writer)
    with os.fdopen(reader) as pipe:
        env['DISPLAY'] = ':' + pipe.readline().strip()
    report = {'ok': False, 'scenario': args.scenario, 'binary': str(args.binary), 'points': []}
    try:
        world = More(args, base, env, args.scenario)
        world.launch()
        world.script('prepare')
        world.quit()
        for point in args.points or DEFAULT_POINTS[args.scenario]:
            copy_env = dict(env)
            folder = scratch / ('point-' + point.replace(':', '-'))
            for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                             ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
                copy_env[key] = str(folder / sub)
            outcome = play(args, base, scratch, copy_env, args.scenario, point)
            report['points'].append(outcome)
            print(json.dumps(outcome), flush=True)
            if outcome['ok']:
                shutil.rmtree(folder, ignore_errors=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

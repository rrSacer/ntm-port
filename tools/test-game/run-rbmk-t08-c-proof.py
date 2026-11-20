#!/usr/bin/env python3
"""T08 cut (C, `rbmk-plano` T08): kill the game at points of the save that follows cryogel set flowing through an RBMK
cooler across a chunk border (native_rbmk_t08_c.py), and count, on both reopens, what the source barrel, the cooler and
the sink barrel hold, read in one go: 16 000 mB, none lost, none made. The vanilla control (hoppers moving 64 stones
across the same border, cut at the same point) is reported beside it: what vanilla keeps across a cut with items moving.

Example (wrap in agent-run, game queue):
  HORIZON_NO_EAGER_SAVE=1 python3 tools/test-game/run-rbmk-t08-c-proof.py --binary PATH --resources PATH --bundle PATH \
      --scratch-root PATH [POINT ...]
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

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('run_crash_proof', HERE / 'run-crash-proof.py')
rcp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rcp)

SCRIPT = 'native_rbmk_t08_c.py'
POINTS = ['kill', 'kv_committed', 'region_record:1', 'chunks_written', 'world_saved']


class World(rcp.World):
    def script(self, mode, check=True, extra=None):
        return subprocess.run([sys.executable, str(HERE / SCRIPT), str(self.folder), mode],
                              cwd=self.folder, env=dict(self.env, **(extra or {})), check=check, timeout=2700).returncode


def evidence(folder, mode):
    path = folder / f'native-rbmk-t08c-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def play(args, base, scratch, env, point, control):
    folder = rcp.point_folder(scratch, point, control)
    rcp.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-rbmk-t08c-*.json'))
    world = World(args, folder, env)
    result = {'point': point, 'vanilla_control': control, 'ok': False}
    try:
        world.launch(None if point == 'kill' else point)
        acted = world.script('act', check=False, extra=rcp.act_extra(point))
        ev = evidence(folder, 'act')
        result['before_act'], result['at_the_save'] = ev.get('before_act'), ev.get('after_act')
        result['died_in_act'] = bool(acted) and rcp.died_in_act(world, point)
        if acted and not result['died_in_act']:
            raise AssertionError('act failed before its save; see act evidence')
        if point == 'kill':
            world.client.kill()
            result['exit'] = world.client.wait()
        else:
            code = world.died()
            result['exit'] = code
            result['killed_at_the_point'] = rcp.reached(world, point)
            if code is None:
                world.client.kill(); world.client.wait()
                raise AssertionError('the game was not killed at ' + point)
            if not result['killed_at_the_point']:
                raise AssertionError('client exited without reaching ' + point)
        (folder / 'control.sock').unlink(missing_ok=True)
        for mode, key in (('verify', 'first_reopen'), ('again', 'second_reopen')):
            world.launch()
            verified = world.script(mode, check=False)
            result[key] = evidence(folder, mode).get('counted' + ('_again' if mode == 'again' else ''))
            world.quit()
            if verified and not control:
                raise AssertionError(key + ': ' + json.dumps(result[key]))
        world.launch()
        world.script('drain')
        world.quit()
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill(); world.client.wait()
    return result


def main():
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('points', nargs='*')
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    args.binary, args.bundle = args.binary.resolve(), args.bundle.resolve()
    scratch = Path(tempfile.mkdtemp(prefix='rbmk-t08c-', dir=args.scratch_root))
    print(scratch, flush=True)
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='',
               NTM_TEST_HOOKS='1')
    reader, writer = os.pipe()
    with (scratch / 'xvfb.log').open('w') as log:
        xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0', '1280x720x24', '-nolisten', 'tcp'],
                                pass_fds=(writer,), stdout=log, stderr=subprocess.STDOUT)
    os.close(writer)
    with os.fdopen(reader) as pipe:
        env['DISPLAY'] = ':' + pipe.readline().strip()
    report = {'ok': False, 'binary_sha256': rcp.digest(args.binary), 'bundle_sha256': rcp.digest(args.bundle), 'points': []}

    def homed(folder, control):
        copy_env = dict(env, NTM_CRASH_VANILLA='1' if control else '0')
        for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                         ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
            copy_env[key] = str(folder / sub)
        return copy_env
    try:
        bases = {False: scratch / 'base', True: scratch / 'base-vanilla'}
        for control, where in bases.items():
            where.mkdir()
            (where / 'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nmaxFps:30\nenableVsync:false\nguiScale:2\ntutorialStep:none\n')
            (where / 'horizon-controller.txt').write_text('enabled:false\n')
            (where / 'horizon-graphics.txt').write_text('workers:2\n')
            world = World(args, where, homed(where, control))
            world.launch()
            prepared = world.script('prepare', check=False)
            report.setdefault('prepare', {})['vanilla' if control else 'ntm'] = evidence(where, 'prepare').get('base')
            world.quit()
            if prepared:
                raise AssertionError('prepare failed; see its evidence')
        for point in args.points or POINTS:
            ntm = play(args, bases[False], scratch, homed(rcp.point_folder(scratch, point, False), False), point, False)
            vanilla = play(args, bases[True], scratch, homed(rcp.point_folder(scratch, point, True), True), point, True)
            totals = lambda r: [(r.get(k) or {}).get('total') for k in ('first_reopen', 'second_reopen')]
            outcome = {'point': point, 'ntm': ntm, 'vanilla': vanilla, 'ntm_totals': totals(ntm), 'vanilla_totals': totals(vanilla)}
            # NTM: exactly 16 000 mB on both reopens, and the same state on both (nothing left to apply twice).
            outcome['ok'] = ntm['ok'] and outcome['ntm_totals'] == [16_000, 16_000]
            report['points'].append(outcome)
            print(json.dumps({k: outcome[k] for k in ('point', 'ok', 'ntm_totals', 'vanilla_totals')}), flush=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

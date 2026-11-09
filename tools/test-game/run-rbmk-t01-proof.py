#!/usr/bin/env python3
"""T01 proof (`rbmk-plano` T01): kill the game at every boundary of the save that follows fuel rods moved by hand into
RBMK channels on both sides of a chunk border, with a second RBMK layer parked in the Nether, and count what the
reopened world holds, item by item, against the vanilla control (barrels where the columns stand) cut at the same point.
The verdicts are run-crash-proof.py's (R7c): as the files, nothing duplicated, no worse than vanilla, the storage
keeping nothing a chunk owes, both reopens alike.

Example (wrap in agent-run, game queue):
  python3 tools/test-game/run-rbmk-t01-proof.py --binary PATH --resources PATH --scratch-root PATH [POINT ...]
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

import disk_count

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('run_crash_proof', HERE / 'run-crash-proof.py')
rcp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rcp)

ITEMS = ('ntm:rbmk_fuel_',)
DIMS = ('overworld', 'the_nether')
TOTAL = 9
POINTS = [*rcp.POINTS, 'kv_committed', 'live_chunk:2,0', 'live_chunk:3,0']


class World(rcp.World):
    def script(self, mode, check=True, extra=None):
        return subprocess.run([sys.executable, str(HERE / 'native_rbmk_t01.py'), str(self.folder), mode],
                              cwd=self.folder, env=dict(self.env, **(extra or {})), check=check, timeout=2700).returncode


def evidence(folder, mode):
    path = folder / f'native-rbmk-t01-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def play(args, base, scratch, env, point, control=False):
    folder = rcp.point_folder(scratch, point, control)
    rcp.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-rbmk-t01-*.json'))
    world = World(args, folder, env)
    result = {'point': point, 'folder': str(folder), 'ok': False, 'vanilla_control': control}
    result['base_items'] = disk_count.items(base, ITEMS, DIMS)
    try:
        world.launch(None if point == 'kill' else point)
        acted = world.script('act', check=False, extra=rcp.act_extra(point))
        result['after_act'] = evidence(folder, 'act').get('after_act')
        result['died_in_act'] = bool(acted) and rcp.died_in_act(world, point)
        if acted and not result['died_in_act']:
            raise AssertionError('act failed before its save; see act evidence')
        if point == 'none':
            result['exit'] = 'survived'
            world.quit()
        elif point == 'kill':
            world.client.kill()
            result['exit'] = world.client.wait()
            (folder / 'control.sock').unlink(missing_ok=True)
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
        result['disk'] = disk_count.count(folder, ITEMS, DIMS)
        result['items'] = disk_count.items(folder, ITEMS, DIMS)
        result['kv_items'] = disk_count.kv_items(folder)
        result['written'] = disk_count.written(base, folder, ITEMS, DIMS)
        for mode, key, items, kv in (('verify', 'first_reopen', 'items_after', 'kv_items_after'),
                                     ('again', 'second_reopen', 'items_again', 'kv_items_again')):
            world.launch()
            verified = world.script(mode, check=False)
            ev = evidence(folder, mode)
            result[key] = ev.get('counted' + ('_again' if mode == 'again' else ''))
            result.setdefault('checks', {})[mode] = ev.get('checks')
            if verified:
                raise AssertionError(key + ' failed; see counted evidence')
            world.quit()
            result[items] = disk_count.items(folder, ITEMS, DIMS)
            result[kv] = disk_count.kv_items(folder)
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
    parser.add_argument('--bundle', type=Path)
    parser.add_argument('--scratch-root', type=Path, required=True)
    parser.add_argument('--keep', action='store_true', help='keep the folders of the points that passed')
    args = parser.parse_args()
    work = HERE.parents[1]
    args.binary = args.binary.resolve()
    args.bundle = (args.bundle or work / 'dist/ntm.hmod').resolve()
    scratch = Path(tempfile.mkdtemp(prefix='rbmk-t01-', dir=args.scratch_root))
    print(scratch, flush=True)
    base = scratch / 'base'
    base.mkdir()
    env = os.environ.copy()
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
    report = {'ok': False, 'binary': str(args.binary), 'binary_sha256': rcp.digest(args.binary),
              'bundle_sha256': rcp.digest(args.bundle), 'points': []}

    def homed(folder, control):
        copy_env = dict(env, NTM_CRASH_VANILLA='1' if control else '0')
        for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                         ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
            copy_env[key] = str(folder / sub)
        return copy_env
    try:
        bases = {False: base, True: scratch / 'base-vanilla'}
        shutil.copytree(base, bases[True])
        for control, where in bases.items():
            world = World(args, where, homed(where, control))
            world.launch()
            prepared = world.script('prepare', check=False)
            report.setdefault('prepare', {})['vanilla' if control else 'ntm'] = evidence(where, 'prepare')
            world.quit()
            if prepared:
                raise AssertionError('prepare failed; see its evidence')
        for point in args.points or POINTS:
            played = {c: play(args, where, scratch, homed(rcp.point_folder(scratch, point, c), c), point, c) for c, where in bases.items()}
            # Exactly the last save only for `kill`: the pause of `kv_committed` writes the player's file (the transfer's
            # other side) ahead of the held regions, the vanilla residue the control shows at the same cut.
            outcome = rcp.emission_verdict(played[False], played[True], TOTAL, TOTAL, exact=point == 'kill')
            report['points'].append(outcome)
            print(json.dumps({k: outcome.get(k) for k in ('point', 'ok', 'error', 'verdict')}), flush=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        rcp.match_cuts(report['points'])
        if not args.keep:
            for outcome in report['points']:
                if outcome['ok']:
                    for c in (False, True):
                        shutil.rmtree(rcp.point_folder(scratch, outcome['point'], c), ignore_errors=True)
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

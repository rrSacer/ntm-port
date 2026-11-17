#!/usr/bin/env python3
"""Step 4 of the sawmill (Onda 2, D3): its cut scenarios (native_sawmill_cut.py) killed at the boundaries of the save
that follows the act, both worlds, the mod's and the vanilla control (barrels where the sawmills stand), reopened twice
and counted atom by atom: the blades wherever they are (loose, held, in a chest, installed, or in a sawmill item without
`ntm:no_blade`), and for `hand` the planks, logs and sawdust. The verdicts are run-crash-proof.py's, as T01's.

  python3 tools/test-game/run-sawmill-proof.py --scenario overspeed|border|break|hand|hopper --binary B --resources R
      --bundle H --scratch-root S [POINT ...]
Run with HORIZON_NO_EAGER_SAVE=1 (the strict cuts).
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

ITEMS = ('ntm:sawblade',)
DIMS = ('overworld',)
TOTAL = 1
SCRIPT = 'native_sawmill_cut.py'
STRICT = ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'world_saved']
SCENARIOS = {
    'overspeed': (('ntm:sawblade',), 1, STRICT),
    # The blade in the chunk north of the core's: both chunks' records cut alone too (R2).
    'border': (('ntm:sawblade',), 1, [*STRICT, 'live_chunk:2,1', 'live_chunk:2,0']),
    'break': (('ntm:sawblade',), 1, STRICT),
    'hand': (('ntm:sawblade', 'minecraft:oak_planks', 'minecraft:oak_log', 'ntm:powder_sawdust'), 11,
             [*STRICT, 'players_saved']),
    # Step 3c: 6 planks, 1 sawdust and 1 log, moved by hoppers within the core's chunk.
    'hopper': (('minecraft:oak_planks', 'minecraft:oak_log', 'ntm:powder_sawdust'), 8, STRICT),
}
POINTS = STRICT


class World(rcp.World):
    def script(self, mode, check=True, extra=None, script=None):
        return subprocess.run([sys.executable, str(HERE / (script or SCRIPT)), str(self.folder), mode],
                              cwd=self.folder, env=dict(self.env, **(extra or {})), check=check, timeout=2700).returncode


def evidence(folder, mode):
    path = folder / f'native-sawmill-cut-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def no_worse_than_a_copy(outcome):
    """A copy the vanilla control makes at a cut is not a bar the mod has to reach: where the control holds more than it
    started with, the mod's loss is compared with none (item by item too), and the rest of the verdict stands."""
    v, control = outcome.get('verdict'), outcome.get('control') or {}
    if not v or v.get('control_lost') is None or v.get('lost') is None:
        return outcome
    v['no_worse_than_vanilla'] = v['lost'] <= max(v['control_lost'], 0)
    base, their_base = outcome.get('base_items') or {}, control.get('base_items') or {}
    worse = []
    for reopen in rcp.REOPENS:
        mine, theirs = outcome.get(reopen), control.get(reopen)
        if mine is None or theirs is None:
            worse.append(False)
            continue
        ids = set(mine) | set(theirs) | set(base) | set(their_base)
        worse.append(all(base.get(i, 0) - mine.get(i, 0) <= max(their_base.get(i, 0) - theirs.get(i, 0), 0) for i in ids))
    v['no_worse_item'] = all(worse)
    outcome['ok'] = rcp.judge(outcome)
    return outcome


def play(args, base, scratch, env, point, control=False):
    folder = rcp.point_folder(scratch, point, control)
    rcp.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-sawmill-cut-*.json'))
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
    parser.add_argument('--scenario', choices=tuple(SCENARIOS), required=True)
    args = parser.parse_args()
    global ITEMS, TOTAL, POINTS
    ITEMS, TOTAL, POINTS = SCENARIOS[args.scenario]
    if not os.environ.get('HORIZON_NO_EAGER_SAVE'):
        raise SystemExit('run with HORIZON_NO_EAGER_SAVE=1')
    work = HERE.parents[1]
    args.binary = args.binary.resolve()
    args.bundle = (args.bundle or work / 'dist/ntm.hmod').resolve()
    scratch = Path(tempfile.mkdtemp(prefix='sawmill-' + args.scenario + '-', dir=args.scratch_root))
    print(scratch, flush=True)
    base = scratch / 'base'
    base.mkdir()
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='',
               NTM_SAW_SCENARIO=args.scenario, NTM_TRACE_EFFECTS='1')
    env.pop('NTM_TEST_HOOKS', None)
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
    report = {'ok': False, 'scenario': args.scenario, 'binary': str(args.binary), 'binary_sha256': rcp.digest(args.binary),
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
            outcome = no_worse_than_a_copy(rcp.emission_verdict(played[False], played[True], TOTAL, TOTAL, exact=point == 'kill'))
            report['points'].append(outcome)
            print(json.dumps({k: outcome.get(k) for k in ('point', 'ok', 'error', 'verdict')}), flush=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        rcp.match_cuts(report['points'])
        for outcome in report['points']:
            no_worse_than_a_copy(outcome)
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

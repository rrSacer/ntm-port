#!/usr/bin/env python3
"""The scenarios of native_crash_more.py, each killed at the boundaries of a save (see run-crash-proof.py).

  python3 tools/test-game/run-crash-more.py --binary B --resources R --scratch-root S SCENARIO [POINT ...]

SCENARIO: coupled-a coupled-b coupled-ea coupled-eb crossing crossing-kill mining break replace origin-pickup
origin-pickup-away origin-merge origin-vanish origin-kill. Points default to the ones that matter for the scenario;
`kill` ends the process with SIGKILL after the action, outside any save. A world that passed is deleted; the report keeps the numbers.
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
import zlib

here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('crash_proof', here / 'run-crash-proof.py')
crash_proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crash_proof)

DEFAULT_POINTS = {
    'vanilla-break': ['chunks_written', 'entities_written'],
    'coupled-a': ['none', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'coupled-b': ['none', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'coupled-ea': ['none', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'coupled-eb': ['none', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'crossing': ['none'],
    'origin-pickup': ['none'],
    'origin-pickup-away': ['none'],
    'origin-merge': ['none'],
    'origin-vanish': ['none'],
    'origin-kill': ['kill'],
    'crossing-kill': ['kill'],
    'save-quit': ['none'],
    'mining': crash_proof.POINTS,
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


def saved_coupled(folder, scenario):
    """Read the two actual chunk records before replay: which side of the transfer reached disk.

    This fixture uses ASCII machine JSON in the NBT string tag `ntm`; finding that named tag keeps the
    diagnostic independent of runtime adoption and avoids treating both geometries as proof of write order.
    """
    if not scenario.startswith('coupled'):
        return None
    region = folder / 'saves/CrashTest/dimensions/minecraft/overworld/region/r.0.0.mca'
    data = region.read_bytes()
    states = {}
    for cx in (2, 3):
        offset = int.from_bytes(data[cx * 4:cx * 4 + 3], 'big') * 4096
        length = int.from_bytes(data[offset:offset + 4], 'big')
        raw = data[offset + 5:offset + 4 + length]
        if data[offset + 4] != 2:
            raise AssertionError('fixture expected zlib chunk records')
        raw = zlib.decompress(raw)
        at, machines = 0, []
        while (tag := raw.find(b'\x08\x00\x03ntm', at)) >= 0:
            size = int.from_bytes(raw[tag + 6:tag + 8], 'big')
            at = tag + 8 + size
            m = json.loads(raw[tag + 8:at])
            d = m.get('data', {})
            if 'Core' in d:
                machines.append({'role': 'core', 'rev': m.get('rev', 0),
                                 'rods': sum(bool(r) for c in d['Core']['channels'] if c['kind'] == 'Fuel' for r in c['rods']),
                                 'rod_kinds': [r['kind'] for c in d['Core']['channels'] if c['kind'] == 'Fuel' for r in c['rods'] if r]})
            elif 'Loader' in d:
                machines.append({'role': 'loader', 'rev': m.get('rev', 0),
                                 'rods': sum(s['count'] for s in d['Loader']['slots'] if s),
                                 'items': [s['id'] if s else None for s in d['Loader']['slots']]})
        states[str(cx)] = sorted(machines, key=lambda m: m['role'])
    return states


def play(args, base, scratch, env, scenario, point):
    folder = scratch / ('point-' + point.replace(':', '-'))
    crash_proof.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-crash-more-act.json',
                                                                 'native-crash-more-verify.json', 'native-crash-more-again.json'))
    world = More(args, folder, env, scenario)
    result = {'scenario': scenario, 'point': point, 'folder': str(folder), 'ok': False}
    try:
        world.launch(point)
        acted = world.script('act', check=False)
        act = evidence(folder, 'act')
        result['act'] = act.get('after_act') or act.get('after_undo')
        result['while_away'] = act.get('while_away')
        result['act_checks'] = {k: {'ok': v['ok'], **({'actual': v['actual']} if not v['ok'] else {})}
                                for k, v in act.get('checks', {}).items()}
        result['died_in_act'] = bool(acted) and crash_proof.died_in_act(world, point)
        if acted and not result['died_in_act']:
            raise AssertionError('action failed before its save; see act evidence')
        if point == 'none':
            result['exit'] = 'survived'
            world.quit()
        elif point == 'kill':
            # No save at all: the process dies where the action left it (SIGKILL keeps what reached the page cache).
            world.client.kill()
            result['exit'] = world.client.wait()
            (folder / 'control.sock').unlink(missing_ok=True)
        else:
            code = world.died()
            result['exit'] = code
            result['killed_at_the_point'] = 'HORIZON_FAILPOINT' in world.log.read_text(errors='replace')
            if code is None:
                world.client.kill()
                world.client.wait()
                raise AssertionError('the game was not killed at ' + point)
            if not result['killed_at_the_point']:
                raise AssertionError('client exited without reaching ' + point)
            (folder / 'control.sock').unlink(missing_ok=True)
        result['disk_before_reopen'] = saved_coupled(folder, scenario)
        if scenario.startswith('coupled') and point.startswith(('live_chunk:', 'region_record:')):
            before, after = saved_coupled(base, scenario), result['disk_before_reopen']
            changed = [c for c in ('2', '3') if before[c] != after[c]]
            result['disk_base'] = before
            result['new_chunks'] = changed
            result['cut_valid'] = len(changed) == 1
            if point.startswith('live_chunk:'):
                target = point.split(':', 1)[1].split(',')[0]
                result['cut_valid'] &= changed == [target]
            if not result['cut_valid']:
                raise AssertionError('invalid cut: must save exactly one of core/loader chunks before replay')
        world.launch()
        verified = world.script('verify', check=False)
        result['first_reopen'] = evidence(folder, 'verify').get('counted')
        if verified:
            raise AssertionError('first reopen failed; see counted evidence')
        world.quit()
        world.launch()
        verified = world.script('again', check=False)
        result['second_reopen'] = evidence(folder, 'again').get('counted_again')
        if verified:
            raise AssertionError('second reopen failed; see counted evidence')
        world.quit()
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
    return result


def main():
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))  # failpoint aborts must not dump gigabytes of game memory
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
    report = {'ok': False, 'scenario': args.scenario, 'binary': str(args.binary),
              'binary_sha256': crash_proof.digest(args.binary), 'bundle_sha256': crash_proof.digest(args.bundle),
              'points': []}
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

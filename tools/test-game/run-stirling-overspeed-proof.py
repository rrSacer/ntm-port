#!/usr/bin/env python3
"""Fatia 5 of Onda 2 (D1+D2): the Stirling's failure blast (native_stirling_overspeed.py) killed at the boundaries of
the save that follows it, then reopened twice. Run with NTM_TRACE_EFFECTS=1 and HORIZON_NO_EAGER_SAVE=1: every blast
the server applies prints `effect <serial>.<n> explosion ...` once in that session's log.

Each session (the act's and each reopen's) passes when its blasts are exactly the gears lost in it plus the batches its
home still owed when it opened, no effect id is applied twice in the whole run, no block around the engine changes, and
a lost gear never comes back.

  python3 tools/test-game/run-stirling-overspeed-proof.py --binary B --resources R --scratch-root S [POINT ...]
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('run_crash_proof', HERE / 'run-crash-proof.py')
rcp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rcp)

SCRIPT = 'native_stirling_overspeed.py'
POINTS = ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'world_saved']
BLAST = re.compile(r'ntm: effect (\d+)\.(\d+) explosion \[([-\d., ]+)\] power=([\d.]+) fire=(\w+) breaks=(\w+)')


class World(rcp.World):
    def script(self, mode, check=True, extra=None):
        return subprocess.run([sys.executable, str(HERE / SCRIPT), str(self.folder), mode],
                              cwd=self.folder, env=dict(self.env, **(extra or {})), check=check, timeout=1200).returncode


def evidence(folder, mode):
    path = folder / f'native-stirling-overspeed-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def blasts(log):
    return [{'id': f'{m[1]}.{m[2]}', 'at': [float(v) for v in m[3].split(',')], 'power': float(m[4]),
             'fire': m[5] == 'true', 'breaks': m[6] == 'true'} for m in BLAST.finditer(log.read_text(errors='replace'))]


def session(ev, key, found):
    """A reopen's verdict: blasts = gears lost here + batches its home still owed at the start."""
    s = (ev or {}).get(key) or {}
    owed = (s.get('start') or {}).get('ledger', {}).get('obligations', 0)
    expected = int(bool(s.get('failed_here'))) + owed
    return {'blasts': len(found), 'expected': expected, 'owed_at_start': owed, 'failed_here': s.get('failed_here'),
            'area_unchanged': s.get('area_unchanged'), 'checks': (ev or {}).get('checks'),
            'ok': len(found) == expected and bool(s.get('area_unchanged')) and all(c.get('ok') for c in ((ev or {}).get('checks') or {}).values())}


def play(args, base, scratch, env, point):
    folder = rcp.point_folder(scratch, point, False)
    rcp.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-stirling-overspeed-*.json'))
    world = World(args, folder, env)
    result = {'point': point, 'folder': str(folder), 'ok': False}
    try:
        world.launch(None if point == 'kill' else point)
        acted = world.script('act', check=False, extra=rcp.act_extra(point))
        act = evidence(folder, 'act')
        result['act_checks'] = act.get('checks')
        result['died_in_act'] = bool(acted) and rcp.died_in_act(world, point)
        if acted and not result['died_in_act']:
            raise AssertionError('act failed before its save; see act evidence')
        if point == 'none':
            world.quit()
        elif point == 'kill':
            world.client.kill()
            world.client.wait()
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
        found = blasts(world.log)
        result['act'] = {'blasts': found, 'ok': len(found) == 1 and all(c.get('ok') for c in (act.get('checks') or {}).values())
                         and found[0]['power'] == 5 and not found[0]['fire'] and not found[0]['breaks']}
        ids = [b['id'] for b in found]
        for mode, key in (('verify', 'session'), ('again', 'session_again')):
            world.launch()
            verified = world.script(mode, check=False)
            here = blasts(world.log)
            ids += [b['id'] for b in here]
            result[mode] = session(evidence(folder, mode), key, here)
            result[mode]['blast_list'] = here
            world.quit()
            if verified:
                raise AssertionError(mode + ' failed; see its evidence')
        result['ids'] = ids
        result['each_id_once'] = len(ids) == len(set(ids))
        result['ok'] = result['act']['ok'] and result['verify']['ok'] and result['again']['ok'] and result['each_id_once']
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
    parser.add_argument('--keep', action='store_true')
    args = parser.parse_args()
    if not (os.environ.get('NTM_TRACE_EFFECTS') and os.environ.get('HORIZON_NO_EAGER_SAVE')):
        raise SystemExit('run with NTM_TRACE_EFFECTS=1 and HORIZON_NO_EAGER_SAVE=1')
    args.binary = args.binary.resolve()
    args.bundle = (args.bundle or HERE.parents[1] / 'dist/ntm.hmod').resolve()
    scratch = Path(tempfile.mkdtemp(prefix='stirling-', dir=args.scratch_root))
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

    def homed(folder):
        copy_env = dict(env)
        for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                         ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
            copy_env[key] = str(folder / sub)
        return copy_env
    try:
        world = World(args, base, homed(base))
        world.launch()
        prepared = world.script('prepare', check=False)
        report['prepare'] = evidence(base, 'prepare').get('checks')
        world.quit()
        if prepared:
            raise AssertionError('prepare failed; see its evidence')
        for point in args.points or POINTS:
            outcome = play(args, base, scratch, homed(rcp.point_folder(scratch, point, False)), point)
            report['points'].append(outcome)
            print(json.dumps({k: outcome.get(k) for k in ('point', 'ok', 'error', 'each_id_once')}
                             | {'act': (outcome.get('act') or {}).get('ok'),
                                'blasts': [len((outcome.get('act') or {}).get('blasts') or []),
                                           (outcome.get('verify') or {}).get('blasts'), (outcome.get('again') or {}).get('blasts')]}), flush=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
            if outcome['ok'] and not args.keep:
                shutil.rmtree(rcp.point_folder(scratch, point, False), ignore_errors=True)
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

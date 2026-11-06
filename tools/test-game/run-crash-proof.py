#!/usr/bin/env python3
"""Kill the game at every boundary of a save and count what the reopened world holds (see native_crash.py).

Example (wrap in agent-run env QUEUE=game):
  python3 tools/test-game/run-crash-proof.py --binary PATH --resources PATH --scratch-root PATH [POINT ...]

One base world is prepared and copied for every point; a copy is played with HORIZON_FAILPOINT=POINT (the game
ends inside the save asked for after the undo), reopened without it, verified, reopened again and verified, and
finally drained. `none` runs the same without a failpoint.

Each point is played twice from bases prepared the same way: with the pile, and as the vanilla control (loose rods put on
the ground instead of ejected, NTM_CRASH_VANILLA=1). Before each reopen the runner counts what the files hold
(disk_count.py). A point passes when both reopens count what the files held, nothing is duplicated, and the pile loses
no more than the control loses at the same cut.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from native_game import Game
import disk_count

POINTS = ['none', 'save_begin', 'region_record:1', 'region_record:2', 'region_record:3', 'chunks_written',
          'entities_written', 'level_saved', 'parked_saved', 'level_dat_written', 'metadata_saved',
          'players_saved', 'world_saved']


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def clone_world(base, folder, ignore):
    """Copy mutable world/player files; reuse only the generated cache of the identical content bundle."""
    def skip(path, names):
        return set(ignore(path, names)) | {'.horizon-cache'}
    shutil.copytree(base, folder, ignore=skip)
    cache = base / '.horizon-cache'
    if cache.exists():
        (folder / '.horizon-cache').symlink_to(cache.resolve(), target_is_directory=True)


class World:
    def __init__(self, args, folder, env):
        self.args, self.folder, self.env = args, folder, env

    def launch(self, failpoint=None):
        env = dict(self.env)
        if failpoint and failpoint != 'none':
            env['HORIZON_FAILPOINT'] = failpoint
        (self.folder / 'control.sock').unlink(missing_ok=True)
        command = [str(self.args.binary), '--resources', str(self.args.resources.resolve()),
                   '--dev-mod', str(self.args.bundle), '--control-socket', str(self.folder / 'control.sock'),
                   '--test-terrain', '--render-distance', '2', '--width', '1280', '--height', '720',
                   '--name', 'CrashTest', '--world', 'CrashTest', '--creative', '--seed', '1']
        n = len(list(self.folder.glob('client-*.log')))
        self.log = self.folder / f'client-{n}.log'
        with self.log.open('w') as log:
            self.client = subprocess.Popen(command, cwd=self.folder, env=env, stdout=log, stderr=log)
        deadline = time.monotonic() + 150
        while not (self.folder / 'control.sock').exists():
            if self.client.poll() is not None:
                raise RuntimeError('client exited before opening its socket')
            if time.monotonic() > deadline:
                raise TimeoutError('control socket')
            time.sleep(.25)

    def script(self, mode, check=True):
        return subprocess.run([sys.executable, str(Path(__file__).with_name('native_crash.py')), str(self.folder), mode],
                              cwd=self.folder, env=self.env, check=check, timeout=900).returncode

    def quit(self):
        Game(self.folder, 'shutdown').ctl('quit')
        self.client.wait(timeout=90)
        (self.folder / 'control.sock').unlink(missing_ok=True)

    def died(self, timeout=120):
        """The game ended by itself (the failpoint); how it ended."""
        try:
            return self.client.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            return None


def evidence(folder, mode):
    path = folder / f'native-crash-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def sweep(args, base, scratch, env):
    """A big batch (the orphan sweep of a pile whose core went away): storage writes and bytes, rods kept."""
    folder = scratch / 'point-sweep'
    clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-crash-*.json'))
    world = World(args, folder, env)
    result = {'point': 'sweep', 'folder': str(folder), 'ok': False}
    try:
        world.launch()
        world.script('sweep')
        result['cost'] = evidence(folder, 'sweep').get('sweep_cost')
        world.quit()
        for mode, key in [('verify', 'first_reopen'), ('again', 'second_reopen')]:
            world.launch()
            verified = world.script(mode, check=False)
            result[key] = evidence(folder, mode).get('counted' + ('_again' if mode == 'again' else ''))
            if verified:
                raise AssertionError(key + ' failed after sweep; see counted evidence')
            if mode == 'again':
                world.script('drain')
            world.quit()
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill(); world.client.wait()
    return result


def died_in_act(world, point):
    """A region record point may be reached before the save the action asks for: the ledger writes its chunk's record
    as soon as the action changes it, and the k-th region record of the session is then that one. The process died
    between region records all the same; the reopens count as for any point."""
    if not point.startswith('region_record') or world.client.poll() is None:
        return False
    return 'HORIZON_FAILPOINT' in world.log.read_text(errors='replace')


ROD = 'ntm:pile_rod'
TOTAL = 12


def emission_verdict(outcome, control, expected, control_expected):
    """An emission against the files and the vanilla control: both reopens count what the files held before them,
    nothing is held twice, and no more is lost than the control loses at the same cut."""
    total = lambda o, r: (o.get(r) or {}).get('total')
    first, again, disk = total(outcome, 'first_reopen'), total(outcome, 'second_reopen'), (outcome.get('disk') or {}).get('total')
    lost, control_lost = None, None
    if first is not None and total(control, 'first_reopen') is not None:
        lost, control_lost = expected - first, control_expected - total(control, 'first_reopen')
    verdict = {'disk': disk, 'first': first, 'second': again, 'expected': expected, 'lost': lost, 'control_lost': control_lost,
               'control_disk': (control.get('disk') or {}).get('total')}
    verdict['as_the_files'] = first is not None and first == disk and again == first
    verdict['no_duplicate'] = first is not None and first <= expected
    verdict['no_worse_than_vanilla'] = lost is not None and lost <= control_lost
    ok = outcome['ok'] and control['ok'] and verdict['as_the_files'] and verdict['no_duplicate'] and verdict['no_worse_than_vanilla']
    return {**outcome, 'control': control, 'verdict': verdict, 'ok': ok}


def point_folder(scratch, point, control):
    return scratch / ('point-' + point.replace(':', '-') + ('-vanilla' if control else ''))


def play(args, base, scratch, env, point, control=False, items=(ROD,)):
    folder = point_folder(scratch, point, control)
    clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-crash-*.json'))
    world = World(args, folder, env)
    result = {'point': point, 'folder': str(folder), 'ok': False, 'vanilla_control': control}
    try:
        world.launch(point)
        acted = world.script('undo', check=False)
        result['after_undo'] = evidence(folder, 'undo').get('after_undo')
        result['died_in_act'] = bool(acted) and died_in_act(world, point)
        if acted and not result['died_in_act']:
            raise AssertionError('undo failed before its save; see undo evidence')
        if point == 'none':
            result['exit'] = 'survived'
            world.quit()
        else:
            code = world.died()
            result['exit'] = code
            result['killed_at_the_point'] = 'HORIZON_FAILPOINT' in world.log.read_text(errors='replace')
            if code is None:
                world.client.kill(); world.client.wait()
                raise AssertionError('the game was not killed at ' + point)
            if not result['killed_at_the_point']:
                raise AssertionError('client exited without reaching ' + point)
            (folder / 'control.sock').unlink(missing_ok=True)
        result['disk'] = disk_count.count(folder, items)
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
    args = parser.parse_args()
    work = Path(__file__).resolve().parents[2]
    args.binary = args.binary.resolve()
    args.bundle = (args.bundle or work / 'dist/ntm.hmod').resolve()
    scratch = Path(tempfile.mkdtemp(prefix='crash-', dir=args.scratch_root))
    print(scratch, flush=True)
    base = scratch / 'base'
    base.mkdir()
    env = os.environ.copy()
    for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                     ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
        env[key] = str(base / sub)
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1',
               WAYLAND_DISPLAY='')
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
    report = {'ok': False, 'binary': str(args.binary), 'binary_sha256': digest(args.binary),
              'bundle_sha256': digest(args.bundle), 'points': []}
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
            world.script('prepare')
            world.quit()
        for point in args.points or [*POINTS, 'sweep']:
            # Each copy is played by the same binary; HORIZON_HOME is inside the copy's own folder.
            if point == 'sweep':
                outcome = sweep(args, base, scratch, homed(scratch / 'point-sweep', False))
            else:
                played = {c: play(args, where, scratch, homed(point_folder(scratch, point, c), c), point, c) for c, where in bases.items()}
                outcome = emission_verdict(played[False], played[True], TOTAL, TOTAL)
            report['points'].append(outcome)
            if outcome['ok']:
                # Each copy of the world is large: a point that passed leaves only its numbers.
                for c in (False, True):
                    shutil.rmtree(point_folder(scratch, point, c), ignore_errors=True)
            print(json.dumps(outcome), flush=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

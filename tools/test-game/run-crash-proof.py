#!/usr/bin/env python3
"""Kill the game at every boundary of a save and count what the reopened world holds (see native_crash.py).

Example (wrap in agent-run env QUEUE=game):
  python3 tools/test-game/run-crash-proof.py --binary PATH --resources PATH --scratch-root PATH [POINT ...]

One base world is prepared and copied for every point; a copy is played with HORIZON_FAILPOINT=POINT (the game
ends inside the save asked for after the undo), reopened without it, verified, reopened again and verified, and
finally drained. `none` runs the same without a failpoint.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from native_game import Game

POINTS = ['none', 'save_begin', 'region_record:1', 'region_record:2', 'region_record:3', 'chunks_written',
          'entities_written', 'level_saved', 'parked_saved', 'level_dat_written', 'metadata_saved',
          'players_saved', 'world_saved']


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
    shutil.copytree(base, folder, ignore=shutil.ignore_patterns('control.sock', 'native-crash-*.json'))
    world = World(args, folder, env)
    result = {'point': 'sweep', 'folder': str(folder), 'ok': False}
    try:
        world.launch()
        world.script('sweep')
        result['cost'] = evidence(folder, 'sweep').get('sweep_cost')
        world.quit()
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill(); world.client.wait()
    return result


def play(args, base, scratch, env, point):
    folder = scratch / ('point-' + point.replace(':', '-'))
    shutil.copytree(base, folder, ignore=shutil.ignore_patterns('control.sock', 'native-crash-*.json'))
    world = World(args, folder, env)
    result = {'point': point, 'folder': str(folder), 'ok': False}
    try:
        world.launch(point)
        world.script('undo', check=False)
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
            (folder / 'control.sock').unlink(missing_ok=True)
        undo = evidence(folder, 'undo')
        result['after_undo'] = undo.get('after_undo')
        world.launch()
        world.script('verify')
        result['first_reopen'] = evidence(folder, 'verify').get('counted')
        world.quit()
        world.launch()
        world.script('again')
        result['second_reopen'] = evidence(folder, 'again').get('counted_again')
        world.script('drain')
        world.quit()
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill(); world.client.wait()
    return result


def main():
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
    report = {'ok': False, 'binary': str(args.binary), 'points': []}
    try:
        world = World(args, base, env)
        world.launch()
        world.script('prepare')
        world.quit()
        for point in args.points or POINTS:
            # Each copy is played by the same binary; HORIZON_HOME is inside the copy's own folder.
            copy_env = dict(env)
            folder = scratch / ('point-' + point.replace(':', '-'))
            for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                             ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
                copy_env[key] = str(folder / sub)
            outcome = (sweep if point == 'sweep' else play)(*((args, base, scratch, copy_env) + (() if point == 'sweep' else (point,))))
            report['points'].append(outcome)
            if outcome['ok']:
                # Each copy of the world is large: a point that passed leaves only its numbers.
                shutil.rmtree(folder, ignore_errors=True)
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

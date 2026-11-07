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

POINTS = ['none', 'kill', 'save_begin', 'region_record:1', 'region_record:2', 'region_record:3', 'chunks_written',
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
        if failpoint == 'kv_committed':
            # The cut of a save that does not wait: its storage is written, its records stay queued (the region writer
            # holds from that save on), and the process dies right after the storage (`NTM_FAILPOINT`).
            env['HORIZON_WRITER_HOLD'] = '1'
            env['NTM_FAILPOINT'] = 'kv_committed'
        elif failpoint and failpoint != 'none':
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

    def script(self, mode, check=True, extra=None):
        return subprocess.run([sys.executable, str(Path(__file__).with_name('native_crash.py')), str(self.folder), mode],
                              cwd=self.folder, env=dict(self.env, **(extra or {})), check=check, timeout=900).returncode

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


# The deaths outside a save's own stages: no save at all (`kill`), or a save without wait whose storage reached the disk
# and none of its records (`kv_committed`). Both bring back the last save, as vanilla does.
KILLS = ('kill', 'kv_committed')


def reached(world, point):
    """The log shows the death at `point`: the failpoint, or the storage written with the writer held."""
    text = world.log.read_text(errors='replace')
    if point == 'kv_committed':
        return 'NTM_FAILPOINT kv_committed' in text and 'HORIZON_WRITER_HOLD held' in text
    return 'HORIZON_FAILPOINT' in text


def act_extra(point):
    """What the action is told of the cut: no save (`kill`), or a save that does not wait (`kv_committed`: the pause)."""
    return {'kill': {'NTM_CRASH_NO_SAVE': '1'}, 'kv_committed': {'NTM_CRASH_PAUSE_SAVE': '1'}}.get(point)


def died_in_act(world, point):
    """A region record point may be reached before the save the action asks for: the ledger writes its chunk's record
    as soon as the action changes it, and the k-th region record of the session is then that one. The process died
    between region records all the same; the reopens count as for any point."""
    if not point.startswith('region_record') or world.client.poll() is None:
        return False
    return 'HORIZON_FAILPOINT' in world.log.read_text(errors='replace')


ROD = 'ntm:pile_rod'
TOTAL = 12


REOPENS = ('items_after', 'items_again')
KV_ITEMS = ('kv_items', 'kv_items_after', 'kv_items_again')


def item_checks(verdict, outcome, control):
    """Item by item (R6-3, R7-4): what each world holds after each reopen, against what its base held and what the
    control holds at the same cut after the same reopen; and that the storage of each world keeps nothing a chunk owes
    (`kv_items`)."""
    base, their_base = outcome.get('base_items') or {}, control.get('base_items') or {}
    verdict['items'] = {'base': base, 'control_base': their_base}
    duplicate, worse = [], []
    for reopen in REOPENS:
        mine, theirs = outcome.get(reopen), control.get(reopen)
        verdict['items'][reopen] = {'mine': mine, 'control': theirs}
        if mine is None or theirs is None:
            duplicate.append(False); worse.append(False)
            continue
        ids = set(mine) | set(theirs) | set(base) | set(their_base)
        duplicate.append(bool(ids) and all(mine.get(i, 0) <= max(base.get(i, 0), theirs.get(i, 0)) for i in ids))
        worse.append(bool(ids) and all(base.get(i, 0) - mine.get(i, 0) <= their_base.get(i, 0) - theirs.get(i, 0) for i in ids))
    verdict['no_duplicate_item'] = all(duplicate)
    verdict['no_worse_item'] = all(worse)


def emission_verdict(outcome, control, expected, control_expected, exact=False):
    """An emission against the files and the vanilla control: both reopens count what the files held before them, nothing
    is held more often than in the control, and no more is lost than the control loses at the same cut. The same cut is
    the one that wrote the same chunks holding the items (`cut`; how the items spread among them is the game's): a cut
    by the index of a record lands on other chunks when the two worlds' batches differ, and is then compared with the
    control's play that wrote the same chunks (`match_cuts`). A play with no cut (`none`, `kill`) has its own criteria."""
    total = lambda o, r: (o.get(r) or {}).get('total')
    first, again, disk = total(outcome, 'first_reopen'), total(outcome, 'second_reopen'), (outcome.get('disk') or {}).get('total')
    verdict = {'disk': disk, 'first': first, 'second': again, 'expected': expected, 'control_expected': control_expected,
               'control_disk': (control.get('disk') or {}).get('total'), 'parts_ok': outcome['ok'] and control['ok']}
    verdict['as_the_files'] = first is not None and disk is not None and first == disk and again == first
    # Both reopens hold what the files held, item by item; neither world's storage keeps what a chunk owes.
    verdict['as_the_files_items'] = outcome.get('items') is not None and all(outcome.get(r) == outcome.get('items') for r in REOPENS)
    verdict['storage_clean'] = all(o.get(k) == 0 for o in (outcome, control) for k in KV_ITEMS)
    control_first = total(control, 'first_reopen')
    verdict['same_cut'] = outcome['point'] in ('none', *KILLS) or (cut(outcome) is not None and cut(outcome) == cut(control))
    compare(verdict, outcome, control)
    if exact:
        # A death outside any save's records: both worlds are the last save, exactly, item by item.
        verdict['as_the_last_save'] = (first == expected and control_first == control_expected
                                       and all(o.get(r) == o.get('base_items') for o in (outcome, control) for r in REOPENS))
    result = {**outcome, 'control': control, 'verdict': verdict}
    result['ok'] = judge(result)
    return result


def cut(play):
    """The chunks holding the items that a play wrote before its cut, per folder."""
    written = play.get('written')
    return None if written is None else {k: sorted((x, z) for x, z, *_ in v) for k, v in written.items()}


def compare(verdict, outcome, control):
    """Every condition against the control at the same cut (its own, or the one `match_cuts` paired): copies, loss,
    and item by item. The control compared is recorded (F5 of the fresh review)."""
    first = verdict['first']
    control_first = (control.get('first_reopen') or {}).get('total')
    verdict['compared_with'] = control.get('point')
    verdict['control_first'] = control_first
    verdict['no_duplicate'] = first is not None and first <= max(verdict['expected'], control_first or 0)
    verdict['lost'] = None if first is None else verdict['expected'] - first
    verdict['control_lost'] = None if control_first is None else verdict['control_expected'] - control_first
    verdict['no_worse_than_vanilla'] = None not in (verdict['lost'], verdict['control_lost']) and verdict['lost'] <= verdict['control_lost']
    item_checks(verdict, outcome, control)


def judge(outcome):
    v = outcome['verdict']
    return bool(v['parts_ok'] and v['as_the_files'] and v['no_duplicate'] and v['no_worse_than_vanilla']
                and v['as_the_files_items'] and v['storage_clean'] and v['no_duplicate_item'] and v['no_worse_item']
                and (v['same_cut'] or v.get('matched')) and v.get('as_the_last_save', True) and v.get('as_the_control', True))


def match_cuts(points):
    """A cut whose chunks the control did not write alike is compared with the control's play that wrote the same
    chunks with the same items (a cut at that chunk, `live_chunk`, or at a stage), when the scenario played one. Without
    one the cut is not compared, and fails."""
    for o in points:
        v = o.get('verdict')
        if not v or v['same_cut'] or o.get('written') is None:
            continue
        twin = next((p for p in points if p is not o and p['point'] not in ('none', *KILLS)
                     and cut(p.get('control') or {}) == cut(o)), None)
        v['matched'] = twin and twin['point']
        if twin:
            compare(v, o, twin['control'])
        o['ok'] = judge(o)


def point_folder(scratch, point, control):
    return scratch / ('point-' + point.replace(':', '-') + ('-vanilla' if control else ''))


def play(args, base, scratch, env, point, control=False, items=(ROD,)):
    folder = point_folder(scratch, point, control)
    clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-crash-*.json'))
    world = World(args, folder, env)
    result = {'point': point, 'folder': str(folder), 'ok': False, 'vanilla_control': control}
    result['base_items'] = disk_count.items(base, items)
    try:
        world.launch(None if point == 'kill' else point)
        acted = world.script('undo', check=False, extra=act_extra(point))
        result['after_undo'] = evidence(folder, 'undo').get('after_undo')
        result['died_in_act'] = bool(acted) and died_in_act(world, point)
        if acted and not result['died_in_act']:
            raise AssertionError('undo failed before its save; see undo evidence')
        if point == 'none':
            result['exit'] = 'survived'
            world.quit()
        elif point == 'kill':
            # No save at all: the process dies where the undo left it.
            world.client.kill()
            result['exit'] = world.client.wait()
            (folder / 'control.sock').unlink(missing_ok=True)
        else:
            code = world.died()
            result['exit'] = code
            result['killed_at_the_point'] = reached(world, point)
            if code is None:
                world.client.kill(); world.client.wait()
                raise AssertionError('the game was not killed at ' + point)
            if not result['killed_at_the_point']:
                raise AssertionError('client exited without reaching ' + point)
            (folder / 'control.sock').unlink(missing_ok=True)
        result['disk'] = disk_count.count(folder, items)
        result['items'] = disk_count.items(folder, items)
        result['kv_items'] = disk_count.kv_items(folder)
        result['written'] = disk_count.written(base, folder, items)
        world.launch()
        verified = world.script('verify', check=False)
        result['first_reopen'] = evidence(folder, 'verify').get('counted')
        if verified:
            raise AssertionError('first reopen failed; see counted evidence')
        world.quit()
        # What the world holds once reopened and saved again, item by item, and what its storage keeps.
        result['items_after'] = disk_count.items(folder, items)
        result['kv_items_after'] = disk_count.kv_items(folder)
        world.launch()
        verified = world.script('again', check=False)
        result['second_reopen'] = evidence(folder, 'again').get('counted_again')
        if verified:
            raise AssertionError('second reopen failed; see counted evidence')
        world.quit()
        # The same after the second reopen, before anything is drained (R7-4).
        result['items_again'] = disk_count.items(folder, items)
        result['kv_items_again'] = disk_count.kv_items(folder)
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
        # The pile's two chunks, each first: the pairs of a cut by record index that wrote one in one world only.
        for point in args.points or [*POINTS, 'kv_committed', 'live_chunk:2,0', 'live_chunk:3,0', 'sweep']:
            # Each copy is played by the same binary; HORIZON_HOME is inside the copy's own folder.
            if point == 'sweep':
                outcome = sweep(args, base, scratch, homed(scratch / 'point-sweep', False))
            else:
                played = {c: play(args, where, scratch, homed(point_folder(scratch, point, c), c), point, c) for c, where in bases.items()}
                outcome = emission_verdict(played[False], played[True], TOTAL, TOTAL, exact=point in KILLS)
            report['points'].append(outcome)
            if outcome['ok']:
                # Each copy of the world is large: a point that passed leaves only its numbers.
                for c in (False, True):
                    shutil.rmtree(point_folder(scratch, point, c), ignore_errors=True)
            print(json.dumps(outcome), flush=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        match_cuts(report['points'])
        for outcome in report['points']:
            if outcome['ok'] and outcome['point'] != 'sweep':
                for c in (False, True):
                    shutil.rmtree(point_folder(scratch, outcome['point'], c), ignore_errors=True)
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

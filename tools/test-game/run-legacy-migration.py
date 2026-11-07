#!/usr/bin/env python3
"""A world opened from an earlier version's storage that holds a batch of effects (R7-1, N2 of the reviews).

  python3 tools/test-game/run-legacy-migration.py --binary B --resources R --bundle H --scratch-root S [CASE ...]

The storage is written in the format R5 wrote: one batch, owned by a machine at (0, 100, 0), that puts one niter out
at a cell whose chunk is not loaded (`owed`: the world owes it in the owner's record until that chunk comes) or is
(`entity`: it drops at once). The world is opened, the batch played, and a save is cut:

* `kv_committed`: a save that does not wait, its storage written while the region writer holds every record;
* `players_saved`: a waited save whose records and entities are on the disk, its storage not (old storage, new
  records: the record's `migrated` marker must keep the storage's copy from adding a second niter);
* `waited`: a waited save, all of it.

Then the world is opened twice. Each opening holds exactly one niter, item by item (disk_count.py, after a quit), the
second as the first; once the drop's chunk is in, it is one entity, and a waited save leaves the storage holding none.
Only the storage of the cut may still hold the batch.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import importlib.util

here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
spec = importlib.util.spec_from_file_location('crash_proof', here / 'run-crash-proof.py')
crash_proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crash_proof)
from native_game import Game  # noqa: E402
from native_crash import ledger  # noqa: E402
import disk_count  # noqa: E402
import ntm_store  # noqa: E402

ITEM = 'ntm:niter'
OWNER = [0, 100, 0]
DROPS = {'owed': [1000, 100, 0], 'entity': [8, 100, 8]}
# The drop's area, forced in for the last opening (the owed niter is paid when its chunk loads).
FORCE = {'owed': '992 0 1007 15', 'entity': '0 0 15 15'}
CUTS = ('kv_committed', 'players_saved', 'waited')
CASES = [f'{branch}:{cut}' for branch in DROPS for cut in CUTS]


def legacy_store(folder, branch):
    """The storage as R5 left it: the world's state with the batch in flight (`machine_effects`)."""
    stores = ntm_store.folders(folder)
    assert len(stores) == 1, stores
    store = stores[0]
    keys = ntm_store.keys(store)
    data = json.loads(keys['world'])
    data['machine_effects'] = [{'serial': 77, 'done': 0, 'owners': [['minecraft:overworld', OWNER]], 'origins': [],
                                'effects': [{'Items': {'dim': 'minecraft:overworld', 'pos': DROPS[branch],
                                                       'items': [[ITEM, 1]]}}],
                                'tombstones': []}]
    keys['world'] = json.dumps(data, separators=(',', ':')).encode()
    raw = bytearray(b'ntm-store 1\n')
    for k, v in keys.items():
        b = k.encode()
        raw += struct.pack('<I', len(b)) + b + struct.pack('<I', len(v)) + v
    (store / 'state').write_bytes(raw)


def opening(world, folder, label, force=None):
    """One opening of the world: what it holds once it settled, item by item, after a quit (a waited save)."""
    world.launch()
    g = Game(folder, label)
    g.ctl('wait-ready 120')
    g.command('tp @s 0.5 110 0.5')
    if force:
        g.command('forceload add ' + force)
    time.sleep(8)
    led = ledger(g)
    world.quit()
    return {'ledger': led, 'items': disk_count.items(folder, (ITEM,)), 'kv_items': disk_count.kv_items(folder)}


def play(args, base, folder, env, case):
    branch, cut = case.split(':')
    crash_proof.clone_world(base, folder, shutil.ignore_patterns('control.sock'))
    legacy_store(folder, branch)
    world = crash_proof.World(args, folder, env)
    result = {'case': case, 'ok': False, 'stored_before': disk_count.kv_items(folder)}
    try:
        world.launch(cut if cut != 'waited' else None)
        g = Game(folder, 'legacy-' + cut)
        g.ctl('wait-ready 120')
        g.command('tp @s 0.5 110 0.5')
        time.sleep(6)
        result['played'] = ledger(g)
        if cut == 'kv_committed':
            try:
                g.ctl('pause')
            except Exception:
                pass
            result['exit'] = world.died()
            result['reached'] = crash_proof.reached(world, cut)
        elif cut == 'players_saved':
            try:
                g.ctl('save')
            except Exception:
                pass
            result['exit'] = world.died()
            result['reached'] = crash_proof.reached(world, cut)
        else:
            g.ctl('save')
            world.client.kill()
            result['exit'] = world.client.wait()
            result['reached'] = True
        (folder / 'control.sock').unlink(missing_ok=True)
        if result['exit'] is None:
            world.client.kill()
            world.client.wait()
            raise AssertionError('not killed at ' + cut)
        result['stored_at_cut'] = disk_count.kv_items(folder)
        result['items_at_cut'] = disk_count.items(folder, (ITEM,))
        result['first'] = opening(world, folder, 'legacy-first')
        result['second'] = opening(world, folder, 'legacy-second')
        result['paid'] = opening(world, folder, 'legacy-paid', FORCE[branch])
        one = {ITEM: 1}
        result['verdict'] = {
            'reached': bool(result['reached']),
            'first_holds_one': result['first']['items'] == one,
            'second_as_first': result['second']['items'] == result['first']['items'],
            'paid_holds_one': result['paid']['items'] == one,
            # Every opening ends with a waited save (the quit): the storage forgets the batch once a record took it.
            'storage_clean': all(result[k]['kv_items'] == 0 for k in ('first', 'second', 'paid')),
            # A save that does not wait retires nothing: the storage of the cut still holds the batch.
            'kept_by_the_cut': result['stored_at_cut'] == (0 if cut == 'waited' else 1),
        }
        result['ok'] = all(result['verdict'].values())
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
    return result


def main():
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('cases', nargs='*')
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    args.binary, args.bundle = args.binary.resolve(), args.bundle.resolve()
    scratch = Path(tempfile.mkdtemp(prefix='legacy-', dir=args.scratch_root))
    print(scratch, flush=True)
    base = scratch / 'base'
    base.mkdir()
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='')
    (base / 'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nmaxFps:30\nenableVsync:false\nguiScale:2\ntutorialStep:none\n')
    (base / 'horizon-controller.txt').write_text('enabled:false\n')
    (base / 'horizon-graphics.txt').write_text('workers:2\n')

    def homed(folder):
        copy = dict(env)
        for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                         ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
            copy[key] = str(folder / sub)
        return copy
    reader, writer = os.pipe()
    with (scratch / 'xvfb.log').open('w') as log:
        xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0', '1280x720x24', '-nolisten', 'tcp'],
                                pass_fds=(writer,), stdout=log, stderr=subprocess.STDOUT)
    os.close(writer)
    with os.fdopen(reader) as pipe:
        env['DISPLAY'] = ':' + pipe.readline().strip()
    report = {'ok': False, 'binary_sha256': crash_proof.digest(args.binary), 'bundle_sha256': crash_proof.digest(args.bundle),
              'cases': []}
    try:
        world = crash_proof.World(args, base, homed(base))
        world.launch()
        g = Game(base, 'legacy-prepare')
        g.ctl('wait-ready 120')
        g.close()
        g.command('gamemode creative @s')
        g.command('tp @s 0.5 110 0.5')
        g.command('forceload add 0 0 15 15')
        time.sleep(3)
        g.ctl('save')
        world.quit()
        for case in args.cases or CASES:
            folder = scratch / ('case-' + case.replace(':', '-'))
            outcome = play(args, base, folder, homed(folder), case)
            report['cases'].append(outcome)
            print(json.dumps(outcome), flush=True)
            if outcome['ok']:
                shutil.rmtree(folder, ignore_errors=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        report['ok'] = bool(report['cases']) and all(c['ok'] for c in report['cases'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

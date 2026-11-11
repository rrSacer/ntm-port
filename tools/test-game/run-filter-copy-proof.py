#!/usr/bin/env python3
"""Save cuts of the Java's filter-copy bug (JAVA_FILTER_COPY_BUG, docs/onda-1-protecao.md) against a vanilla control.

Example (wrap in agent-run env QUEUE=game):
  python3 tools/test-game/run-filter-copy-proof.py --binary PATH --resources PATH --scratch-root PATH [POINT ...]

A base world (native_filter_copy.py prepare: one filter, inside a worn helmet's attachment mask) is copied for every
point and for the control. The mod's copy is exposed to lung gas, which writes the worn copy into the helmet (two
filters, the Java's bug); the control's gets one filter by `/give` (two filters too). Then a save the failpoint ends at
POINT. Before each reopen the runner counts, in the player's file, every filter nested anywhere in it; both reopens
must count what the file held, and the mod's world must hold what the control holds at the same cut: the save adds no
loss and no second duplication beyond the Java's.
"""
import argparse
import gzip
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
spec = importlib.util.spec_from_file_location('crash_proof', here / 'run-crash-proof.py')
crash_proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crash_proof)
disk_count = crash_proof.disk_count

FILTER = 'ntm:gas_mask_filter'
# The cuts that touch the player's file or come before it, and the two deaths outside a save's stages.
POINTS = ['none', 'kill', 'save_begin', 'chunks_written', 'level_saved', 'players_saved', 'world_saved', 'kv_committed']


class Copy(crash_proof.World):
    def script(self, mode, check=True, extra=None):
        return subprocess.run([sys.executable, str(here / 'native_filter_copy.py'), str(self.folder), mode],
                              cwd=self.folder, env=dict(self.env, **(extra or {})), check=check, timeout=1200).returncode


def evidence(folder, mode):
    path = folder / f'native-filter-copy-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def nested(value):
    """Filters anywhere in a parsed NBT value: every compound whose id is the filter counts its `count`."""
    if isinstance(value, dict):
        own = int(value.get('count', value.get('Count', 1)) or 0) if value.get('id') == FILTER else 0
        return own + sum(nested(v) for v in value.values())
    if isinstance(value, list):
        return sum(nested(v) for v in value)
    return 0


def on_disk(folder):
    """The filters the owner's player file holds, nested ones included. The owner is level.dat's
    `singleplayer_uuid`, as disk_count reads it: a test world also keeps a second player file."""
    world = Path(folder) / 'saves' / 'CrashTest'
    data = disk_count.parse(gzip.decompress((world / 'level.dat').read_bytes())).get('Data', {})
    owner = data.get('singleplayer_uuid')
    if not (isinstance(owner, list) and len(owner) == 4):
        raise AssertionError('no singleplayer_uuid in level.dat')
    h = ''.join(f'{v & 0xffffffff:08x}' for v in owner)
    path = world / 'players' / 'data' / f'{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}.dat'
    return nested(disk_count.parse(gzip.decompress(path.read_bytes()))) if path.exists() else 0


def play(args, base, scratch, env, point, control):
    folder = scratch / ('point-' + point + ('-vanilla' if control else ''))
    crash_proof.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-filter-copy-*.json'))
    world = Copy(args, folder, env)
    result = {'point': point, 'vanilla_control': control, 'ok': False, 'base_disk': on_disk(base)}
    try:
        world.launch(None if point == 'kill' else point)
        acted = world.script('act', check=False, extra=crash_proof.act_extra(point))
        act = evidence(folder, 'act')
        result['after_act'] = act.get('after_act')
        result['act_checks'] = {k: v['ok'] for k, v in act.get('checks', {}).items()}
        if acted and world.client.poll() is None:
            raise AssertionError('action failed before its save; see act evidence')
        if point == 'none':
            world.quit()
        elif point == 'kill':
            world.client.kill()
            world.client.wait()
            (folder / 'control.sock').unlink(missing_ok=True)
        else:
            code = world.died()
            if code is None:
                world.client.kill()
                world.client.wait()
                raise AssertionError('the game was not killed at ' + point)
            result['killed_at_the_point'] = crash_proof.reached(world, point)
            if not result['killed_at_the_point']:
                raise AssertionError('client exited without reaching ' + point)
            (folder / 'control.sock').unlink(missing_ok=True)
        result['disk'] = on_disk(folder)
        for mode, key in (('verify', 'first_reopen'), ('again', 'second_reopen')):
            world.launch()
            failed = world.script(mode, check=False)
            counted = evidence(folder, mode).get('counted' + ('_again' if mode == 'again' else ''))
            result[key] = (counted or {}).get('total')
            world.quit()
            result[key + '_disk'] = on_disk(folder)
            if failed:
                raise AssertionError(key + ' failed; see its evidence')
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
    return result


def verdict(mine, control):
    point = mine['point']
    out = {'point': point, 'mod': mine, 'control': control}
    # The reopens hold what the files hold. At `kv_committed` the player's file can lag the storage the save already
    # committed (the reopen then holds the committed state, in the mod's world and in the control's alike), so the
    # file before the reopen is reported and the file after it must agree.
    out['as_the_file'] = all(o.get('first_reopen') == o.get('second_reopen') == o.get('second_reopen_disk')
                             and (point == 'kv_committed' or o.get('first_reopen') == o.get('disk'))
                             for o in (mine, control))
    out['same_as_control'] = mine.get('first_reopen') is not None and mine.get('first_reopen') == control.get('first_reopen')
    out['within_the_java'] = mine.get('first_reopen') in (1, 2)
    if point in ('none',):
        out['exact'] = mine.get('first_reopen') == 2 and control.get('first_reopen') == 2
    elif point == 'kill':
        out['exact'] = mine.get('first_reopen') == 1 and control.get('first_reopen') == 1
    else:
        out['exact'] = True
    out['ok'] = mine['ok'] and control['ok'] and all(out[k] for k in ('as_the_file', 'same_as_control', 'within_the_java', 'exact'))
    return out


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
    work = here.parents[1]
    args.binary = args.binary.resolve()
    args.bundle = (args.bundle or work / 'dist/ntm.hmod').resolve()
    scratch = Path(tempfile.mkdtemp(prefix='filter-copy-', dir=args.scratch_root))
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
    report = {'ok': False, 'binary_sha256': crash_proof.digest(args.binary), 'bundle_sha256': crash_proof.digest(args.bundle),
              'points': []}

    def homed(folder, control):
        copy_env = dict(env, NTM_CRASH_VANILLA='1' if control else '0')
        for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                         ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
            copy_env[key] = str(folder / sub)
        return copy_env
    try:
        world = Copy(args, base, homed(base, False))
        world.launch()
        world.script('prepare')
        world.quit()
        report['base_disk'] = on_disk(base)
        for point in args.points or POINTS:
            played = {c: play(args, base, scratch, homed(scratch / ('point-' + point + ('-vanilla' if c else '')), c), point, c)
                      for c in (False, True)}
            outcome = verdict(played[False], played[True])
            report['points'].append(outcome)
            print(json.dumps({'point': point, 'ok': outcome['ok'], 'mod': played[False].get('first_reopen'),
                              'control': played[True].get('first_reopen')}), flush=True)
            if outcome['ok']:
                for c in ('', '-vanilla'):
                    shutil.rmtree(scratch / ('point-' + point + c), ignore_errors=True)
        report['ok'] = report['base_disk'] == 1 and all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'ok': report['ok']}))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""Save cuts of a player's state kept in a player attachment (docs/onda-1-protecao.md, engine defect of the owner's
attachments): the armor table, a RadAway's treatment, the herbal pill's sickness.

Example (wrap in agent-run env QUEUE=game):
  python3 tools/test-game/run-player-cut-proof.py SCENARIO --binary PATH --resources PATH --scratch-root PATH [POINT ...]

SCENARIO is table, radaway, herbal, irradiation, migration or field (native_player_cut.py). A base world is prepared once and copied for every point;
`table` also plays a vanilla control (the two stacks moved into the Ender chest). After each cut both reopens are
counted in game. `table`: each item must be held once (inventory, Ender chest, the ground the table drops it on, nested
in a stack), as in the control. `radaway`/`herbal`: the used item and its effect go together: either the item is still
held, or the treatment (RadAway ticks, a lower dose) / the sickness is there; an item gone without its effect is a loss.
`irradiation` (slice 5c, with the Ender chest control): the radiation potion and the dose it gave go together, and the
effect shows the potion. `migration` (with the control): prepared by `--prepare-binary` (the binary before 5c, whose
potion lives in the world's storage); after every cut the potion is in the attachment, at its amplifier, with its
effect, and the doses it gave plus its ticks left are no more than its 600 ticks (nothing dosed twice).
`field` (slice 4a, with the control): a chunk at 100 000 by `/ntmrad`; its save is the NTM storage's own key, committed
after the chunks. After every cut the key on the disk is one of the two commits, chunk by chunk: the last one exactly,
or the act's save, one instant of the map (every loaded chunk as it was right before the save, or every loaded chunk as
the next update gave it; a chunk not loaded keeps its last save exactly); a chunk lost, over its value, two instants
mixed or an unloaded chunk rewritten is neither. The source chunk is never lost.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time

here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
spec = importlib.util.spec_from_file_location('crash_proof', here / 'run-crash-proof.py')
crash_proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crash_proof)
disk_count = crash_proof.disk_count

FILTER = 'ntm:gas_mask_filter'
# The migration's potion budget (doses given + ticks left), from the prepare's evidence.
BUDGET = [None]
RADAWAY_MILLIRAD = 140 * 1000
HERBAL_MILLIRAD = 100 * 1000
# The cuts that touch the player's file or come before it, and the two deaths outside a save's stages.
POINTS = ['none', 'kill', 'save_begin', 'level_saved', 'players_saved', 'world_saved', 'kv_committed']


class Copy(crash_proof.World):
    def launch(self, failpoint=None):
        """A client may exit before it opens its socket when the machine is loaded, right after the one a cut killed
        (review 5d P3-4: the vanilla control at players_saved and kv_committed). Wait and start it again, up to three
        times; each early exit is kept for the report with the end of its log."""
        self.early_exits = getattr(self, 'early_exits', [])
        for attempt in range(3):
            try:
                return super().launch(failpoint)
            except RuntimeError as error:
                if 'before opening its socket' not in str(error) or attempt == 2:
                    raise
                tail = self.log.read_text(errors='replace')[-400:] if self.log.exists() else ''
                self.early_exits.append({'failpoint': failpoint, 'attempt': attempt + 1, 'log_tail': tail})
                time.sleep(5 * (attempt + 1))

    def script(self, mode, check=True, extra=None):
        return subprocess.run([sys.executable, str(here / 'native_player_cut.py'), str(self.folder), mode],
                              cwd=self.folder, env=dict(self.env, **(extra or {})), check=check, timeout=1200).returncode


def evidence(folder, mode):
    path = folder / f'native-player-cut-{os.environ["NTM_CUT_SCENARIO"]}-{mode}.json'
    return json.loads(path.read_text()) if path.exists() else {}


def play(args, base, scratch, env, point, control):
    folder = scratch / ('point-' + point + ('-vanilla' if control else ''))
    crash_proof.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-player-cut-*.json'))
    world = Copy(args, folder, env)
    result = {'point': point, 'vanilla_control': control, 'ok': False}
    try:
        world.launch(None if point == 'kill' else point)
        acted = world.script('act', check=False, extra=crash_proof.act_extra(point))
        act = evidence(folder, 'act')
        result['after_act'] = act.get('after_act')
        result['act_before'] = act.get('before')
        result['before_cut'] = act.get('before_cut')
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
        for mode, key in (('verify', 'first_reopen'), ('again', 'second_reopen')):
            world.launch()
            failed = world.script(mode, check=False)
            counted = evidence(folder, mode).get('counted' + ('_again' if mode == 'again' else ''))
            result[key] = counted
            world.quit()
            if failed:
                raise AssertionError(key + ' failed; see its evidence')
        result['ok'] = True
        result['early_exits'] = getattr(world, 'early_exits', [])
    except Exception as error:
        result['early_exits'] = getattr(world, 'early_exits', [])
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
    return result


def coherent(scenario, c, base, may_have_ended=False):
    """No item gone without its effect (radaway/herbal), each table piece held once."""
    if c is None:
        return False
    if scenario == 'table':
        return all(v == 1 for v in c['items'].values())
    if scenario in ('irradiation', 'migration'):
        st = c['state']
        shown = 'radiation' in st.get('effects', [])
        attached = st.get('irradiated', 0) > 0 and st.get('irradiated_level') == 3 and shown
        if scenario == 'migration':
            # Every dose the potion gave (0.2 RAD a tick) plus the ticks it has left is at most its budget G0, measured
            # at the end of the prepare (re-review RR-3): a potion taken in twice would give its time again (review
            # P2-1). The first 200 ticks of a reopening count down without a dose, so the sum only falls. A potion that
            # ran out (0 ticks, no effect) is accepted only at the second reopening, after the first showed it
            # (re-review RR3-2): a migration that threw the potion away must not pass for one that ran out.
            given = (st.get('dose', base) - base) // 200
            left = st.get('irradiated', 0)
            ended = may_have_ended and left == 0 and not shown
            return BUDGET[0] is not None and (attached or ended) and given + left <= BUDGET[0]
        # Before 5c the potion lives in the world's storage, its dose in the player's file; since, both in one
        # attachment. Either way the potion and the dose it gave go together: both or neither.
        potion = attached or any(level == 3 for _, level in st.get('kv_irradiated', []))
        return potion == (st.get('dose', base) > base) and (potion or not shown)
    item = next(iter(c['items']))
    held, st = c['items'][item], c['state']
    if held:
        return True
    if scenario == 'radaway':
        # A RadAway is 140 ticks of -1 RAD (consumables spec `radaway: 140`): once used, what was given plus
        # what is still owed is the whole treatment, so the dose ends at base - 140 RAD.
        return st.get('dose', base) - 1000 * st.get('radaway', 0) == base - RADAWAY_MILLIRAD
    # The herbal pill takes 100 RAD at once (spec dose -100_000) and starts the sickness timer: both or neither.
    return st.get('dose', base) == base - HERBAL_MILLIRAD and st.get('herbal', 0) > 0


def field_store_verdict(base, pre, first):
    """The field's key at a reopening against the two commits it may be, chunk by chunk, compared as f32 (the storage
    writes each f32 as its shortest decimal). `last`: the last commit, exactly, every chunk and no other. `new`: the
    act's save, one instant of the map: every loaded chunk as the map held it right before the save, or every loaded
    chunk as the next update gave it (an update may fall between that reading and the save); a chunk not loaded keeps
    its last save exactly. A chunk the key lacks is 0: a loss unless the instant had it at 0. Anything else (a chunk
    lost, one over its value, two instants mixed, an unloaded chunk rewritten) is neither."""
    f32 = lambda x: struct.unpack('<f', struct.pack('<f', x))[0]
    base = {k: f32(v) for k, v in base.items()}
    first = {k: f32(v) for k, v in first.items()}
    if first == base:
        return 'last', {}
    keys = set(base) | set(first) | set(pre)
    best = None
    for instant in (0, 1):
        wrong = {}
        for key in keys:
            raw, nxt, loaded = pre.get(key, [None, 0.0, False])
            want = f32((raw, nxt)[instant] or 0.0) if loaded else base.get(key, 0.0)
            if first.get(key, 0.0) != want:
                wrong[key] = [first.get(key), want, 'loaded' if loaded else 'not loaded']
        if not wrong:
            return 'new', {}
        best = wrong if best is None or len(wrong) < len(best) else best
    return 'neither', best


def field_coherent(mine):
    """`field`: the key on the disk at the first reopen is one of the two commits ([`field_store_verdict`]): the last
    one (the act's `before`, read before its session saved) or the act's save; the source chunk is in it, and still at
    the second reopen."""
    try:
        base = mine['act_before']['field']['store']
        pre = mine['before_cut']['chunks']
        first = mine['first_reopen']['field']['store']
        second = mine['second_reopen']['field']['store']
    except (KeyError, TypeError):
        return False, None
    if 'error' in base or 'error' in first or 'error' in second:
        return False, None
    commit, wrong = field_store_verdict(base, pre, first)
    source = '1,0'
    ok = commit != 'neither' and first.get(source, 0.0) > 0.0 and second.get(source, 0.0) > 0.0
    return ok, {'commit': commit, 'wrong': wrong, 'source': [base.get(source), (pre.get(source) or [None])[0], first.get(source), second.get(source)]}


def verdict(scenario, mine, control):
    point = mine['point']
    out = {'point': point, 'mod': mine, 'control': control}
    if scenario == 'field':
        out['coherent'], out['field'] = field_coherent(mine)
        out['reopens_agree'] = True
        out['control_coherent'] = control is not None and all(coherent('table', control.get(r), 300000) for r in ('first_reopen', 'second_reopen'))
        out['ok'] = bool(mine['ok'] and out['coherent'] and control is not None and control['ok'] and out['control_coherent'])
        return out
    first = coherent(scenario, mine.get('first_reopen'), 300000)
    out['coherent'] = first and coherent(scenario, mine.get('second_reopen'), 300000, may_have_ended=first)
    out['reopens_agree'] = mine.get('first_reopen', {}) and (mine['first_reopen'] or {}).get('items') == (mine.get('second_reopen') or {}).get('items')
    if control is not None:
        # The control is the table's: the two stacks moved into the Ender chest, each held once.
        out['control_coherent'] = all(coherent('table', control.get(r), 300000) for r in ('first_reopen', 'second_reopen'))
    out['ok'] = bool(mine['ok'] and out['coherent'] and out['reopens_agree'] and (control is None or (control['ok'] and out['control_coherent'])))
    return out


def main():
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('scenario', choices=['table', 'radaway', 'herbal', 'irradiation', 'migration', 'field'])
    parser.add_argument('--prepare-binary', type=Path, help='the binary that prepares the base world (migration)')
    parser.add_argument('points', nargs='*')
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    work = here.parents[1]
    args.binary = args.binary.resolve()
    args.bundle = (args.bundle or work / 'dist/ntm.hmod').resolve()
    scratch = Path(tempfile.mkdtemp(prefix='player-cut-' + args.scenario + '-', dir=args.scratch_root))
    print(scratch, flush=True)
    base = scratch / 'base'
    base.mkdir()
    env = os.environ.copy()
    env['NTM_CUT_SCENARIO'] = os.environ['NTM_CUT_SCENARIO'] = args.scenario
    # `field` reads the map through `ntmtest rad`.
    if args.scenario == 'field':
        env['NTM_TEST_HOOKS'] = '1'
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
        controls = (False, True) if args.scenario in ('table', 'irradiation', 'migration', 'field') else (False,)
        bases = {c: scratch / ('base' + ('-vanilla' if c else '')) for c in controls}
        for c, where in bases.items():
            where.mkdir(exist_ok=True)
            for f in ('options.txt', 'horizon-controller.txt', 'horizon-graphics.txt'):
                if (base / f).exists() and where != base:
                    shutil.copy(base / f, where / f)
            binary = args.binary
            if args.prepare_binary:
                args.binary = args.prepare_binary.resolve()
                report['prepare_binary_sha256'] = crash_proof.digest(args.binary)
            world = Copy(args, where, homed(where, c))
            world.launch()
            world.script('prepare')
            world.quit()
            args.binary = binary
            if args.scenario == 'migration' and not c:
                prepared = json.loads((where / 'native-player-cut-migration-prepare.json').read_text())
                BUDGET[0] = (prepared.get('budget') or {}).get('g0')
                report['budget'] = prepared.get('budget')
                print(json.dumps({'budget': report['budget']}), flush=True)
        for point in args.points or POINTS:
            played = {c: play(args, bases[c], scratch, homed(scratch / ('point-' + point + ('-vanilla' if c else '')), c), point, c)
                      for c in controls}
            outcome = verdict(args.scenario, played[False], played.get(True))
            report['points'].append(outcome)
            print(json.dumps({'point': point, 'ok': outcome['ok'], 'field': outcome.get('field'), 'mod': played[False].get('first_reopen'),
                              'control': (played.get(True) or {}).get('first_reopen')}), flush=True)
            if outcome['ok']:
                for c in ('', '-vanilla'):
                    shutil.rmtree(scratch / ('point-' + point + c), ignore_errors=True)
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'ok': report['ok']}))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

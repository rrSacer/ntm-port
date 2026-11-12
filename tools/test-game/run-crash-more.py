#!/usr/bin/env python3
"""The scenarios of native_crash_more.py, each killed at the boundaries of a save (see run-crash-proof.py).

  python3 tools/test-game/run-crash-more.py --binary B --resources R --scratch-root S SCENARIO [POINT ...]

SCENARIO: coupled-a coupled-b coupled-ea coupled-eb crossing crossing-kill mining break replace frozen-break frozen-unload origin-pickup
origin-pickup-away origin-merge origin-vanish origin-kill merge-stale origin-away pickup-merge. Points default to the
ones that matter for the scenario; `kill` ends the process with SIGKILL after the action, outside any save. A world that
passed is deleted; the report keeps the numbers.

A scenario built on a player's break (`BROKEN` of native_crash_more.py) is played twice at each point, with the mixers
and with vanilla chests in their place, from bases prepared the same way: it passes when both reopens count what the
chests count. A scenario of an effect of the mod (`EMITTING`) is played with its vanilla control too (loose items, or a
chest broken by hand): it passes when both reopens count what the files held before them (disk_count.py), nothing is
held twice and no more is lost than the control loses (`emission_verdict` of run-crash-proof.py).
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
INCOMING = 'ntm:pile_rod__zr'
BROKEN = ('break', 'replace', 'furnace-break', 'keyforge-break', 'funnel-break', 'orbus-break', 'iron-break', 'steel-break', 'combination-break', 'frozen-break', 'frozen-unload', 'origin-pickup', 'origin-pickup-away', 'origin-merge', 'origin-vanish', 'origin-kill',
          'merge-stale', 'origin-away', 'pickup-merge')
# The scenarios whose action writes files itself (a chunk it unloads): after a death outside a save they hold what those
# files hold, not the last save.
WRITES_IN_ACT = ('crossing', 'crossing-kill', 'origin-pickup', 'origin-pickup-away', 'origin-merge', 'origin-vanish',
                 'origin-kill', 'merge-stale', 'origin-away', 'pickup-merge',
                 # The loader's chunk leaves before the break, its record and entities written.
                 'coupled-break',
                 # The core's chunk leaves after the break, its record and entities written.
                 'frozen-unload')
# scenario -> (items counted, what the mod's world holds, what the control's holds)
# The transfers between two chunks, with what both worlds hold: counted equal to the control, as a break is.
TRANSFERS = {'coupled-a': 1, 'coupled-b': 1, 'coupled-ea': 6, 'coupled-eb': 6}
EMITTING = {'crossing': (('ntm:pile_rod',), 12, 12), 'crossing-kill': (('ntm:pile_rod',), 10, 10),
            'save-quit': (('ntm:pile_rod',), 12, 12), 'mining': (('minecraft:diamond',), 7, 7),
            # R6-2: the transfer of a Zr into a full channel of five Ra-226Be, then the core broken while a member's
            # chunk is out: its rods go out (owed in the core's record while their chunk is out).
            'coupled-break': (('ntm:pile_rod',), 6, 6)}


# The multiblock furnaces' break scenarios: the items counted before the cut (6 + 1, 6 + 3, 8 + 2).
MB_TOTALS = {'keyforge-break': 2, 'funnel-break': 9, 'orbus-break': 6, 'iron-break': 7, 'steel-break': 9, 'combination-break': 10}


# Breaks of a machine holding rods (its hidden contents), with what both worlds hold before: counted equal to the control.
HIDDEN = {'frozen-break': 12, 'frozen-unload': 12}


def items_of(scenario):
    if scenario == 'furnace-smelt':
        return ('minecraft:raw_iron', 'minecraft:iron_ingot')
    if scenario == 'welder-run':
        return ('ntm:plate_steel', 'ntm:motor')
    if scenario in ('iron-state', 'steel-state', 'combination-state'):
        return {'iron-state': ('minecraft:raw_iron', 'minecraft:iron_ingot'), 'steel-state': ('minecraft:iron_ore', 'minecraft:iron_ingot'),
                'combination-state': ('minecraft:coal', 'ntm:coke__coal')}[scenario]
    if scenario == 'keyforge-state':
        return ('ntm:key', 'ntm:padlock')
    if scenario == 'keyforge-break':
        return ('ntm:key', 'ntm:padlock')
    if scenario == 'funnel-state':
        return ('minecraft:gold_ingot', 'minecraft:gold_block')
    if scenario == 'funnel-break':
        return ('minecraft:dirt', 'minecraft:stick')
    if scenario == 'orbus-state':
        return ('minecraft:water_bucket',)
    if scenario == 'orbus-break':
        return ('ntm:fluid_tank_empty',)
    if scenario in ('iron-break', 'steel-break', 'combination-break'):
        return {'iron-break': ('minecraft:raw_iron', 'ntm:upgrade_speed_1'), 'steel-break': ('minecraft:raw_iron', 'minecraft:raw_copper'),
                'combination-break': ('minecraft:coal', 'minecraft:bucket')}[scenario]
    if scenario in EMITTING:
        return EMITTING[scenario][0]
    return ('ntm:niter',) if scenario in BROKEN and scenario not in HIDDEN else ('ntm:pile_rod',)

DEFAULT_POINTS = {
    'vanilla-break': ['kill', 'kv_committed', 'chunks_written', 'entities_written'],
    'coupled-a': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'coupled-b': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'coupled-ea': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'coupled-eb': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:2,0', 'live_chunk:3,0'],
    'crossing': ['none', 'kill', 'kv_committed'],
    'origin-pickup': ['none', 'kill', 'kv_committed'],
    'origin-pickup-away': ['none', 'kill', 'kv_committed'],
    'origin-merge': ['none', 'kill', 'kv_committed'],
    'origin-vanish': ['none', 'kill', 'kv_committed'],
    'origin-kill': ['kill', 'kv_committed'],
    'crossing-kill': ['kill', 'kv_committed'],
    'merge-stale': ['kill', 'kv_committed'],
    'origin-away': ['kill', 'kv_committed'],
    'pickup-merge': ['kill', 'kv_committed'],
    'save-quit': ['none', 'kill', 'kv_committed'],
    'coupled-break': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written'],
    # The chest's chunk first, as the pair of a cut by record index that wrote it in one world only.
    'mining': [*crash_proof.POINTS, 'kv_committed', 'live_chunk:1,0'],
    # The mixer's chunk first (`live_chunk`): a cut after the first record of a batch hits another chunk in the control.
    'break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'furnace-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'welder-run': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'furnace-smelt': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'keyforge-state': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'keyforge-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'funnel-state': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'funnel-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'orbus-state': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'orbus-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'iron-state': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'steel-state': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'combination-state': ['none', 'kill', 'kv_committed', 'save_begin', 'chunks_written', 'entities_written', 'players_saved'],
    'iron-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'steel-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'combination-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'replace': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:8,0', 'chunks_written', 'entities_written'],
    'frozen-break': ['none', 'kill', 'kv_committed', 'save_begin', 'live_chunk:2,0', 'chunks_written', 'entities_written'],
    'frozen-unload': ['none', 'kill', 'kv_committed'],
}


class More(crash_proof.World):
    def __init__(self, args, folder, env, scenario):
        super().__init__(args, folder, env)
        self.scenario = scenario

    def script(self, mode, check=True, extra=None):
        env = dict(self.env, NTM_CRASH_SCENARIO=self.scenario, **(extra or {}))
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


def record(folder, kind, cx, cz):
    """The decompressed record of chunk (cx, cz) in `kind` (region/entities) of the test world, or b''."""
    path = folder / f'saves/CrashTest/dimensions/minecraft/overworld/{kind}/r.{cx >> 5}.{cz >> 5}.mca'
    if not path.exists():
        return b''
    data = path.read_bytes()
    i = ((cx & 31) + (cz & 31) * 32) * 4
    offset = int.from_bytes(data[i:i + 3], 'big') * 4096
    if offset == 0:
        return b''
    length = int.from_bytes(data[offset:offset + 4], 'big')
    return zlib.decompress(data[offset + 5:offset + 4 + length])


def stale_disk(folder, scenario):
    """What the files say before the reopen, for the scenarios built on an entity file that lags: tagged niter
    entities in each holder's file, and the folded markers of the origin's ledger."""
    if scenario not in ('merge-stale', 'origin-away', 'pickup-merge'):
        return None
    import re
    tag = rb'[0-9a-f]{16}:\d+@-?\d+,-?\d+'
    out = {}
    for name, (cx, cz) in {'A': (8, 0), 'B': (31, 0), 'C': (50, 0)}.items():
        raw = record(folder, 'entities', cx, cz)
        out[name] = {'niter_entities': raw.count(b'ntm:niter'), 'tags': [t.decode() for t in re.findall(tag, raw)]}
    ledger = record(folder, 'region', 8, 0)
    found = re.search(rb'\{"entries".*?\}(?=\x00|$)', ledger, re.S) or re.search(rb'\{"(applied|gone)".*?\}(?=\x00|$)', ledger, re.S)
    out['origin_ledger'] = found.group(0).decode(errors='replace')[:3000] if found else None
    return out


def point_folder(scratch, point, control):
    return scratch / ('point-' + point.replace(':', '-') + ('-vanilla' if control else ''))


def play(args, base, scratch, env, scenario, point, control=False):
    folder = point_folder(scratch, point, control)
    crash_proof.clone_world(base, folder, shutil.ignore_patterns('control.sock', 'native-crash-more-act.json',
                                                                 'native-crash-more-verify.json', 'native-crash-more-again.json'))
    world = More(args, folder, env, scenario)
    result = {'scenario': scenario, 'point': point, 'folder': str(folder), 'ok': False, 'vanilla_control': control}
    result['base_items'] = crash_proof.disk_count.items(base, items_of(scenario))
    try:
        world.launch(None if point == 'kill' else point)
        # `kill`: the action asks for no save, and the process dies where it left the world; `kv_committed`: its save
        # does not wait (the pause).
        acted = world.script('act', check=False, extra=crash_proof.act_extra(point))
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
            result['killed_at_the_point'] = crash_proof.reached(world, point)
            if code is None:
                world.client.kill()
                world.client.wait()
                raise AssertionError('the game was not killed at ' + point)
            if not result['killed_at_the_point']:
                raise AssertionError('client exited without reaching ' + point)
            (folder / 'control.sock').unlink(missing_ok=True)
        result['disk_before_reopen'] = saved_coupled(folder, scenario) or stale_disk(folder, scenario)
        result['disk'] = crash_proof.disk_count.count(folder, items_of(scenario))
        result['items'] = crash_proof.disk_count.items(folder, items_of(scenario))
        result['kv_items'] = crash_proof.disk_count.kv_items(folder)
        result['written'] = crash_proof.disk_count.written(base, folder, items_of(scenario))
        if scenario.startswith('coupled') and point.startswith(('live_chunk:', 'region_record:')) and not control:
            before, after = saved_coupled(base, scenario), result['disk_before_reopen']
            changed = [c for c in ('2', '3') if before[c] != after[c]]
            result['disk_base'] = before
            result['new_chunks'] = changed
            result['cut_valid'] = len(changed) == 1
            if point.startswith('live_chunk:'):
                target = point.split(':', 1)[1].split(',')[0]
                result['cut_valid'] &= changed == [target]
            # Both new: the chunk that holds a debt of the step had its record written at the end of that step,
            # before the save could cut between the two. That divergence is no longer reachable; count all the same.
            result['cut_preempted'] = changed == ['2', '3']
            if not result['cut_valid'] and not result['cut_preempted']:
                raise AssertionError('invalid cut: must save exactly one of core/loader chunks before replay')
        world.launch()
        verified = world.script('verify', check=False)
        result['first_reopen'] = evidence(folder, 'verify').get('counted')
        if verified:
            raise AssertionError('first reopen failed; see counted evidence')
        world.quit()
        result['items_after'] = crash_proof.disk_count.items(folder, items_of(scenario))
        result['kv_items_after'] = crash_proof.disk_count.kv_items(folder)
        world.launch()
        verified = world.script('again', check=False)
        result['second_reopen'] = evidence(folder, 'again').get('counted_again')
        if verified:
            raise AssertionError('second reopen failed; see counted evidence')
        world.quit()
        # The same after the second reopen (R7-4).
        result['items_again'] = crash_proof.disk_count.items(folder, items_of(scenario))
        result['kv_items_again'] = crash_proof.disk_count.kv_items(folder)
        result['ok'] = True
    except Exception as error:
        result['error'] = repr(error)
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
    return result


def total(outcome, reopen):
    return (outcome.get(reopen) or {}).get('total')


def against_control(outcome, control):
    """The mod's break counts what the chest counts at the same cut, at both reopens, item by item (R6-3); the reopen
    holds what the files held; neither world's storage keeps what a chunk owes."""
    same = {r: (total(outcome, r), total(control, r)) for r in ('first_reopen', 'second_reopen')}
    equal = all(a is not None and a == b for a, b in same.values())
    checks = {
        'same_items': all(outcome.get(r) is not None and outcome.get(r) == control.get(r) for r in crash_proof.REOPENS),
        'as_the_files_items': all(o.get('items') is not None and all(o.get(r) == o.get('items') for r in crash_proof.REOPENS)
                                  for o in (outcome, control)),
        'storage_clean': all(o.get(k) == 0 for o in (outcome, control) for k in crash_proof.KV_ITEMS),
    }
    ok = outcome['ok'] and control['ok'] and equal and all(checks.values())
    return {**outcome, 'control': control, 'against_control': same, 'item_checks': checks, 'ok': ok}


def eager_verdict(outcome, control):
    """A death outside a save after eager chunk saves wrote files (`crash_proof.wrote_outside_a_save`): when that pass
    ran differs between the worlds, so the mod's world is not held equal to the control. It holds what the files held at
    both reopens, item by item; no item is lost that the control keeps (L) nor held more often than in the base or the
    control (D); neither world's storage keeps what a chunk owes."""
    verdict = {'disk': None, 'first': total(outcome, 'first_reopen'), 'second': total(outcome, 'second_reopen')}
    crash_proof.item_checks(verdict, outcome, control)
    checks = {
        'as_the_files_items': all(o.get('items') is not None and all(o.get(r) == o.get('items') for r in crash_proof.REOPENS)
                                  for o in (outcome, control)),
        'storage_clean': all(o.get(k) == 0 for o in (outcome, control) for k in crash_proof.KV_ITEMS),
        'no_worse_item': verdict['no_worse_item'],
        'no_duplicate_item': verdict['no_duplicate_item'],
    }
    same = {r: (total(outcome, r), total(control, r)) for r in ('first_reopen', 'second_reopen')}
    better = all(a is not None and b is not None and a > b for a, b in same.values())
    if crash_proof.cut(outcome) == crash_proof.cut(control):
        # Both worlds wrote the same chunks: they must hold the same, item by item, as without eager saves.
        checks['same_items'] = all(a is not None and a == b for a, b in same.values()) and all(
            outcome.get(r) == control.get(r) for r in crash_proof.REOPENS)
    ok = outcome['ok'] and control['ok'] and all(checks.values())
    return {**outcome, 'control': control, 'against_control': same, 'item_checks': checks, 'ok': ok,
            'criterion': 'L/D', 'criterion_reason': crash_proof.criterion(outcome['point'], outcome, control)[1],
            'proof': False, 'better_than_control': better}


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
    controlled = args.scenario in BROKEN or args.scenario in EMITTING or args.scenario in TRANSFERS
    try:
        bases = {False: base}
        if controlled:
            bases[True] = scratch / 'base-vanilla'
            shutil.copytree(base, bases[True])
        for control, where in bases.items():
            prepare_env = dict(env, NTM_CRASH_VANILLA='1' if control else '0')
            for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                             ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
                prepare_env[key] = str(where / sub)
            world = More(args, where, prepare_env, args.scenario)
            world.launch()
            world.script('prepare')
            world.quit()
        for point in args.points or DEFAULT_POINTS[args.scenario]:
            outcomes = {}
            for control, where in bases.items():
                copy_env = dict(env, NTM_CRASH_VANILLA='1' if control else '0')
                folder = point_folder(scratch, point, control)
                for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                                 ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
                    copy_env[key] = str(folder / sub)
                outcomes[control] = play(args, where, scratch, copy_env, args.scenario, point, control)
            exact = point in crash_proof.KILLS and args.scenario not in WRITES_IN_ACT
            # Eager chunk saves wrote files outside a save in a world: not the last save; judged by L and D item by item.
            # (a scenario with no vanilla control, such as a machine's own state, has nothing to compare: not eager)
            eager = exact and True in outcomes and crash_proof.wrote_outside_a_save(outcomes[False], outcomes[True])
            exact = exact and not eager
            if args.scenario in EMITTING:
                _, expected, control_expected = EMITTING[args.scenario]
                outcome = crash_proof.emission_verdict(outcomes[False], outcomes[True], expected, control_expected, exact)
                if point in crash_proof.KILLS and not exact and not eager:
                    # A death outside a save after the action wrote a chunk (unloaded it): each chunk is its last
                    # write, in both worlds, so both reopens hold what the control's hold, exactly, item by item.
                    same = {r: (total(outcomes[False], r), total(outcomes[True], r)) for r in ('first_reopen', 'second_reopen')}
                    outcome['verdict']['as_the_control'] = (all(a is not None and a == b for a, b in same.values())
                                                            and all(outcomes[False].get(r) == outcomes[True].get(r)
                                                                    for r in crash_proof.REOPENS))
                    outcome['ok'] = crash_proof.judge(outcome)
            elif eager and controlled:
                outcome = eager_verdict(outcomes[False], outcomes[True])
            else:
                outcome = against_control(outcomes[False], outcomes[True]) if controlled else outcomes[False]
                if exact and controlled:
                    # A death outside any save: both worlds are the last save, exactly (the break, the transfer never happened).
                    base = TRANSFERS.get(args.scenario) or HIDDEN.get(args.scenario) or MB_TOTALS.get(args.scenario) or int(os.environ.get('NTM_CRASH_NITER', '16'))
                    last = {r: (total(outcomes[False], r), total(outcomes[True], r)) for r in ('first_reopen', 'second_reopen')}
                    outcome['as_the_last_save'] = all(v == (base, base) for v in last.values()) and all(
                        o.get(r) == o.get('base_items') for o in outcomes.values() for r in crash_proof.REOPENS)
                    outcome['ok'] = outcome['ok'] and outcome['as_the_last_save']
            report['points'].append(outcome)
            print(json.dumps(outcome), flush=True)
            if outcome['ok']:
                for control in bases:
                    shutil.rmtree(point_folder(scratch, point, control), ignore_errors=True)
            (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        crash_proof.match_cuts(report['points'])
        for outcome in report['points']:
            if outcome['ok']:
                for control in bases:
                    shutil.rmtree(point_folder(scratch, outcome['point'], control), ignore_errors=True)
        report['ok'] = all(p['ok'] for p in report['points'])
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

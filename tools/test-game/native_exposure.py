#!/usr/bin/env python3
"""The exposure chamber (docs/onda-2-sublote-2.md) on a disposable native world.

setup          a higgs capsule pair, uranium ingots, a creative battery and then a speed upgrade through the screen: a capsule is
               loaded (eight uses, its empty one in the second slot), one ingot of schraranium every 200 ticks and the upgrade makes
               them come faster; a second chamber fed by a hopper at one leg and emptied by one at another: the hopper puts in
               a dark capsule and plutonium ingots and nothing it refuses (dirt, a stick), and takes the empty capsule and the
               euphemium out;
capture-state  the first chamber saved with a capsule loaded and an ingot in the making;
check-state    after reopening, the slots and the uses left are as saved and the chamber goes on from where it was.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game
from native_furnaces import core_of
from native_combustion import world_cell, facing_of
from native_orbus import open_through

X, Z = 40, 8
SAVED = 'exposure-saved.json'
MARKER = '"consumption"'


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 900
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def state(g, core):
    text = ' '.join(g.command(f'data get block {core[0]} {core[1]} {core[2]} ntm').get('chat', [])).replace('\\"', '"')
    found = re.search(r'\{.*\}', text)
    data = json.loads(found[0]) if found else {}

    def find(node):
        if isinstance(node, dict):
            if 'consumption' in node and 'saved' in node:
                return node
            for v in node.values():
                r = find(v)
                if r is not None:
                    return r
        return None
    return find(data) or {}


def items(g, p):
    """slot -> (id, count) from the block entity's (or a container's) items."""
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    out, depth, start, entries = {}, 0, None, []
    for i, c in enumerate(text):
        if c == '{':
            if depth == 1:
                start = i
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 1 and start is not None:
                entries.append(text[start:i + 1])
                start = None
        elif c == '[' and depth == 0:
            depth = 1
    for entry in entries:
        slot = re.search(r'[sS]lot: ?(\d+)', entry)
        item = re.search(r'(?:^|[,{] ?)id: ?"([\w:]+)"(?=[,}])', entry)
        count = re.search(r'[cC]ount: ?(\d+)', entry)
        if slot and item:
            out[int(slot[1])] = (item[1], int(count[1]) if count else 1)
    return out


def totals(contents):
    out = {}
    for item, n in contents.values():
        out[item] = out.get(item, 0) + n
    return out


def build(g, dz=0):
    g.close()
    g.ground(X, 100, Z + dz, width=18, depth=8)
    g.command('clear @s')
    g.command('give @s ntm:machine_exposure_chamber 1')
    g.command(f'tp @s {X + .5} 100 {Z + dz + 3.5}')
    time.sleep(1)
    g.ctl(f'place {X} 100 {Z + dz} --item ntm:machine_exposure_chamber')
    time.sleep(1.5)
    g.command('clear @s')
    core = core_of(g, (X, 100, Z + dz), MARKER)
    return core, facing_of(g, core)


def body(g, core):
    """The dummy cells of the main box (the first a player can click opens the screen)."""
    cells = []
    for dy in range(0, 5):
        for dx in range(-2, 3):
            for dz in range(-2, 3):
                p = (core[0] + dx, core[1] + dy, core[2] + dz)
                if g.ctl('block %d %d %d' % p).get('state', '').startswith('ntm:dummy_machine'):
                    cells.append(p)
    return cells


def open_chamber(g, core):
    return open_through(g, body(g, core))


def put(g, item, slot, count=1):
    g.put(item, count, slot)


def setup(g):
    core, facing = build(g)
    g.evidence.update(core=list(core), facing=facing)
    open_chamber(g, core)
    put(g, 'ntm:particle_higgs', 0, 2)
    put(g, 'ntm:ingot_uranium', 2, 6)
    put(g, 'ntm:battery_creative', 4)
    g.close()
    sprint(g, 450)
    s, st = items(g, core), state(g, core)
    g.evidence['first'] = {'items': s, 'state': {k: st.get(k) for k in ('saved', 'progress', 'loaded', 'process_time', 'consumption')}}
    g.check('two_ingots_in_the_output', s.get(3) == ('ntm:ingot_schraranium', 2), s)
    g.check('the_empty_capsule_came_out_and_one_capsule_waits', s.get(1) == ('ntm:particle_empty', 1) and s.get(0) == ('ntm:particle_higgs', 1), s)
    g.check('the_ingredient_is_spent_one_by_one', s.get(2) == ('ntm:ingot_uranium', 4), s)
    g.check('six_uses_are_left_and_the_particle_is_loaded', st.get('saved') == 6 and st.get('loaded') == 'ntm:particle_higgs', st)
    open_chamber(g, core)
    put(g, 'ntm:upgrade_speed_3', 5)
    g.close()
    sprint(g, 100)
    s, st = items(g, core), state(g, core)
    g.check('the_speed_upgrade_sets_the_time_to_fifty', st.get('process_time') == 50 and st.get('consumption') == 20_000, st)
    g.check('the_upgrade_makes_them_faster', s.get(3, ('', 0))[1] >= 4, s)
    automation(g)


def automation(g):
    dz = 16
    core, facing = build(g, dz)
    cell = lambda x, y, z: world_cell(core, facing, (x, y, z))
    delta = lambda v: [a - b for a, b in zip(cell(*v), cell(0, 0, 0))]
    name = lambda d: next(n for n, v in (('east', [1, 0, 0]), ('west', [-1, 0, 0]), ('south', [0, 0, 1]), ('north', [0, 0, -1])) if v == d)
    open_chamber(g, core)
    put(g, 'ntm:battery_creative', 4)
    g.close()
    feeder_hopper, feeder_chest = cell(7, 0, -2), cell(7, 1, -2)
    taker_hopper, taker_chest = cell(8, -1, 1), cell(8, -2, 1)
    g.command(f'setblock {feeder_hopper[0]} {feeder_hopper[1]} {feeder_hopper[2]} minecraft:hopper[facing={name(delta((0, 0, 1)))}]')
    g.command(f'setblock {feeder_chest[0]} {feeder_chest[1]} {feeder_chest[2]} minecraft:chest')
    g.command(f'setblock {taker_hopper[0]} {taker_hopper[1]} {taker_hopper[2]} minecraft:hopper[facing=down]')
    g.command(f'setblock {taker_chest[0]} {taker_chest[1]} {taker_chest[2]} minecraft:chest')
    for n, (item, count) in enumerate([('minecraft:dirt', 2), ('ntm:particle_dark', 1), ('minecraft:stick', 1), ('ntm:ingot_plutonium', 3)]):
        g.command(f'item replace block {feeder_chest[0]} {feeder_chest[1]} {feeder_chest[2]} container.{n} with {item} {count}')
    sprint(g, 600)
    chamber, out = items(g, core), items(g, taker_chest)
    stuck = totals(items(g, feeder_chest))
    held = totals(items(g, feeder_hopper))
    g.evidence['automation'] = {'chamber': chamber, 'out': out, 'chest': stuck, 'hopper': held, 'state': {k: state(g, core).get(k) for k in ('saved', 'progress')}}
    g.check('the_hopper_put_in_the_capsule_and_the_plutonium', totals(chamber).get('ntm:ingot_plutonium', 0) + totals(out).get('ntm:ingot_euphemium', 0) >= 1 and state(g, core).get('saved', 8) < 8, [chamber, out])
    g.check('it_took_nothing_that_no_recipe_asks_for', 'minecraft:dirt' not in totals(chamber) and 'minecraft:stick' not in totals(chamber), chamber)
    g.check('the_refused_items_stay_in_the_chest_and_the_hopper', stuck.get('minecraft:dirt', 0) + held.get('minecraft:dirt', 0) == 2 and stuck.get('minecraft:stick', 0) + held.get('minecraft:stick', 0) == 1, [stuck, held])
    g.check('the_hopper_under_a_leg_took_the_empty_capsule_and_the_euphemium', totals(out).get('ntm:particle_empty') == 1 and totals(out).get('ntm:ingot_euphemium', 0) >= 1, out)


def capture(g):
    core, facing = build(g)
    g.evidence.update(core=list(core), facing=facing)
    open_chamber(g, core)
    put(g, 'ntm:particle_higgs', 0, 2)
    put(g, 'ntm:ingot_uranium', 2, 6)
    put(g, 'ntm:battery_creative', 4)
    g.close()
    sprint(g, 300)
    s, st = items(g, core), state(g, core)
    g.check('one_ingot_and_an_ingot_in_the_making_before_the_save', s.get(3) == ('ntm:ingot_schraranium', 1) and st.get('saved') == 7 and 0 < st.get('progress', 0) < 200, [s, st])
    (g.folder / SAVED).write_text(json.dumps({'core': list(core), 'items': {str(k): list(v) for k, v in s.items()}, 'saved': st.get('saved'), 'progress': st.get('progress'), 'loaded': st.get('loaded')}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    core = tuple(saved['core'])
    sprint(g, 5)
    s, st = items(g, core), state(g, core)
    g.check('the_slots_are_as_saved', {str(k): list(v) for k, v in s.items()} == saved['items'], [saved['items'], s])
    g.check('the_loaded_particle_and_its_uses_are_as_saved', st.get('loaded') == saved['loaded'] and st.get('saved') == saved['saved'], [saved, st])
    g.check('the_progress_goes_on_from_where_it_was', st.get('progress', 0) >= saved['progress'], [saved['progress'], st.get('progress')])
    sprint(g, 200)
    s = items(g, core)
    g.check('the_next_ingot_comes_out', s.get(3) == ('ntm:ingot_schraranium', 2), s)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'exposure-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

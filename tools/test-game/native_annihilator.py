#!/usr/bin/env python3
"""The annihilator (`ntm:machine_annihilator`) on a disposable native world.

setup          builds the 3x9x3 + 3x6x3 chimney multiblock (134 dummies + core), feeds items
               through a hopper into a front port and through the request slot, verifies monitor
               slot tracking, rejects fluid on a non-port cell and accepts it on a front/side port
               only with a matching fluid identifier, and verifies uranium item radiation lands at
               the chimney chunk while sas3 fluid radiation lands at the core chunk;
capture-state  destroys cobblestone, opens a fluid tank for water, places iron_ore in request, saves
               mid-way;
check-state    after reopening, slots, monitor, tank type and remaining request items come back and
               finish without loss or duplication.
"""
import json
import re
import sys
import time
from native_game import Game
from native_recipes import core_of
from native_combustion import world_cell, facing_of
from native_orbus import open_through, tank as orbus_tank
from native_stations import fill_barrel, type_duct

X, Z = 64, 21
SAVED = 'annihilator-saved.json'


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
            if 'tank' in node and 'pool' in node and 'monitor' in node:
                return node
            for v in node.values():
                r = find(v)
                if r is not None:
                    return r
        return None
    return find(data) or {}


def items(g, p):
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


def body(g, core):
    cells = []
    for dy in range(0, 3):
        for dx in range(-4, 5):
            for dz in range(-4, 5):
                p = (core[0] + dx, core[1] + dy, core[2] + dz)
                if g.ctl('block %d %d %d' % p).get('state', '').startswith('ntm:dummy_machine'):
                    cells.append(p)
    cells.sort(key=lambda c: (-c[2], c[1], abs(c[0] - core[0])))
    return cells


def all_dummies(g, core):
    cells = []
    for dy in range(0, 9):
        for dx in range(-4, 5):
            for dz in range(-4, 5):
                p = (core[0] + dx, core[1] + dy, core[2] + dz)
                if g.ctl('block %d %d %d' % p).get('state', '').startswith('ntm:dummy_machine'):
                    cells.append(p)
    return cells


def open_annihilator(g, core):
    return open_through(g, body(g, core))


def put(g, item, slot, count=1):
    g.put(item, count, slot)


def put_fluid_id(g, fluid, slot):
    g.command('clear @s')
    g.command(f'give @s ntm:fluid_identifier_multi[minecraft:custom_data={{ntm_fluid:"{fluid}"}}] 1')
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        screen = g.ctl('screen')
        source = next((s['slot'] for s in screen.get('slots', [])
                       if s['slot'] < 36 and s.get('item') == 'ntm:fluid_identifier_multi'), None)
        if source is not None:
            break
        time.sleep(.2)
    else:
        raise AssertionError(f'fluid_identifier not received for {fluid}')
    g.ctl(f'click-slot {source}')
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.4)
    screen = g.ctl('screen')
    g.check(f'insert_fluid_id_{fluid}_{slot}', not screen.get('carried'), screen)


def clear_slot(g, slot):
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.2)
    g.command('clear @s')


def read_geiger(g, block_pos):
    g.close()
    g.command(f'tp @s {block_pos[0] + .5} {block_pos[1]} {block_pos[2] + 1.5} facing {block_pos[0] + .5} {block_pos[1] + .5} {block_pos[2] + .5}')
    time.sleep(.8)
    g.ctl(f'interact {block_pos[0]} {block_pos[1]} {block_pos[2]} south')
    time.sleep(.5)
    text = ' | '.join(g.ctl('chat-log 12').get('chat', []))
    seen = re.findall(r'Current chunk radiation: (?:§.)?([0-9.]+) RAD/s', text)
    g.check(f'geiger_line_at_{block_pos}', bool(seen), text)
    return float(seen[0])


def build(g, x=X, z=Z):
    g.close()
    g.ground(x - 8, 100, z - 12, width=18, depth=20)
    g.command(f'fill {x - 8} 100 {z - 12} {x + 10} 115 {z + 8} minecraft:air')
    g.command('clear @s')
    g.command('give @s ntm:machine_annihilator 1')
    g.command(f'tp @s {x + .5} 100 {z + 3.5} facing {x + .5} 100 {z}')
    time.sleep(1)
    g.ctl(f'place {x} 100 {z} --item ntm:machine_annihilator')
    time.sleep(1.5)
    g.command('clear @s')
    core = core_of(g, 'ntm:machine_annihilator', x, 100, z, reach=(6, 6))
    facing = facing_of(g, core)
    g.evidence.update(core=list(core), facing=facing)
    return core, facing


def setup(g):
    core, facing = build(g)
    cell = lambda lx, ly, lz: world_cell(core, facing, (lx, ly, lz))
    dummies = all_dummies(g, core)
    st0 = state(g, core)
    g.check('multiblock_has_134_dummies', len(dummies) == 134, len(dummies))
    g.check('initial_annihilator_state', st0.get('pool') == 'Recycling' and st0.get('tank', {}).get('cap') == 2_500_000 and st0.get('monitor') == '0', st0)

    # 1. Hopper into side-front port (-1, 0, -3) + Request slot (9) + Monitor slot (8).
    delta = lambda v: [a - b for a, b in zip(cell(*v), cell(0, 0, 0))]
    dir_name = lambda d: next(n for n, v in (('east', [1, 0, 0]), ('west', [-1, 0, 0]), ('south', [0, 0, 1]), ('north', [0, 0, -1])) if v == d)
    in_dir = dir_name(delta((1, 0, 0)))  # from (-2, 0, -3) towards (-1, 0, -3)
    hopper = cell(-2, 0, -3)
    chest = cell(-2, 1, -3)
    g.command(f'setblock {hopper[0]} {hopper[1]} {hopper[2]} minecraft:hopper[facing={in_dir}]')
    g.command(f'setblock {chest[0]} {chest[1]} {chest[2]} minecraft:chest')
    g.command(f'item replace block {chest[0]} {chest[1]} {chest[2]} container.0 with minecraft:cobblestone 12')
    g.command(f'item replace block {chest[0]} {chest[1]} {chest[2]} container.1 with minecraft:iron_ore 8')

    open_annihilator(g, core)
    put(g, 'minecraft:cobblestone', 8, 1)
    put(g, 'minecraft:iron_ore', 9, 3)
    g.close()
    sprint(g, 190)

    st1 = state(g, core)
    s1 = items(g, core)
    g.check('chest_and_hopper_emptied', not items(g, chest) and not items(g, hopper), [items(g, chest), items(g, hopper)])
    g.check('request_consumed_and_monitor_kept_cobble', s1.get(9) is None and s1.get(8) == ('minecraft:cobblestone', 1) and st1.get('monitor') == '12', [s1, st1])

    open_annihilator(g, core)
    clear_slot(g, 8)
    put(g, 'minecraft:iron_ore', 8, 1)
    g.close()
    sprint(g, 3)
    st2 = state(g, core)
    g.check('monitor_shows_iron_ore_from_input_plus_request', st2.get('monitor') == '11', st2)

    # 2. Fluid ports vs non-port side: non-port side (2, 0, 0) refuses water; front port (0, 0, -4) drains water.
    bad_duct, bad_barrel = cell(2, 0, 0), cell(2, 1, 0)
    g.command(f'setblock {bad_duct[0]} {bad_duct[1]} {bad_duct[2]} ntm:fluid_duct_neo')
    g.command(f'setblock {bad_barrel[0]} {bad_barrel[1]} {bad_barrel[2]} ntm:barrel_steel[facing=north]')
    type_duct(g, 'water', [bad_duct], cell(2, 0, 2))
    fill_barrel(g, bad_barrel, 1, 'minecraft:water_bucket')
    open_annihilator(g, core)
    put_fluid_id(g, 'water', 1)
    clear_slot(g, 8)
    put_fluid_id(g, 'water', 8)
    g.close()
    sprint(g, 40)
    g.check('non_port_side_never_drains_water', orbus_tank(g, bad_barrel)[1] == 1000 and state(g, core).get('monitor') == '0', [orbus_tank(g, bad_barrel), state(g, core)])

    good_duct, good_barrel = cell(0, 0, -5), cell(0, 1, -5)
    g.command(f'setblock {good_duct[0]} {good_duct[1]} {good_duct[2]} ntm:fluid_duct_neo')
    g.command(f'setblock {good_barrel[0]} {good_barrel[1]} {good_barrel[2]} ntm:barrel_steel[facing=north]')
    type_duct(g, 'water', [good_duct], cell(0, 0, -7))
    fill_barrel(g, good_barrel, 1, 'minecraft:water_bucket')
    sprint(g, 40)
    st3 = state(g, core)
    g.check('front_port_drains_water_into_pool', orbus_tank(g, good_barrel)[1] == 0 and st3.get('monitor') == '1000', [orbus_tank(g, good_barrel), st3])

    # 3. Radiation emissions: uranium item -> chimney chunk (4, 0), sas3 fluid -> core chunk (4, 1).
    chimney_top = cell(0, 9, 3)  # exact CHIMNEY [0, 9, 3] directly on top of the chimney, chunk (4, 0)
    core_side = cell(4, 0, 1)     # x=68, z=16 (chunk (4, 1)), unobstructed
    g.command(f'setblock {chimney_top[0]} {chimney_top[1]} {chimney_top[2]} ntm:geiger[facing=south]')
    g.command(f'setblock {core_side[0]} {core_side[1]} {core_side[2]} ntm:geiger[facing=south]')

    open_annihilator(g, core)
    put(g, 'ntm:ingot_uranium', 9, 6)
    g.close()
    sprint(g, 8)
    rad_chimney = read_geiger(g, chimney_top)
    rad_core_before = read_geiger(g, core_side)
    g.evidence['item_radiation'] = {'chimney': rad_chimney, 'core': rad_core_before}
    g.check('uranium_item_radiates_chimney_chunk_not_core', rad_chimney >= 1.0 and rad_core_before < 1.0, g.evidence['item_radiation'])

    sas3_duct, sas3_barrel = cell(2, 0, -3), cell(2, 1, -3)
    g.command(f'setblock {sas3_duct[0]} {sas3_duct[1]} {sas3_duct[2]} ntm:fluid_duct_neo')
    g.command(f'setblock {sas3_barrel[0]} {sas3_barrel[1]} {sas3_barrel[2]} ntm:barrel_steel[facing=north]')
    type_duct(g, 'sas3', [sas3_duct], cell(3, 0, -3))
    fill_barrel(g, sas3_barrel, 1, 'ntm:fluid_barrel_full__sas3')
    open_annihilator(g, core)
    clear_slot(g, 1)
    put_fluid_id(g, 'sas3', 1)
    g.close()
    sprint(g, 40)
    rad_core_after = read_geiger(g, core_side)
    g.evidence['fluid_radiation'] = {'core_after_sas3': rad_core_after}
    g.check('sas3_fluid_radiates_core_chunk', rad_core_after >= 1000.0, g.evidence['fluid_radiation'])


def capture(g):
    core, facing = build(g, x=64, z=45)
    open_annihilator(g, core)
    put(g, 'minecraft:granite', 0, 20)
    put(g, 'minecraft:granite', 8, 1)
    put_fluid_id(g, 'lava', 1)
    g.close()
    sprint(g, 5)

    open_annihilator(g, core)
    g.command('tick freeze')
    put(g, 'minecraft:gold_ore', 9, 4)
    g.close()
    g.command('tick step 2')
    time.sleep(.3)

    s, st = items(g, core), state(g, core)
    g.check('capture_has_remaining_request_and_monitor_20', s.get(9) == ('minecraft:gold_ore', 2) and st.get('monitor') == '20' and st.get('tank', {}).get('fluid') == 'lava', [s, st])
    (g.folder / SAVED).write_text(json.dumps({'core': list(core), 'items': {str(k): list(v) for k, v in s.items()}, 'monitor': st.get('monitor'), 'fluid': st.get('tank', {}).get('fluid')}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    core = tuple(saved['core'])
    g.command('tick unfreeze')
    sprint(g, 5)
    s, st = items(g, core), state(g, core)
    g.check('tank_fluid_and_monitor_came_back', st.get('tank', {}).get('fluid') == saved['fluid'] and st.get('monitor') == saved['monitor'], [saved, st])
    g.check('request_finished_after_reopen', s.get(9) is None and s.get(8) == ('minecraft:granite', 1) and s.get(1) == ('ntm:fluid_identifier_multi', 1), s)
    open_annihilator(g, core)
    clear_slot(g, 8)
    put(g, 'minecraft:gold_ore', 8, 1)
    g.close()
    sprint(g, 3)
    st2 = state(g, core)
    g.check('all_4_requested_gold_ores_counted_once', st2.get('monitor') == '4', st2)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'annihilator-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

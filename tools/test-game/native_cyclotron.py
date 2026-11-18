#!/usr/bin/env python3
"""The cyclotron (docs/onda-2-sublote-2.md) on a disposable native world.

setup          a cyclotron with a speed and an effect upgrade (speed 3, effect 3: four ticks of progress a tick, 500 mB of water a
               tick), fed by a hopper at its west side (a lithium particle and lithium dust, and the dirt it refuses) and by hand in
               the second lane, on a creative battery; its water comes from a big store (an orbus of barrels, in output mode) through
               typed ducts under it and its spent steam goes to another orbus: one cycle of 173 ticks makes beryllium and uranium
               dust, 75 mB of antimatter, drinks 86 000 mB and gives the same of steam; a hopper under the east side takes the dust
               out; the water in the two stores and the cyclotron is the same at the end as at the start;
capture-state  the cyclotron saved with a cycle half-way, and its two lanes' items;
check-state    after reopening, the slots, the progress and the tanks are as saved and the cycle ends from where it was.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game
from native_furnaces import core_of as find_core
from native_combustion import world_cell, facing_of
from native_orbus import dummies, open_through, set_output, tank
from native_stations import type_duct

X, Z = 60, 24
SAVED = 'cyclotron-saved.json'
BARRELS = 12


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
            if 'tanks' in node and 'progress' in node and 'power' in node:
                return node
            for v in node.values():
                r = find(v)
                if r is not None:
                    return r
        return None
    return find(data) or {}


def tanks(g, core):
    return [(t.get('fluid', ''), t['mb']) for t in state(g, core).get('tanks', [])]


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
        for dx in range(-2, 3):
            for dz in range(-2, 3):
                p = (core[0] + dx, core[1] + dy, core[2] + dz)
                if g.ctl('block %d %d %d' % p).get('state', '').startswith('ntm:dummy_machine'):
                    cells.append(p)
    return cells


def layout(g, with_orbus=True):
    """The cyclotron and, when `with_orbus`, the two stores and the ducts; returns the cores and the facing."""
    g.close()
    g.ground(X - 16, 100, Z - 6, width=36, depth=14)
    g.command('clear @s')
    g.command('give @s ntm:machine_cyclotron 1')
    g.command(f'tp @s {X + .5} 100 {Z + 3.5} facing {X + .5} 100 {Z}')
    time.sleep(1)
    g.ctl(f'place {X} 100 {Z} --item ntm:machine_cyclotron')
    time.sleep(1.5)
    g.command('clear @s')
    core = find_core(g, (X, 100, Z), '"progress"')
    facing = facing_of(g, core)
    g.evidence.update(core=list(core), facing=facing)
    cell = lambda x, y, z: world_cell(core, facing, (x, y, z))
    out = {'core': core, 'facing': facing, 'cell': cell}
    if not with_orbus:
        return out
    ducts(g, core, cell)
    for key, local in (('water', (-10, 0, -1)), ('steam', (10, 0, -1))):
        target = cell(*local)
        placer = cell(local[0], local[1], local[2] - 1)
        g.command('clear @s')
        g.command('give @s ntm:machine_orbus 1')
        g.command(f'tp @s {placer[0] + .5} 100 {placer[2] + 3.5} facing {placer[0] + .5} 100 {placer[2]}')
        time.sleep(1)
        g.ctl(f'place {placer[0]} {placer[1]} {placer[2]} --item ntm:machine_orbus')
        time.sleep(1.5)
        g.command('clear @s')
        found = find_core(g, placer, '"cap":512000')
        g.check(f'the_{key}_orbus_is_where_it_was_meant', tuple(found) == tuple(target), [found, target])
        out[key] = found
    return out


def ducts(g, core, cell):
    """Typed ducts from the cyclotron's side ports to the cells under the stores' ports: the water's from the west port out and
    down, the steam's from the east port out and down (the two rows are apart, so the networks stay apart). They are laid and typed
    before the stores are placed, because the volume of a store hides the ducts under it."""
    rows = {
        'water': [(x, 0, 0) for x in range(-3, -9, -1)] + [(x, -1, 0) for x in range(-8, -12, -1)],
        'spentsteam': [(x, 0, 0) for x in range(3, 8)] + [(x, -1, 0) for x in range(7, 10)],
    }
    for fluid, local in rows.items():
        # The low ones first, each typed from the cell over it, before the high ones are laid over them; then the high ones from the
        # cell beside them.
        for low in (True, False):
            group = [l for l in local if (l[1] == -1) == low]
            for l in group:
                p = cell(*l)
                g.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:fluid_duct_neo')
            for l in group:
                type_duct(g, fluid, [cell(*l)], cell(l[0], 0, 0) if low else cell(l[0], 0, 1))
    g.command('clear @s')


def pour_barrels(g, cells, n):
    g.command('clear @s')
    g.command(f'give @s ntm:fluid_barrel_full__water {n}')
    time.sleep(1)
    open_through(g, cells)
    screen = g.ctl('screen')
    hot = next(s['slot'] for s in screen['slots'] if s.get('item') == 'ntm:fluid_barrel_full__water' and s['slot'] < 36)
    g.ctl(f'click-slot {hot}')
    g.ctl('click-slot 302')
    time.sleep(1)
    g.close()


def identify(g, cells, fluid):
    g.command('clear @s')
    g.command(f'give @s ntm:fluid_identifier_multi[minecraft:custom_data={{ntm_fluid:"{fluid}"}}] 1')
    time.sleep(1)
    open_through(g, cells)
    screen = g.ctl('screen')
    hot = next(s['slot'] for s in screen['slots'] if s.get('item') == 'ntm:fluid_identifier_multi' and s['slot'] < 36)
    g.ctl(f'click-slot {hot}')
    g.ctl('click-slot 300')
    time.sleep(.6)
    g.close()


def open_cyclotron(g, core):
    return open_through(g, body(g, core))


def put(g, item, slot, count=1):
    g.put(item, count, slot)


def setup(g):
    lay = layout(g)
    core, cell = lay['core'], lay['cell']
    w_cells, s_cells = dummies(g, lay['water']), dummies(g, lay['steam'])
    pour_barrels(g, w_cells, BARRELS)
    sprint(g, 30)
    set_output(g, w_cells)
    identify(g, s_cells, 'spentsteam')
    water = tank(g, lay['water'])
    own = tanks(g, core)
    # The cyclotron's own tank has already taken its 32 000 mB over the typed ducts.
    g.check('the_barrels_filled_the_store_and_the_ducts_carry_it', water is not None and water[0] == 'water' and water[1] + own[0][1] == BARRELS * 16_000 and own[0][1] > 0, [water, own])
    open_cyclotron(g, core)
    put(g, 'ntm:part_beryllium', 1)
    put(g, 'ntm:powder_thorium', 4)
    put(g, 'ntm:upgrade_speed_3', 10)
    put(g, 'ntm:upgrade_effect_3', 11)
    g.close()
    feeder, feeder_chest = cell(-3, 0, 1), cell(-3, 1, 1)
    taker, taker_chest = cell(2, -1, 1), cell(2, -2, 1)
    delta = [a - b for a, b in zip(cell(1, 0, 0), cell(0, 0, 0))]
    name = next(n for n, v in (('east', [1, 0, 0]), ('west', [-1, 0, 0]), ('south', [0, 0, 1]), ('north', [0, 0, -1])) if v == delta)
    g.command(f'setblock {feeder[0]} {feeder[1]} {feeder[2]} minecraft:hopper[facing={name}]')
    g.command(f'setblock {feeder_chest[0]} {feeder_chest[1]} {feeder_chest[2]} minecraft:chest')
    g.command(f'setblock {taker[0]} {taker[1]} {taker[2]} minecraft:hopper[facing=down]')
    g.command(f'setblock {taker_chest[0]} {taker_chest[1]} {taker_chest[2]} minecraft:chest')
    for n, (item, count) in enumerate([('minecraft:dirt', 2), ('ntm:part_lithium', 1), ('ntm:powder_lithium', 1)]):
        g.command(f'item replace block {feeder_chest[0]} {feeder_chest[1]} {feeder_chest[2]} container.{n} with {item} {count}')
    sprint(g, 30)  # Feed both inputs before powering the machine.
    start = {'water': tank(g, lay['water'])[1], 'steam': tank(g, lay['steam'])[1], 'own': tanks(g, core)}
    g.check('the_steam_tanks_start_empty', start['steam'] == 0 and start['own'][1][1] == 0, start)
    open_cyclotron(g, core)
    put(g, 'ntm:battery_creative', 9)
    g.close()
    sprint(g, 400)
    cyc, out = items(g, core), items(g, taker_chest)
    t = tanks(g, core)
    w, s = tank(g, lay['water']), tank(g, lay['steam'])
    g.evidence['after'] = {'cyclotron': cyc, 'taken': out, 'tanks': t, 'water_store': w, 'steam_store': s, 'start': start,
                           'chest': totals(items(g, feeder_chest)), 'hopper': totals(items(g, feeder))}
    g.check('the_hopper_fed_the_first_lane', totals(cyc).get('ntm:part_lithium', 0) + totals(out).get('ntm:powder_beryllium', 0) >= 1, [cyc, out])
    g.check('the_dirt_the_cyclotron_refuses_stays_out', 'minecraft:dirt' not in totals(cyc), cyc)
    g.check('both_lanes_made_their_powder', totals(out).get('ntm:powder_beryllium') == 1 and totals(out).get('ntm:powder_uranium') == 1, out)
    g.check('antimatter_is_the_recipes_seventy_five', t[2] == ('amat', 75), t)
    water_total = lambda: w[1] + s[1] + t[0][1] + t[1][1]
    g.check('the_water_is_still_the_same_water', water_total() == start['water'] + start['steam'] + sum(mb for _, mb in start['own'][:2]), [w, s, t, start])
    g.check('a_cycle_drank_eighty_six_thousand', 86_000 <= s[1] + t[1][1] <= 87_000, [s, t])
    g.check('the_progress_is_back_at_zero_or_a_new_cycle', 0 <= state(g, core).get('progress', -1) < 690, state(g, core).get('progress'))


def capture(g):
    lay = layout(g)
    core, cell = lay['core'], lay['cell']
    w_cells, s_cells = dummies(g, lay['water']), dummies(g, lay['steam'])
    pour_barrels(g, w_cells, BARRELS)
    sprint(g, 30)
    set_output(g, w_cells)
    identify(g, s_cells, 'spentsteam')
    open_cyclotron(g, core)
    put(g, 'ntm:part_lithium', 0, 2)
    put(g, 'ntm:powder_lithium', 3, 2)
    put(g, 'ntm:upgrade_speed_1', 10)
    put(g, 'ntm:upgrade_effect_3', 11)
    put(g, 'ntm:battery_creative', 9)
    g.close()
    sprint(g, 80)
    st, s = state(g, core), items(g, core)
    g.check('the_cycle_is_half_way_before_the_save', 100 < st.get('progress', 0) < 600 and s.get(6) is None, [st.get('progress'), s])
    (g.folder / SAVED).write_text(json.dumps({'core': list(core), 'progress': st.get('progress'), 'items': {str(k): list(v) for k, v in s.items()}, 'tanks': tanks(g, core)}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    core = tuple(saved['core'])
    sprint(g, 5)
    s, st = items(g, core), state(g, core)
    g.check('the_slots_are_as_saved', {str(k): list(v) for k, v in s.items()} == saved['items'], [saved['items'], s])
    g.check('the_progress_goes_on_from_where_it_was', st.get('progress', 0) >= saved['progress'], [saved['progress'], st.get('progress')])
    sprint(g, 300)
    s = items(g, core)
    g.check('the_cycle_ends_with_its_powder', s.get(6) == ('ntm:powder_beryllium', 1) and s.get(0) == ('ntm:part_lithium', 1) and s.get(3) == ('ntm:powder_lithium', 1), s)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'cyclotron-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

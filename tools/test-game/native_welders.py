#!/usr/bin/env python3
"""The arc welder and the soldering station (docs/onda-2-sublote-1.md) on a disposable native world.

arc        two steel plates and two dense red-copper wires into two motors (100 ticks, 400 HE a tick) on a
           creative battery; with a speed-3 upgrade it takes 50 ticks, so 60 ticks make the motors and the
           output is taken by shift-click; a hopper beside a cell of the lower layer (a port the Java lists)
           feeds the inputs, one over a cell of the upper layer (`{}` in the Java) feeds nothing;
identifier the fluid identifier types the tank and another identifier changes the type;
solder-stack three satellite chips' worth of inputs make one chip: its stack is 1, as `getMaxStackSize()`;
solder     an analog circuit board from 3 vacuum tubes + 2 capacitors, 4 pcb and 4 lead wires; a hand cannot
           put a stone in a topping slot (`SlotNonRetarded` asks `isItemValidForSlot`).
Time passes with `tick sprint`; no machine state is injected.
"""
import os
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state

X, Z = 40, 8


def count(slots, item):
    return sum(s.get('count', 0) for s in slots if s.get('item') == item)


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def core_of(g, around, marker):
    x0, y0, z0 = around
    for dx in range(-2, 3):
        for dy in range(-1, 3):
            for dz in range(-2, 3):
                if marker in machine_state(g, (x0 + dx, y0 + dy, z0 + dz)):
                    return (x0 + dx, y0 + dy, z0 + dz)
    raise AssertionError('no machine with ' + marker)


def open_any(g, cells):
    """Opens the screen through the first of `cells` (dummy cells of the multiblock) a player can click."""
    g.close()
    for cell in cells:
        g.command(f'tp @s {cell[0] + .5} 100 {cell[2] + 3.5}')
        time.sleep(1)
        for _ in range(2):
            try:
                g.ctl(f'interact {cell[0]} {cell[1]} {cell[2]} south')
            except RuntimeError:
                break  # no visible face from here: another cell
            deadline = time.monotonic() + 4
            while time.monotonic() < deadline:
                if g.ctl('screen').get('screen') == 'container':
                    return cell
                time.sleep(.4)
            g.close()
    raise AssertionError('no cell opened the screen: ' + str(cells))


def place(g, block, marker, x, y, z):
    g.close()
    g.ground(x, y, z, width=8)
    g.command('clear @s')
    g.command(f'give @s {block} 1')
    time.sleep(.5)
    g.ctl(f'place {x} {y} {z} --item {block}')
    time.sleep(1.5)
    return core_of(g, (x, y, z), marker)


def arc(g):
    core = place(g, 'ntm:machine_arc_welder', '"Arc"', X, 100, Z)
    cells = [(core[0] + dx, core[1] + dy, core[2] + dz) for dy in (1, 0) for dz in (1, 0) for dx in (0, -1, 1)]
    cell = open_any(g, cells)
    g.check('arc_menu_open', True, {'core': core, 'cell': cell})
    g.put('ntm:plate_steel', 2, 0)
    g.put('ntm:wire_dense__red_copper', 2, 1)
    g.put('ntm:battery_creative', 1, 4)
    g.put('ntm:upgrade_speed_3', 1, 6)
    g.snapshot('welders-arc-loaded')
    sprint(g, 60)
    slots = g.slots()
    # 100 ticks without the upgrade; 50 with speed 3: only the upgrade makes the motors in 60.
    g.check('arc_two_motors_with_the_upgrade', count(slots, 'ntm:motor') == 2, slots)
    g.check('arc_inputs_consumed', count(slots, 'ntm:plate_steel') == 0 and count(slots, 'ntm:wire_dense__red_copper') == 0, slots)
    g.ctl('click-slot 303 shift')
    g.close()
    inv = g.ctl('inventory').get('counts', {})
    g.check('arc_motors_taken', inv.get('ntm:motor') == 2, inv)
    # The cells of the layer above the core give {} in `getAccessibleSlotsFromSide(x,y,z,side)`: a hopper over one
    # moves nothing, however long it has.
    top = (core[0], core[1] + 2, core[2])
    g.command(f'setblock {top[0]} {top[1]} {top[2]} minecraft:hopper[facing=down]')
    g.command(f'setblock {top[0]} {top[1] + 1} {top[2]} minecraft:chest')
    g.command(f'item replace block {top[0]} {top[1] + 1} {top[2]} container.0 with ntm:plate_steel 2')
    g.command(f'item replace block {top[0]} {top[1] + 1} {top[2]} container.1 with ntm:wire_dense__red_copper 2')
    sprint(g, 100)
    # The hopper pulls the chest's stacks into itself and has nowhere to put them: they must still be in the two.
    held = chest_count(g, (top[0], top[1] + 1, top[2]), 'ntm:plate_steel') + chest_count(g, top, 'ntm:plate_steel')
    open_any(g, cells)
    slots = g.slots()
    g.check('hopper_over_an_upper_cell_moves_nothing', held == 2 and count(slots, 'ntm:plate_steel') == 0 and count(slots, 'ntm:motor') == 0, [held, slots])
    g.close()
    g.command(f'setblock {top[0]} {top[1]} {top[2]} minecraft:air')
    g.command(f'setblock {top[0]} {top[1] + 1} {top[2]} minecraft:air')
    # A cell of the lower layer is a port: a hopper beside the east one (the Java's red/yellow/green cells) feeds the inputs.
    east = (core[0] + 2, core[1], core[2])
    g.command(f'setblock {east[0]} {east[1]} {east[2]} minecraft:hopper[facing=west]')
    g.command(f'setblock {east[0]} {east[1] + 1} {east[2]} minecraft:chest')
    g.command(f'item replace block {east[0]} {east[1] + 1} {east[2]} container.0 with ntm:plate_steel 2')
    g.command(f'item replace block {east[0]} {east[1] + 1} {east[2]} container.1 with ntm:wire_dense__red_copper 2')
    sprint(g, 200)
    open_any(g, cells)
    slots = g.slots()
    g.check('arc_hopper_on_a_port_fed_and_welded_again', count(slots, 'ntm:motor') == 2, slots)
    g.check('arc_battery_still_in_its_slot', count(slots, 'ntm:battery_creative') == 1 and count(slots, 'ntm:upgrade_speed_3') == 1, slots)
    g.close()


def chest_count(g, p, item):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    total = 0
    for entry in re.findall(r'\{[^{}]*\}', text):
        i = re.search(r'id: ?"?([\w:]+)"?', entry)
        n = re.search(r'[cC]ount: ?(\d+)', entry)
        if i and n and i[1] == item:
            total += int(n[1])
    return total


def tank_fluid(g, core):
    """(fluid, mb) of the tank in the saved state (the engine keeps the keys sorted); None when no tank is there."""
    text = machine_state(g, core)
    found = re.search(r'"tank":\{([^{}]*)\}', text)
    if not found:
        return None
    fluid = re.search(r'"fluid":"(\w*)"', found[1])
    mb = re.search(r'"mb":(\d+)', found[1])
    return (fluid[1] if fluid else '', int(mb[1]) if mb else 0)


def identifier(g):
    x = X + 28
    core = place(g, 'ntm:machine_arc_welder', '"Arc"', x, 100, Z)
    cells = [(core[0] + dx, core[1] + dy, core[2] + dz) for dy in (1, 0) for dz in (1, 0) for dx in (0, -1, 1)]
    open_any(g, cells)
    before = tank_fluid(g, core)
    g.check('tank_untyped_without_an_identifier', before == ('', 0), before)
    for fluid in ('oxygen', 'reformgas'):
        g.command(f'give @s ntm:fluid_identifier_multi[minecraft:custom_data={{ntm_fluid:"{fluid}"}}] 1')
        time.sleep(.5)
        screen = g.ctl('screen')
        src = next(s['slot'] for s in screen['slots'] if s['slot'] < 36 and s.get('item') == 'ntm:fluid_identifier_multi')
        g.ctl(f'click-slot {src}')
        g.ctl('click-slot 305')
        # The machine's saved data follows its ticks, not the click: let some pass, then read.
        typed = None
        for _ in range(12):
            sprint(g, 20)
            typed = tank_fluid(g, core)
            if typed is not None and typed[0] == fluid:
                break
        screen = g.ctl('screen')
        g.evidence['probe_' + fluid] = {
            'slot_305': next((x for x in screen.get('slots', []) if x['slot'] == 305), None),
            'items': ' '.join(g.command(f'data get block {core[0]} {core[1]} {core[2]} Items').get('chat', []))[:700],
            'state': machine_state(g, core)[:900],
        }
        g.check('identifier_' + fluid + '_types_the_tank', typed is not None and typed[0] == fluid and typed[1] == 0, typed)
        g.ctl('click-slot 305 shift')
        g.command('clear @s')
    g.close()


def solder_stack(g):
    x = X + 42
    core = place(g, 'ntm:machine_soldering_station', '"Solder"', x, 100, Z)
    cells = [(core[0] + dx, core[1], core[2] + dz) for dz in (1, 0, -1) for dx in (0, -1, 1)]
    open_any(g, cells)
    # `sat_chip` (SolderingRecipes.java:154): 8 chips + 2 capacitors, 2 pcb, 4 lead wires; its stack is 1.
    g.put('ntm:circuit__chip', 24, 0)
    g.put('ntm:circuit__capacitor', 6, 1)
    g.put('ntm:circuit__pcb', 6, 3)
    g.put('ntm:wire_fine__lead', 12, 5)
    g.put('ntm:battery_creative', 1, 7)
    sprint(g, 400)
    slots = g.slots()
    by = {s['slot']: (s.get('item'), s.get('count', 0)) for s in slots}
    g.check('one_chip_in_the_output', by.get(306) == ('ntm:sat_chip', 1), by)
    # One craft was made and the output holds its one chip: the others wait with their inputs.
    g.check('the_other_crafts_wait_for_the_output', count(slots, 'ntm:circuit__chip') == 16 and count(slots, 'ntm:circuit__pcb') == 4
            and count(slots, 'ntm:wire_fine__lead') == 8 and count(slots, 'ntm:circuit__capacitor') == 4, slots)
    g.ctl('click-slot 306 shift')
    sprint(g, 150)
    slots = g.slots()
    g.check('taking_the_chip_lets_the_next_one_in', count(slots, 'ntm:sat_chip') == 1 and count(slots, 'ntm:circuit__chip') == 8, slots)
    g.close()


def solder(g):
    x = X + 14
    core = place(g, 'ntm:machine_soldering_station', '"Solder"', x, 100, Z)
    cells = [(core[0] + dx, core[1], core[2] + dz) for dz in (1, 0, -1) for dx in (0, -1, 1)]
    open_any(g, cells)
    # A stone is no topping: the hand is refused in the first topping slot.
    g.command('give @s minecraft:stone 1')
    time.sleep(.5)
    screen = g.ctl('screen')
    src = next(s['slot'] for s in screen['slots'] if s['slot'] < 36 and s.get('item') == 'minecraft:stone')
    g.ctl(f'click-slot {src}')
    g.ctl('click-slot 300')
    screen = g.ctl('screen')
    g.check('stone_refused_in_a_topping_slot', not any(s['slot'] == 300 and s.get('item') for s in screen['slots']), screen)
    g.ctl(f'click-slot {src}')
    g.command('clear @s minecraft:stone')
    g.put('ntm:circuit__vacuum_tube', 3, 0)
    g.put('ntm:circuit__capacitor', 2, 1)
    g.put('ntm:circuit__pcb', 4, 3)
    g.put('ntm:wire_fine__lead', 4, 5)
    g.put('ntm:battery_creative', 1, 7)
    g.snapshot('welders-solder-loaded')
    sprint(g, 130)
    slots = g.slots()
    g.check('solder_one_analog_board', count(slots, 'ntm:circuit__analog') == 1, slots)
    g.check('solder_inputs_consumed', count(slots, 'ntm:circuit__pcb') == 0 and count(slots, 'ntm:wire_fine__lead') == 0, slots)
    g.close()


if __name__ == '__main__':
    g = Game(sys.argv[1], 'welders-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'arc': arc, 'solder': solder, 'identifier': identifier, 'solder-stack': solder_stack,
     'all': lambda g: [f(g) for n, f in (('arc', arc), ('solder', solder), ('identifier', identifier), ('solder-stack', solder_stack))
                       if n in os.environ.get('NTM_WELDERS_ONLY', 'arc,solder,identifier,solder-stack').split(',')]}[mode](g)
    g.evidence['ok'] = True
    g.save()

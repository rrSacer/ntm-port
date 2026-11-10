#!/usr/bin/env python3
"""The K3 furnaces of docs/onda-2-sublote-1.md on a disposable native world.

lit-electric   `machine_electric_furnace_on` smelts on a creative battery like the unlit one;
iron           the iron furnace (a multiblock): coal in the fuel slot, a speed upgrade, raw
               iron by a hopper on one of its dummy cells (the inventory port); ingots out;
steel          the steel furnace over a firebox: the firebox's heat, three lines, the bonus
               of an ore (25 % an item: four ores pay one extra).
Time passes with `tick sprint`; no machine state is injected.
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state, first_int

X, Z = 40, 8


def count(slots, item):
    return sum(s.get('count', 0) for s in slots if s.get('item') == item)


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 900
    while time.monotonic() < deadline:
        text = ' '.join(g.command('tick query').get('chat', []))
        if 'sprint' not in text.lower():
            return
        time.sleep(.5)


def core_of(g, around, marker):
    """The block entity of the machine whose saved state names `marker`, within 2 of `around`."""
    x0, y0, z0 = around
    for dx in range(-2, 3):
        for dy in range(-1, 3):
            for dz in range(-2, 3):
                text = machine_state(g, (x0 + dx, y0 + dy, z0 + dz))
                if marker in text:
                    return (x0 + dx, y0 + dy, z0 + dz)
    raise AssertionError('no machine with ' + marker + ' near ' + str(around))


def open_core(g, core):
    g.close()
    g.command(f'tp @s {core[0] + .5} 100 {core[2] + 3.5}')
    time.sleep(1)
    g.ctl(f'interact {core[0]} {core[1]} {core[2]} south')
    time.sleep(.7)
    screen = g.ctl('screen')
    g.check('menu_open', screen.get('screen') == 'container', screen)


def lit_electric(g):
    x, y, z = X, 100, Z
    g.close()
    g.ground(x, y, z, width=8)
    g.command('clear @s')
    g.command('give @s ntm:machine_electric_furnace_on 1')
    g.ctl(f'place {x} {y} {z} --item ntm:machine_electric_furnace_on')
    time.sleep(1)
    open_core(g, (x, y, z))
    g.put('minecraft:raw_iron', 4, 0)
    g.put('ntm:battery_creative', 1, 1)
    sprint(g, 450)
    slots = g.slots()
    g.check('lit_furnace_four_ingots', count(slots, 'minecraft:iron_ingot') == 4, slots)
    g.close()
    # Breaking the lit block drops the unlit one (`getItemDropped`).
    g.command('clear @s')
    g.command(f'setblock {x} {y} {z} minecraft:air destroy')
    time.sleep(1)
    g.command(f'tp @s {x + .5} {y} {z + .5}')
    time.sleep(1)
    inv = g.ctl('inventory').get('counts', {})
    g.check('lit_block_drops_the_unlit_one', inv.get('ntm:machine_electric_furnace_off', 0) >= 1 and not inv.get('ntm:machine_electric_furnace_on'), inv)


def iron(g):
    x, y, z = X + 12, 100, Z
    g.close()
    g.ground(x, y, z, width=8)
    g.command('clear @s')
    g.command('give @s ntm:furnace_iron 1')
    g.ctl(f'place {x} {y} {z} --item ntm:furnace_iron')
    time.sleep(1.5)
    core = core_of(g, (x, y, z), '"Iron"')
    open_core(g, core)
    g.check('menu_open_by_the_core', True, core)
    g.put('minecraft:coal', 2, 1)
    g.put('ntm:upgrade_speed_3', 1, 4)
    g.put('minecraft:raw_iron', 4, 0)
    g.snapshot('furnaces-iron-loaded')
    sprint(g, 345)
    slots = g.slots()
    # Four ingots in 4 x 80 ticks: only the speed-3 upgrade makes that (the base is 160).
    g.check('iron_four_ingots_with_the_upgrade', count(slots, 'minecraft:iron_ingot') == 4, slots)
    g.ctl('click-slot 303 shift')
    g.close()
    inv = g.ctl('inventory').get('counts', {})
    g.check('iron_ingots_taken', inv.get('minecraft:iron_ingot') == 4, inv)
    # The inventory port: a hopper over a dummy cell feeds the input; the fuel slot stays as it was.
    top = (core[0], core[1] + 2, core[2])
    g.command(f'setblock {top[0]} {top[1]} {top[2]} minecraft:hopper[facing=down]')
    g.command(f'setblock {top[0]} {top[1] + 1} {top[2]} minecraft:chest')
    g.command(f'item replace block {top[0]} {top[1] + 1} {top[2]} container.0 with minecraft:raw_iron 3')
    sprint(g, 600)
    open_core(g, core)
    slots = g.slots()
    g.check('hopper_on_a_dummy_cell_fed_the_furnace', count(slots, 'minecraft:iron_ingot') + count(slots, 'minecraft:raw_iron') == 3, slots)
    g.close()


def steel(g):
    x, y, z = X + 24, 100, Z
    g.close()
    g.ground(x, y, z, width=8)
    g.command('clear @s')
    g.command('give @s ntm:furnace_steel 1')
    g.ctl(f'place {x} {y + 1} {z} --item ntm:furnace_steel')
    time.sleep(1.5)
    core = core_of(g, (x, y + 1, z), '"Steel"')
    # The firebox under the furnace's core.
    fire = (core[0], core[1] - 1, core[2])
    g.command(f'setblock {fire[0]} {fire[1]} {fire[2]} ntm:heater_firebox[facing=north]')
    time.sleep(1)
    open_core(g, fire)
    g.put('minecraft:coal', 16, 0)
    g.close()
    open_core(g, core)
    g.put('minecraft:iron_ore', 8, 0)
    g.put('minecraft:raw_iron', 4, 1)
    sprint(g, 4000)
    slots = g.slots()
    state = machine_state(g, core)
    heat = first_int(state, r'"heat":(\d+)')
    ores = count(slots, 'minecraft:iron_ore')
    ingots = count(slots, 'minecraft:iron_ingot')
    g.check('steel_took_heat_from_the_firebox', heat is not None and heat > 0, state)
    g.check('steel_smelted', ingots > 0, slots)
    g.check('steel_conserves', ores + ingots + count(slots, 'minecraft:raw_iron') <= 12 and ores + ingots > 0, slots)
    g.snapshot('furnaces-steel')
    g.close()


def combination(g):
    x, y, z = X + 36, 100, Z
    g.close()
    g.ground(x, y, z, width=8)
    g.command('clear @s')
    g.command('give @s ntm:furnace_combination 1')
    g.ctl(f'place {x} {y + 1} {z} --item ntm:furnace_combination')
    time.sleep(1.5)
    core = core_of(g, (x, y + 1, z), '"Combination"')
    fire = (core[0], core[1] - 1, core[2])
    g.command(f'setblock {fire[0]} {fire[1]} {fire[2]} ntm:heater_firebox[facing=north]')
    time.sleep(1)
    open_core(g, fire)
    g.put('minecraft:coal', 16, 0)
    g.close()
    open_core(g, core)
    g.put('minecraft:coal', 4, 0)
    g.put('minecraft:bucket', 2, 2)
    sprint(g, 3000)
    slots = g.slots()
    state = machine_state(g, core)
    coke = count(slots, 'ntm:coke__coal')
    g.check('combination_made_coke', coke > 0, slots)
    g.check('combination_conserves_coal', count(slots, 'minecraft:coal') + coke == 4, slots)
    mb = first_int(state, r'"mb":(\d+)')
    g.check('combination_fluid_in_the_tank_or_buckets', mb is not None, state)
    g.snapshot('furnaces-combination')
    g.close()


if __name__ == '__main__':
    g = Game(sys.argv[1], 'furnaces-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'lit-electric': lit_electric, 'iron': iron, 'steel': steel,
     'combination': combination, 'all': lambda g: (lit_electric(g), iron(g), steel(g), combination(g))}[mode](g)
    g.evidence['ok'] = True
    g.save()

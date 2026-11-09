#!/usr/bin/env python3
"""Brick furnace acceptance (docs/onda-2-sublote-1.md) on a disposable native world.

setup          the furnace by its item; the fuel slot refuses a stone; 16 raw iron
               and 2 coal through the screen smelt to 16 ingots and one coal ash
               (3 200 burn ticks of coal ash cross the 2 000 threshold once); the
               automation: a hopper over it feeds the input only, one beside it
               the fuel, one under it takes the output and the ash;
capture-state  a second furnace stopped in the middle of a smelt;
check-state    after reopening the world it goes on and finishes with every item.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state, first_int

X, Z = 40, 8
FURNACE = (X, 102, Z)
SECOND = (X + 8, 100, Z)
SAVED = 'furnace-brick-saved.json'


def count(slots, item):
    return sum(s.get('count', 0) for s in slots if s.get('item') == item)


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        text = ' '.join(g.command('tick query').get('chat', []))
        if 'sprint' not in text.lower():
            return
        time.sleep(.5)


def open_at(g, p):
    g.close()
    g.command(f'tp @s {p[0] + .5} 100 {p[2] + 3.5}')
    time.sleep(1)
    g.ctl(f'interact {p[0]} {p[1]} {p[2]} south')
    time.sleep(.7)
    screen = g.ctl('screen')
    g.check('menu_open', screen.get('screen') == 'container', screen)


def state(g, p):
    text = machine_state(g, p)
    return {k: first_int(text, rf'"{k}":(\d+)') for k in ('burn_time', 'max_burn_time', 'progress', 'ash_wood', 'ash_coal', 'ash_misc')}


def chest_count(g, p, item):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    return sum(int(n) for n, i in re.findall(r'count: (\d+), id: "?([\w:]+)"?', text) if i == item) or sum(
        int(n) for i, n in re.findall(r'id: "?([\w:]+)"?, count: (\d+)', text) if i == item)


def setup(g):
    x, y, z = FURNACE
    g.close()
    g.ground(x, 100, z, width=14)
    g.command('clear @s')
    g.command('give @s ntm:machine_furnace_brick_off 2')
    g.ctl(f'place {x} {y} {z} --item ntm:machine_furnace_brick_off')
    time.sleep(1)
    block = ' '.join(g.command(f'data get block {x} {y} {z} ntm').get('chat', []))
    g.check('furnace_placed_as_a_machine', 'BrickFurnace' in block, block)
    open_at(g, FURNACE)
    # The fuel slot (1) refuses what does not burn; the input (0) takes anything.
    g.command('give @s minecraft:stone 1')
    time.sleep(.5)
    screen = g.ctl('screen')
    src = next(s['slot'] for s in screen['slots'] if s['slot'] < 36 and s.get('item') == 'minecraft:stone')
    g.ctl(f'click-slot {src}')
    g.ctl('click-slot 301')
    screen = g.ctl('screen')
    g.check('stone_refused_in_the_fuel_slot', not any(s['slot'] == 301 and s.get('item') for s in screen['slots']), screen)
    g.ctl(f'click-slot {src}')
    g.command('clear @s minecraft:stone')
    g.put('minecraft:raw_iron', 16, 0)
    g.put('minecraft:coal', 2, 1)
    g.snapshot('furnace-brick-loaded')
    sprint(g, 3300)
    slots = g.slots()
    g.check('sixteen_ingots', count(slots, 'minecraft:iron_ingot') == 16, slots)
    g.check('one_coal_ash', count(slots, 'ntm:powder_ash__coal') == 1, slots)
    g.check('no_raw_iron_left', count(slots, 'minecraft:raw_iron') == 0, slots)
    g.check('ash_level_after', state(g, FURNACE)['ash_coal'] == 3200 - 2000, state(g, FURNACE))
    g.snapshot('furnace-brick-done')
    g.ctl('click-slot 302 shift')
    g.ctl('click-slot 303 shift')
    g.close()
    inv = g.ctl('inventory').get('counts', {})
    g.check('ingots_taken', inv.get('minecraft:iron_ingot') == 16, inv)
    g.check('ash_taken', inv.get('ntm:powder_ash__coal') == 1, inv)
    automation(g)
    g.evidence['ok'] = True


def automation(g):
    x, y, z = FURNACE
    # raw iron chest -> hopper -> furnace top; coal chest -> hopper -> furnace side;
    # furnace bottom -> hopper -> output chest.
    for cmd in (
        f'setblock {x} {y + 1} {z} minecraft:hopper[facing=down]',
        f'setblock {x} {y + 2} {z} minecraft:chest',
        f'setblock {x + 1} {y} {z} minecraft:hopper[facing=west]',
        f'setblock {x + 2} {y} {z} minecraft:chest',
        f'setblock {x} {y - 1} {z} minecraft:hopper[facing=down]',
        f'setblock {x} {y - 2} {z} minecraft:chest',
    ):
        g.command(cmd)
    g.command(f'item replace block {x} {y + 2} {z} container.0 with minecraft:raw_iron 8')
    g.command(f'item replace block {x} {y + 2} {z} container.1 with minecraft:coal 4')
    g.command(f'item replace block {x + 2} {y} {z} container.0 with minecraft:coal 2')
    g.command(f'item replace block {x + 2} {y} {z} container.1 with minecraft:iron_ingot 3')
    sprint(g, 2500)
    sprint(g, 400)
    out = (x, y - 2, z)
    ingots = chest_count(g, out, 'minecraft:iron_ingot')
    fuel_in_out = chest_count(g, out, 'minecraft:coal')
    g.check('automation_smelted_8_ingots', ingots == 8, [ingots, g.command(f'data get block {out[0]} {out[1]} {out[2]} Items')])
    # The bottom face leaves the fuel slot alone (canExtractItem is slot >= 2): the coal stays in.
    g.check('hopper_below_does_not_pull_the_fuel', fuel_in_out == 0, fuel_in_out)
    # The ingots in the side chest never went in: only slot 1 (fuel) is open to the sides.
    g.check('side_hopper_fed_only_the_fuel_slot', chest_count(g, (x + 2, y, z), 'minecraft:iron_ingot') == 3, 'side chest')


def capture(g):
    x, y, z = SECOND
    g.close()
    g.command('clear @s')
    g.command('give @s ntm:machine_furnace_brick_off 1')
    g.ctl(f'place {x} {y} {z} --item ntm:machine_furnace_brick_off')
    time.sleep(1)
    open_at(g, SECOND)
    g.put('minecraft:raw_iron', 16, 0)
    g.put('minecraft:coal', 3, 1)
    sprint(g, 1700)
    slots = g.slots()
    s = state(g, SECOND)
    g.check('mid_smelt', s['burn_time'] > 0 and 0 < count(slots, 'minecraft:iron_ingot') < 16, [s, slots])
    raw = count(slots, 'minecraft:raw_iron')
    ingots = count(slots, 'minecraft:iron_ingot')
    g.check('conserved_before_save', raw + ingots == 16, slots)
    g.close()
    (g.folder / SAVED).write_text(json.dumps({'state': s, 'raw': raw, 'ingots': ingots, 'coal': count(slots, 'minecraft:coal')}))
    g.ctl('save')


def check(g):
    old = json.loads((g.folder / SAVED).read_text())
    now = state(g, SECOND)
    g.check('burn_state_survived_the_reopen', now['max_burn_time'] == old['state']['max_burn_time'] and now['burn_time'] >= old['state']['burn_time'] - 5, [old, now])
    open_at(g, SECOND)
    slots = g.slots()
    g.check('items_survived_the_reopen', count(slots, 'minecraft:raw_iron') + count(slots, 'minecraft:iron_ingot') == 16, slots)
    g.close()
    sprint(g, 2000)
    open_at(g, SECOND)
    slots = g.slots()
    g.check('finished_after_the_reopen', count(slots, 'minecraft:iron_ingot') == 16 and count(slots, 'minecraft:raw_iron') == 0, slots)
    g.close()


if __name__ == '__main__':
    g = Game(sys.argv[1], 'furnace-brick-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

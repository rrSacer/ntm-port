#!/usr/bin/env python3
"""Physical mining regression, using the native UI/terrain fixture.

Adapts bedrock.py's excavator and petroleo_d well/fracker scenarios to the
native socket driver. No machine state is injected.
"""
import sys
import time
from native_game import Game
from native_solar_stirling import place
from native_stations import fill_barrel, open_machine, type_duct
from native_zirnox import machine
from native_recipes import screen_json, core_of


def state(g, p):
    return g.ctl('block ' + ' '.join(map(str, p))).get('state', '')


def wait(g, name, probe, predicate, timeout=120):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = probe()
        if predicate(value):
            g.check(name, True, value)
            return value
        time.sleep(.4)
    g.check(name, False, value)


def place_machine(g, block, x, y, z):
    try:
        return place(g, block, x, y, z)
    except RuntimeError as error:
        # The excavator's clicked bottom/front centre is intentionally hollow.
        # The socket's single-cell placement check can report unchanged there;
        # require the real core instead of accepting that report blindly.
        if 'the block did not change' not in str(error):
            raise
        core = core_of(g, 'ntm:'+block, x, y, z, reach=(4, 4))
        g.check(block+'_real_core_placed', state(g, core).startswith('ntm:'+block), core)
        return core


def open_excavator(g, core):
    cx, cy, cz = core
    g.close()
    g.command(f'setblock {cx} {cy-1} {cz+6} minecraft:stone')
    g.command(f'tp @s {cx+.5} {cy} {cz+6.5}')
    g.ctl(f'interact {cx} {cy} {cz+3} south')
    time.sleep(.7)
    g.check('excavator_menu_open', g.ctl('screen').get('screen') == 'container', g.ctl('screen'))


def excavator(g):
    g.evidence['phase'] = 'excavator'
    x, y, z = 20, 100, 8
    g.ground(x-4, y, z-4, width=12, height=18)
    core = place_machine(g, 'machine_excavator', x, y, z)
    cx, cy, cz = core
    target = (cx, cy-4, cz)
    g.command(f'fill {cx-1} {cy-4} {cz-1} {cx+1} {cy-4} {cz+1} minecraft:iron_ore')
    chest = (cx+1, cy-4, cz)
    g.command('setblock ' + ' '.join(map(str, chest)) + ' minecraft:chest')
    g.command('item replace block ' + ' '.join(map(str, chest)) + ' container.0 with minecraft:diamond 7')
    open_excavator(g, core)
    g.put('ntm:battery_creative', 1, 0)
    g.put('ntm:drillbit__ferro', 1, 4)
    screen = screen_json('machine_excavator')
    button = next(b for b in screen['buttons'] if b['id'] == 0)
    g.button(button['rect'], screen['size'])
    time.sleep(.6)
    g.close()
    wait(g, 'real_ore_removed', lambda: state(g, target), lambda s: s.startswith('minecraft:air'))
    wait(g, 'chest_removed_without_second_loot', lambda: state(g, chest), lambda s: s.startswith('minecraft:air'))
    open_excavator(g, core)
    actual = g.slots()
    raw = sum(s.get('count', 0) for s in actual if s.get('item') == 'minecraft:iron_ore')
    g.check('real_ore_credited_once', raw == 8, actual)
    g.close()
    # The drop falls down the shaft the excavator keeps digging: a wide radius, not a race against the fall.
    items = g.ctl('entities 150')['entities']
    diamonds = sum((e.get('item') or {}).get('count', 0) for e in items
                   if (e.get('item') or {}).get('item') == 'minecraft:diamond')
    g.check('mined_be_contents_retained_once', diamonds == 7, items)
    g.evidence['excavator_core'] = core


def oil(g, fracker=False):
    name = 'fracker' if fracker else 'well'
    g.evidence['phase'] = name
    x, y, z = (90 if fracker else 55), 100, 8
    g.ground(x-4, y, z-4, width=12, height=18)
    block = 'machine_fracking_tower' if fracker else 'machine_well'
    core = place_machine(g, block, x, y, z)
    cx, cy, cz = core
    # A finite column of normal oil lets both machines exhaust a real deposit.
    g.command(f'fill {cx} {cy-6} {cz} {cx} {cy-1} {cz} ntm:ore_oil')
    open_machine(g, x, y, z)
    g.put('ntm:battery_creative', 1, 0)
    g.put('ntm:upgrade_overdrive_3', 1, 5)
    g.close()
    if fracker:
        barrel = (cx-1, cy, cz)
        # Same west/core fluid contact used by bedrock.py:petroleo_d.
        g.command('setblock ' + ' '.join(map(str, barrel)) + ' ntm:barrel_steel[facing=south]')
        g.command(f'setblock {cx-1} {cy-1} {cz+2} minecraft:stone')
        g.command(f'tp @s {cx-.5} {cy} {cz+2.5}')
        fill_barrel(g, barrel, 1, 'ntm:fluid_barrel_full__fracksol')
    def tanks():
        return machine(g, core)['data']['tanks']
    wait(g, name+'_produces_oil', tanks, lambda ts: ts[0]['mb'] >= 500)
    wait(g, name+'_exhausts_real_deposit', lambda: state(g, (cx, cy-1, cz)),
         lambda s: s.startswith('ntm:ore_oil_empty') or s.startswith('ntm:oil_pipe'), timeout=180)
    g.evidence[name+'_core'] = core
    g.evidence[name+'_tanks'] = tanks()


def main():
    g = Game(sys.argv[1], 'mining')
    g.command('gamemode creative')
    excavator(g)
    oil(g)
    oil(g, True)
    g.command('forceload remove all')
    g.snapshot('mining-final')
    g.evidence['ok'] = True
    g.save()


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Game proof of fluid identifier handling in machine_combustion_engine (CombustionEngine.java:81, slot 4).

The combustion engine reads the fluid identifier from slot 4 (`if(tank.setType(4, slots)) tenth = 0`).
A fluid identifier types the tank, empties any previous fuel, and resets tenth.
An identifier without a type (bare, {ntm_fluid:"none"}, or unknown) empties the tank
and leaves it untyped ('', 0), per FluidTank.java:132-150.
"""
import json
import re
import sys
import time

from native_game import Game
from native_recipes import core_of, screen_json


def machine_state(game, at):
    """The machine's saved state (`ntm` string of its block entity), as text."""
    result = game.command(f'data get block {at[0]} {at[1]} {at[2]} ntm')
    return ' '.join(result.get('chat', [])).replace('\\"', '"')


def tank_state(game, at):
    """(fluid, mb) of the machine's `tank` in its saved state; None when it has no tank."""
    found = re.search(r'"tank":\{([^{}]*)\}', machine_state(game, at))
    if not found:
        return None
    fluid = re.search(r'"fluid":"(\w*)"', found[1])
    mb = re.search(r'"mb":(\d+)', found[1])
    return (fluid[1] if fluid else '', int(mb[1]) if mb else 0)


def engine_tenth(game, at):
    found = re.search(r'"tenth":(\d+)', machine_state(game, at))
    return int(found.group(1)) if found else 0


def identifier_in_slot(game, slot, custom):
    """Put an identifier (`custom` is custom_data string or None for bare) into container slot (300 + slot)."""
    game.ctl(f'click-slot {300 + slot} shift')
    game.command('clear @s')
    comp = '' if custom is None else f'[minecraft:custom_data={custom}]'
    game.command(f'give @s ntm:fluid_identifier_multi{comp} 1')
    time.sleep(0.5)
    screen = game.ctl('screen')
    src = next(s['slot'] for s in screen.get('slots', []) if s['slot'] < 36 and s.get('item') == 'ntm:fluid_identifier_multi')
    game.ctl(f'click-slot {src}')
    game.ctl(f'click-slot {300 + slot}')
    time.sleep(0.4)
    game.command('tick sprint 20')
    time.sleep(2)


def wait_tank(game, at, expected_fluid, retries=12):
    for _ in range(retries):
        st = tank_state(game, at)
        if st is not None and st[0] == expected_fluid:
            return st
        game.command('tick sprint 20')
        time.sleep(1)
    return tank_state(game, at)



def start_area(game, x, y, z, wide):
    game.close()
    game.command(f"forceload add {x - 6} {z - 8} {x + wide} {z + 8}")
    time.sleep(3)
    game.command(f"fill {x - 6} {y} {z - 8} {x + wide} {y + 6} {z + 8} minecraft:air")
    game.command(f"fill {x - 6} {y - 1} {z - 8} {x + wide} {y - 1} {z + 8} minecraft:stone")
    game.command("clear @s")

def open_near(game, x, y, z):
    """Open a machine player-placed at (x, y, z) from the south, a cell of its column."""
    for height in range(0, 3):
        for back in (3.5, 4.5, 2.5):
            try:
                game.command(f"tp @s {x + .5} {y} {z + back} 180 0")
                time.sleep(.5)
                game.ctl(f"interact {x} {y + height} {z} south")
                time.sleep(.8)
                if game.ctl("screen").get("screen") == "container":
                    return
            except RuntimeError:
                pass
    raise AssertionError("cannot open the machine")


def run_test(game):
    game.evidence['phase'] = 'ident_engine'
    x, y, z = 320, 100, 8
    game.close()
    start_area(game, x, y, z, 10)

    game.command('give @s ntm:machine_combustion_engine 1')
    game.command(f'tp @s {x + 0.5} {y} {z + 3.5} 180 0')
    time.sleep(2)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_combustion_engine')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_combustion_engine', x, y, z, reach=(4, 3))
    open_near(game, x, y, z)
    game.check('menu_open', game.ctl('screen').get('screen') == 'container', None)

    # 1. Fill tank with diesel via canister in slot 0
    game.put('ntm:canister_full__diesel', 2, 0)
    filled = None
    for _ in range(12):
        game.command('tick sprint 20')
        time.sleep(1)
        filled = tank_state(game, core)
        if filled and filled[0] == 'diesel' and filled[1] > 0:
            break
    game.check('tank_holds_diesel_first', filled is not None and filled[0] == 'diesel' and filled[1] > 0, filled)

    # 2. Establish precondition with tenth > 0 before inserting fluid identifier:
    # Insert a steel piston into slot 2, freeze ticks, set slider to setting 1 (speed=2 tenths/t),
    # ignite and step 1 tick so tenth becomes non-zero (8), then shut off and unfreeze.
    machine = screen_json('machine_combustion_engine')
    game.put('ntm:piston_set__steel', 1, 2)
    slider_1 = next(b for b in machine['buttons'] if b['id'] == 101)
    slider_0 = next(b for b in machine['buttons'] if b['id'] == 100)
    ignition = next(b for b in machine['buttons'] if b['id'] == 0)

    game.command('tick freeze')
    time.sleep(0.2)
    game.button(slider_1['rect'], machine['size'])
    game.button(ignition['rect'], machine['size'])
    game.command('tick step 1')
    time.sleep(0.5)

    tenth_before = engine_tenth(game, core)
    if tenth_before == 0:
        game.command('tick step 1')
        time.sleep(0.5)
        tenth_before = engine_tenth(game, core)

    # Shut off: reset slider to 0 and toggle ignition off
    game.button(slider_0['rect'], machine['size'])
    game.button(ignition['rect'], machine['size'])
    game.command('tick step 1')
    time.sleep(0.5)
    game.command('tick unfreeze')
    time.sleep(0.5)

    tenth_before = engine_tenth(game, core)
    game.evidence['tenth_before'] = tenth_before
    game.check('engine_tenth_nonzero_before', tenth_before > 0, tenth_before)

    # 3. Put kerosene identifier into slot 4: tank types to kerosene, empties, and resets tenth to 0
    # (CombustionEngine.java:81: if(tank.setType(4, slots)) tenth = 0)
    identifier_in_slot(game, 4, '{ntm_fluid:"kerosene"}')
    typed_k = wait_tank(game, core, 'kerosene')
    tenth_after = engine_tenth(game, core)
    game.evidence['tenth_after'] = tenth_after
    game.evidence['kerosene_state'] = machine_state(game, core)[:600]
    game.check('kerosene_identifier_types_and_empties_engine_tank', typed_k == ('kerosene', 0) and tenth_after == 0, (typed_k, tenth_after))

    # 4. Bare identifier in slot 4: leaves tank untyped and empty ('', 0)
    identifier_in_slot(game, 4, None)
    untyped_bare = wait_tank(game, core, '')
    game.evidence['bare_state'] = machine_state(game, core)[:600]
    game.check('bare_identifier_leaves_engine_tank_untyped', untyped_bare == ('', 0), untyped_bare)

    # 5. Type to diesel again, then test {ntm_fluid:"none"}
    identifier_in_slot(game, 4, '{ntm_fluid:"diesel"}')
    wait_tank(game, core, 'diesel')
    identifier_in_slot(game, 4, '{ntm_fluid:"none"}')
    untyped_none = wait_tank(game, core, '')
    game.evidence['none_state'] = machine_state(game, core)[:600]
    game.check('none_identifier_leaves_engine_tank_untyped', untyped_none == ('', 0), untyped_none)

    # 6. Type to diesel again, then test unknown fluid name {ntm_fluid:"bogus"}
    identifier_in_slot(game, 4, '{ntm_fluid:"diesel"}')
    wait_tank(game, core, 'diesel')
    identifier_in_slot(game, 4, '{ntm_fluid:"bogus"}')
    untyped_unknown = wait_tank(game, core, '')
    game.evidence['unknown_state'] = machine_state(game, core)[:600]
    game.check('unknown_identifier_leaves_engine_tank_untyped', untyped_unknown == ('', 0), untyped_unknown)

    game.ctl('click-slot 304 shift')
    game.command('clear @s')
    game.close()


if __name__ == '__main__':
    game = Game(sys.argv[1], 'ident-engine')
    game.ctl('wait-ready 120')
    run_test(game)
    game.evidence['ok'] = True
    game.save()

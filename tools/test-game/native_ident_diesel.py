#!/usr/bin/env python3
"""Game proof of fluid identifier handling in machine_diesel (TileEntityMachineDiesel.java:167, slot 3).

The diesel generator reads the fluid identifier from slot 3 (`tank.setType(3, slots)`).
A fluid identifier types the tank and empties any previous fuel.
An identifier without a type (bare, {ntm_fluid:"none"}, or unknown) empties the tank
and leaves it untyped ('', 0), per FluidTank.java:132-150.
"""
import json
import re
import sys
import time

from native_game import Game


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
    time.sleep(1.5)


def wait_tank(game, at, expected_fluid, retries=12):
    for _ in range(retries):
        st = tank_state(game, at)
        if st is not None and st[0] == expected_fluid:
            return st
        game.command('tick sprint 20')
        time.sleep(1)
    return tank_state(game, at)


def run_test(game):
    game.evidence['phase'] = 'ident_diesel'
    x, y, z = 260, 100, 8
    game.close()
    game.command(f'forceload add {x - 6} {z - 6} {x + 6} {z + 6}')
    time.sleep(2)
    game.command(f'fill {x - 6} {y} {z - 6} {x + 6} {y + 4} {z + 6} minecraft:air')
    game.command(f'fill {x - 6} {y - 1} {z - 6} {x + 6} {y - 1} {z + 6} minecraft:stone')
    game.command('clear @s')

    game.command(f'setblock {x} {y} {z} ntm:machine_diesel[facing=south,honey_level=0]')
    game.command(f'tp @s {x + 0.5} {y} {z + 3.5}')
    time.sleep(1.5)
    game.ctl(f'interact {x} {y} {z} south')
    time.sleep(1)
    game.check('menu_open', game.ctl('screen').get('screen') == 'container', None)

    core = (x, y, z)
    initial = tank_state(game, core)
    # TileEntityMachineDiesel.java:105: tank = new FluidTank(Fluids.DIESEL, fuelCap)\ngame.check('tank_typed_diesel_initially', initial == ('diesel', 0), initial)

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

    # 2. Put kerosene identifier into slot 3: tank types to kerosene and empties (FluidTank.setType)
    identifier_in_slot(game, 3, '{ntm_fluid:"kerosene"}')
    typed_k = wait_tank(game, core, 'kerosene')
    game.evidence['kerosene_state'] = machine_state(game, core)[:600]
    game.check('kerosene_identifier_types_and_empties_tank', typed_k == ('kerosene', 0), typed_k)

    # 3. Put diesel identifier into slot 3: tank types to diesel
    identifier_in_slot(game, 3, '{ntm_fluid:"diesel"}')
    typed_d = wait_tank(game, core, 'diesel')
    game.check('diesel_identifier_types_tank', typed_d == ('diesel', 0), typed_d)

    # 4. Bare identifier in slot 3: leaves tank untyped and empty ('', 0)
    identifier_in_slot(game, 3, None)
    untyped_bare = wait_tank(game, core, '')
    game.evidence['bare_state'] = machine_state(game, core)[:600]
    game.check('bare_identifier_leaves_tank_untyped', untyped_bare == ('', 0), untyped_bare)

    # 5. Type to diesel again, then test {ntm_fluid:"none"}
    identifier_in_slot(game, 3, '{ntm_fluid:"diesel"}')
    wait_tank(game, core, 'diesel')
    identifier_in_slot(game, 3, '{ntm_fluid:"none"}')
    untyped_none = wait_tank(game, core, '')
    game.evidence['none_state'] = machine_state(game, core)[:600]
    game.check('none_identifier_leaves_tank_untyped', untyped_none == ('', 0), untyped_none)

    # 6. Type to diesel again, then test unknown fluid name {ntm_fluid:"unknown_fluid_xyz"}
    identifier_in_slot(game, 3, '{ntm_fluid:"diesel"}')
    wait_tank(game, core, 'diesel')
    identifier_in_slot(game, 3, '{ntm_fluid:"unknown_fluid_xyz"}')
    untyped_unknown = wait_tank(game, core, '')
    game.evidence['unknown_state'] = machine_state(game, core)[:600]
    game.check('unknown_identifier_leaves_tank_untyped', untyped_unknown == ('', 0), untyped_unknown)

    game.ctl('click-slot 303 shift')
    game.command('clear @s')
    game.close()


if __name__ == '__main__':
    game = Game(sys.argv[1], 'ident-diesel')
    game.ctl('wait-ready 120')
    run_test(game)
    game.evidence['ok'] = True
    game.save()

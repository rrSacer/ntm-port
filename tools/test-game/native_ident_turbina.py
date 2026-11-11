#!/usr/bin/env python3
"""Game proof of fluid identifier handling in machine_turbinegas (TileEntityMachineTurbineGas.java:107-110, slot 1).

The gas turbine only accepts combustible fluids of GAS grade (syngas, biogas, gas, etc.) in slot 1.
An identifier of a non-gas fluid (diesel, kerosene, water) or an identifier without a type (bare,
{ntm_fluid:"none"}, unknown) CANNOT change the tank type (TileEntityMachineTurbineGas.java:107-110).
Another gas-grade identifier changes the type.
"""
import json
import re
import sys
import time

from native_game import Game
from native_recipes import core_of


def machine_state(game, at):
    """The machine's saved state (`ntm` string of its block entity), as text."""
    result = game.command(f'data get block {at[0]} {at[1]} {at[2]} ntm')
    return ' '.join(result.get('chat', [])).replace('\\"', '"')


def gas_tank_state(game, at):
    """(fluid, mb) of the machine's `gas` tank in its saved state; None when it has no gas tank."""
    found = re.search(r'"gas":\{([^{}]*)\}', machine_state(game, at))
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
    time.sleep(2)


def wait_gas_tank(game, at, expected_fluid, retries=12):
    for _ in range(retries):
        st = gas_tank_state(game, at)
        if st is not None and st[0] == expected_fluid:
            return st
        game.command('tick sprint 20')
        time.sleep(1)
    return gas_tank_state(game, at)



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
    game.evidence['phase'] = 'ident_turbina'
    x, y, z = 330, 100, 8
    game.close()
    start_area(game, x - 4, y, z, 16)

    game.command('give @s ntm:machine_turbinegas 1')
    game.command(f'tp @s {x + 0.5} {y} {z + 3.5} 180 0')
    time.sleep(2)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_turbinegas')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_turbinegas', x, y, z, reach=(6, 3))
    open_near(game, x, y, z)
    game.check('menu_open', game.ctl('screen').get('screen') == 'container', None)

    # 1. Type gas tank with syngas (grade GAS) in slot 1
    identifier_in_slot(game, 1, '{ntm_fluid:"syngas"}')
    typed_syn = wait_gas_tank(game, core, 'syngas')
    game.evidence['syngas_state'] = machine_state(game, core)[:600]
    game.check('syngas_identifier_types_gas_tank', typed_syn is not None and typed_syn[0] == 'syngas', typed_syn)

    # 2. Diesel identifier in slot 1 (HIGH grade, not GAS): must be REJECTED, tank stays syngas
    identifier_in_slot(game, 1, '{ntm_fluid:"diesel"}')
    st_diesel = gas_tank_state(game, core)
    game.check('diesel_identifier_rejected_by_gas_turbine', st_diesel is not None and st_diesel[0] == 'syngas', st_diesel)

    # 3. Kerosene identifier in slot 1 (AERO grade, not GAS): must be REJECTED, tank stays syngas
    identifier_in_slot(game, 1, '{ntm_fluid:"kerosene"}')
    st_kero = gas_tank_state(game, core)
    game.check('kerosene_identifier_rejected_by_gas_turbine', st_kero is not None and st_kero[0] == 'syngas', st_kero)

    # 4. Bare identifier in slot 1: NONE is not a gas, must be REJECTED, tank stays syngas
    identifier_in_slot(game, 1, None)
    st_bare = gas_tank_state(game, core)
    game.check('bare_identifier_rejected_by_gas_turbine', st_bare is not None and st_bare[0] == 'syngas', st_bare)

    # 5. {ntm_fluid:"none"} in slot 1: NONE is not a gas, must be REJECTED, tank stays syngas
    identifier_in_slot(game, 1, '{ntm_fluid:"none"}')
    st_none = gas_tank_state(game, core)
    game.check('none_identifier_rejected_by_gas_turbine', st_none is not None and st_none[0] == 'syngas', st_none)

    # 6. Unknown fluid in slot 1: unknown is not a gas, must be REJECTED, tank stays syngas
    identifier_in_slot(game, 1, '{ntm_fluid:"unknown_gas_xyz"}')
    st_unknown = gas_tank_state(game, core)
    game.check('unknown_identifier_rejected_by_gas_turbine', st_unknown is not None and st_unknown[0] == 'syngas', st_unknown)

    # 7. Another valid gas-grade identifier (biogas, grade GAS): types the gas tank to biogas!
    identifier_in_slot(game, 1, '{ntm_fluid:"biogas"}')
    typed_bio = wait_gas_tank(game, core, 'biogas')
    game.evidence['biogas_state'] = machine_state(game, core)[:600]
    game.check('biogas_identifier_types_gas_tank', typed_bio is not None and typed_bio[0] == 'biogas', typed_bio)

    game.ctl('click-slot 301 shift')
    game.command('clear @s')
    game.close()


if __name__ == '__main__':
    game = Game(sys.argv[1], 'ident-turbina')
    game.ctl('wait-ready 120')
    run_test(game)
    game.evidence['ok'] = True
    game.save()

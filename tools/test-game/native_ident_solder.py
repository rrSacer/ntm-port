#!/usr/bin/env python3
"""Game proof of fluid identifier handling in machine_soldering_station (TileEntityMachineSolderingStation.java:94, slot 8).

The soldering station reads the fluid identifier from slot 8 (`tank.setType(8, slots)`).
A fluid identifier types the tank and empties any previous fluid.
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


def core_of(g, around, marker):
    x0, y0, z0 = around
    for dx in range(-2, 3):
        for dy in range(-1, 3):
            for dz in range(-2, 3):
                if marker in machine_state(g, (x0 + dx, y0 + dy, z0 + dz)):
                    return (x0 + dx, y0 + dy, z0 + dz)
    raise AssertionError('no machine with ' + marker)


def open_any(g, cells):
    g.close()
    for cell in cells:
        g.command(f'tp @s {cell[0] + 0.5} 100 {cell[2] + 3.5}')
        time.sleep(0.8)
        for _ in range(2):
            try:
                g.ctl(f'interact {cell[0]} {cell[1]} {cell[2]} south')
            except RuntimeError:
                break
            deadline = time.monotonic() + 4
            while time.monotonic() < deadline:
                if g.ctl('screen').get('screen') == 'container':
                    return cell
                time.sleep(0.3)
            g.close()
    raise AssertionError('no cell opened the screen: ' + str(cells))


def run_test(game):
    game.evidence['phase'] = 'ident_solder'
    x, y, z = 96, 100, 8
    game.close()
    game.ground(x, y, z, width=8)
    game.command('clear @s')
    game.command('give @s ntm:machine_soldering_station 1')
    time.sleep(0.5)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_soldering_station')
    time.sleep(1.5)
    core = core_of(game, (x, y, z), '"Solder"')
    cells = [(core[0] + dx, core[1], core[2] + dz) for dz in (1, 0, -1) for dx in (0, -1, 1)]
    open_any(game, cells)
    game.check('menu_open', game.ctl('screen').get('screen') == 'container', None)

    # Initial state: untyped
    initial = tank_state(game, core)
    game.check('solder_tank_untyped_initially', initial == ('', 0), initial)

    # 1. Oxygen identifier into slot 8: types solder tank to oxygen
    identifier_in_slot(game, 8, '{ntm_fluid:"oxygen"}')
    typed_ox = wait_tank(game, core, 'oxygen')
    game.evidence['oxygen_state'] = machine_state(game, core)[:600]
    game.check('oxygen_identifier_types_solder_tank', typed_ox == ('oxygen', 0), typed_ox)

    # 2. Reformgas identifier into slot 8: changes type to reformgas
    identifier_in_slot(game, 8, '{ntm_fluid:"reformgas"}')
    typed_ref = wait_tank(game, core, 'reformgas')
    game.evidence['reformgas_state'] = machine_state(game, core)[:600]
    game.check('reformgas_identifier_changes_solder_tank_type', typed_ref == ('reformgas', 0), typed_ref)

    # 3. Bare identifier in slot 8: leaves tank untyped and empty ('', 0)
    identifier_in_slot(game, 8, None)
    untyped_bare = wait_tank(game, core, '')
    game.evidence['bare_state'] = machine_state(game, core)[:600]
    game.check('bare_identifier_leaves_solder_tank_untyped', untyped_bare == ('', 0), untyped_bare)

    # 4. Re-type with oxygen, then test {ntm_fluid:"none"}
    identifier_in_slot(game, 8, '{ntm_fluid:"oxygen"}')
    wait_tank(game, core, 'oxygen')
    identifier_in_slot(game, 8, '{ntm_fluid:"none"}')
    untyped_none = wait_tank(game, core, '')
    game.evidence['none_state'] = machine_state(game, core)[:600]
    game.check('none_identifier_leaves_solder_tank_untyped', untyped_none == ('', 0), untyped_none)

    # 5. Re-type with oxygen, then test unknown fluid name {ntm_fluid:"unknown_fluid_xyz"}
    identifier_in_slot(game, 8, '{ntm_fluid:"oxygen"}')
    wait_tank(game, core, 'oxygen')
    identifier_in_slot(game, 8, '{ntm_fluid:"unknown_fluid_xyz"}')
    untyped_unknown = wait_tank(game, core, '')
    game.evidence['unknown_state'] = machine_state(game, core)[:600]
    game.check('unknown_identifier_leaves_solder_tank_untyped', untyped_unknown == ('', 0), untyped_unknown)

    game.ctl('click-slot 308 shift')
    game.command('clear @s')
    game.close()


if __name__ == '__main__':
    game = Game(sys.argv[1], 'ident-solder')
    game.ctl('wait-ready 120')
    run_test(game)
    game.evidence['ok'] = True
    game.save()

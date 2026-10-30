#!/usr/bin/env python3
"""Native energy batch 3 (docs/content-energy-combustion.md): fuel into HE on a battery.

Usage: python3 tools/test-game/native_combustion.py GAME [diesel|motor|turbofan|turbina ...]
Each phase puts the generator by a cable or by a buffer battery, fills it with
fuel from a full container in its slot, starts it as a player does (the screen's
ignition button, the engine's slider) and reads the battery's energy, as
native_redes.py does. The ports of the multiblocks are where the NTM has them:
the battery goes next to one of the port cells (container_bridge.rs).
  diesel     1 027 HE a tick from diesel, ignition; the buffer battery fills;
  motor      a steel piston set, the slider at 30, diesel: the battery fills;
  turbina    natural gas, lubricant and water from three barrels by typed ducts; start,
             29 s, the slider at the top: power on the battery and dense steam;
  turbofan   kerosene burns with no ignition; a redstone block by a connector
             stops it, taking it away starts it again.
"""
import json
import re
import sys
import time

from native_game import Game
from native_recipes import core_of, screen_json
from native_stations import fill_barrel, type_duct

FACING = {'north': (0, 0, -1), 'south': (0, 0, 1), 'east': (1, 0, 0), 'west': (-1, 0, 0)}


def world_cell(core, facing, cell):
    """container_bridge::world_cell: the placer's frame (x right, y up, z away) to the world."""
    f = FACING[facing]
    fwd = (-f[0], -f[2])
    right = (-fwd[1], fwd[0])
    return (core[0] + right[0] * cell[0] + fwd[0] * cell[2], core[1] + cell[1],
            core[2] + right[1] * cell[0] + fwd[1] * cell[2])


def machine_state(game, at):
    """The machine's saved state (the `ntm` string of its block entity), as text."""
    result = game.command(f'data get block {at[0]} {at[1]} {at[2]} ntm')
    return ' '.join(result.get('chat', [])).replace('\\"', '"')


def first_int(text, pattern):
    found = re.search(pattern, text)
    return int(found.group(1)) if found else None


def battery_power(game, at):
    return first_int(machine_state(game, at), r'"power":(\d+)')


def tank_mb(game, at):
    return first_int(machine_state(game, at), r'"tank":\{"fluid":"[a-z_]+","mb":(\d+)')


def facing_of(game, at):
    found = re.search(r'"facing":"(\w+)"', machine_state(game, at))
    return found.group(1) if found else 'north'


def start_area(game, x, y, z, wide):
    game.close()
    game.command(f'forceload add {x - 6} {z - 8} {x + wide} {z + 8}')
    time.sleep(3)
    game.command(f'fill {x - 6} {y} {z - 8} {x + wide} {y + 6} {z + 8} minecraft:air')
    game.command(f'fill {x - 6} {y - 1} {z - 8} {x + wide} {y - 1} {z + 8} minecraft:stone')
    game.command('clear @s')


def open_near(game, x, y, z):
    """Open a machine player-placed at (x, y, z) from the south, a cell of its column."""
    for height in range(0, 3):
        for back in (3.5, 4.5, 2.5):
            try:
                game.command(f'tp @s {x + .5} {y} {z + back}')
                time.sleep(.5)
                game.ctl(f'interact {x} {y + height} {z} south')
                time.sleep(.8)
                if game.ctl('screen').get('screen') == 'container':
                    return
            except RuntimeError:
                pass
    raise AssertionError('cannot open the machine')


def grows(game, battery, seconds=8):
    """Whether the battery's energy rises over `seconds`."""
    before = battery_power(game, battery) or 0
    time.sleep(seconds)
    after = battery_power(game, battery) or 0
    return before, after


def diesel(game):
    game.evidence['phase'] = 'diesel'
    x, y, z = 260, 100, 8
    start_area(game, x, y, z, 8)
    game.command(f'setblock {x} {y} {z} ntm:machine_diesel[facing=south,honey_level=0]')
    game.command(f'setblock {x + 1} {y} {z} ntm:red_cable')
    game.command(f'setblock {x + 2} {y} {z} ntm:machine_battery[facing=south]')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(2)
    game.ctl(f'interact {x} {y} {z} south')
    time.sleep(1)
    game.check('menu_open', game.ctl('screen').get('screen') == 'container', None)
    machine = screen_json('machine_diesel')
    game.put('ntm:canister_full__diesel', 2, 0)
    ignition = next(b for b in machine['buttons'] if b['id'] == 0)
    game.button(ignition['rect'], machine['size'])
    time.sleep(1)
    game.snapshot('combustion-diesel')
    game.close()
    before, after = grows(game, (x + 2, y, z))
    fuel = tank_mb(game, (x, y, z))
    game.evidence['battery'] = [before, after]
    game.evidence['diesel_mb'] = fuel
    game.check('battery_fills', after > before + 20_000, [before, after])
    game.check('fuel_burns', fuel is not None and fuel < 2_000 and fuel > 1_000, fuel)


def motor(game):
    game.evidence['phase'] = 'motor'
    x, y, z = 280, 100, 8
    start_area(game, x, y, z, 10)
    game.command('give @s ntm:machine_combustion_engine 1')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(2)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_combustion_engine')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_combustion_engine', x, y, z, reach=(4, 3))
    facing = facing_of(game, core)
    battery = world_cell(core, facing, (1, 0, -1))
    game.evidence.update(core=core, facing=facing, battery_at=battery)
    game.command(f'setblock {battery[0]} {battery[1]} {battery[2]} ntm:machine_battery[facing=south]')
    open_near(game, x, y, z)
    machine = screen_json('machine_combustion_engine')
    game.put('ntm:canister_full__diesel', 2, 0)
    game.put('ntm:piston_set__steel', 1, 2)
    slider = next(b for b in machine['buttons'] if b['id'] == 130)
    game.button(slider['rect'], machine['size'])
    ignition = next(b for b in machine['buttons'] if b['id'] == 0)
    game.button(ignition['rect'], machine['size'])
    time.sleep(1)
    game.snapshot('combustion-motor')
    game.close()
    before, after = grows(game, battery)
    game.evidence['battery'] = [before, after]
    game.check('battery_fills', after > before + 20_000, [before, after])
    game.check('fuel_burns', (tank_mb(game, core) or 0) < 2_000, tank_mb(game, core))


def turbofan(game):
    game.evidence['phase'] = 'turbofan'
    x, y, z = 300, 100, 8
    start_area(game, x, y, z, 12)
    game.command('give @s ntm:machine_turbofan 1')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(2)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_turbofan')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_turbofan', x, y, z, reach=(4, 3))
    facing = facing_of(game, core)
    battery = world_cell(core, facing, (0, 0, -2))
    stop = world_cell(core, facing, (1, 0, 2))
    game.evidence.update(core=core, facing=facing, battery_at=battery, redstone_at=stop)
    game.command(f'setblock {battery[0]} {battery[1]} {battery[2]} ntm:machine_battery[facing=south]')
    open_near(game, x, y, z)
    game.put('ntm:canister_full__kerosene', 3, 0)
    game.snapshot('combustion-turbofan')
    game.close()
    before, after = grows(game, battery, 4)
    game.evidence['burning'] = [before, after]
    game.check('burns_without_ignition', after > before + 10_000, [before, after])
    game.command(f'setblock {stop[0]} {stop[1]} {stop[2]} minecraft:redstone_block')
    time.sleep(3)
    held_a, held_b = grows(game, battery, 4)
    game.evidence['stopped'] = [held_a, held_b]
    game.check('redstone_stops_it', held_b == held_a, [held_a, held_b])
    game.command(f'setblock {stop[0]} {stop[1]} {stop[2]} minecraft:air')
    again_a, again_b = grows(game, battery, 4)
    game.evidence['again'] = [again_a, again_b]
    game.check('runs_again', again_b > again_a + 10_000, [again_a, again_b])


def turbina(game):
    game.evidence['phase'] = 'turbina'
    x, y, z = 330, 100, 8
    start_area(game, x - 4, y, z, 16)
    game.command('give @s ntm:machine_turbinegas 1')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(2)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_turbinegas')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_turbinegas', x, y, z, reach=(6, 3))
    facing = facing_of(game, core)
    cell = lambda c: world_cell(core, facing, c)
    # The power port is at the west end a layer up; the three fluids come in at
    # the middle's sides, each by a duct from a barrel of the full containers.
    battery = cell((-5, 1, 0))
    feeds = [('gas', 'ntm:fluid_barrel_full__gas', (-1, 0, 2), (-1, 0, 3)),
             ('lubricant', 'ntm:fluid_barrel_full__lubricant', (-1, 0, -2), (-1, 0, -3)),
             ('water', 'ntm:fluid_barrel_full__water', (4, 0, 2), (4, 0, 3))]
    game.evidence.update(core=core, facing=facing, battery_at=battery)
    game.command(f'setblock {battery[0]} {battery[1]} {battery[2]} ntm:machine_battery[facing=south]')
    for name, container, duct, barrel in feeds:
        d, b = cell(duct), cell(barrel)
        game.command(f'setblock {d[0]} {d[1]} {d[2]} ntm:fluid_duct_neo')
        game.command(f'setblock {b[0]} {b[1]} {b[2]} ntm:barrel_steel[facing=south]')
        side = cell((duct[0] + (2 if duct[0] > 0 else -2), 0, duct[2]))
        type_duct(game, name, [d], side)
        fill_barrel(game, b, 1, container)
    open_near(game, x, y, z)
    machine = screen_json('machine_turbinegas')
    start = next(b for b in machine['buttons'] if b['id'] == 0)
    slider = next(b for b in machine['buttons'] if b['id'] == 160)
    game.snapshot('combustion-turbina-off')
    game.button(start['rect'], machine['size'])
    # 580 ticks to start (29 s), then the slider at the top.
    time.sleep(36)
    game.button(slider['rect'], machine['size'])
    game.snapshot('combustion-turbina-run')
    game.close()
    time.sleep(1)
    state = machine_state(game, core)
    game.evidence['state'] = state[:600]
    game.check('running', '"state":1' in state, state[:300])
    before, after = grows(game, battery, 25)
    game.evidence['battery'] = [before, after]
    steam = first_int(machine_state(game, core), r'"steam":\{"fluid":"hotsteam","mb":(\d+)')
    game.evidence['dense_steam_mb'] = steam
    game.check('battery_fills', after > before + 5_000, [before, after])
    game.check('boils_water', steam is not None and steam > 0, steam)


PHASES = {'diesel': diesel, 'motor': motor, 'turbofan': turbofan, 'turbina': turbina}

if __name__ == '__main__':
    game = Game(sys.argv[1], 'combustion')
    phases = sys.argv[2:] or list(PHASES)
    unknown = set(phases) - set(PHASES)
    if unknown:
        raise SystemExit(f'unknown phases: {sorted(unknown)}')
    game.ctl('wait-ready 120')
    for phase in phases:
        PHASES[phase](game)
    game.evidence['ok'] = True
    game.save()

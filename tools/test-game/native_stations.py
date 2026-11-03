#!/usr/bin/env python3
"""Native acceptance of the recipe-list machines: chemical plant, mixer, PUREX.

Usage: python3 tools/test-game/native_stations.py GAME [planta|misturador|purex ...]
Each phase places the machine with a buffer battery (filled by a creative
battery in a hopper) beside it, picks the recipe on the native list
(`native_recipes.pick`), feeds it and checks what it makes.
  planta      Dinamite: sugar, niter and sand in, 2 dynamite out (item only);
  misturador  Refrigerante: 2 water buckets in a barrel, a typed duct, niter: 2000 mB coolant;
  purex       Haste de Plutônio-239: a rod, sulfuric acid by barrel and duct: 2 Pu-239 billets and 1 uranium;
  m3_fluidos  Oxigênio: 8 air tanks in a barrel, a typed duct, a chemical plant: 500 mB oxygen;
  m3_centrifuga  2 UF6 cells in a barrel, a duct, a gas centrifuge with a creative battery in its slot 4: a U-238 nugget.
"""
import sys
import time

from native_game import Game
from native_recipes import core_of, count, pick


def power(game, x, y, z, beside=2):
    """A battery `beside` blocks east of a machine's core (2 for a 3x3 one, 1 for a single
    column), filled by a creative battery in a hopper and set to give by a redstone block."""
    game.command(f'setblock {x + beside} {y} {z} ntm:machine_battery[facing=south]')
    game.battery_output(x + beside, y, z)
    game.command(f'setblock {x + beside} {y + 1} {z} minecraft:hopper[facing=down]')
    game.command(f'item replace block {x + beside} {y + 1} {z} container.0 with ntm:battery_creative 1')


def open_machine(game, x, y, z):
    """Open a player-placed multiblock from the cell the player can see: the
    placing point's column, from the bottom up."""
    last = None
    for height in range(0, 3):
        try:
            return game.open(x, y, z, dummy_height=height)
        except RuntimeError as error:
            last = error
    raise last


def type_duct(game, fluid, positions, standing):
    """Give the ducts their fluid with the identifier in hand, as a player does."""
    game.command('clear @s')
    game.command(f'give @s ntm:fluid_identifier_multi[minecraft:custom_data={{ntm_fluid:"{fluid}"}}] 1')
    time.sleep(1)
    game.ctl('select ntm:fluid_identifier_multi')
    game.command(f'tp @s {standing[0] + .5} {standing[1]} {standing[2] + .5}')
    time.sleep(1)
    for px, py, pz in positions:
        game.ctl(f'interact {px} {py} {pz}')
        time.sleep(.5)


def fill_barrel(game, barrel, buckets, item='minecraft:water_bucket'):
    """Full containers (`item`) into a barrel through its screen, then its mode to output
    (button 0 twice: input, buffer, output). The screen's origin is the first
    hotbar slot less its place in the machine file."""
    for _ in range(buckets):
        game.command(f'give @s {item} 1')
        time.sleep(1)
        game.ctl(f'interact {barrel[0]} {barrel[1]} {barrel[2]}')
        time.sleep(1.5)
        screen = game.ctl('screen')
        hot = next(s['slot'] for s in screen['slots'] if s.get('item') == item and s['slot'] < 36)
        game.ctl(f'click-slot {hot}')
        game.ctl('click-slot 302')
        time.sleep(1)
        game.ctl('close')
    game.ctl(f'interact {barrel[0]} {barrel[1]} {barrel[2]}')
    time.sleep(1.5)
    slot0 = next(w for w in game.ctl('ui')['widgets'] if w.get('id') == 'slot-0')['bounds']
    ox, oy = slot0[0] - 8, slot0[1] - (84 + 58)
    for _ in range(2):
        game.ctl(f'click-at {ox + 151 + 9} {oy + 35 + 9}')
        time.sleep(.8)
    game.ctl('close')


def tank_mb(game, core, fluid):
    """The milli-buckets of `fluid` in the machine's block entity state."""
    import json
    import re
    text = json.dumps(game.command(f'data get block {core[0]} {core[1]} {core[2]} ntm')).replace('\\"', '"')
    return sum(int(m) for m in re.findall(r'"mb":(\d+),"fluid":"' + fluid + '"', text)) + sum(
        int(m) for m in re.findall(r'"fluid":"' + fluid + r'"[^}]*?"mb":(\d+)', text))


def misturador(game):
    game.evidence['phase'] = 'misturador'
    x, y, z = 140, 100, 8
    game.close()
    game.ground(x - 3, y, z, width=8)
    game.command('clear @s')
    # barrel | duct | mixer; the creative battery goes in the mixer's own slot.
    game.command(f'setblock {x - 2} {y} {z} ntm:barrel_steel[facing=south]')
    game.command(f'setblock {x - 1} {y} {z} ntm:fluid_duct_neo')
    game.command('give @s ntm:machine_mixer 1')
    game.ctl(f'place {x} {y} {z} --item ntm:machine_mixer')
    game.command('clear @s')
    type_duct(game, 'water', [(x - 1, y, z)], (x - 1, y, z + 3))
    fill_barrel(game, (x - 2, y, z), 2)
    open_machine(game, x, y, z)
    core = core_of(game, 'ntm:machine_mixer', x, y, z)
    game.evidence['core'] = core
    chosen = pick(game, 'machine_mixer', 'refrigerante', core, 30)
    game.evidence['recipe_index_plus_one'] = chosen
    game.put('ntm:battery_creative', 1, 0)
    game.put('ntm:niter', 1, 1)
    deadline = time.monotonic() + 120
    made = 0
    while time.monotonic() < deadline:
        made = tank_mb(game, core, 'coolant')
        if made >= 2000:
            break
        time.sleep(2)
    game.snapshot('stations-misturador')
    game.evidence['coolant_mb'] = made
    game.check('coolant_made', made >= 2000, made)
    game.close()


def purex(game):
    game.evidence['phase'] = 'purex'
    x, y, z = 166, 100, 9
    game.close()
    game.ground(x - 8, y, z, width=18)
    game.command('clear @s')
    game.command('give @s ntm:machine_purex 1')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(1)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_purex')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_purex', x, y, z)
    game.evidence['core'] = core
    cx, cy, cz = core
    # barrel | duct | the 5x5x5 footprint (core +- 2)
    game.command(f'setblock {cx - 3} {cy} {cz} ntm:fluid_duct_neo')
    game.command(f'setblock {cx - 4} {cy} {cz} ntm:barrel_steel[facing=south]')
    type_duct(game, 'sulfuric_acid', [(cx - 3, cy, cz)], (cx - 3, cy, cz + 3))
    fill_barrel(game, (cx - 4, cy, cz), 1, 'ntm:fluid_tank_full__sulfuric_acid')
    game.command(f'tp @s {x + .5} {y} {z + 6.5}')
    open_machine(game, x, y, z)
    chosen = pick(game, 'machine_purex', 'plutônio-239', core, 30)
    game.evidence['recipe_index_plus_one'] = chosen
    game.put('ntm:battery_creative', 1, 0)
    game.put('ntm:pile_rod__pu239', 1, 4)
    deadline = time.monotonic() + 120
    slots = []
    while time.monotonic() < deadline:
        slots = game.slots()
        if count(slots, 'ntm:billet_pu239') >= 2:
            break
        time.sleep(2)
    game.snapshot('stations-purex')
    game.check('two_pu239_billets', count(slots, 'ntm:billet_pu239') == 2, [s for s in slots if s.get('item')])
    game.check('one_uranium_billet', count(slots, 'ntm:billet_uranium') == 1, [s for s in slots if s.get('item')])
    game.close()


def m3_fluidos(game):
    game.evidence['phase'] = 'm3_fluidos'
    x, y, z = 190, 100, 9
    game.close()
    game.ground(x - 6, y, z, width=14)
    game.command('clear @s')
    game.command('give @s ntm:machine_chemical_plant 1')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(1)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_chemical_plant')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_chemical_plant', x, y, z)
    cx, cy, cz = core
    power(game, cx, cy, cz)
    game.command(f'setblock {cx - 3} {cy} {cz} ntm:barrel_steel[facing=south]')
    game.command(f'setblock {cx - 2} {cy} {cz} ntm:fluid_duct_neo')
    type_duct(game, 'air', [(cx - 2, cy, cz)], (cx - 2, cy, cz + 3))
    fill_barrel(game, (cx - 3, cy, cz), 8, 'ntm:fluid_tank_full__air')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    open_machine(game, x, y, z)
    chosen = pick(game, 'machine_chemical_plant', 'oxigênio', core, 30)
    game.evidence['recipe_index_plus_one'] = chosen
    deadline = time.monotonic() + 150
    made = 0
    while time.monotonic() < deadline:
        made = tank_mb(game, core, 'oxygen')
        if made >= 500:
            break
        time.sleep(2)
    game.snapshot('stations-m3-fluidos')
    game.evidence['oxygen_mb'] = made
    game.check('oxygen_made', made >= 500, made)
    game.close()


def m3_centrifuga(game):
    game.evidence['phase'] = 'm3_centrifuga'
    x, y, z = 210, 100, 9
    game.close()
    game.ground(x - 6, y, z, width=10)
    game.command('clear @s')
    game.command(f'setblock {x - 2} {y} {z} ntm:barrel_steel[facing=south]')
    game.command(f'setblock {x - 1} {y} {z} ntm:fluid_duct_neo')
    game.command('give @s ntm:machine_gascent 1')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(1)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_gascent')
    game.command('clear @s')
    type_duct(game, 'uf6', [(x - 1, y, z)], (x - 1, y, z + 3))
    fill_barrel(game, (x - 2, y, z), 2, 'ntm:cell_uf6')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    open_machine(game, x, y, z)
    game.put('ntm:battery_creative', 1, 4)
    slots = game.poll(lambda s: count(s, 'ntm:nugget_u238') >= 1, timeout=150)
    game.snapshot('stations-m3-centrifuga')
    for name, at in (('barrel', (x - 2, y, z)), ('gascent', (x, y, z)), ('duct', (x - 1, y, z))):
        game.evidence[name + '_data'] = game.command(f'data get block {at[0]} {at[1]} {at[2]}').get('chat')
    game.check('u238_nugget', count(slots, 'ntm:nugget_u238') >= 1, [s for s in slots if s.get('item')])
    game.close()


def planta(game):
    game.evidence['phase'] = 'planta'
    x, y, z = 120, 100, 8
    game.close()
    game.ground(x, y, z, width=6)
    game.command('clear @s')
    power(game, x, y, z)
    game.command('give @s ntm:machine_chemical_plant 1')
    game.ctl(f'place {x} {y} {z} --item ntm:machine_chemical_plant')
    game.command('clear @s')
    open_machine(game, x, y, z)
    core = core_of(game, 'ntm:machine_chemical_plant', x, y, z)
    chosen = pick(game, 'machine_chemical_plant', 'dinamite', core, 30)
    game.evidence['recipe_index_plus_one'] = chosen
    game.put('minecraft:sugar', 1, 0)
    game.put('ntm:niter', 1, 1)
    game.put('minecraft:sand', 1, 2)
    slots = game.poll(lambda s: count(s, 'ntm:ball_dynamite') >= 2, timeout=120)
    game.snapshot('stations-planta')
    game.check('two_dynamite', count(slots, 'ntm:ball_dynamite') == 2, [s for s in slots if s.get('item')])
    game.close()


PHASES = {'planta': planta, 'misturador': misturador, 'purex': purex,
          'm3_fluidos': m3_fluidos, 'm3_centrifuga': m3_centrifuga}

if __name__ == '__main__':
    game = Game(sys.argv[1], 'stations')
    phases = sys.argv[2:] or list(PHASES)
    unknown = set(phases) - set(PHASES)
    if unknown:
        raise SystemExit(f'unknown phases: {sorted(unknown)}')
    game.ctl('wait-ready 120')
    for phase in phases:
        PHASES[phase](game)
    game.evidence['ok'] = True
    game.save()

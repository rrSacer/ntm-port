#!/usr/bin/env python3
"""Native recipe list (L2) in game: open it, search, choose, close, and the machine uses it.

Usage: python3 tools/test-game/native_recipes.py GAME [montadora|bigorna ...]
Run the game from build-native.sh with NTM content and GAME/control.sock; DISPLAY
is the game's Xvfb (the montadora phase types with real X11 key events, the
bigorna with `horizon-ctl type`).

The list is NTM's GUIScreenRecipeSelector (176x132, 8x5 grid, search of 32,
Enter toggles its focus, Escape/E/Close return to the machine and send the
choice only when it changed). Phases:
  montadora  open the list from the button, Enter, type "ferro", pick the
             first match, Escape; the data slot holds the recipe + 1; 4 iron
             ingots and a buffer battery (filled from a creative one) make 4 iron plates;
  bigorna    a tier 3 anvil: "aço" (typed by ctl), pick the steel plate, the
             close button; 20 steel ingots and "Construir uma vez" give 1 plate,
             Shift gives the other 19.
"""
import json
import math
import os
import re
import sys
import time
from pathlib import Path

from native_game import Game
from xinput import XInput

ROOT = Path(__file__).resolve().parents[2]
SELECTOR = (176, 132)
# GUIScreenRecipeSelector: the grid starts at (7, 17); cells are 18 square.
FIRST_CELL = (7 + 9, 17 + 9)
CLOSE = (152 + 8, 90 + 8)


def screen_json(name):
    return json.loads((ROOT / f'content/assets/ntm/horizon_screen/{name}.json').read_text())


def origin(game, size):
    width, height = game.ctl('ui')['gui_size']
    return math.trunc((width - size[0]) / 2), math.trunc((height - size[1]) / 2)


def click(game, size, point):
    ox, oy = origin(game, size)
    return game.ctl(f'click-at {ox + point[0]} {oy + point[1]}')


def count(slots, item):
    return sum(s.get('count', 0) for s in slots if s.get('item') == item)


def core_of(game, name, x, y, z):
    """Where a player-placed multiblock's core is: placing at (x, y, z) leaves a
    dummy there, the block entity is in the core cell near it."""
    for dy in range(0, 4):
        for dx in range(-2, 3):
            for dz in range(-2, 3):
                found = game.ctl(f'block {x + dx} {y + dy} {z + dz}').get('state', '')
                if found.startswith(name + '[') or found == name:
                    return x + dx, y + dy, z + dz
    raise AssertionError(f'no {name} near {x} {y} {z}')


def recipe_data(game, x, y, z, index):
    """The menu data slot `index` of the machine's block entity, from `data get`."""
    result = game.command(f'data get block {x} {y} {z}')
    text = json.dumps(result)
    found = re.search(r'data: ?\[I; ?([-0-9, ]+)\]', text.replace('\\"', '"'))
    if not found:
        raise AssertionError(f'no data array in {text[:400]}')
    values = [int(v) for v in found.group(1).split(',')]
    return values[index] if index < len(values) else None


def pick(game, screen_name, query, core, data_slot, label_check=None):
    """Open the machine's recipe list from its button 0, search `query` (ctl type,
    Enter focuses), choose the first match and close with the Close button.
    Returns the data slot's value (recipe + 1) once the server has it."""
    machine = screen_json(screen_name)
    opener = next(b for b in machine['buttons'] if b['id'] == 0)
    ox, oy = origin(game, machine['size'])
    rx, ry, rw, rh = opener['rect']
    game.ctl(f'click-at {ox + rx + rw / 2} {oy + ry + rh / 2}')
    time.sleep(.5)
    game.ctl('key enter tap')
    game.ctl(f'type {query}')
    time.sleep(.5)
    game.snapshot(f'recipes-{screen_name}-search')
    click(game, SELECTOR, FIRST_CELL)
    time.sleep(.3)
    click(game, SELECTOR, CLOSE)
    time.sleep(1)
    chosen = recipe_data(game, *core, data_slot)
    game.check(f'{screen_name}_recipe_sent', chosen is not None and chosen > 0, chosen)
    return chosen


def montadora(game, keys):
    game.evidence['phase'] = 'montadora'
    x, y, z = 80, 100, 8
    game.close()
    game.ground(x, y, z, width=6)
    game.command('clear @s')
    game.command('give @s ntm:machine_assembly_machine 1')
    game.ctl(f'place {x} {y} {z} --item ntm:machine_assembly_machine')
    # Power: a buffer battery beside the machine, filled by a creative
    # battery in a hopper above it (as native_redes.py does).
    game.command(f'setblock {x + 2} {y} {z} ntm:machine_battery[facing=south]')
    game.command(f'setblock {x + 2} {y + 1} {z} minecraft:hopper[facing=down]')
    game.command(f'item replace block {x + 2} {y + 1} {z} container.0 with ntm:battery_creative 1')
    game.open(x, y, z, dummy_height=1)
    machine = screen_json('machine_assembly_machine')
    opener = next(b for b in machine['buttons'] if b['id'] == 0)
    core = core_of(game, 'ntm:machine_assembly_machine', x, y, z)
    game.evidence['core'] = core
    game.snapshot('recipes-montadora-machine')
    before = game.ctl('screen')
    game.check('machine_has_inventory', any(s['slot'] < 36 for s in before.get('slots', [])), before)
    # Open the list: the inventory and the machine's slots go away.
    ox, oy = origin(game, machine['size'])
    rx, ry, rw, rh = opener['rect']
    game.ctl(f'click-at {ox + rx + rw / 2} {oy + ry + rh / 2}')
    time.sleep(.5)
    listed = game.ctl('screen')
    game.snapshot('recipes-montadora-list')
    game.check('list_hides_slots', not listed.get('slots'), listed)
    # Real keys: Enter focuses the search, "ferro" filters, a click picks the first match.
    keys.focus()
    keys.tap('Return')
    keys.text('ferro')
    time.sleep(.5)
    game.snapshot('recipes-montadora-search')
    click(game, SELECTOR, FIRST_CELL)
    time.sleep(.3)
    game.snapshot('recipes-montadora-chosen')
    # Nothing is sent until the list closes.
    unsent = recipe_data(game, *core, 30)
    game.check('nothing_sent_before_close', unsent in (0, None), unsent)
    keys.tap('Escape')
    time.sleep(1)
    back = game.ctl('screen')
    game.snapshot('recipes-montadora-back')
    game.check('machine_returns', any(s['slot'] < 36 for s in back.get('slots', [])) and any(s['slot'] >= 300 for s in back.get('slots', [])), back)
    chosen = recipe_data(game, *core, 30)
    game.check('recipe_sent_on_close', chosen is not None and chosen > 0, chosen)
    game.evidence['recipe_index_plus_one'] = chosen
    # The machine uses it: 4 ingots in its first input slot, 4 plates.
    game.put('minecraft:iron_ingot', 4, 4)
    slots = game.poll(lambda s: count(s, 'ntm:plate_iron') >= 4, timeout=120)
    game.snapshot('recipes-montadora-done')
    game.check('four_plates', count(slots, 'ntm:plate_iron') >= 4, slots)
    game.close()


def bigorna(game):
    game.evidence['phase'] = 'bigorna'
    x, y, z = 100, 100, 8
    game.close()
    game.ground(x, y, z, width=5)
    game.command('clear @s')
    game.command('give @s ntm:anvil_desh 1')
    game.ctl(f'place {x} {y} {z} --item ntm:anvil_desh')
    game.command('clear @s')
    game.open(x, y, z)
    machine = screen_json('anvil_iron')
    opener = next(b for b in machine['buttons'] if b['id'] == 0)
    ox, oy = origin(game, machine['size'])
    rx, ry, rw, rh = opener['rect']
    game.ctl(f'click-at {ox + rx + rw / 2} {oy + ry + rh / 2}')
    time.sleep(.5)
    game.snapshot('recipes-bigorna-list')
    game.ctl('key enter tap')
    game.ctl('type aço')
    time.sleep(.5)
    game.snapshot('recipes-bigorna-search')
    click(game, SELECTOR, FIRST_CELL)
    time.sleep(.3)
    click(game, SELECTOR, CLOSE)
    time.sleep(1)
    chosen = recipe_data(game, x, y, z, 1)
    game.check('recipe_sent_on_close', chosen is not None and chosen > 0, chosen)
    game.evidence['recipe_index_plus_one'] = chosen
    game.put('ntm:ingot_steel', 20, 3)
    build = next(b for b in machine['buttons'] if b['id'] == 1)
    game.button(build['rect'], machine['size'])
    time.sleep(1.5)
    once = game.slots()
    plates = count(once, 'ntm:plate_steel')
    game.check('one_plate', plates == 1, once)
    game.ctl('key shift down')
    game.button(build['rect'], machine['size'])
    game.ctl('key shift up')
    time.sleep(1.5)
    full = game.slots()
    game.snapshot('recipes-bigorna-built')
    game.check('twenty_plates', count(full, 'ntm:plate_steel') == 20, full)
    game.close()


if __name__ == '__main__':
    game = Game(sys.argv[1], 'recipes')
    phases = sys.argv[2:] or ['montadora', 'bigorna']
    unknown = set(phases) - {'montadora', 'bigorna'}
    if unknown:
        raise SystemExit(f'unknown phases: {sorted(unknown)}')
    game.ctl('wait-ready 120')
    keys = XInput(os.environ['DISPLAY']) if 'montadora' in phases else None
    for phase in phases:
        montadora(game, keys) if phase == 'montadora' else bigorna(game)
    game.evidence['ok'] = True
    game.save()

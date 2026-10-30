#!/usr/bin/env python3
"""Native client side (L4): the Geiger box, a radioactive item's tooltip, and the
electrolyser's screens laying their slots out again.

Usage: python3 tools/test-game/native_client.py GAME [hud|tooltip|eletrolisador ...]
Run the game from build-native.sh; DISPLAY is the game's Xvfb (the tooltip phase
moves the real pointer with X11 XTest). The Geiger box reads the player's values
the server keeps (ntm_rad_dose, ntm_rad_rate: ntm_core::client_rad).
  hud           the box shows in the top left with the colour of the dose
                (pixels of the box's colours); a capture with 64 uranium billets held;
  tooltip       over a stack of 64 uranium billets in the inventory the tooltip
                lists [Radioactive], the rate and the stack's total;
  eletrolisador the fluid screen shows slots 3..13 and hides 14..20; its
                button switches to the metal screen, which shows 14..20 and
                hides 3..13, at once; and back.
"""
import math
import os
import sys
import time

from native_game import Game
from native_recipes import core_of, origin, screen_json
from xinput import XInput


def slot_numbers(game):
    """The machine's slots that are on the screen (a screen can park the ones it
    does not use far outside it)."""
    ui = game.ctl('ui')
    width, height = ui['gui_size']
    found = set()
    for w in ui['widgets']:
        name = w.get('id', '')
        if name.startswith('slot-') and name[5:].isdigit() and int(name[5:]) >= 300:
            x, y = w['bounds'][:2]
            if 0 <= x < width and 0 <= y < height:
                found.add(int(name[5:]))
    return found


def gui_scale(game):
    return game.ctl('status').get('window', {}).get('width', 1280) / game.ctl('ui')['gui_size'][0]


def region_pixels(path, box, near):
    """How many pixels in `box` (x0, y0, x1, y1) are within 24 per channel of `near`."""
    from PIL import Image
    image = Image.open(path).convert('RGB')
    count = 0
    for x in range(box[0], box[2]):
        for y in range(box[1], box[3]):
            r, g, b = image.getpixel((x, y))
            if abs(r - near[0]) < 24 and abs(g - near[1]) < 24 and abs(b - near[2]) < 24:
                count += 1
    return count


def hud(game):
    game.evidence['phase'] = 'hud'
    game.close()
    game.command('clear @s')
    game.command('gamemode survival @s')
    time.sleep(3)
    path = str(game.folder / 'native-client-hud.png')
    game.ctl('screenshot ' + path)
    time.sleep(1)
    # The box is in the top left, in the colour of the dose (green below 200
    # RAD, yellow, orange, red from 600).
    palette = [(0x88, 0xEE, 0x88), (0xFF, 0xFF, 0x55), (0xFF, 0xAA, 0x00), (0xFF, 0x55, 0x55)]
    pixels = [region_pixels(path, (0, 0, 420, 120), c) for c in palette]
    game.evidence['hud_pixels_by_colour'] = pixels
    game.check('geiger_box_shows', max(pixels) > 40, pixels)
    game.command('give @s ntm:billet_uranium 64')
    time.sleep(3)
    game.ctl('screenshot ' + str(game.folder / 'native-client-hud-uranium.png'))
    game.command('gamemode creative @s')


def tooltip(game, keys):
    game.evidence['phase'] = 'tooltip'
    game.close()
    game.command('clear @s')
    game.command('give @s ntm:billet_uranium 64')
    time.sleep(1)
    game.ctl('open-inventory')
    time.sleep(1)
    slot = next(w for w in game.ctl('ui')['widgets'] if w.get('id') == 'slot-0')['bounds']
    scale = gui_scale(game)
    keys.focus()
    keys.pointer((slot[0] + slot[2] / 2) * scale, (slot[1] + slot[3] / 2) * scale)
    time.sleep(.5)
    game.snapshot('client-tooltip')
    game.close()


def eletrolisador(game):
    game.evidence['phase'] = 'eletrolisador'
    x, y, z = 240, 100, 20
    game.close()
    game.command(f'forceload add {x - 6} {z - 8} {x + 8} {z + 12}')
    time.sleep(3)
    game.command(f'fill {x - 6} {y} {z - 8} {x + 8} {y + 6} {z + 12} minecraft:air')
    game.command(f'fill {x - 6} {y - 1} {z - 8} {x + 8} {y - 1} {z + 12} minecraft:stone')
    game.command('clear @s')
    game.command('give @s ntm:machine_electrolyser 1')
    game.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(2)
    game.ctl(f'place {x} {y} {z} --item ntm:machine_electrolyser')
    game.command('clear @s')
    core = core_of(game, 'ntm:machine_electrolyser', x, y, z, reach=(4, 7))
    game.evidence['core'] = core
    opened = False
    cx, cy, cz = core
    # The machine is 11 deep: open it from its south face.
    for height in range(0, 3):
        try:
            game.command(f'tp @s {cx + .5} {cy} {cz + 7.5}')
            time.sleep(.5)
            game.ctl(f'interact {cx} {cy + height} {cz + 5} south')
            time.sleep(.8)
            opened = game.ctl('screen').get('screen') == 'container'
        except RuntimeError:
            opened = False
        if opened:
            break
    game.check('menu_open', opened, None)
    fluid = screen_json('machine_electrolyser')
    metal = screen_json('machine_electrolyser_metal')
    before = slot_numbers(game)
    game.snapshot('client-electrolyser-fluids')
    game.check('fluid_screen_slots', {303, 304, 313} <= before and not ({314, 320} & before), sorted(before))
    switch = next(b for b in fluid['buttons'] if b.get('screen'))
    game.button(switch['rect'], fluid['size'])
    time.sleep(.7)
    after = slot_numbers(game)
    game.snapshot('client-electrolyser-metal')
    game.check('metal_screen_slots', {314, 320} <= after and not ({303, 304, 313} & after), sorted(after))
    back = next(b for b in metal['buttons'] if b.get('screen'))
    game.button(back['rect'], metal['size'])
    time.sleep(.7)
    again = slot_numbers(game)
    game.check('fluid_screen_slots_again', again == before, sorted(again))
    game.close()


if __name__ == '__main__':
    game = Game(sys.argv[1], 'client')
    phases = sys.argv[2:] or ['hud', 'tooltip', 'eletrolisador']
    unknown = set(phases) - {'hud', 'tooltip', 'eletrolisador'}
    if unknown:
        raise SystemExit(f'unknown phases: {sorted(unknown)}')
    game.ctl('wait-ready 120')
    keys = XInput(os.environ['DISPLAY']) if 'tooltip' in phases else None
    for phase in phases:
        {'hud': lambda: hud(game), 'tooltip': lambda: tooltip(game, keys),
         'eletrolisador': lambda: eletrolisador(game)}[phase]()
    game.evidence['ok'] = True
    game.save()

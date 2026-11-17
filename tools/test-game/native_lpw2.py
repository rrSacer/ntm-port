#!/usr/bin/env python3
"""The LPW2 (docs/onda-2-sublote-2.md, group 2b), decorative in the 1.7.10 itself, on a disposable native world.

setup          placed, it fills its frame (MachineLPW2 {6,0,3,3,9,10}, offset 3: x -9..10, y 0..6, z -3..3 in the placer's
               frame): its corners and middle are its parts; a click on a part opens nothing (no onBlockActivated);
capture-state  the world is saved;
check-state    after reopening, the frame is whole; a player breaking a far corner takes the whole of it and drops one LPW2.
"""
import re
import sys
import time
from native_game import Game
from native_combustion import facing_of, world_cell

X, Y, Z = 60, 100, 20
MIN, MAX = (-9, 0, -3), (10, 6, 3)
SAVED = 'lpw2-saved.json'


def is_(g, p, block):
    return 'passed' in ' '.join(g.command(f'execute if block {p[0]} {p[1]} {p[2]} {block}').get('chat', [])).lower()


def find_core(g):
    for dx in range(-4, 5):
        for dz in range(-4, 5):
            at = (X + dx, Y, Z + dz)
            if is_(g, at, 'ntm:machine_lpw2'):
                return at
    raise AssertionError('no LPW2 core near the placement')


def corners(core, facing):
    cells = [(x, y, z) for x in (MIN[0], MAX[0]) for y in (MIN[1], MAX[1]) for z in (MIN[2], MAX[2])]
    cells += [(0, 3, 0), (5, 0, 0), (-5, 6, 2)]
    return [world_cell(core, facing, c) for c in cells]


def whole(g, core, facing):
    return all(is_(g, c, 'ntm:dummy_machine') for c in corners(core, facing)) and is_(g, core, 'ntm:machine_lpw2')


def place(g):
    g.close()
    g.ground(X - 14, Y, Z - 14, width=28, height=9, depth=28)
    g.command('clear @s')
    g.command('give @s ntm:machine_lpw2 1')
    g.command(f'tp @s {X + .5} {Y} {Z + 3.5} facing {X + .5} {Y} {Z}')
    time.sleep(1)
    g.ctl(f'place {X} {Y} {Z} --item ntm:machine_lpw2')
    time.sleep(3)
    g.command('clear @s')


def setup(g):
    place(g)
    core = find_core(g)
    # Its core has no block entity of its own (no machine logic): the facing is the block state's.
    state = g.ctl(f'block {core[0]} {core[1]} {core[2]}').get('state', '')
    found = re.search(r'facing=(\w+)', state)
    facing = found[1] if found else facing_of(g, core)
    g.evidence.update(core=list(core), facing=facing)
    g.check('it_fills_its_frame', whole(g, core, facing), [core, facing])
    part = world_cell(core, facing, (5, 0, 0))
    g.command(f'tp @s {part[0] + .5} {Y} {part[2] + 6.5}')
    time.sleep(1)
    try:
        g.ctl(f'interact {part[0]} {part[1]} {part[2]} south')
    except RuntimeError as e:
        g.evidence['interact'] = str(e)
    time.sleep(1.5)
    screen = g.ctl('screen').get('screen')
    g.check('a_click_opens_nothing', screen != 'container', screen)
    g.close()


def capture(g):
    import json
    core = tuple(json.loads((g.folder / 'native-lpw2-setup.json').read_text())['core'])
    facing = json.loads((g.folder / 'native-lpw2-setup.json').read_text())['facing']
    g.check('it_is_whole_before_the_save', whole(g, core, facing), core)
    g.ctl('save')


def check(g):
    import json
    setup_ev = json.loads((g.folder / 'native-lpw2-setup.json').read_text())
    core, facing = tuple(setup_ev['core']), setup_ev['facing']
    g.check('it_is_whole_after_reopening', whole(g, core, facing), core)
    g.command('kill @e[type=minecraft:item]')
    g.command('gamemode survival @s')
    corner = world_cell(core, facing, (MAX[0], 0, MAX[2]))
    g.command(f'tp @s {corner[0] + 2.5} {Y} {corner[2] + 2.5} facing {corner[0] + .5} {Y + .5} {corner[2] + .5}')
    g.command('give @s minecraft:diamond_pickaxe 1')
    time.sleep(1)
    g.ctl('select minecraft:diamond_pickaxe')
    g.evidence['break'] = g.ctl(f'break {corner[0]} {corner[1]} {corner[2]} --timeout 90')
    time.sleep(3)
    left = [c for c in corners(core, facing) + [core] if not is_(g, c, 'minecraft:air')]
    text = ' '.join(g.command('execute if entity @e[type=minecraft:item,nbt={Item:{id:"ntm:machine_lpw2"}}]').get('chat', []))
    count = re.search(r'count: (\d+)', text)
    g.check('breaking_a_corner_takes_the_whole_of_it', not left, left)
    g.check('and_drops_one_lpw2', 'passed' in text.lower() and (count is None or count[1] == '1'), text)
    g.command('gamemode creative @s')


if __name__ == '__main__':
    g = Game(sys.argv[1], 'lpw2-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

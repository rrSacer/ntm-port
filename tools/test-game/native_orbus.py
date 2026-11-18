#!/usr/bin/env python3
"""The orbus (docs/onda-2-sublote-2.md) on a disposable native world.

setup  the orbus by its item makes the 80-cell multiblock of `MachineOrbus.getDimensions {4,0,2,1,2,1}` (79 dummies and the
       core, a 512 000 mB store); its screen is the barrel's: three water buckets in, and in the default input mode two
       typed ducts on its eight ports' cells (one at the bottom, one at the top) take nothing from it; in output mode the water
       leaves by both ducts to two barrels and the orbus empties (conservation across the three tanks);
capture-state / check-state  the same with the world saved in the middle: the tank, its fluid and its mode come back.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state, world_cell, facing_of
from native_recipes import core_of
from native_stations import type_duct

X, Z = 40, 8
SAVED = 'orbus-saved.json'


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def tank(g, at):
    """(fluid, mb, cap, mode) of the store at `at`, from its saved state."""
    text = machine_state(g, at)
    found = re.search(r'"tank":\{([^{}]*)\}', text)
    if not found:
        return None
    fluid = re.search(r'"fluid":"(\w*)"', found[1])
    mb = re.search(r'"mb":(\d+)', found[1])
    cap = re.search(r'"cap":(\d+)', found[1])
    mode = re.search(r'"mode":"(\w+)"', text)
    return (fluid[1] if fluid else '', int(mb[1]) if mb else 0, int(cap[1]) if cap else 0, mode[1] if mode else '')


def dummies(g, core):
    """The cells of the multiblock: the dummy blocks in the box that can hold it."""
    cells = []
    for dy in range(0, 5):
        for dx in range(-3, 4):
            for dz in range(-3, 4):
                state = g.ctl(f'block {core[0] + dx} {core[1] + dy} {core[2] + dz}').get('state', '')
                if state.startswith('ntm:dummy_machine'):
                    cells.append((core[0] + dx, core[1] + dy, core[2] + dz))
    return cells


def open_through(g, cells):
    """The screen, through the first of `cells` a player can click from the south."""
    g.close()
    for cell in cells:
        g.command(f'tp @s {cell[0] + .5} {max(cell[1], 100)} {cell[2] + 3.5}')
        time.sleep(1.2)
        try:
            g.ctl(f'interact {cell[0]} {cell[1]} {cell[2]} south')
        except RuntimeError:
            continue
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if g.ctl('screen').get('screen') == 'container':
                return cell
            time.sleep(.4)
        g.close()
    raise AssertionError('no cell opened the screen')


def pour(g, cells, buckets):
    for _ in range(buckets):
        g.command('give @s minecraft:water_bucket 1')
        time.sleep(.8)
        open_through(g, cells)
        screen = g.ctl('screen')
        hot = next(s['slot'] for s in screen['slots'] if s.get('item') == 'minecraft:water_bucket' and s['slot'] < 36)
        g.ctl(f'click-slot {hot}')
        g.ctl('click-slot 302')
        time.sleep(1)
        g.close()
        g.command('clear @s minecraft:bucket')


def set_output(g, cells):
    """The mode button twice: input, buffer, output (the barrel's; button 0)."""
    open_through(g, cells)
    slot0 = next(w for w in g.ctl('ui')['widgets'] if w.get('id') == 'slot-0')['bounds']
    ox, oy = slot0[0] - 8, slot0[1] - (84 + 58)
    for _ in range(2):
        g.ctl(f'click-at {ox + 151 + 9} {oy + 35 + 9}')
        time.sleep(.8)
    g.close()


def build(g):
    g.close()
    g.ground(X, 100, Z, width=14, depth=10)
    g.command('fill %d 100 %d %d 112 %d minecraft:air' % (X - 4, Z - 4, X + 10, Z + 10))
    g.command('clear @s')
    g.command('give @s ntm:machine_orbus 1')
    g.command(f'tp @s {X + .5} 100 {Z + 3.5}')
    time.sleep(1)
    g.ctl(f'place {X} 100 {Z} --item ntm:machine_orbus')
    time.sleep(1.5)
    core = core_of(g, 'ntm:machine_orbus', X, 100, Z, reach=(4, 4))
    facing = facing_of(g, core)
    g.evidence.update(core=list(core), facing=facing)
    return core, facing


def lay_out(g, core, facing):
    """A typed duct under a bottom port and one over a top port, each to a barrel. The ports (NTM tuples 0..1, -1..0 and 4)
    are at (-x, -z) of the core: (-1, 0, 1) at the bottom and (-1, 4, 0) at the top, their free faces down and up."""
    cell = lambda x, y, z: world_cell(core, facing, (x, y, z))
    low_duct, low_barrel = cell(-1, -1, 1), cell(-2, -1, 1)
    high_duct, high_barrel = cell(-1, 5, 0), cell(-2, 5, 0)
    for p in (cell(-2, -2, 1), cell(-1, -2, 1), cell(0, -2, 1), cell(-1, 5, -1)):
        g.command(f'setblock {p[0]} {p[1]} {p[2]} minecraft:stone')
    for p in (low_duct, high_duct):
        g.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:fluid_duct_neo')
    for p in (low_barrel, high_barrel):
        g.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:barrel_steel[facing=north]')
    # Typed from beside them: the bottom one from the cell under the core's column, the top one from a stone over the box (its barrel beside the duct, so the duct's top face is in the player's view).
    air = cell(0, -1, 1)
    g.command(f'setblock {air[0]} {air[1]} {air[2]} minecraft:air')
    type_duct(g, 'water', [low_duct], air)
    high_stand = cell(-1, 6, -1)
    type_duct(g, 'water', [high_duct], high_stand)
    g.command('clear @s')
    return low_barrel, high_barrel


def setup(g):
    core, facing = build(g)
    state = machine_state(g, core)
    g.check('the_orbus_is_a_store_of_512000', tank(g, core) is not None and tank(g, core)[2] == 512_000, state[:300])
    cells = dummies(g, core)
    g.check('the_multiblock_has_79_dummies_and_a_core', len(cells) == 79, len(cells))
    g.evidence['dummies'] = len(cells)
    top = [c for c in cells if c[1] == core[1] + 4]
    pour(g, cells, 3)
    held = tank(g, core)
    g.check('three_buckets_poured', held[0] == 'water' and held[1] == 3000, held)
    low, high = lay_out(g, core, facing)
    sprint(g, 80)
    still = (tank(g, core), tank(g, low), tank(g, high))
    g.check('input_mode_gives_nothing_through_the_ports', still[0][1] == 3000 and still[1][1] == 0 and still[2][1] == 0, still)
    set_output(g, cells)
    sprint(g, 200)
    after = (tank(g, core), tank(g, low), tank(g, high))
    total = after[0][1] + after[1][1] + after[2][1]
    g.check('output_mode_empties_through_both_ports', after[0][1] == 0 and after[1][1] > 0 and after[2][1] > 0, after)
    g.check('conserved_across_the_three_tanks', total == 3000, after)
    g.evidence['after'] = after


def capture(g):
    core, facing = build(g)
    cells = dummies(g, core)
    pour(g, cells, 2)
    g.check('two_buckets_in_before_the_save', tank(g, core)[1] == 2000, tank(g, core))
    set_output(g, cells)
    (g.folder / SAVED).write_text(json.dumps({'core': list(core), 'tank': tank(g, core)}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    core = tuple(saved['core'])
    now = tank(g, core)
    g.check('the_tank_fluid_and_mode_came_back', now == tuple(saved['tank']), [saved['tank'], now])


if __name__ == '__main__':
    g = Game(sys.argv[1], 'orbus-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

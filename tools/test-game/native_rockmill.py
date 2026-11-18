#!/usr/bin/env python3
"""The rock mill (docs/onda-2-sublote-2.md, group 2b) on a disposable native world.

setup          placed, it fills its 5x5x3 frame; water comes by a barrel and a typed duct to one of its eight port cells;
               the recipe list (button 0) picks "Cascalho" (rock.cobble), a creative battery in slot 0, 4 cobblestone in
               slot 2: it crushes one into gravel or quartz (ChanceOutputMulti, by weight) and 250 mB colloid, then
               stops: the output slot must be empty to pick again (`canFitOutput`);
capture-state  the world is saved with it stopped;
check-state    after reopening, the same slots, recipe and colloid; the output taken, it crushes the next one.
"""
import json
import sys
import time
from native_game import Game
from native_recipes import core_of, count, pick, recipe_data
from native_stations import fill_barrel, tank_mb, type_duct

X, Y, Z = 60, 100, 34
POOL = ('minecraft:gravel', 'ntm:powder_quartz')
SAVED = 'native-rockmill-setup.json'


def is_(g, p, block):
    return 'passed' in ' '.join(g.command(f'execute if block {p[0]} {p[1]} {p[2]} {block}').get('chat', [])).lower()


def output(slots):
    return [s for s in slots if s['slot'] == 305 and s.get('item')]


def settled(g, core):
    """Its slots, colloid and water once ten more seconds (two recipes' time) went by."""
    time.sleep(10)
    slots = g.slots()
    return slots, tank_mb(g, core, 'colloid'), tank_mb(g, core, 'water')


def setup(g):
    g.close()
    g.ground(X - 8, Y, Z - 3, width=16, depth=10)
    g.command('clear @s')
    g.command('give @s ntm:machine_rockmill 1')
    g.command(f'tp @s {X + .5} {Y} {Z + 3.5} facing {X + .5} {Y} {Z}')
    time.sleep(1)
    g.ctl(f'place {X} {Y} {Z} --item ntm:machine_rockmill')
    time.sleep(2)
    g.command('clear @s')
    core = core_of(g, 'ntm:machine_rockmill', X, Y, Z, reach=(3, 3))
    cx, cy, cz = core
    g.evidence['core'] = list(core)
    frame = [(cx + dx, cy + dy, cz + dz) for dx in (-2, 2) for dy in (0, 2) for dz in (-2, 2)]
    g.check('it_fills_its_frame', all(is_(g, c, 'ntm:dummy_machine') for c in frame), frame)
    # barrel | duct | the port cell (cx - 2, cz + 1), whose face opens west.
    g.command(f'setblock {cx - 3} {cy} {cz + 1} ntm:fluid_duct_neo')
    g.command(f'setblock {cx - 4} {cy} {cz + 1} ntm:barrel_steel[facing=south]')
    type_duct(g, 'water', [(cx - 3, cy, cz + 1)], (cx - 3, cy, cz + 4))
    fill_barrel(g, (cx - 4, cy, cz + 1), 2)
    g.open(cx, cy, cz + 2)
    chosen = pick(g, 'machine_rockmill', 'moer cobblestone', core, 30)
    g.evidence['recipe_index_plus_one'] = chosen
    g.check('the_list_picks_rock_cobble', chosen == 1, chosen)
    g.put('ntm:battery_creative', 1, 0)
    g.put('minecraft:cobblestone', 4, 2)
    g.poll(lambda s: output(s), timeout=60)
    slots, colloid, water = settled(g, core)
    g.snapshot('rockmill-crushed')
    out = output(slots)
    g.evidence.update(output=out, colloid=colloid, water=water)
    g.check('one_crushed_into_gravel_or_quartz', len(out) == 1 and out[0]['item'] in POOL and out[0].get('count') == 1, out)
    g.check('and_250_mb_colloid', colloid == 250, colloid)
    g.check('then_it_waits_for_the_output_slot', count(slots, 'minecraft:cobblestone') == 3, [s for s in slots if s.get('item')])
    g.close()


def capture(g):
    core = tuple(json.loads((g.folder / SAVED).read_text())['core'])
    g.open(core[0], core[1], core[2] + 2)
    slots = g.slots()
    g.check('it_is_stopped_before_the_save', count(slots, 'minecraft:cobblestone') == 3 and len(output(slots)) == 1, [s for s in slots if s.get('item')])
    g.close()
    g.ctl('save')


def check(g):
    before = json.loads((g.folder / SAVED).read_text())
    core = tuple(before['core'])
    g.open(core[0], core[1], core[2] + 2)
    slots, colloid, _ = settled(g, core)
    out = output(slots)
    g.check('after_reopening_the_same_slots', count(slots, 'minecraft:cobblestone') == 3 and out == before['output'], [s for s in slots if s.get('item')])
    g.check('the_same_recipe', recipe_data(g, *core, 30) == 1, None)
    g.check('and_the_same_colloid', colloid == 250, colloid)
    g.ctl('click-slot 305 shift')
    time.sleep(1)
    slots = g.poll(lambda s: count(s, 'minecraft:cobblestone') == 2 and output(s), timeout=30)
    g.snapshot('rockmill-reopened')
    g.check('the_output_taken_it_crushes_the_next', count(slots, 'minecraft:cobblestone') == 2 and len(output(slots)) == 1, [s for s in slots if s.get('item')])
    g.check('and_more_colloid', tank_mb(g, core, 'colloid') == 500, tank_mb(g, core, 'colloid'))
    g.close()


if __name__ == '__main__':
    g = Game(sys.argv[1], 'rockmill-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

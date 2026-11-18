#!/usr/bin/env python3
"""The automatic crafting table (docs/onda-2-sublote-2.md, group 2b) on a disposable native world.

setup          placed, a creative battery in slot 20; two oak planks clicked into the template's slots 1 and 4 stay on
               the cursor and leave copies (`SlotPattern`); the result shows the sticks the world's recipes make of them
               (the engine's crafting query); each copy has its `initPatternSmart` mode (wildcard for planks) and a right
               click turns slot 1's to the next (`plankWood`); 2 planks in the inputs 11 and 14 craft twice: 8 sticks in
               the output, the inputs empty; a right click on the result with one recipe stays on it;
capture-state  the world is saved;
check-state    after reopening, the copies, their modes, the result and the 8 sticks are there; one more plank in each
               input crafts 4 more: the recipes are asked again after the load.
"""
import json
import re
import sys
import time
from native_game import Game

X, Y, Z = 60, 100, 46
AT = (X, Y, Z)
MODE = 'ntm_autocrafter_mode'


def slots(g):
    return {s['slot'] - 300: s for s in g.slots() if s.get('item')}


def template(g, item, slot):
    """`item` held and clicked onto template slot `slot`: the copy goes in, the cursor keeps the stack."""
    g.command(f'give @s {item} 3')
    time.sleep(1)
    hot = next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot'] < 36 and s.get('item') == item)
    g.ctl(f'click-slot {hot}')
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.5)
    carried = (g.ctl('screen').get('carried') or {}).get('count')
    g.ctl(f'click-slot {hot}')
    g.command(f'clear @s {item}')
    return carried


def modes(g):
    """The template copies' modes, from the block entity's items (each copy's custom data), sorted."""
    text = json.dumps(g.command(f'data get block {X} {Y} {Z} Items')).replace('\\"', '"')
    return sorted(re.findall(MODE + r'\W*([A-Za-z]+)', text))


def count(s, i, item):
    return s[i]['count'] if i in s and s[i]['item'] == item else 0


def setup(g):
    g.close()
    g.ground(X - 3, Y, Z - 3, width=8, depth=8)
    g.command('clear @s')
    g.command('give @s ntm:machine_autocrafter 1')
    g.command(f'tp @s {X + .5} {Y} {Z + 3.5} facing {X + .5} {Y} {Z}')
    time.sleep(1)
    g.ctl(f'place {X} {Y} {Z} --item ntm:machine_autocrafter')
    time.sleep(1.5)
    g.command('clear @s')
    g.open(X, Y, Z)
    g.put('ntm:battery_creative', 1, 20)
    held = [template(g, 'minecraft:oak_planks', i) for i in (1, 4)]
    time.sleep(1)
    s = slots(g)
    g.evidence['template'] = {i: s.get(i) for i in (1, 4, 9)}
    g.check('a_click_leaves_a_copy_and_the_cursor_whole', held == [3, 3] and count(s, 1, 'minecraft:oak_planks') == 1
            and count(s, 4, 'minecraft:oak_planks') == 1, [held, s.get(1), s.get(4)])
    g.check('the_result_is_the_worlds_recipe', count(s, 9, 'minecraft:stick') == 4, s.get(9))
    m = modes(g)
    g.evidence['modes'] = m
    g.check('each_copy_has_its_smart_mode', m == ['wildcard', 'wildcard'], m)
    g.ctl('click-slot 301 right')
    time.sleep(1)
    m = modes(g)
    g.evidence['modes_turned'] = m
    g.check('a_right_click_turns_the_mode', m == ['plankWood', 'wildcard'], m)
    g.put('minecraft:oak_planks', 2, 11)
    g.put('minecraft:oak_planks', 2, 14)
    s = g.poll(lambda s: any(x['slot'] == 319 and x.get('count') == 8 for x in s), timeout=30)
    s = {x['slot'] - 300: x for x in s if x.get('item')}
    g.evidence['crafted'] = {i: s.get(i) for i in (11, 14, 19)}
    g.check('it_crafts_one_a_tick_into_the_output', count(s, 19, 'minecraft:stick') == 8 and 11 not in s and 14 not in s,
            [s.get(11), s.get(14), s.get(19)])
    g.ctl('click-slot 309 right')
    time.sleep(1)
    s = slots(g)
    g.check('one_recipe_turns_back_to_itself', count(s, 9, 'minecraft:stick') == 4, s.get(9))
    g.snapshot('autocrafter')
    g.close()


def capture(g):
    g.open(X, Y, Z)
    s = slots(g)
    g.check('it_is_as_crafted_before_the_save', count(s, 19, 'minecraft:stick') == 8 and count(s, 9, 'minecraft:stick') == 4, s)
    g.close()
    g.ctl('save')


def check(g):
    g.open(X, Y, Z)
    time.sleep(1)
    s = slots(g)
    m = modes(g)
    g.evidence['reopened'] = {'slots': {i: s.get(i) for i in (1, 4, 9, 19, 20)}, 'modes': m}
    g.check('after_reopening_the_template_its_modes_the_result_and_the_output', count(s, 1, 'minecraft:oak_planks') == 1
            and count(s, 4, 'minecraft:oak_planks') == 1 and m == ['plankWood', 'wildcard']
            and count(s, 9, 'minecraft:stick') == 4 and count(s, 19, 'minecraft:stick') == 8, [s, m])
    g.put('minecraft:oak_planks', 1, 11)
    g.put('minecraft:oak_planks', 1, 14)
    s = g.poll(lambda s: any(x['slot'] == 319 and x.get('count') == 12 for x in s), timeout=30)
    s = {x['slot'] - 300: x for x in s if x.get('item')}
    g.check('and_it_crafts_on', count(s, 19, 'minecraft:stick') == 12 and 11 not in s and 14 not in s, [s.get(11), s.get(14), s.get(19)])
    g.snapshot('autocrafter-reopened')
    g.close()


if __name__ == '__main__':
    g = Game(sys.argv[1], 'autocrafter-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

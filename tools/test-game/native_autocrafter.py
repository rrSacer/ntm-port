#!/usr/bin/env python3
"""The automatic crafting table (docs/onda-2-sublote-2.md, group 2b) on a disposable native world.

setup          machine A, a creative battery in slot 20:
               - two oak planks clicked into the template's slots 1 and 4 stay on the cursor and leave copies
                 (`SlotPattern`); the result shows the sticks of the world's recipes (the engine's crafting query);
               - each copy has its `initPatternSmart` mode (exact: planks have several metas in 1.7.10); a right click
                 turns slot 1's to wildcard; the same planks clicked onto slot 1 again give it exact again;
               - a hand follows the inputs' rule (`SlotNonRetarded`): 16 planks stay on the cursor, 4 go in;
               - with 4 planks in each of the inputs 11 and 14, one craft a tick: 4, 8, 12, 16 sticks, a tick apart;
               - a shift-click on a template slot clears it and puts no copy in the inventory;
               - honey bottles in four template slots and one in each input make a honey block and leave four glass
                 bottles where the bottles ran out (`getContainerItem`);
               - a uranium ingot alone matches two recipes: the result's right click turns 1/2 to 2/2;
               machine B, a hopper above it with four birch planks and a template of oak planks: exact refuses them,
               wildcard takes them and B crafts sticks of them;
capture-state  the world is saved;
check-state    after reopening, A's template (the ingot), its mode, the second recipe and the honey block are there; A
               broken by hand drops itself, the honey block, the bottles and the battery, and none of the template's
               copies nor the result's.
"""
import json
import re
import sys
import time
from native_game import Game

X, Y, Z = 60, 100, 46
AT = (X, Y, Z)
B = (X + 4, Y, Z)
MODE = 'ntm_autocrafter_mode'


def slots(g):
    return {s['slot'] - 300: s for s in g.slots() if s.get('item')}


def hold(g, item, n):
    """`n` of `item` in the hotbar and picked up on the cursor; returns the hotbar slot."""
    g.command(f'clear @s {item}')
    g.command(f'give @s {item} {n}')
    time.sleep(1)
    hot = next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot'] < 36 and s.get('item') == item)
    g.ctl(f'click-slot {hot}')
    return hot


def carried(g):
    return (g.ctl('screen').get('carried') or {}).get('count')


def template(g, item, slot):
    """`item` held and clicked onto template slot `slot`: the copy goes in, the cursor keeps the stack."""
    hot = hold(g, item, 3)
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.5)
    left = carried(g)
    g.ctl(f'click-slot {hot}')
    g.command(f'clear @s {item}')
    return left


def clear_template(g, slot):
    """An empty cursor clicked onto a template slot clears it."""
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.3)


def modes(g, at=AT):
    """The template copies' modes, from the block entity's items (each copy's custom data), sorted."""
    text = json.dumps(g.command(f'data get block {at[0]} {at[1]} {at[2]} Items')).replace('\\"', '"')
    return sorted(re.findall(MODE + r'\W*([A-Za-z]+)', text))


def data(g, at=AT):
    """The machine's menu data: power, its limit, the recipe's place + 1, the recipes' count."""
    text = json.dumps(g.command(f'data get block {at[0]} {at[1]} {at[2]}')).replace('\\"', '"')
    found = re.search(r'data: ?\[I; ?([-0-9, ]+)\]', text)
    return [int(v) for v in found[1].split(',')] if found else []


def items(g, at=AT):
    """slot -> (id, count) of the block entity's items."""
    import native_funnel as F
    return F.items(g, at)


def count(s, i, item):
    return s[i]['count'] if i in s and s[i]['item'] == item else 0


def place(g, at):
    g.command('give @s ntm:machine_autocrafter 1')
    g.command(f'tp @s {at[0] + .5} {at[1]} {at[2] + 3.5} facing {at[0] + .5} {at[1]} {at[2]}')
    time.sleep(1)
    g.ctl(f'place {at[0]} {at[1]} {at[2]} --item ntm:machine_autocrafter')
    time.sleep(1.5)
    g.command('clear @s')


def setup(g):
    g.close()
    g.ground(X - 3, Y, Z - 3, width=12, depth=8)
    g.command('clear @s')
    place(g, AT)
    g.open(*AT)
    g.put('ntm:battery_creative', 1, 20)
    held = [template(g, 'minecraft:oak_planks', i) for i in (1, 4)]
    time.sleep(1)
    s = slots(g)
    g.check('a_click_leaves_a_copy_and_the_cursor_whole', held == [3, 3] and count(s, 1, 'minecraft:oak_planks') == 1
            and count(s, 4, 'minecraft:oak_planks') == 1, [held, s.get(1), s.get(4)])
    g.check('the_result_is_the_worlds_recipe', count(s, 9, 'minecraft:stick') == 4, s.get(9))
    m = modes(g)
    g.check('each_copy_has_its_smart_mode', m == ['exact', 'exact'], m)
    g.ctl('click-slot 301 right')
    time.sleep(1)
    turned = modes(g)
    template(g, 'minecraft:oak_planks', 1)
    time.sleep(1)
    again = modes(g)
    g.check('a_right_click_turns_the_mode_and_the_same_item_again_resets_it', turned == ['exact', 'wildcard'] and again == ['exact', 'exact'],
            [turned, again])
    # A hand follows the inputs' rule: a stack of 16 stays on the cursor.
    hot = hold(g, 'minecraft:oak_planks', 16)
    g.ctl('click-slot 311')
    time.sleep(.5)
    refused = carried(g) == 16 and 11 not in slots(g)
    g.ctl(f'click-slot {hot}')
    g.command('clear @s minecraft:oak_planks')
    g.check('a_hand_cannot_put_16_into_an_input', refused, slots(g).get(11))
    # One craft a tick, the server's ticks stepped one by one.
    g.command('tick freeze')
    try:
        g.put('minecraft:oak_planks', 4, 11)
        g.put('minecraft:oak_planks', 4, 14)
        out = []
        for _ in range(4):
            g.command('tick step 1')
            time.sleep(1)
            out.append(items(g).get(19, ('', 0))[1])
    finally:
        g.command('tick unfreeze')
    g.evidence['one_a_tick'] = out
    g.check('one_craft_a_tick', out == [4, 8, 12, 16], out)
    g.ctl('click-slot 319 shift')
    # Honey bottles: the template in 0, 1, 3, 4 and one bottle in each input below; the bottles stay, empty.
    clear_template(g, 1)
    # A shift-click on a template slot is its own click (`slotClick` mode 1, an empty hand): it clears it, and no copy
    # reaches the inventory (the mods' shift-click is not asked).
    g.ctl('click-slot 304 shift')
    time.sleep(.5)
    inventory = [s for s in g.ctl('screen')['slots'] if s['slot'] < 36 and s.get('item') == 'minecraft:oak_planks']
    g.check('a_shift_click_clears_a_template_and_moves_no_copy', 4 not in slots(g) and not inventory, [slots(g).get(4), inventory])
    for i in (0, 1, 3, 4):
        template(g, 'minecraft:honey_bottle', i)
    for i in (10, 11, 13, 14):
        g.put('minecraft:honey_bottle', 1, i)
    s = g.poll(lambda s: any(x['slot'] == 319 and x.get('item') == 'minecraft:honey_block' for x in s), timeout=20)
    s = {x['slot'] - 300: x for x in s if x.get('item')}
    g.evidence['honey'] = {i: s.get(i) for i in (10, 11, 13, 14, 19)}
    g.check('a_container_stays_where_its_stack_ran_out', count(s, 19, 'minecraft:honey_block') == 1
            and all(count(s, i, 'minecraft:glass_bottle') == 1 for i in (10, 11, 13, 14)), g.evidence['honey'])
    for i in (10, 11, 13, 14):
        g.ctl(f'click-slot {300 + i} shift')
    # A uranium ingot alone: two recipes (the catalog's and the bulk one), the result's right click turns them.
    for i in (0, 1, 3, 4):
        clear_template(g, i)
    template(g, 'ntm:ingot_uranium', 0)
    time.sleep(1)
    first = data(g)[2:4]
    g.ctl('click-slot 309 right')
    time.sleep(1)
    second = data(g)[2:4]
    g.check('two_recipes_turn_on_the_result', first == [1, 2] and second == [2, 2] and count(slots(g), 9, 'ntm:nugget_uranium') == 9,
            [first, second])
    g.snapshot('autocrafter')
    g.close()
    hopper(g)


def hopper(g):
    """Machine B fed by a hopper above it: the template's modes decide what the hopper may put in."""
    place(g, B)
    g.command(f'setblock {B[0]} {B[1] + 1} {B[2]} minecraft:hopper[facing=down]')
    g.open(*B)
    g.put('ntm:battery_creative', 1, 20)
    for i in (1, 4):
        template(g, 'minecraft:oak_planks', i)
    g.close()
    g.command(f'item replace block {B[0]} {B[1] + 1} {B[2]} container.0 with minecraft:birch_planks 4')
    time.sleep(4)
    held = items(g, (B[0], B[1] + 1, B[2])).get(0, ('', 0))[1]
    g.check('exact_keeps_birch_planks_in_the_hopper', held == 4 and not any(10 <= k <= 18 for k in items(g, B)), [held, items(g, B)])
    g.open(*B)
    g.ctl('click-slot 301 right')
    g.ctl('click-slot 304 right')
    time.sleep(1)
    g.evidence['b_modes'] = modes(g, B)
    g.close()
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and items(g, B).get(19, ('', 0))[1] < 8:
        time.sleep(1)
    b = items(g, B)
    g.check('wildcard_lets_the_hopper_in_and_b_crafts', g.evidence['b_modes'] == ['wildcard', 'wildcard'] and b.get(19) == ('minecraft:stick', 8)
            and not items(g, (B[0], B[1] + 1, B[2])), [g.evidence['b_modes'], b])


def capture(g):
    g.open(*AT)
    s = slots(g)
    g.check('it_is_as_left_before_the_save', count(s, 0, 'ntm:ingot_uranium') == 1 and count(s, 19, 'minecraft:honey_block') == 1, s)
    g.close()
    g.ctl('save')


def check(g):
    g.open(*AT)
    time.sleep(1)
    s = slots(g)
    m = modes(g)
    d = data(g)
    g.evidence['reopened'] = {'slots': {i: s.get(i) for i in (0, 9, 19, 20)}, 'modes': m, 'data': d}
    g.check('after_reopening_the_template_its_mode_the_second_recipe_and_the_output', count(s, 0, 'ntm:ingot_uranium') == 1
            and len(m) == 1 and d[2:4] == [2, 2] and count(s, 9, 'ntm:nugget_uranium') == 9 and count(s, 19, 'minecraft:honey_block') == 1,
            g.evidence['reopened'])
    g.close()
    # Broken by hand: the block, its real items, none of the copies.
    real = items(g)
    g.evidence['before_break'] = real
    g.command('kill @e[type=minecraft:item]')
    g.command('gamemode survival @s')
    g.command(f'tp @s {X + .5} {Y} {Z + 2.5} facing {X + .5} {Y + .5} {Z + .5}')
    g.command('give @s minecraft:diamond_pickaxe 1')
    time.sleep(1)
    g.ctl('select minecraft:diamond_pickaxe')
    brk = g.ctl(f'break {X} {Y} {Z} --timeout 60')
    g.evidence['break'] = brk
    time.sleep(3)
    # What the break dropped: on the ground or already picked up by the player standing by (`drops` sums both).
    ground = {i: g.dropped(i) for i in ('ntm:machine_autocrafter', 'minecraft:honey_block', 'ntm:battery_creative', 'ntm:ingot_uranium',
                                        'ntm:nugget_uranium', 'minecraft:oak_planks', 'minecraft:stick')}
    dropped = brk.get('drops')
    g.evidence['dropped'] = {'drops': dropped, 'picked_up': brk.get('picked_up'), 'ground': ground}
    g.check('breaking_it_drops_its_items_and_no_copy', dropped == {'ntm:machine_autocrafter': 1, 'minecraft:honey_block': 1,
                                                                 'ntm:battery_creative': 1}
            and not any(ground[i] for i in ('ntm:ingot_uranium', 'ntm:nugget_uranium', 'minecraft:oak_planks', 'minecraft:stick')),
            [g.evidence['dropped'], real])
    g.command('gamemode creative @s')


if __name__ == '__main__':
    g = Game(sys.argv[1], 'autocrafter-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

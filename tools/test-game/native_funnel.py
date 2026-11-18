#!/usr/bin/env python3
"""The funnel (docs/onda-2-sublote-2.md) on a disposable native world.

setup          gold ingots and glowstone dust on top turn into a block and glowstone below (9 and 4 at a time, the rest stays);
               the button cycles 3x3 then 2x2 -> 3x3 only -> 2x2 only -> back, each mode changing what compresses; a hopper over
               it puts in what a recipe takes and the dirt that merges into an occupied slot; a hopper under it takes the results out;
capture-state  the world is saved in the 2x2-only mode with gold that would compress in the others;
check-state    after reopening the mode is still 2x2 only (the gold stays) and four glowstone dust still compress.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game
from native_recipes import screen_json

X, Z = 40, 8
FUNNEL = (X, 100, Z)
SAVED = 'funnel-saved.json'


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def items(g, p=None):
    """slot -> (id, count) from the block entity's items."""
    p = p or FUNNEL
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    out, depth, start, entries = {}, 0, None, []
    for i, c in enumerate(text):
        if c == '{':
            if depth == 1:
                start = i
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 1 and start is not None:
                entries.append(text[start:i + 1])
                start = None
        elif c == '[' and depth == 0:
            depth = 1
    for entry in entries:
        slot = re.search(r'[sS]lot: ?(\d+)', entry)
        item = re.search(r'(?:^|[,{] ?)id: ?"([\w:]+)"(?=[,}])', entry)
        count = re.search(r'[cC]ount: ?(\d+)', entry)
        if slot and item:
            out[int(slot[1])] = (item[1], int(count[1]) if count else 1)
    return out


def open_funnel(g):
    g.close()
    g.command(f'tp @s {X + .5} 100 {Z + 3.5}')
    time.sleep(1)
    g.ctl(f'interact {X} 100 {Z} south')
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if g.ctl('screen').get('screen') == 'container':
            return
        time.sleep(.4)
    raise AssertionError('the funnel did not open')


def press_mode(g, times=1):
    screen = screen_json('machine_funnel')
    button = next(b for b in screen['buttons'] if b['id'] == 0)
    for _ in range(times):
        g.button(button['rect'], screen['size'])
        time.sleep(.5)


def build(g):
    g.close()
    g.ground(X, 100, Z, width=14)
    g.command('clear @s')
    g.command('give @s ntm:machine_funnel 1')
    g.ctl(f'place {X} 100 {Z} --item ntm:machine_funnel')
    time.sleep(1.5)
    g.command('clear @s')


def setup(g):
    build(g)
    open_funnel(g)
    g.put('minecraft:gold_ingot', 20, 4)
    g.put('minecraft:glowstone_dust', 5, 7)
    g.put('minecraft:dirt', 1, 1)
    g.close()
    sprint(g, 20)
    s = items(g)
    g.check('nine_gold_ingots_make_a_block_at_a_time', s.get(13) == ('minecraft:gold_block', 2) and s.get(4) == ('minecraft:gold_ingot', 2), s)
    g.check('four_glowstone_dust_make_glowstone_and_one_stays', s.get(16) == ('minecraft:glowstone', 1) and s.get(7) == ('minecraft:glowstone_dust', 1), s)
    g.check('what_no_recipe_takes_stays', s.get(1) == ('minecraft:dirt', 1), s)
    # Mode 3x3 only: four dust are not touched.
    open_funnel(g)
    press_mode(g)
    g.put('minecraft:glowstone_dust', 3, 7)
    g.close()
    sprint(g, 20)
    s = items(g)
    g.check('mode_3x3_only_leaves_four_dust', s.get(7) == ('minecraft:glowstone_dust', 4) and s.get(16) == ('minecraft:glowstone', 1), s)
    # Mode 2x2 only: the dust goes, nine gold ingots do not.
    open_funnel(g)
    press_mode(g)
    g.put('minecraft:gold_ingot', 9, 4)
    g.close()
    sprint(g, 20)
    s = items(g)
    g.check('mode_2x2_only_takes_the_dust_and_not_the_gold', s.get(16) == ('minecraft:glowstone', 2) and s.get(4) == ('minecraft:gold_ingot', 11), s)
    # Back to the default: the gold goes.
    open_funnel(g)
    press_mode(g)
    g.close()
    sprint(g, 20)
    s = items(g)
    g.check('the_third_press_is_the_default_again', s.get(13) == ('minecraft:gold_block', 3) and s.get(4) == ('minecraft:gold_ingot', 2), s)
    # The automation: a hopper over it puts in what it takes, one under it takes the results.
    g.command(f'setblock {X} 99 {Z} minecraft:hopper[facing=down]')
    g.command(f'setblock {X} 98 {Z} minecraft:chest')
    g.command(f'setblock {X} 101 {Z} minecraft:hopper[facing=down]')
    g.command(f'setblock {X} 102 {Z} minecraft:chest')
    g.command(f'item replace block {X} 102 {Z} container.0 with minecraft:iron_ingot 9')
    g.command(f'item replace block {X} 102 {Z} container.1 with minecraft:dirt 4')
    g.command(f'item replace block {X} 102 {Z} container.2 with minecraft:stick 3')
    sprint(g, 400)
    s = items(g)
    low = ' '.join(g.command(f'data get block {X} 98 {Z} Items').get('chat', []))
    hopper = ' '.join(g.command(f'data get block {X} 101 {Z} Items').get('chat', []))
    g.evidence['containers'] = {str(y): ' '.join(g.command(f'data get block {X} {y} {Z} Items').get('chat', []))[:500] for y in (98, 99, 100, 101, 102)}
    g.check('a_hopper_over_it_puts_in_the_dirt_that_merges_and_the_iron', s.get(1, ('', 0)) == ('minecraft:dirt', 5) and 'minecraft:iron_block' in low, [s, low[:300]])
    # The hopper over it pulled the whole chest; what it could not put in (the sticks) stays in the hopper.
    g.check('no_stick_goes_into_an_empty_slot', 'minecraft:stick' in hopper and not any(v[0] == 'minecraft:stick' for v in s.values()), [s, hopper[:200]])
    g.check('a_hopper_under_it_takes_the_results_out', 'minecraft:gold_block' in low and s.get(4, ('', 0))[0] != 'minecraft:gold_block', [s, low[:300]])


def capture(g):
    build(g)
    open_funnel(g)
    press_mode(g, 2)
    g.put('minecraft:gold_ingot', 11, 2)
    g.put('minecraft:glowstone_dust', 3, 5)
    g.close()
    sprint(g, 20)
    state = items(g)
    g.check('mode_2x2_only_before_the_save', state.get(2) == ('minecraft:gold_ingot', 11) and state.get(5) == ('minecraft:glowstone_dust', 3), state)
    (g.folder / SAVED).write_text(json.dumps({str(k): v for k, v in state.items()}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    sprint(g, 10)
    now = items(g)
    g.check('the_items_are_as_saved_and_the_mode_held', now.get(2) == tuple(saved['2']) and now.get(5) == tuple(saved['5']) and not any(k > 8 for k in now), [saved, now])
    open_funnel(g)
    g.put('minecraft:glowstone_dust', 1, 5)
    g.close()
    sprint(g, 10)
    after = items(g)
    g.check('four_dust_still_compress', after.get(14) == ('minecraft:glowstone', 1) and 5 not in after, after)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'funnel-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

#!/usr/bin/env python3
"""BrokenItem acceptance (docs/onda-2-sublote-1.md, "BrokenItem") on a disposable native world.

setup          two products' broken items are given with their `custom_data`
               (`{ntm_broken:{id:"..."}}`), kept by the player's inventory and put by hand into the two
               slots of a machine; a second broken item of the other product does not pile on the
               first one's stack;
capture-state  the world is saved;
check-state    after reopening, the machine's slots hold the same items with the same data.
The precision assembler's own run (the faulty product and the recycling recipe) is a later
delivery: this proves the item and its data, not the machine.
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state

X, Z = 40, 8
HOST = (X, 100, Z)
A = 'ntm:upgrade_speed_2'
B = 'ntm:upgrade_speed_1'
SAVED = 'broken-saved.json'


def tag(product):
    return f'{{ntm_broken:{{id:"{product}"}}}}'


def items(g, p):
    """The block entity's slots, by slot: [id, count, product named by the stack's custom_data or None]."""
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    out = {}
    # An entry nests (components, custom_data, ntm_broken): split at each `Slot:` instead of matching braces.
    for entry in re.split(r'\{\s*[sS]lot:', text)[1:]:
        slot = re.match(r'\s*(\d+)', entry)
        ident = re.search(r'id: ?"?([\w:]+)"?', entry)
        count = re.search(r'[cC]ount: ?(\d+)', entry)
        product = re.search(r'ntm_broken: ?\{ ?id: ?"([\w:]+)"', entry)
        if slot and ident and count:
            out[int(slot[1])] = [ident[1], int(count[1]), product[1] if product else None]
    g.evidence['raw_items'] = text[:600]
    return out


def give(g, product, n):
    g.command(f'give @s ntm:broken_item[minecraft:custom_data={tag(product)}] {n}')
    time.sleep(.5)


def setup(g):
    x, y, z = HOST
    g.close()
    g.ground(x, 100, z, width=14)
    g.command('clear @s')
    g.command('give @s ntm:machine_furnace_brick_off 1')
    g.ctl(f'place {x} {y} {z} --item ntm:machine_furnace_brick_off')
    time.sleep(1)
    g.command('clear @s')
    give(g, A, 2)
    give(g, B, 1)
    inv = ' '.join(g.command('data get entity @s Inventory').get('chat', []))
    g.check('player_holds_both_products', f'"{A}"' in inv and f'"{B}"' in inv and inv.count('ntm:broken_item') == 2, inv)
    g.close()
    g.command(f'tp @s {x + .5} 100 {z + 3.5}')
    time.sleep(1)
    g.ctl(f'interact {x} {y} {z} south')
    time.sleep(.7)
    screen = g.ctl('screen')
    g.check('menu_open', screen.get('screen') == 'container', screen)
    for item, slot in (('ntm:broken_item', 0), ('ntm:broken_item', 1)):
        s = g.ctl('screen')
        src = next(e['slot'] for e in s['slots'] if e['slot'] < 36 and e.get('item') == item)
        g.ctl(f'click-slot {src}')
        g.ctl(f'click-slot {300 + slot}')
        time.sleep(.4)
    g.close()
    held = items(g, HOST)
    # whichever the player held first went to slot 0; the data came with it
    g.check('slots_hold_both_products', sorted(v[2] or '' for v in held.values()) == sorted([A, B]) and all(v[0] == 'ntm:broken_item' for v in held.values()), held)
    counts = {v[2]: v[1] for v in held.values()}
    g.check('counts_kept', counts.get(A) == 2 and counts.get(B) == 1, held)
    # a second broken item of the other product, by shift-click: it must not pile on the first stack
    give(g, B, 1)
    g.ctl(f'interact {x} {y} {z} south')
    time.sleep(.7)
    s = g.ctl('screen')
    src = next(e['slot'] for e in s['slots'] if e['slot'] < 36 and e.get('item') == 'ntm:broken_item')
    g.ctl(f'click-slot {src} shift')
    time.sleep(.4)
    g.close()
    after = items(g, HOST)
    a_slots = [v for v in after.values() if v[2] == A]
    g.check('other_product_never_piles_on_the_first', len(a_slots) == 1 and a_slots[0][1] == 2, after)
    g.evidence['held'] = held
    g.evidence['after'] = after


def capture(g):
    held = items(g, HOST)
    g.check('state_captured', len(held) >= 2, held)
    (g.folder / SAVED).write_text(json.dumps({str(k): v for k, v in held.items()}))
    g.ctl('save')


def check(g):
    old = json.loads((g.folder / SAVED).read_text())
    now = items(g, HOST)
    g.check('same_items_and_data_after_the_reopen', {str(k): v for k, v in now.items()} == old, [old, now])
    g.check('every_stack_still_names_its_product', all(v[2] in (A, B) for v in now.values()), now)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'broken-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

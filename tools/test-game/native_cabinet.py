#!/usr/bin/env python3
"""The filing cabinet (docs/content-cabinet.md) on a disposable native world.

setup          a hand fills its eight slots (a named, enchanted sword among them: its data comes out as it went in), a shift-click
               from the inventory fills the first free slot, one from the cabinet gives the stack back whole; a hopper over it
               puts nothing in and a hopper under it takes nothing out (no automation); broken, it spills every stack once;
capture-state  the world is saved with the eight slots full;
check-state    after reopening, every slot holds what it held (the sword with its name and enchantment), and breaking it then
               spills each stack exactly once.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game

X, Z = 40, 8
CABINET = (X, 100, Z)
SAVED = 'cabinet-saved.json'
SWORD = "minecraft:diamond_sword[minecraft:custom_name='\"Dossier\"',minecraft:enchantments={\"minecraft:sharpness\":3}]"
FILL = [('minecraft:paper', 64), ('minecraft:diamond_sword', 1), ('minecraft:gold_ingot', 17), ('ntm:key', 1),
        ('minecraft:coal', 5), ('minecraft:glass', 33), ('minecraft:stick', 2), ('minecraft:bread', 9)]


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def raw(g, p=None):
    p = p or CABINET
    return ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))


def items(g, p=None):
    """slot -> (id, count) from the block entity's items."""
    text = raw(g, p)
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


def give(g, item, count):
    g.command(f'give @s {item} {count}')
    time.sleep(.5)


def inventory_slot(g, item_id, count=None):
    screen = g.ctl('screen')
    return next(s['slot'] for s in screen['slots'] if s['slot'] < 36 and s.get('item') == item_id and (count is None or s.get('count') == count))


def put(g, item, count, slot):
    """A hand: the stack picked up from the inventory and put in the cabinet's slot."""
    give(g, item, count)
    item_id = item.split('[')[0]
    src = inventory_slot(g, item_id)
    g.ctl(f'click-slot {src}')
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.4)


def open_cabinet(g):
    g.close()
    g.command(f'tp @s {X + .5} 100 {Z + 3.5}')
    time.sleep(1)
    g.ctl(f'interact {X} 100 {Z} south')
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if g.ctl('screen').get('screen') == 'container':
            return
        time.sleep(.4)
    raise AssertionError('the cabinet did not open')


def build(g):
    g.close()
    g.ground(X, 100, Z, width=14)
    g.command('clear @s')
    g.command('give @s ntm:filing_cabinet 1')
    g.ctl(f'place {X} 100 {Z} --item ntm:filing_cabinet')
    time.sleep(1.5)
    g.command('clear @s')


def fill_all(g):
    open_cabinet(g)
    for slot, (item_id, count) in enumerate(FILL):
        put(g, SWORD if item_id == 'minecraft:diamond_sword' else item_id, count, slot)
    g.close()


def dropped(g, item_id):
    """The item entities of that id near the cabinet: how many entities and their total count."""
    sel = f'@e[type=minecraft:item,x={X},y=100,z={Z},distance=..6,nbt={{Item:{{id:"{item_id}"}}}}]'
    text = ' '.join(g.command(f'execute if entity {sel}').get('chat', []))
    found = re.search(r'count: ?(\d+)', text)
    entities = int(found[1]) if found else 0
    total = 0
    for n in range(entities):
        got = ' '.join(g.command(f'data get entity @e[type=minecraft:item,x={X},y=100,z={Z},distance=..6,nbt={{Item:{{id:"{item_id}"}}}},limit=1,sort=furthest] Item.count').get('chat', []))
        # Each one is read, then picked out of the way so the next read is another.
        number = re.search(r'(\d+)', got.split('data:')[-1]) if got else None
        total += int(number[1]) if number else 0
        g.command(f'kill @e[type=minecraft:item,x={X},y=100,z={Z},distance=..6,nbt={{Item:{{id:"{item_id}"}}}},limit=1,sort=furthest]')
    return entities, total


def setup(g):
    build(g)
    open_cabinet(g)
    # A hand fills the slots one by one.
    for slot, (item_id, count) in enumerate(FILL[:6]):
        put(g, SWORD if item_id == 'minecraft:diamond_sword' else item_id, count, slot)
    first = items(g)
    g.check('a_hand_puts_stacks_in_its_slots', all(first.get(s) == FILL[s] for s in range(6)), first)
    text = raw(g)
    g.check('the_named_enchanted_sword_keeps_its_data', 'Dossier' in text and 'sharpness' in text, text[:300])
    # A shift-click from the inventory fills the first free slot (`mergeItemStack(.., 0, 8, false)`).
    give(g, 'minecraft:stick', 2)
    g.ctl(f'click-slot {inventory_slot(g, "minecraft:stick")} shift')
    time.sleep(.4)
    after = items(g)
    g.check('a_shift_click_fills_the_first_free_slot', after.get(6) == ('minecraft:stick', 2) and 7 not in after, after)
    # A shift-click from the cabinet gives the stack back to the inventory whole.
    g.ctl('click-slot 302 shift')
    time.sleep(.4)
    back = items(g)
    g.check('a_shift_click_gives_the_stack_back_whole', 2 not in back and back.get(0) == FILL[0] and back.get(6) == ('minecraft:stick', 2), back)
    gold = [s for s in g.ctl('screen')['slots'] if s['slot'] < 36 and s.get('item') == 'minecraft:gold_ingot']
    g.check('and_the_inventory_has_all_of_it', len(gold) == 1 and gold[0].get('count') == 17, gold)
    # The rest is put by hand, the gold back in its slot included.
    g.close()
    fill_all_missing(g)
    full = items(g)
    g.evidence['full'] = {str(k): v for k, v in full.items()}
    g.check('all_eight_slots_hold_what_was_put', len(full) == 8 and all(full.get(s, (None,))[0] for s in range(8)), full)
    # The automation: a hopper over it fed by a chest puts nothing in; one under it, with a chest below, takes nothing out.
    g.command(f'setblock {X} 101 {Z} minecraft:hopper[facing=down]')
    g.command(f'setblock {X} 102 {Z} minecraft:chest')
    g.command(f'setblock {X} 99 {Z} minecraft:hopper[facing=down]')
    g.command(f'setblock {X} 98 {Z} minecraft:chest')
    for n in range(3):
        g.command(f'item replace block {X} 102 {Z} container.{n} with minecraft:dirt 5')
    sprint(g, 80)
    held = raw(g, (X, 101, Z))
    chest = raw(g, (X, 102, Z))
    below = raw(g, (X, 98, Z))
    bottom = raw(g, (X, 99, Z))
    now = items(g)
    g.evidence['hoppers'] = {'over': held[:200], 'chest': chest[:200], 'under': bottom[:120], 'below': below[:120]}
    dirt = sum(int(n) for n in re.findall(r'[cC]ount: ?(\d+)', held + chest))
    g.check('a_hopper_over_it_puts_nothing_in', now == full and dirt == 15, [dirt, now])
    taken = sum(int(n) for n in re.findall(r'[cC]ount: ?(\d+)', below + bottom))
    g.check('a_hopper_under_it_takes_nothing_out', taken == 0, [below[:120], bottom[:120]])
    g.command(f'setblock {X} 101 {Z} minecraft:air')
    g.command(f'setblock {X} 102 {Z} minecraft:air')
    g.command(f'setblock {X} 99 {Z} minecraft:air')
    g.command(f'setblock {X} 98 {Z} minecraft:air')
    # Broken, it spills every stack once and holds none.
    broke(g, full)


def fill_all_missing(g):
    have = items(g)
    open_cabinet(g)
    for slot, (item_id, count) in enumerate(FILL):
        if slot not in have:
            put(g, SWORD if item_id == 'minecraft:diamond_sword' else item_id, count, slot)
    g.close()


def broke(g, expected):
    g.command(f'setblock {X} 100 {Z} minecraft:air destroy')
    time.sleep(1.5)
    sums = {}
    for _, (item_id, count) in sorted(expected.items()):
        sums[item_id] = dropped(g, item_id)
    g.evidence['spilled'] = sums
    ok = all(sums[i][0] >= 1 and sums[i][1] == c for i, c in expected.values())
    g.check('broken_it_spills_every_stack_once', ok, sums)
    g.check('and_nothing_is_left_in_it', 'Items' not in raw(g) or items(g) == {}, raw(g)[:100])


def capture(g):
    build(g)
    fill_all(g)
    sprint(g, 20)
    state = items(g)
    g.check('the_eight_slots_are_full_before_the_save', len(state) == 8 and state.get(0) == FILL[0], state)
    g.check('the_sword_keeps_its_data_before_the_save', 'Dossier' in raw(g) and 'sharpness' in raw(g), raw(g)[:200])
    (g.folder / SAVED).write_text(json.dumps({str(k): v for k, v in state.items()}))
    g.ctl('save')


def check(g):
    saved = {int(k): tuple(v) for k, v in json.loads((g.folder / SAVED).read_text()).items()}
    now = items(g)
    g.check('the_slots_are_as_saved', now == saved, [saved, now])
    text = raw(g)
    g.check('the_sword_is_named_and_enchanted_still', 'Dossier' in text and 'sharpness' in text, text[:200])
    sprint(g, 40)
    later = items(g)
    g.check('nothing_changes_with_time', later == saved, later)
    broke(g, saved)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'cabinet-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

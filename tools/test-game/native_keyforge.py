#!/usr/bin/env python3
"""The locksmith table (docs/onda-2-sublote-2.md) on a disposable native world.

setup          a key with pins in the first slot, a padlock in the second and a reinforced padlock in the third: the second gets the
               key's pins, the third is re-rolled every tick (three digits, different readings); a fake key in the second keeps its
               own pins; a hopper under the table takes the second slot's item out, a hopper over it puts nothing in (a hopper
               puts nothing in a locksmith table);
capture-state  the world is saved with the copy made;
check-state    after reopening, the pins are as saved (the first two) and the third is still rolling.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game

X, Z = 40, 8
TABLE = (X, 100, Z)
SAVED = 'keyforge-saved.json'


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def slots(g, p=TABLE):
    """slot -> (id, pins or None) from the block entity's items."""
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    out = {}
    depth, start = 0, None
    entries = []
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
        pins = re.search(r'pins: ?(-?\d+)', entry)
        if slot and item:
            out[int(slot[1])] = (item[1], int(pins[1]) if pins else None)
    return out


def give(g, item, custom=None):
    component = '' if custom is None else f'[minecraft:custom_data={custom}]'
    g.command(f'give @s {item}{component} 1')
    time.sleep(.5)


def put(g, item, slot, custom=None):
    give(g, item, custom)
    screen = g.ctl('screen')
    src = next(s['slot'] for s in screen['slots'] if s['slot'] < 36 and s.get('item') == item)
    g.ctl(f'click-slot {src}')
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.4)


def open_table(g):
    g.close()
    g.command(f'tp @s {X + .5} 100 {Z + 3.5}')
    time.sleep(1)
    g.ctl(f'interact {X} 100 {Z} south')
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if g.ctl('screen').get('screen') == 'container':
            return
        time.sleep(.4)
    raise AssertionError('the table did not open')


def build(g):
    g.close()
    g.ground(X, 100, Z, width=14)
    g.command('clear @s')
    g.command('give @s ntm:machine_keyforge 1')
    g.ctl(f'place {X} 100 {Z} --item ntm:machine_keyforge')
    time.sleep(1.5)
    g.command('clear @s')


def setup(g):
    build(g)
    open_table(g)
    put(g, 'ntm:key', 0, '{pins:456}')
    put(g, 'ntm:padlock', 1)
    put(g, 'ntm:padlock_reinforced', 2)
    g.close()
    sprint(g, 20)
    first = slots(g)
    g.check('the_second_got_the_first_ones_pins', first.get(1) == ('ntm:padlock', 456) and first.get(0) == ('ntm:key', 456), first)
    seen = {first[2][1]}
    for _ in range(4):
        sprint(g, 7)
        seen.add(slots(g)[2][1])
    g.check('the_third_rolls_three_digits_every_tick', all(p is not None and 100 <= p <= 999 for p in seen) and len(seen) >= 3, sorted(seen, key=lambda v: v or 0))
    # A fake key in the second slot: no pins copied into it.
    open_table(g)
    g.ctl('click-slot 301 shift')
    put(g, 'ntm:key_fake', 1, '{pins:7}')
    g.close()
    sprint(g, 20)
    fake = slots(g)
    g.check('a_fake_key_keeps_its_own_pins', fake.get(1) == ('ntm:key_fake', 7), fake)
    # The automation: a hopper under the table takes what is in the second slot; one over it puts nothing in.
    g.command(f'setblock {X} {99} {Z} minecraft:hopper[facing=down]')
    g.command(f'setblock {X} {98} {Z} minecraft:chest')
    g.command(f'setblock {X} 101 {Z} minecraft:hopper[facing=down]')
    g.command(f'setblock {X} 102 {Z} minecraft:chest')
    g.command(f'item replace block {X} 102 {Z} container.0 with ntm:key 3')
    sprint(g, 60)
    text = ' '.join(g.command(f'data get block {X} 98 {Z} Items').get('chat', []))
    g.check('a_hopper_under_it_takes_the_second_slot_out', 'ntm:key_fake' in text, text[:200])
    above = ' '.join(g.command(f'data get block {X} 102 {Z} Items').get('chat', []))
    after = slots(g)
    g.check('a_hopper_over_it_puts_nothing_in', 'ntm:key' in above and after.get(0, ('', 0))[0] == 'ntm:key' and after.get(0)[1] == 456, [above[:120], after])


def capture(g):
    build(g)
    open_table(g)
    put(g, 'ntm:key', 0, '{pins:123}')
    put(g, 'ntm:padlock_rusty', 1)
    put(g, 'ntm:padlock', 2)
    g.close()
    sprint(g, 20)
    state = slots(g)
    g.check('the_copy_is_made_before_the_save', state.get(1) == ('ntm:padlock_rusty', 123), state)
    (g.folder / SAVED).write_text(json.dumps({str(k): v for k, v in state.items()}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    now = slots(g)
    g.check('the_first_two_are_as_saved', now.get(0) == tuple(saved['0']) and now.get(1) == tuple(saved['1']), [saved, now])
    sprint(g, 5)
    after = slots(g)
    g.check('the_third_still_rolls', after.get(2) is not None and after[2][1] is not None and 100 <= after[2][1] <= 999, after)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'keyforge-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

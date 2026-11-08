#!/usr/bin/env python3
"""K2 closure in game: the seven bottles need the bottle opener in the inventory.

Own roteiro (it does not touch native_items.py). Run it on a fresh creative world
built by the shared fork build, never by /home/sacer/Games/horizon:

    HORIZON_MOD=/home/sacer/.cache/ntm-native-main/target/release/cargo-horizon-mod \
        python3 tools/test-game/run_k2_closure.py <world-folder>

Java (ItemEnergy.onItemRightClick): without bottle_opener in the player's inventory
the use does not start and the bottle stays; with it, the bottle is consumed, the
cap (`makeBottle(bottle, cap)`) is given, and the opener itself is not consumed.
"""
import sys
import time
from native_game import Game

# (bottle, empty container it becomes, cap it leaves): ItemEnergy.makeBottle(empty, cap)
# gives the cap and replaces the bottle in its slot with the empty container.
BOTTLES = [
    ('ntm:bottle_nuka', 'ntm:bottle_empty', 'ntm:cap_nuka'),
    ('ntm:bottle_cherry', 'ntm:bottle_empty', 'ntm:cap_nuka'),
    ('ntm:bottle_quantum', 'ntm:bottle_empty', 'ntm:cap_quantum'),
    ('ntm:bottle_rad', 'ntm:bottle_empty', 'ntm:cap_rad'),
    ('ntm:bottle_sparkle', 'ntm:bottle_empty', 'ntm:cap_sparkle'),
    ('ntm:bottle2_korl', 'ntm:bottle2_empty', 'ntm:cap_korl'),
    ('ntm:bottle2_fritz', 'ntm:bottle2_empty', 'ntm:cap_fritz'),
]
OPENER = 'ntm:bottle_opener'
# A drink takes 32 ticks (1.6 s); the hold is longer because a click reaches the server late.
HOLD = 3.0


def counts(g):
    return g.ctl('inventory')['counts']


def clear_hotbar(g):
    # `clear @s` without an item leaves the stacks in place: empty every hotbar slot instead.
    for slot in range(9):
        g.command(f'item replace entity @s hotbar.{slot} with minecraft:air')
    time.sleep(.5)
    left = counts(g)
    g.check('hotbar_empty', not left, left)


def give_hand(g, item, count=1, slot=0):
    g.command(f'item replace entity @s hotbar.{slot} with {item} {count}')
    g.ctl(f'select {slot + 1}')
    time.sleep(.4)


def use(g, seconds=0):
    g.close()
    g.ctl('look-at 20 160 8')
    if seconds:
        g.ctl('mouse right down')
        time.sleep(seconds)
        try:
            g.ctl('mouse right up')
        except RuntimeError as error:
            if 'dead' not in str(error):
                raise
    else:
        g.ctl('mouse right click')
    time.sleep(1.5)


def without_opener(g):
    """The use is refused: the bottle stays, no empty container, no cap."""
    for bottle, empty, cap in BOTTLES:
        clear_hotbar(g)
        give_hand(g, bottle, 1, slot=0)
        before = counts(g)
        use(g, HOLD)
        after = counts(g)
        g.check(f'refused_without_opener_{bottle}',
                after.get(bottle, 0) == 1 and after.get(empty, 0) == 0 and after.get(cap, 0) == 0,
                {'before': before, 'after': after})


def with_opener(g):
    """The use is accepted: the bottle becomes the empty container, the cap is given, the opener stays."""
    for bottle, empty, cap in BOTTLES:
        clear_hotbar(g)
        give_hand(g, OPENER, 1, slot=1)
        give_hand(g, bottle, 1, slot=0)
        before = counts(g)
        use(g, HOLD)
        after = counts(g)
        g.check(f'bottle_used_{bottle}',
                after.get(bottle, 0) == 0 and after.get(empty, 0) == 1,
                {'before': before, 'after': after})
        g.check(f'cap_given_{bottle}',
                after.get(cap, 0) == 1,
                {'cap': cap, 'before': before, 'after': after})
        g.check(f'opener_not_consumed_{bottle}',
                after.get(OPENER, 0) == 1,
                {'before': before, 'after': after})


if __name__ == '__main__':
    mode = sys.argv[2] if len(sys.argv) > 2 else 'k2-bottles'
    g = Game(sys.argv[1], 'k2-closure-' + mode)
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    # ItemEnergy consumes and gives the cap only outside creative mode.
    g.check('survival_world', g.ctl('status').get('creative') is False, g.ctl('status').get('creative'))
    without_opener(g) if mode == 'without-opener' else with_opener(g)
    g.evidence['ok'] = True
    g.save()

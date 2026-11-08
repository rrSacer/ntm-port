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

# (bottle, cap it leaves) as in consumables.json (`gives`, `requires`).
BOTTLES = [
    ('ntm:bottle_nuka', 'ntm:cap_nuka'),
    ('ntm:bottle_cherry', 'ntm:cap_nuka'),
    ('ntm:bottle_quantum', 'ntm:cap_quantum'),
    ('ntm:bottle_rad', 'ntm:cap_rad'),
    ('ntm:bottle_sparkle', 'ntm:cap_sparkle'),
    ('ntm:bottle2_korl', 'ntm:cap_korl'),
    ('ntm:bottle2_fritz', 'ntm:cap_fritz'),
]
OPENER = 'ntm:bottle_opener'


def counts(g):
    return g.ctl('inventory')['counts']


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
    time.sleep(.8)


def without_opener(g):
    """The use is refused: the bottle stays, no cap, and the refusal is in chat."""
    for bottle, cap in BOTTLES:
        g.command(f'clear @s {OPENER}')
        give_hand(g, bottle, 1)
        before = counts(g)
        use(g, 2.2)
        after = counts(g)
        g.check(f'refused_without_opener_{bottle}',
                after.get(bottle, 0) == before.get(bottle, 0) and after.get(cap, 0) == before.get(cap, 0),
                {'before': before, 'after': after})
        g.command(f'clear @s {bottle}')


def with_opener(g):
    """The use is accepted: one bottle less, its cap, and the opener stays."""
    for bottle, cap in BOTTLES:
        give_hand(g, OPENER, 1, slot=1)
        give_hand(g, bottle, 1, slot=0)
        before = counts(g)
        use(g, 2.2)
        after = counts(g)
        g.check(f'consumed_with_opener_{bottle}',
                after.get(bottle, 0) == before.get(bottle, 0) - 1,
                {'before': before, 'after': after})
        g.check(f'cap_given_{bottle}',
                after.get(cap, 0) == before.get(cap, 0) + 1,
                {'cap': cap, 'before': before, 'after': after})
        g.check(f'opener_not_consumed_{bottle}',
                after.get(OPENER, 0) == before.get(OPENER, 0),
                {'before': before, 'after': after})
        g.command(f'clear @s {OPENER}')
        g.command(f'clear @s {cap}')


if __name__ == '__main__':
    mode = sys.argv[2] if len(sys.argv) > 2 else 'k2-bottles'
    g = Game(sys.argv[1], 'k2-closure-' + mode)
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    without_opener(g) if mode == 'without-opener' else with_opener(g)
    g.evidence['ok'] = True
    g.save()

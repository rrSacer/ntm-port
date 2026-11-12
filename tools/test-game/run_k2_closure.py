#!/usr/bin/env python3
"""K2 closure in game: the seven bottles need the bottle opener in the inventory.

Own roteiro (it does not touch native_items.py). Run on a fresh survival world
of the shared fork client, with the content mod loaded by --dev-mod:

    python3 tools/test-game/run_k2_closure.py <game-folder> [bottles|cherry-probe]

`bottles` (default): for each bottle, in the same session and the same state,
the use is first tried WITHOUT the opener (must be refused: the bottle stays, no
empty container, no cap, no effect, no dose), then WITH the opener (must be
consumed: the empty container takes its slot, the cap is given, the opener stays,
the effects of the use table are on the player at their amplifiers, and the dose
grows by the table's millirad). Each check has its own name, so the evidence
counts every check it runs.

`cherry-probe`: a fresh world, the cherry bottle alone, the chat reply and the
inventory before and after, to see whether it is reported as ntm:bottle_nuka.

Java (ItemEnergy.onItemRightClick): `requiresOpener && !player.inventory.hasItem(
bottle_opener)` refuses the start; `if(!creative) --stackSize`; the cap goes first,
then the empty container (replacing the slot when the stack is gone).
"""
import json
import re
import sys
import time
from pathlib import Path
from native_game import Game

ROOT = Path(__file__).resolve().parents[2]
TABLE = json.loads((ROOT / 'crates/ntm-core/data/consumables.json').read_text())['entries']

# (bottle, empty container it becomes, cap it leaves)
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


def give_hand(g, item, slot):
    g.command(f'item replace entity @s hotbar.{slot} with {item} 1')
    g.ctl(f'select {slot + 1}')
    time.sleep(.4)


def use(g, seconds):
    g.close()
    g.ctl('look-at 20 160 8')
    g.ctl('mouse right down')
    time.sleep(seconds)
    try:
        g.ctl('mouse right up')
    except RuntimeError as error:
        if 'dead' not in str(error):
            raise
    time.sleep(1.5)


def effects(g):
    player = g.ctl('state').get('player') or {}
    return {e['id'].split(':')[-1]: e['amplifier'] for e in player.get('effects', [])}


def dose(g):
    text = ' '.join(g.command('ntmitem inspect').get('chat', []))
    found = re.search(r'dose:\s*(\d+)', text)
    if not found:
        raise AssertionError(f'no dose in: {text}')
    return int(found.group(1))


def run_bottle(g, bottle, empty, cap):
    name = bottle.split(':')[-1]
    spec = TABLE[bottle]
    clear_hotbar(g)
    # A stronger active effect is not replaced by a weaker one: clear them so each bottle is judged alone.
    g.command('effect clear @s')
    effects_before = effects(g)
    give_hand(g, bottle, 0)
    before = counts(g)
    dose_before = dose(g)

    # 1. Without the opener: refused, and nothing else changes.
    use(g, HOLD)
    refused = counts(g)
    g.check(f'refused_without_opener_{name}',
            refused.get(bottle, 0) == 1 and refused.get(empty, 0) == 0 and refused.get(cap, 0) == 0,
            {'before': before, 'after': refused})
    g.check(f'refused_no_effect_{name}', effects(g) == effects_before, effects(g))
    g.check(f'refused_no_dose_{name}', dose(g) == dose_before, {'before': dose_before, 'after': dose(g)})

    # 2. The same bottle, the same state, with the opener in the inventory: accepted.
    give_hand(g, OPENER, 1)
    give_hand(g, bottle, 0)
    before = counts(g)
    dose_before = dose(g)
    use(g, HOLD)
    after = counts(g)
    g.check(f'bottle_used_{name}',
            after.get(bottle, 0) == 0 and after.get(empty, 0) == 1,
            {'before': before, 'after': after})
    g.check(f'cap_given_{name}', after.get(cap, 0) == 1, {'cap': cap, 'after': after})
    g.check(f'opener_not_consumed_{name}', after.get(OPENER, 0) == 1, {'after': after})
    expected = {eid: amp for eid, _ticks, amp in spec.get('effects', [])}
    now = effects(g)
    g.check(f'effects_{name}',
            all(now.get(eid) == amp for eid, amp in expected.items()),
            {'expected': expected, 'actual': now})
    # Without a dose in the table (the bottle2 ones) the dose must not move.
    expected_dose = spec.get('dose', {}).get('millirad', 0)
    gained = dose(g) - dose_before
    g.check(f'dose_{name}', gained == expected_dose,
            {'expected_millirad': expected_dose, 'gained': gained})


def bottles(g):
    for bottle, empty, cap in BOTTLES:
        run_bottle(g, bottle, empty, cap)


def cherry_probe(g):
    """The cherry bottle alone: what the chat and the inventory report."""
    clear_hotbar(g)
    reply = g.command('item replace entity @s hotbar.0 with ntm:bottle_cherry 1')
    g.ctl('select 1')
    time.sleep(.4)
    g.note('chat_reply', ' | '.join(reply.get('chat', [])))
    held = g.ctl('inventory')
    g.note('inventory_after_give', held['counts'])
    g.check('cherry_reported_as_itself', held['counts'] == {'ntm:bottle_cherry': 1}, held['counts'])


if __name__ == '__main__':
    folder = sys.argv[1]
    mode = sys.argv[2] if len(sys.argv) > 2 else 'bottles'
    g = Game(folder, 'k2-closure-' + mode)
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    # ItemEnergy consumes and gives the cap only outside creative mode.
    g.check('survival_world', g.ctl('status').get('creative') is False, g.ctl('status').get('creative'))
    cherry_probe(g) if mode == 'cherry-probe' else bottles(g)
    g.evidence['ok'] = True
    g.save()

#!/usr/bin/env python3
"""T01, the review's pending C and probes (`rbmk-plano` T01, review `revisao-rbmk-t01`), on a disposable world.

Scenario C (the runner, run-rbmk-t01-proof.py --scenario c, owns the process and the cut). Needs NTM_TEST_HOOKS=1:
  prepare  three fuel channels across the border x=48 (46 and 47 in chunk 2, 48 in chunk 3), one rod each, by hand
  act      the rod of 46 leaves as an item entity in chunk 3 through its reactor's entity effect (`ntmtest eject`); a
           cursor edit for the rod of 47 is refused (with and without the menu open) and the rod stays; the rod of 48
           goes out of the hand as a thrown drop in one transfer (`ntmtest take`); then the save the runner cuts
  verify / again / drain   as native_rbmk_t01.py
With NTM_CRASH_VANILLA=1 the vanilla control: barrels at the same cells, and the act empties each barrel and puts the
rod where the mod does (`summon item` at the same place, the second barrel untouched, `summon item` at the player),
before the save.

Probe mode (`probe`, NTM only, on a prepared base of native_rbmk_t01.py): P0-1 a column at (640, 101, 640) does not
hold the reactor, loaded or not; P1-1 a fuel channel replaced in place by `/setblock`, and broken and put back, leaves
the reactor stepping; P1-2 a rod with a custom name, lore and a foreign component keeps them in the channel and when
the column is broken.
"""
import os
import re
import sys
import time
from native_game import Game
from native_pile import block
import native_rbmk_t01 as t

Y = t.Y
CHANNELS = [(46, Y, 8), (47, Y, 8), (48, Y, 8)]
EJECT_AT = (49.5, 102.0, 12.5)
VANILLA = t.VANILLA
TOTAL = 3


def count(g):
    over = sum(t.slot_rods(g, p) for p in CHANNELS)
    return {'columns': over, 'nether': 0, 'entities': t.entity_rods(g), 'inventory': t.inventory_rods(g), 'ledger': t.ledger(g)}


def settled(g, timeout=90):
    deadline = time.monotonic() + timeout
    previous = None
    while time.monotonic() < deadline:
        now = count(g)
        key = (now['ledger']['pending'], now['entities'], now['columns'], now['inventory'])
        if now['ledger']['pending'] == 0 and key == previous:
            return now
        previous = key
        time.sleep(2.5)
    return count(g)


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 64 24')
    time.sleep(3)
    g.command('fill 40 99 3 56 108 18 minecraft:air')
    g.command('fill 40 100 3 56 100 18 minecraft:stone')
    g.command('clear @s')
    for p in CHANNELS:
        g.command(f'setblock {p[0]} {p[1]} {p[2]} ' + ('minecraft:barrel' if VANILLA else t.column('rbmk_rod')))
    time.sleep(2)
    for p in CHANNELS:
        t.give(g, t.FUEL, 1)
        t.put_by_hand(g, p, t.FUEL)
        g.command(f'clear @s {t.FUEL}')
    g.command('tp @s 47.5 101 4.5')
    time.sleep(2)
    base = count(g)
    g.check('base_rods', base['columns'] == TOTAL and base['inventory'] == 0 and base['entities'] == 0, base)
    g.evidence['base'] = base
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    g.command('tp @s 47.5 101 4.5')
    before = count(g)
    g.check('start_with_three_in_the_channels', before['columns'] == TOTAL, before)
    if VANILLA:
        g.command(f'item replace block 46 {Y} 8 container.0 with minecraft:air')
        g.command(f'summon minecraft:item {EJECT_AT[0]} {EJECT_AT[1]} {EJECT_AT[2]} {{Item:{{id:"{t.FUEL}",count:1}},PickupDelay:10s}}')
        # The mod's cursor edit is refused: the control's rod stays in its barrel too.
        g.command(f'item replace block 48 {Y} 8 container.0 with minecraft:air')
        g.command(f'execute at @s run summon minecraft:item ~ ~1.5 ~ {{Item:{{id:"{t.FUEL}",count:1}},PickupDelay:40s}}')
    else:
        replies = []
        replies.append(g.command(f'ntmtest eject 46 {Y} 8 {EJECT_AT[0]} {EJECT_AT[1]} {EJECT_AT[2]}').get('chat'))
        time.sleep(1.5)
        # The cursor: the server keeps no screen of its own for its players (a command runs on the integrated server's
        # thread, whose screen is always none), so the glue refuses every cursor edit for now (T05 brings the menu
        # context). Refused with or without the player's menu open, and nothing moves: the rod stays in its channel.
        refused = [g.command(f'ntmtest take 47 {Y} 8 cursor').get('chat')]
        g.ctl('open-inventory')
        time.sleep(.5)
        refused.append(g.command(f'ntmtest take 47 {Y} 8 cursor').get('chat'))
        g.close()
        g.evidence['cursor_refused'] = refused
        g.check('cursor_edits_are_refused_without_loss', all(r and 'ok=false' in ' '.join(r) for r in refused) and t.slot_rods(g, (47, Y, 8)) == 1, refused)
        replies.append(g.command(f'ntmtest take 48 {Y} 8 drop').get('chat'))
        g.evidence['hooks'] = replies
        g.check('hooks_answered', all(r and 'ok=true' in ' '.join(r) for r in replies), replies)
    # The player steps away at once, in both worlds: a drop is not picked up before the save (the mod's is thrown away
    # from the eyes, the control's summoned at the feet; who picks what up first is timing, not the transfer).
    g.command('tp @s 30.5 101 4.5')
    # Both chunks change a block too, in both worlds (the engine's `item replace block` alone leaves a chunk unsaved).
    for x in (44, 50):
        g.command(f'setblock {x} 100 14 minecraft:cobblestone')
    time.sleep(1.5)
    after = count(g)
    g.evidence['after_act'] = after
    g.check('two_rods_moved_once_one_stayed', after['columns'] == 1 and after['columns'] + after['entities'] + after['inventory'] == TOTAL, after)
    g.save()
    if t.NO_SAVE:
        return
    if t.PAUSE_SAVE:
        g.evidence['pause_save'] = True
        g.save()
        try:
            g.ctl('pause')
        except Exception:
            return
        deadline = time.monotonic() + 270
        while time.monotonic() < deadline:
            try:
                g.ctl('status')
            except Exception:
                return
            time.sleep(.5)
        return
    try:
        g.ctl('save')
        g.evidence['survived_save'] = True
    except Exception as error:
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s 47.5 101 4.5')
    now = settled(g)
    now['total'] = now['columns'] + now['entities'] + now['inventory']
    g.evidence['counted' + ('_again' if again else '')] = now
    g.check('no_more_than_twice', now['total'] <= 2 * TOTAL, now)
    g.check('nothing_left_to_spawn', now['ledger']['pending'] == 0, now['ledger'])


def drain(g):
    g.ctl('wait-ready 120')
    g.command('kill @e[type=minecraft:item]')
    time.sleep(1.5)
    led = t.ledger(g)
    g.check('nothing_owed_when_the_items_are_gone', led['pending'] == 0, led)
    g.ctl('save')


def advancing(g, name, seconds=4):
    first = t.heat(g, t.SOURCE)
    time.sleep(seconds)
    later = t.heat(g, t.SOURCE)
    g.evidence[name] = [first, later]
    return first is not None and later is not None and later != first


def probe(g):
    """P0-1, P1-1, P1-2 in game, on a prepared base of native_rbmk_t01.py (the reactor around the source at 47,101,9)."""
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s 47.5 101 4.5')
    time.sleep(3)
    g.check('reactor_steps', advancing(g, 'p0_before'), g.evidence['p0_before'])
    # P0-1: a column at (640, 101, 640), its chunk forceloaded, then not.
    g.command('forceload add 640 640')
    time.sleep(3)
    g.command(f'setblock 640 {Y} 640 ' + t.column('rbmk_blank'))
    time.sleep(2)
    g.check('p0_far_column_loaded_does_not_hold_the_reactor', advancing(g, 'p0_far_loaded'), g.evidence['p0_far_loaded'])
    g.command('forceload remove 640 640')
    time.sleep(6)
    g.check('p0_far_column_unloaded_does_not_hold_the_reactor', advancing(g, 'p0_far_unloaded'), g.evidence['p0_far_unloaded'])
    # P1-1: a rod into channel 46 by hand, then the channel replaced in place by a blank; then air and a channel again.
    g.command('gamemode survival @s')
    t.give(g, t.FUEL, 1)
    t.put_by_hand(g, (46, Y, 8), t.FUEL)
    g.command('gamemode creative @s')
    g.check('p1_rod_in', t.slot_rods(g, (46, Y, 8)) == 1, t.slot_rods(g, (46, Y, 8)))
    g.command(f'setblock 46 {Y} 8 ' + t.column('rbmk_blank'))
    time.sleep(3)
    g.check('p1_replaced_in_place_reactor_steps_on', advancing(g, 'p1_replaced'), g.evidence['p1_replaced'])
    g.command(f'setblock 46 {Y} 8 minecraft:air')
    time.sleep(2)
    g.command(f'setblock 46 {Y} 8 ' + t.column('rbmk_rod'))
    time.sleep(3)
    g.check('p1_broken_and_put_back_reactor_steps_on', advancing(g, 'p1_put_back'), g.evidence['p1_put_back'])
    # P1-2: a rod with a custom name, lore and a foreign custom field, by hand into channel 48: all kept, and in the item
    # the column spills when it is broken.
    g.command('gamemode survival @s')
    g.command(f'give @s {t.FUEL}[minecraft:custom_name=\'"Barra X"\',minecraft:lore=[\'"lore"\'],minecraft:custom_data={{probe_marker:7}}] 2')
    t.put_by_hand(g, (48, Y, 8), t.FUEL)
    g.command('gamemode creative @s')
    time.sleep(2)
    text = ' '.join(g.command(f'data get block 48 {Y} 8 Items').get('chat', []))
    g.evidence['p12_channel'] = text
    g.check('p12_name_lore_and_foreign_field_in_the_channel', 'Barra X' in text and 'lore' in text and 'probe_marker' in text and 'hull' in text, text)
    hand = g.ctl('inventory')
    g.evidence['p12_hand'] = hand.get('held')
    g.command(f'setblock 48 {Y} 8 minecraft:air destroy')
    time.sleep(2)
    spilled = ' '.join(g.command(f'execute as @e[type=minecraft:item,x=48,y=101,z=8,distance=..6,nbt={{Item:{{id:"{t.FUEL}"}}}}] run data get entity @s Item').get('chat', []))
    g.evidence['p12_spilled'] = spilled
    g.check('p12_kept_when_the_column_is_broken', 'Barra X' in spilled and 'lore' in spilled and 'probe_marker' in spilled, spilled)


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t01-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain, 'probe': probe}[mode](g)
    g.evidence['ok'] = True
    g.save()

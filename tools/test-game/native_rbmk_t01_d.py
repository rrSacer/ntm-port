#!/usr/bin/env python3
"""Owed/drain proof (Onda 2, D1+D2 fatia 4): an `Items` effect into a chunk that is genuinely out, on a disposable world.

Scenario D (the runner, run-rbmk-t01-proof.py --scenario d, owns the process and the cut). Needs NTM_TEST_HOOKS=1:
  prepare  as scenario C: three fuel channels across the border x=48 (46 and 47 in chunk 2, 48 in chunk 3), one rod each
  act      the rod of 46 leaves its channel as an `Items` effect of its reactor's step (`ntmtest drop`), bound for the
           cell (200, 101, 8) of chunk (12, 0), which is not loaded: no block effect goes before it in the batch, so
           the item is owed at once in the record of the reactor's chunk (`pending` > 0 on the ledger), with the
           channel emptied in the same step; then the save the runner cuts
  verify   reopen; while (12, 0) is still out the debt stays owed; then the player goes there and the debt is paid
           there, once; back at the channels, the count is the channels, the item entities at both places, the
           player and what is still owed
  again    the same on the second reopen: nothing is owed any more and nothing is paid twice
  drain    the item entities killed; nothing is owed
With NTM_CRASH_VANILLA=1 the vanilla control: barrels at the same cells. A barrel keeps its rod while (12, 0) is out,
as the mod's record keeps its debt; once that chunk is loaded the control puts the rod at the same cell (`summon
item`) and, back at the barrels, takes it out of its barrel, once (the stock of a container waiting for its destination).
"""
import re
import sys
import time
from native_game import Game
import native_rbmk_t01 as t
import native_rbmk_t01_c as c

Y = t.Y
SOURCE = (46, Y, 8)
FAR = (200, 101, 8)
FAR_CHUNK = (FAR[0] >> 4, FAR[2] >> 4)
VANILLA = t.VANILLA
TOTAL = 3
# The item entities near the player and at the far cell, ~153 blocks away.
RADIUS = 260


def far_loaded(g):
    return bool(g.ctl('block %d %d %d' % FAR)['loaded'])


def far_rods(g):
    items = g.ctl(f'entities {RADIUS}')['entities']
    return [(e['item']['item'], e['item']['count'], e['pos']) for e in items
            if e.get('kind') == 'item' and e['item']['item'].startswith(t.PREFIX)]


def count(g):
    rods = far_rods(g)
    led = t.ledger(g)
    over = sum(t.slot_rods(g, p) for p in c.CHANNELS)
    return {'columns': over, 'entities': sum(n for _, n, _ in rods), 'entity_list': rods,
            'inventory': t.inventory_rods(g), 'ledger': led, 'far_loaded': far_loaded(g)}


def prepare(g):
    c.prepare(g)
    g.check('far_chunk_out_after_prepare', not far_loaded(g), g.ctl('block %d %d %d' % FAR))


def act(g):
    g.ctl('wait-ready 120')
    g.command('tp @s 47.5 101 4.5')
    time.sleep(2)
    before = count(g)
    g.evidence['before_act'] = before
    g.check('start_with_three_in_the_channels', before['columns'] == TOTAL and before['entities'] == 0, before)
    g.check('far_chunk_out_before_the_drop', not before['far_loaded'], before)
    if VANILLA:
        pass
    else:
        reply = g.command(f'ntmtest drop {SOURCE[0]} {SOURCE[1]} {SOURCE[2]} {FAR[0]} {FAR[1]} {FAR[2]}').get('chat')
        g.evidence['hook'] = reply
        g.check('hook_answered', reply and 'ok=true' in ' '.join(reply), reply)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline and t.ledger(g)['pending'] == 0:
            time.sleep(.5)
    # Both chunks of the channels change a block too, in both worlds (the cut then has their records).
    for x in (44, 50):
        g.command(f'setblock {x} 100 14 minecraft:cobblestone')
    time.sleep(1.5)
    after = count(g)
    g.evidence['after_act'] = after
    g.check('far_chunk_still_out', not after['far_loaded'], after)
    if VANILLA:
        g.check('control_keeps_its_rod_in_the_barrel', after['columns'] == TOTAL and after['entities'] == 0, after)
    else:
        g.check('rod_owed_in_the_reactors_record', after['ledger']['pending'] >= 1 and after['columns'] == TOTAL - 1
                and after['entities'] == 0, after)
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


def at_a(g):
    g.command('tp @s 47.5 101 4.5')
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline and not g.ctl('block %d %d %d' % SOURCE)['loaded']:
        time.sleep(1)


def at_b(g):
    """The player goes to the far cell (a forced chunk that far is not brought in), as a player would; the channels'
    chunks stay forced since prepare."""
    # Fourteen blocks south of the cell: the paid rod is not picked up (it is counted where it lies).
    g.command(f'tp @s {FAR[0] + .5} 140 {FAR[2] + 14.5}')
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline and not far_loaded(g):
        time.sleep(1)


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    at_a(g)
    time.sleep(2)
    suffix = '_again' if again else ''
    away = count(g)
    g.evidence['while_away' + suffix] = away
    g.check('far_chunk_out_on_reopen', not away['far_loaded'], away)
    stock = t.slot_rods(g, SOURCE) if VANILLA else 0
    at_b(g)
    g.check('far_chunk_loaded_for_payment', far_loaded(g), g.ctl('block %d %d %d' % FAR))
    g.evidence['channels_while_at_b' + suffix] = g.ctl('block %d %d %d' % SOURCE)
    if VANILLA and stock:
        # The control's stock goes out once its destination is in: the rod at the cell, then the barrel emptied.
        g.command(f'summon minecraft:item {FAR[0] + .5} {FAR[1]} {FAR[2] + .5} {{Item:{{id:"{t.FUEL}",count:1}},PickupDelay:10s}}')
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline and t.ledger(g)['pending']:
        time.sleep(1)
    time.sleep(2)
    paid = {'entity_list': far_rods(g), 'ledger': t.ledger(g)}
    paid['entities'] = sum(n for _, n, _ in paid['entity_list'])
    g.evidence['at_b' + suffix] = paid
    at_a(g)
    if VANILLA and stock:
        g.command(f'item replace block {SOURCE[0]} {SOURCE[1]} {SOURCE[2]} container.0 with minecraft:air')
    time.sleep(2)
    here = count(g)
    now = {'columns': here['columns'], 'entities': paid['entities'] + here['entities'], 'inventory': here['inventory'],
           'ledger': here['ledger'], 'entity_list': paid['entity_list'] + here['entity_list']}
    now['total'] = now['columns'] + now['entities'] + now['inventory']
    g.evidence['counted' + suffix] = now
    g.check('no_more_than_twice', now['total'] <= 2 * TOTAL, now)
    g.check('nothing_left_to_spawn', now['ledger']['pending'] == 0, now['ledger'])
    # A paid rod lies at the far cell's chunk, never anywhere else.
    g.check('paid_only_at_the_destination', all((int(p[0]) >> 4, int(p[2]) >> 4) == FAR_CHUNK for _, _, p in paid['entity_list'])
            and not here['entity_list'], now['entity_list'])
    g.ctl('save')


def drain(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    at_b(g)
    g.command('kill @e[type=minecraft:item]')
    time.sleep(1.5)
    at_a(g)
    time.sleep(1.5)
    led = t.ledger(g)
    g.check('nothing_owed_when_the_items_are_gone', led['pending'] == 0, led)
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t01-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)

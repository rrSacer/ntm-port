#!/usr/bin/env python3
"""Atomic-save proof on a disposable world: a pile that straddles the chunk border x=48 gives up the rods of both
its fuel channels (drill undo through the loaders), and the game is killed at one boundary of the save.

Modes (the runner, run-crash-proof.py, owns the game process and the kill):
  prepare  build the pile, load 5 rods into each of its two channels, save, and stop there
  undo     undo both channels, then ask for a save (the armed failpoint kills the game inside it)
  verify   after a reopen: wait until the ledger settles, then count every rod the world holds
  again    verify once more after a second clean reopen (nothing must spawn again)
  drain    remove every rod entity and check that what was owed is retired
Rods: 10 = 5 + 5 `ntm:pile_rod__ra226be` in the pile, plus 2 plain ones on the ground where they fall: 12, in channels,
loaders, item entities or owed by the ledger.

With NTM_CRASH_VANILLA=1 the same world is the vanilla control: no pile, the two plain rods, and the undo puts 5 + 5
loose rods on the ground on both sides of the border instead of ejecting them; the runner compares its losses.
"""
import os
import json
import re
import sys
import time
from native_game import Game
from native_pile import interact, machine, tool, block

CORE = (47, 102, 8)
CHANNELS = [(46, 102, 8), (48, 102, 8)]
LOADERS = [(46, 102, 7), (48, 102, 7)]
ROD = 'ntm:pile_rod__ra226be'
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
# Where the control's loose rods are put, one side of the border each.
LOOSE = (46.5, 48.5)
PILE = 10
COMMON = 2  # plain rods lying where the pile's rods fall, for the merge cases
TOTAL = PILE + COMMON


def ledger(g):
    """`/ntmledger` of the server: the effects the loaded chunks hold applied, and what an older record still owes."""
    text = ' '.join(g.command('ntmledger').get('chat', []))
    found = dict(re.findall(r'(\w+)=(\d+)', text))
    if not found:
        raise RuntimeError('no /ntmledger answer: ' + text)
    return {k: int(v) for k, v in found.items()}


def rod_entities(g):
    items = g.ctl('entities 90')['entities']
    return sum(e['item']['count'] for e in items if e.get('kind') == 'item' and e['item']['item'] == ROD)


def rods_in_machines(g):
    """Rods inside the pile (channels) and in the loaders, as the machines say."""
    channels = loaders = 0
    if block(g, CORE) == 'ntm:pile_block__core':
        core = machine(g, CORE)['data']['Core']
        channels = sum(1 for c in core['channels'] if c['kind'] == 'Fuel' for r in c['rods'] if r)
    for p in LOADERS:
        if block(g, p) == 'ntm:pile_device__loader':
            slots = machine(g, p)['data']['Loader']['slots']
            loaders += sum(s['count'] for s in slots if s)
    return channels, loaders


def inventory_rods(g):
    inv = g.ctl('inventory')
    stacks = [s for part in ('hotbar', 'main', 'inventory') for s in inv.get(part, []) or []]
    return sum(s.get('count', 0) for s in stacks if s and s.get('item') == ROD)


def count(g):
    channels, loaders = rods_in_machines(g)
    led = ledger(g)
    entities = rod_entities(g)
    return {'channels': channels, 'loaders': loaders, 'entities': entities, 'inventory': inventory_rods(g),
            'ledger': led}


def settled(g, timeout=90):
    """Nothing is owed and nothing changed for two readings."""
    deadline = time.monotonic() + timeout
    previous = None
    while time.monotonic() < deadline:
        now = count(g)
        key = (now['ledger']['pending'], now['entities'])
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
    g.command('fill 40 99 3 56 99 18 minecraft:stone')
    if VANILLA:
        # Near the pile, as the mod's world leaves its player, out of reach of the rods.
        g.command('tp @s 47.5 101 1.5')
        for x in (46.5, 48.5):
            g.command(f'summon minecraft:item {x} 103 13.5 {{Item:{{id:"{ROD}",count:1}}}}')
        time.sleep(2)
        g.evidence['base'] = count(g)
        g.ctl('save')
        return
    g.command('fill 45 100 8 49 104 12 ntm:pile_brick')
    tool(g)
    interact(g, CORE, 'north', (47.5, 101, 5.5))
    formed = machine(g, CORE)['data']['Core']
    g.check('formed_across_the_border', formed['shape']['at'] == list(CORE), formed['shape'])
    g.command('fill 46 104 6 48 104 7 minecraft:stone')
    for p in CHANNELS:
        interact(g, p, 'north', (p[0] + .5, 101, 5.5))
    for p in LOADERS:
        g.command('clear @s'); g.command('give @s ntm:pile_device__loader 1')
        g.command(f'tp @s {p[0] + .5} 101 {p[2] - 2.5}')
        g.ctl(f'place {p[0]} {p[1]} {p[2]} --item ntm:pile_device__loader')
    for p in LOADERS:
        for _ in range(5):
            g.command('clear @s'); g.command(f'give @s {ROD} 1'); g.ctl(f'select {ROD}')
            interact(g, p, 'north', (p[0] + .5, 101, p[2] - 2.5))
            interact(g, p, 'north')
            time.sleep(1.6)
    for x in (46.5, 48.5):
        g.command(f'summon minecraft:item {x} 103 13.5 {{Item:{{id:"{ROD}",count:1}}}}')
    time.sleep(2)
    base = count(g)
    g.check('ten_rods_in_the_pile', base['channels'] + base['loaders'] == PILE and base['entities'] == COMMON, base)
    g.command('clear @s')
    g.evidence['base'] = base
    g.ctl('save')


def sweep(g):
    """A big batch: the pile's core goes away and the orphan sweep reverts every other member in one batch.
    Counts the storage writes and bytes it costs (not one per effect)."""
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    before = ledger(g)
    g.command('setblock 47 102 8 minecraft:air')
    time.sleep(12)
    now = settled(g)
    after = now['ledger']
    cost = {'kv_writes': after['kv_writes'] - before['kv_writes'], 'kv_bytes': after['kv_bytes'] - before['kv_bytes']}
    g.evidence['sweep_cost'] = cost
    g.check('sweep_batch_writes_storage_a_bounded_number_of_times', cost['kv_writes'] <= 40, cost)
    g.check('sweep_keeps_every_rod', now['channels'] + now['loaders'] + now['entities'] + now['inventory'] == TOTAL, now)


def undo(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    before = count(g)
    if VANILLA:
        g.check('start_from_the_plain_rods', before['entities'] == COMMON, before)
        for x in LOOSE:
            for _ in range(PILE // 2):
                g.command(f'summon minecraft:item {x} 103 9.5 {{Item:{{id:"{ROD}",count:1}}}}')
        # The chunks the pile's save writes change too, so that every cut has its records.
        for x, z in ((40, 4), (50, 4), (50, 20)):
            g.command(f'setblock {x} 99 {z} minecraft:cobblestone')
    else:
        g.check('start_from_ten_in_the_pile', before['channels'] + before['loaders'] == PILE and before['entities'] == COMMON, before)
        tool(g)
        for p in LOADERS:
            interact(g, p, 'north', (p[0] + .5, 101, p[2] - 3.5))
    time.sleep(2)
    after = count(g)
    g.evidence['after_undo'] = after
    g.save()
    try:
        g.ctl('save')
        g.evidence['survived_save'] = True
    except Exception as error:  # the failpoint ends the process inside the save
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s 53.5 101 15.5')
    now = settled(g)
    held = now['channels'] + now['loaders'] + now['entities'] + now['inventory']
    now['total'] = held
    g.evidence['counted' + ('_again' if again else '')] = now
    led = now['ledger']
    # What the files held and the vanilla control at the same cut are the runner's to compare; never more than there was.
    g.check('no_rod_duplicated', held <= TOTAL, now)
    g.check('nothing_left_to_spawn', led['pending'] == 0, led)
    if VANILLA:
        return
    # The pile's cells: whole, or all reverted — never half.
    cells = [(x, y, z) for x in range(45, 50) for y in range(100, 105) for z in range(8, 13)]
    names = {block(g, c) for c in cells}
    parts = {n for n in names if n.startswith('ntm:pile_block__')}
    g.evidence['pile_blocks' + ('_again' if again else '')] = sorted(names)
    g.check('pile_is_one_state', not parts or block(g, CORE) == 'ntm:pile_block__core', sorted(names))


def drain(g):
    g.ctl('wait-ready 120')
    g.command('kill @e[type=minecraft:item]')
    time.sleep(1.5)
    led = ledger(g)
    g.check('nothing_owed_when_the_items_are_gone', led['pending'] == 0, led)
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'crash-' + mode)
    {'prepare': prepare, 'undo': undo, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain, 'sweep': sweep}[mode](g)
    g.evidence['ok'] = True
    g.save()

#!/usr/bin/env python3
"""T01 proof on a disposable world (`rbmk-plano` T01): fuel rods go from the player's hand into RBMK fuel channels on
both sides of a chunk border, each in one transfer with the hand, while a ReaSim source keeps the layer stepping (its
random and its queued streams kept by the server's storage) and a second layer stands in the Nether, parked during
the act. The runner (run-rbmk-t01-proof.py) kills the game at each boundary of the save that follows and counts what
the reopened world holds, against the vanilla control.

Modes:
  prepare  build both layers, the source and the Nether's two channels fuelled by hand, the player holding 4+2 rods
  act      put 4 rods by hand into the 4 empty channels (2 in chunk 2, 2 in chunk 3), then ask for a save
  verify   after a reopen: count every rod the world holds, and check that both layers step on
  again    verify once more after a second clean reopen
  drain    remove every item entity and check that nothing is owed

With NTM_CRASH_VANILLA=1 the same world is the vanilla control: barrels where the columns stand (adjacent chests would pair into double chests), the source's rod and the
Nether's two rods in barrels, and the act moves each rod from the hand into a barrel (`item replace` then `clear`, before
the save, as the transfer does). NTM_CRASH_NO_SAVE=1 and NTM_CRASH_PAUSE_SAVE=1 as in native_crash.py.
"""
import os
import re
import sys
import time
from native_game import Game
from native_pile import block

Y = 101
# Four empty channels across the border x=48, and the ReaSim source next to them.
EMPTY = [(46, Y, 8), (47, Y, 8), (48, Y, 8), (49, Y, 8)]
SOURCE = (47, Y, 9)
REFLECTOR = (48, Y, 9)
NETHER = 'minecraft:the_nether'
NY = 100
NETHER_RODS = [((0, NY, 0), 'rbmk_rod_reasim', 'ntm:rbmk_fuel_po210be'), ((1, NY, 0), 'rbmk_rod', 'ntm:rbmk_fuel_meu')]
FUEL = 'ntm:rbmk_fuel_ueu'
SOURCE_ROD = 'ntm:rbmk_fuel_po210be'
PREFIX = 'ntm:rbmk_fuel_'
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
IN_HAND = 4
SPARE = 2
TOTAL = IN_HAND + SPARE + 1 + len(NETHER_RODS)


def column(id_):
    return f'ntm:{id_}[facing=north,honey_level=0]'


def ledger(g):
    text = ' '.join(g.command('ntmledger').get('chat', []))
    found = dict(re.findall(r'(\w+)=(\d+)', text))
    if not found:
        raise RuntimeError('no /ntmledger answer: ' + text)
    return {k: int(v) for k, v in found.items()}


def slot_rods(g, p, dim='minecraft:overworld'):
    """Rods the container at `p` holds (an RBMK channel's slot, or the control's barrel)."""
    text = ' '.join(g.command(f'execute in {dim} run data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    # Each stack prints `{count: N, Slot: Sb, id: "..."...}`: its count before its id.
    return sum(int(n) for n, _ in re.findall(r'count: ?(\d+)[^{}]*?id: ?"(ntm:rbmk_fuel_\w+)"', text))


def inventory_rods(g):
    inv = g.ctl('inventory')
    stacks = [s for part in ('hotbar', 'main') for s in inv.get(part, []) or []] + [inv.get('offhand'), inv.get('carried')]
    return sum(s.get('count', 0) for s in stacks if isinstance(s, dict) and str(s.get('item') or '').startswith(PREFIX))


def entity_rods(g):
    items = g.ctl('entities 90')['entities']
    return sum(e['item']['count'] for e in items if e.get('kind') == 'item' and e['item']['item'].startswith(PREFIX))


def heat(g, p, dim='minecraft:overworld'):
    """The column's heat, from its model (the doubles are kept by their bits)."""
    text = ' '.join(g.command(f'execute in {dim} run data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))
    m = re.search(r'\\?"heat\\?":(\d+)', text)
    if not m:
        return None
    import struct
    return struct.unpack('<d', struct.pack('<Q', int(m[1])))[0]


def count(g):
    over = sum(slot_rods(g, p) for p in EMPTY + [SOURCE])
    nether = sum(slot_rods(g, p, NETHER) for p, _, _ in NETHER_RODS)
    return {'columns': over, 'nether': nether, 'entities': entity_rods(g), 'inventory': inventory_rods(g), 'ledger': ledger(g)}


def settled(g, timeout=90):
    deadline = time.monotonic() + timeout
    previous = None
    while time.monotonic() < deadline:
        now = count(g)
        key = (now['ledger']['pending'], now['entities'], now['columns'], now['nether'])
        if now['ledger']['pending'] == 0 and key == previous:
            return now
        previous = key
        time.sleep(2.5)
    return count(g)


def give(g, item, n):
    g.command(f'give @s {item} {n}')


def put_by_hand(g, p, item, dim='minecraft:overworld'):
    """One rod from the hand into the channel (NTM: the click; control: the barrel by command, then the hand)."""
    if VANILLA:
        g.command(f'execute in {dim} run item replace block {p[0]} {p[1]} {p[2]} container.0 with {item} 1')
        g.command(f'clear @s {item} 1')
        return
    # Standing in front of the column's open side: the source behind the row is clicked from the south.
    side, dz = ('south', 2.5) if p[2] == SOURCE[2] and dim == 'minecraft:overworld' else ('north', -2.5)
    g.command(f'execute in {dim} run tp @s {p[0] + .5} {p[1]} {p[2] + .5 + dz}')
    time.sleep(.3)
    g.ctl(f'select {item}')
    g.ctl(f'interact {p[0]} {p[1]} {p[2]} {side}')
    time.sleep(.4)


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    # Creative except for the act's transfers: the rods' radiation (a Po-210/Be source in hand) kills a survival player
    # in seconds, and a creative hand is not debited (`!isCreativeMode`): each copy left is cleared.
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 64 24')
    time.sleep(3)
    g.command('fill 40 99 3 56 108 18 minecraft:air')
    g.command('fill 40 100 3 56 100 18 minecraft:stone')
    g.command('clear @s')
    place = (lambda p, id_: g.command(f'setblock {p[0]} {p[1]} {p[2]} minecraft:barrel')) if VANILLA else \
        (lambda p, id_: g.command(f'setblock {p[0]} {p[1]} {p[2]} {column(id_)}'))
    for p in EMPTY:
        place(p, 'rbmk_rod_mod' if p[0] % 2 else 'rbmk_rod')
    place(SOURCE, 'rbmk_rod_reasim')
    if not VANILLA:
        g.command(f'setblock {REFLECTOR[0]} {REFLECTOR[1]} {REFLECTOR[2]} {column("rbmk_reflector")}')
    g.command('tp @s 47.5 101 4.5')
    time.sleep(2)
    give(g, SOURCE_ROD, 1)
    put_by_hand(g, SOURCE, SOURCE_ROD)
    g.command(f'clear @s {SOURCE_ROD}')
    # The Nether's layer: two channels fuelled by hand there, forceloaded, then left (parked) for the act.
    g.command('effect give @s minecraft:fire_resistance 600 0 true')
    g.command('effect give @s minecraft:resistance 600 4 true')
    g.command(f'execute in {NETHER} run forceload add -16 -16 16 16')
    g.command(f'execute in {NETHER} run tp @s 0.5 100 -2.5')
    for _ in range(60):
        if 'Successfully' in ' '.join(g.command(f'execute in {NETHER} run fill -4 98 -5 5 106 4 minecraft:air').get('chat', [])):
            break
        time.sleep(1)
    g.command(f'execute in {NETHER} run fill -4 99 -5 5 99 4 minecraft:netherrack')
    g.command(f'execute in {NETHER} run tp @s 0.5 100 -2.5')
    time.sleep(3)
    for p, id_, rod in NETHER_RODS:
        if VANILLA:
            g.command(f'execute in {NETHER} run setblock {p[0]} {p[1]} {p[2]} minecraft:barrel')
        else:
            g.command(f'execute in {NETHER} run setblock {p[0]} {p[1]} {p[2]} {column(id_)}')
    time.sleep(2)
    for p, _, rod in NETHER_RODS:
        give(g, rod, 1)
        put_by_hand(g, p, rod, NETHER)
        if VANILLA:
            give(g, rod, 1)
    g.check('creative_keeps_the_hand', inventory_rods(g) == len(NETHER_RODS), inventory_rods(g))
    for _, _, rod in NETHER_RODS:
        g.command(f'clear @s {rod}')
    time.sleep(3)
    if not VANILLA:
        g.check('nether_layer_steps', all((heat(g, p, NETHER) or 0) > 20 for p, _, _ in NETHER_RODS),
                [heat(g, p, NETHER) for p, _, _ in NETHER_RODS])
    g.command('execute in minecraft:overworld run tp @s 47.5 101 4.5')
    time.sleep(4)
    give(g, FUEL, IN_HAND)
    give(g, 'ntm:rbmk_fuel_lep', SPARE)
    time.sleep(2)
    base = count(g)
    g.check('base_rods', base['columns'] + base['nether'] + base['inventory'] + base['entities'] == TOTAL, base)
    if not VANILLA:
        g.check('source_heats_the_layer', (heat(g, SOURCE) or 0) > 20, heat(g, SOURCE))
    g.evidence['base'] = base
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    before = count(g)
    g.check('start_with_four_in_hand', before['inventory'] == IN_HAND + SPARE and before['columns'] == 1, before)
    # Survival for the transfers only: each takes the rod from the hand.
    g.command('gamemode survival @s')
    for p in EMPTY:
        put_by_hand(g, p, FUEL)
    g.command('gamemode creative @s')
    # Both chunks of the act change a block too, in both worlds: the engine's `item replace block` does not mark the
    # barrel's chunk changed, and a save would skip the control's records (its rods lost even without a cut).
    for x in (44, 50):
        g.command(f'setblock {x} 100 14 minecraft:cobblestone')
    # The parked Nether's chunk too: the mod's layer there keeps stepping (its chunk is written at every save) while
    # the control's barrels stay; both worlds write it, so a cut by record index lands on the same chunks.
    g.command(f'execute in {NETHER} run setblock 3 99 3 minecraft:cobblestone')
    time.sleep(1)
    after = count(g)
    g.evidence['after_act'] = after
    g.check('four_rods_moved_once', after['columns'] == 1 + IN_HAND and after['inventory'] == SPARE, after)
    if not VANILLA:
        g.evidence['heat_before_save'] = {str(p): heat(g, p) for p in EMPTY + [SOURCE]}
    g.save()
    if NO_SAVE:
        return
    if PAUSE_SAVE:
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
    except Exception as error:  # the failpoint ends the process inside the save
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s 47.5 101 4.5')
    now = settled(g)
    held = now['columns'] + now['nether'] + now['entities'] + now['inventory']
    now['total'] = held
    g.evidence['counted' + ('_again' if again else '')] = now
    # A cut may pair a player's file and regions of different saves, as vanilla's does (the moved rods in both the hand
    # and the channels, or in neither): the runner compares with the control at the same cut. Never more than that.
    g.check('no_more_than_the_moved_rods_twice', held <= TOTAL + IN_HAND, now)
    g.check('nothing_left_to_spawn', now['ledger']['pending'] == 0, now['ledger'])
    if VANILLA:
        return
    # The layer steps on from what the save kept: its columns heat, and keep stepping.
    first = heat(g, SOURCE)
    time.sleep(3)
    later = heat(g, SOURCE)
    g.evidence['source_heat' + ('_again' if again else '')] = [first, later]
    g.check('the_layer_steps_after_reopening', first is not None and later is not None and later != first, [first, later])
    for p in EMPTY + [SOURCE]:
        g.check(f'column_is_whole_{p[0]}_{p[2]}', block(g, p).startswith('ntm:rbmk_rod'), block(g, p))


def drain(g):
    g.ctl('wait-ready 120')
    g.command('kill @e[type=minecraft:item]')
    time.sleep(1.5)
    led = ledger(g)
    g.check('nothing_owed_when_the_items_are_gone', led['pending'] == 0, led)
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t01-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    g.evidence['ok'] = True
    g.save()

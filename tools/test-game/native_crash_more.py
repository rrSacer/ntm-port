#!/usr/bin/env python3
"""More ways to break an atomic save, one scenario per finding of the independent review (see run-crash-more.py).

Every scenario has three steps: `prepare` (a world saved cleanly), `act` (do the thing, then ask for a save that a
failpoint may end) and `verify` (after a reopen: wait for the world to settle, then count exactly). `again` is a
second reopen. Scenarios:
  coupled-a / coupled-b          a loader and its core in different chunks (core in the lower / the upper chunk),
                                 one rod pushed in: nothing is ejected, so no effect is owed
  coupled-ea / coupled-eb        the same with a full channel: the push ejects a rod
  crossing                       no crash: drops of a pile move into a chunk that then unloads
  crossing-kill                  the same, and the process dies while that chunk's entities are on disk only
  origin-pickup(-away)           a broken mixer's contents are picked up in another chunk while the mixer's chunk
                                 is unloaded (and, in -away, it stays unloaded until the save)
  origin-merge / origin-vanish   its fragments fold into one stack / are killed there instead
  origin-kill                    they reach another chunk that unloads, and the process dies before any save
  merge-stale                    a stack whose older copy stays in an unloaded file folds into another; the process
                                 dies, and the file comes back only after a confirmed save
  origin-away                    merge-stale reopened in that older holder, its copy picked up before the origin loads
                                 (R4-1 of the fourth review)
  pickup-merge                   with a file that lags them, one stack is picked up while two others fold
  save-quit                      the ejected rods survive a clean quit
  mining                         an excavator mines a chest of diamonds
  break / replace                a mixer with items is broken (and, in replace, another put in its place)

A player's break of a machine spills its contents as a vanilla chest does: the scenarios built on one (`BROKEN`) run
again with a chest in the mixers' place (NTM_CRASH_VANILLA=1), and the count must be the chest's at the same cut, not a
number (run-crash-more.py compares them). The pile's, the mining's and the ejection's items are effects of the mod
(`EMITTING`): their control is a vanilla dropper holding the same rods where the effect drops them, saved with the world
and fired by the action (a chest broken by hand in place of the mined one); after a death it comes back with the stock of
the last save, as the machine does. The runner checks that the reopens count what the files held, that nothing is held
twice, and that no more is lost than the control loses at the same cut. The transfers without an ejection are counted
exactly. `kill` is a death outside any save (NTM_CRASH_NO_SAVE=1).
"""
import json
import os
import re
import sys
import time
from native_game import Game
from native_pile import interact, machine, tool, block
from native_crash import ledger, dropper, fire, held

SCENARIO = os.environ.get('NTM_CRASH_SCENARIO', 'coupled-a')
# A chest in each mixer's place: the vanilla control of a scenario of `BROKEN`.
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
# `kv_committed`: the action saves by pausing the game, a save that does not wait; the process dies after its storage.
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
BROKEN = ('break', 'replace', 'furnace-break', 'iron-break', 'steel-break', 'combination-break', 'frozen-break', 'frozen-unload', 'origin-pickup', 'origin-pickup-away', 'origin-merge', 'origin-vanish', 'origin-kill',
          'merge-stale', 'origin-away', 'pickup-merge')
# The transfers between two chunks (Loader→Core), with or without the core's ejection: their control is a hopper across
# the border into a chest (and a dropper), counted equal to the control at each cut, as a break is.
TRANSFERS = ('coupled-a', 'coupled-b', 'coupled-ea', 'coupled-eb')
# The scenarios whose items an effect of the mod drops, with the items counted.
EMITTING = {'crossing': ('ntm:pile_rod',), 'coupled-break': ('ntm:pile_rod',),
            'crossing-kill': ('ntm:pile_rod',), 'save-quit': ('ntm:pile_rod',), 'mining': ('minecraft:diamond',)}
# The scenarios built on an entity file that lags its stacks.
STALE = ('merge-stale', 'origin-away')
ROD = 'ntm:pile_rod__ra226be'
INCOMING = 'ntm:pile_rod__zr'

# scenario -> (core x, which loader, a full channel first)
COUPLED = {'coupled-a': (47, 'R', False), 'coupled-b': (48, 'L', False),
           'coupled-ea': (47, 'R', True), 'coupled-eb': (48, 'L', True), 'coupled-break': (47, 'R', True)}


def entities(g, item, radius=150):
    items = g.ctl(f'entities {radius}')['entities']
    return sum(e['item']['count'] for e in items if e.get('kind') == 'item' and e['item']['item'] == item)


def inventory(g, item):
    inv = g.ctl('inventory')
    stacks = [s for part in ('hotbar', 'main') for s in inv.get(part, []) or []]
    return sum(s.get('count', 0) for s in stacks if s and s.get('item') == item)


def machine_present(g, p):
    text = ' '.join(g.command('ntmledger machine %d %d %d' % p).get('chat', []))
    return 'present' in text


def quiet(g, seconds=8):
    """Let the server tick: block entities are rewritten from the models and what an older record owes is dropped."""
    time.sleep(seconds)
    deadline = time.monotonic() + 60
    last = None
    while time.monotonic() < deadline:
        led = ledger(g)
        key = (led['pending'], led['applied'])
        if led['pending'] == 0 and key == last:
            return led
        last = key
        time.sleep(2.5)
    return ledger(g)


# --------------------------------------------------------------------------------------------- coupled / crossing

def geometry(cx):
    return {'core': (cx, 102, 8), 'L': ((cx - 1, 102, 7), (cx - 1, 102, 8)), 'R': ((cx + 1, 102, 7), (cx + 1, 102, 8))}


def build_pile(g, cx):
    g.close()
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 64 24')
    time.sleep(3)
    g.command('fill 40 99 3 56 108 18 minecraft:air')
    g.command('fill 40 99 3 56 99 18 minecraft:stone')
    g.command(f'fill {cx - 2} 100 8 {cx + 2} 104 12 ntm:pile_brick')
    tool(g)
    core = (cx, 102, 8)
    interact(g, core, 'north', (cx + .5, 101, 5.5))
    formed = machine(g, core)['data']['Core']
    g.check('formed', formed['shape']['at'] == list(core), formed['shape'])
    g.command(f'fill {cx - 1} 104 6 {cx + 1} 104 7 minecraft:stone')
    geo = geometry(cx)
    for side in ('L', 'R'):
        p = geo[side][1]
        interact(g, p, 'north', (p[0] + .5, 101, 5.5))
    for side in ('L', 'R'):
        p = geo[side][0]
        g.command('clear @s')
        g.command('give @s ntm:pile_device__loader 1')
        g.command(f'tp @s {p[0] + .5} 101 {p[2] - 2.5}')
        g.ctl(f'place {p[0]} {p[1]} {p[2]} --item ntm:pile_device__loader')
    return geo


def load_rod(g, loader):
    g.command('clear @s')
    g.command(f'give @s {ROD} 1')
    g.ctl(f'select {ROD}')
    interact(g, loader, 'north', (loader[0] + .5, 101, loader[2] - 2.5))
    interact(g, loader, 'north')
    time.sleep(1.8)


def pile_rods(g, cx):
    """Rods in the loaders and in the core's channels, as the block entities say."""
    geo = geometry(cx)
    channels = loaders = 0
    if block(g, geo['core']) == 'ntm:pile_block__core':
        core = machine(g, geo['core'])['data']['Core']
        channels = sum(1 for c in core['channels'] if c['kind'] == 'Fuel' for r in c['rods'] if r)
    for side in ('L', 'R'):
        p = geo[side][0]
        if block(g, p) == 'ntm:pile_device__loader':
            loaders += sum(s['count'] for s in machine(g, p)['data']['Loader']['slots'] if s)
    # The control's containers stand where the core and the loaders do, and beside the core (its hopper, its dropper).
    spots = {geo['core'], geo['L'][0], geo['R'][0], hopper_at(cx), shooter_at(cx)}
    loaders += sum(held(g, p, '') for p in spots)
    return channels, loaders


def hopper_at(cx):
    """The control's hopper: beside the core across the chunk border, where the loader feeds it."""
    side = 'R' if COUPLED.get(SCENARIO, (47, 'R'))[1] == 'R' else 'L'
    return (cx + 1 if side == 'R' else cx - 1, 102, 8)


def shooter_at(cx):
    """The control's dropper for the core's ejection, in the core's chunk. In `coupled-break` it shoots into (3,0), as
    the core's channel at x=48 ejects: that chunk's unload writes the rod in both worlds alike."""
    return (cx, 102, 12) if SCENARIO == 'coupled-break' else (cx, 102, 10)


def shooter_facing():
    return 'east' if SCENARIO == 'coupled-break' else 'south'


def touch(g, *cells):
    """The control changes a block in each chunk the effect's save would write, so that the cut has its records."""
    for x, z in cells:
        g.command(f'setblock {x} 99 {z} minecraft:cobblestone')


def coupled_prepare(g):
    cx, side, full = COUPLED[SCENARIO]
    if VANILLA:
        # A hopper (locked by a redstone block under it) holds the rod the loader pushes, across the chunk border from a
        # chest where the core stands; with a full channel the chest holds the four the core keeps and a dropper the rod
        # it ejects.
        g.close()
        g.command('gamemode creative @s')
        g.command('forceload add 30 0 64 24')
        time.sleep(3)
        g.command('fill 40 99 3 56 108 18 minecraft:air')
        g.command('fill 40 99 3 56 99 18 minecraft:stone')
        hx = hopper_at(cx)
        facing = 'west' if hx[0] > cx else 'east'
        g.command('setblock %d %d %d minecraft:redstone_block' % (hx[0], hx[1] - 1, hx[2]))
        g.command('setblock %d %d %d minecraft:hopper[facing=%s]' % (*hx, facing))
        g.command('item replace block %d %d %d container.0 with %s 1' % (*hx, INCOMING if full else ROD))
        g.command('setblock %d %d %d minecraft:chest' % geometry(cx)['core'])
        if full:
            g.command('item replace block %d %d %d container.0 with %s 4' % (*geometry(cx)['core'], ROD))
            dropper(g, shooter_at(cx), ROD, 1, shooter_facing())
        g.ctl('save')
        return
    geo = build_pile(g, cx)
    loader = geo[side][0]
    if full:
        for _ in range(5):
            load_rod(g, loader)
    # Persist the rod in the loader first. The crash must cut a transfer of an already saved item,
    # not the preceding player insertion (whose playerdata has its own save boundary).
    g.command('clear @s')
    # A different kind makes a full-channel shift observable even when its total stays five.
    rod = INCOMING if full else ROD
    g.command(f'give @s {rod} 1')
    g.ctl(f'select {rod}')
    interact(g, loader, 'north', (loader[0] + .5, 101, loader[2] - 2.5))
    channels, loaders = pile_rods(g, cx)
    g.check('prepared', channels == (5 if full else 0) and loaders == 1, (channels, loaders))
    g.command('clear @s')
    g.ctl('save')


def coupled_act(g):
    coupled_transfer(g)
    save_expecting_death(g)


def coupled_transfer(g):
    """The loader pushes its rod into the core (the control's hopper into its chest); with a full channel the core
    ejects one (the control's dropper fires)."""
    cx, side, full = COUPLED[SCENARIO]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    if VANILLA:
        # The hopper is let go and pushes its rod into the chest (the loader into the core); with a full channel the
        # dropper fires the rod the core ejects. The loader's and the core's chunks changed.
        hx = hopper_at(cx)
        g.command('setblock %d %d %d minecraft:stone' % (hx[0], hx[1] - 1, hx[2]))
        deadline = time.monotonic() + 10
        while held(g, hx, '') and time.monotonic() < deadline:
            time.sleep(.2)
        if full:
            fire(g, shooter_at(cx), 1)
        touch(g, (40, 4), (50, 4))
        time.sleep(1.8)
        g.evidence['after_act'] = {'rods': pile_rods(g, cx), 'entities': entities(g, ROD)}
        g.check('transfer_completed_before_the_cut', held(g, hx, '') == 0, g.evidence['after_act'])
        return
    loader = geometry(cx)[side][0]
    g.command('clear @s')
    g.check('saved_rod_starts_in_the_loader', pile_rods(g, cx) == (5 if full else 0, 1), pile_rods(g, cx))
    interact(g, loader, 'north', (loader[0] + .5, 101, loader[2] - 2.5))
    time.sleep(1.8)
    deadline = time.monotonic() + 30
    expected = (5 if full else 1, 0)
    while pile_rods(g, cx) != expected and time.monotonic() < deadline:
        time.sleep(.2)
    g.evidence['after_act'] = {'rods': pile_rods(g, cx), 'entities': entities(g, ROD) + entities(g, INCOMING)}
    g.check('transfer_completed_before_the_cut', tuple(g.evidence['after_act']['rods']) == expected, g.evidence['after_act'])


def coupled_verify(g, again=False):
    cx, side, full = COUPLED[SCENARIO]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s 53.5 101 15.5')
    led = quiet(g)
    channels, loaders = pile_rods(g, cx)
    outside = entities(g, ROD) + entities(g, INCOMING)
    carried = inventory(g, ROD) + inventory(g, INCOMING)
    held = channels + loaders + outside + carried
    expected = 6 if full else 1
    detail = {'channels': channels, 'loaders': loaders, 'entities': outside, 'inventory': carried, 'total': held, 'expected': expected, 'ledger': led}
    counted(g, 'no_rod_lost_or_duplicated', detail, again)
    if VANILLA:
        return
    core = geometry(cx)['core']
    g.check('the_core_is_alive', machine_present(g, core), 'core machine missing')


def is_loaded(g, p):
    lines = g.command('execute if loaded %d %d %d run say NTM_LOADED' % tuple(p)).get('chat', [])
    return any('NTM_LOADED' in s for s in lines)


def coupled_break_act(g):
    """R6-2 of the sixth review: the transfer of `coupled-ea` (a Zr into a full channel, a Ra-226Be out), then the core
    broken while the loader's chunk (3,0) is out and the core's (2,0) in; then the save. No chunk is forced: a forced
    chunk keeps its neighbours loaded, so the player walks west until (3,0) leaves the loaded area and (2,0) is its edge."""
    cx, side, full = COUPLED[SCENARIO]
    core, member = geometry(cx)['core'], (49, 102, 10)
    coupled_transfer(g)
    # The rod the core ejected (the control's dropper fired) lies in (3,0) in both worlds: an item's flight could take
    # it across the border, into the other chunk's entities.
    g.command('tp @e[type=minecraft:item] 52.5 100.5 12.5')
    time.sleep(2)
    g.command('forceload remove all')
    found = None
    for px in range(-8, -200, -16):
        g.command(f'tp @s {px + .5} 101 8.5')
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline and is_loaded(g, member):
            time.sleep(1)
        if not is_loaded(g, member):
            found = px if is_loaded(g, core) else None
            break
    g.evidence['player_x'] = found
    g.check('member_chunk_out_core_chunk_in', found is not None and is_loaded(g, core) and not is_loaded(g, member), found)
    g.command('setblock %d %d %d minecraft:air%s' % (*core, ' destroy' if VANILLA else ''))
    time.sleep(3)
    g.evidence['after_break'] = {'ledger': ledger(g), 'core': block(g, core)}
    if not VANILLA:
        g.check('the_core_is_gone', not machine_present(g, core), g.evidence['after_break'])
    save_expecting_death(g)


def coupled_break_verify(g, again=False):
    cx, side, full = COUPLED[SCENARIO]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 64 24')
    g.command('tp @s 53.5 101 15.5')
    led = quiet(g, 12)
    channels, loaders = pile_rods(g, cx)
    outside = entities(g, ROD) + entities(g, INCOMING)
    carried = inventory(g, ROD) + inventory(g, INCOMING)
    detail = {'channels': channels, 'loaders': loaders, 'entities': outside, 'inventory': carried,
              'total': channels + loaders + outside + carried, 'ledger': led}
    counted(g, 'no_rod_lost_or_duplicated', detail, again)


# -------------------------------------------------------------------------------------------------------- crossing

def crossing_prepare(g):
    # The pile of the main scenario, its drops in the lower chunk and the upper one.
    cx = 47
    if VANILLA:
        g.close()
        g.command('gamemode creative @s')
        g.command('forceload add 30 0 64 24')
        time.sleep(3)
        g.command('fill 40 99 3 56 108 18 minecraft:air')
        g.command('fill 40 99 3 56 99 18 minecraft:stone')
        # Near the pile, out of reach of what the droppers fire.
        g.command('tp @s 47.5 101 -8.5')
        for side in ('L', 'R'):
            dropper(g, geometry(cx)[side][0], ROD, 5)
    else:
        geo = build_pile(g, cx)
        for side in ('L', 'R'):
            for _ in range(5):
                load_rod(g, geo[side][0])
    # Two plain rods beside the pile's. Not in crossing-kill: moved and then killed before any save, a plain item
    # is in both entity files, as in vanilla; only the ledger's rods are this scenario's to count.
    for x in (46.5, 48.5) if SCENARIO != 'crossing-kill' else ():
        g.command(f'summon minecraft:item {x} 103 13.5 {{Item:{{id:"{ROD}",count:1}}}}')
    if SCENARIO in ('crossing', 'crossing-kill'):
        # Forced tickets keep two full neighbour rings. An adjacent destination cannot unload while the
        # source is forced, so put the destination beyond that halo and prove its unload explicitly.
        g.command('forceload add 128 0 143 15')
        time.sleep(3)
        g.command('fill 128 99 0 143 99 15 minecraft:stone')
    elif SCENARIO == 'save-quit':
        # Nether ticks last: reconciliation must also visit the parked Overworld, where these drops live.
        g.command('execute in minecraft:the_nether run forceload add 0 0')
    g.command('clear @s')
    g.ctl('save')


def eject(g):
    """The pile gives up both channels through its loaders; in the control, the dropper at each loader fires its five."""
    for side in ('L', 'R'):
        p = geometry(47)[side][0]
        if VANILLA:
            fire(g, p, 5)
        else:
            interact(g, p, 'north', (p[0] + .5, 101, p[2] - 3.5))


def crossing_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    tool(g)
    eject(g)
    led = quiet(g, 4)
    g.evidence['after_undo'] = {'entities': entities(g, ROD), 'ledger': led}
    # Every drop goes to the upper chunk; then that chunk is let go while the lower one stays.
    g.command('tp @e[type=minecraft:item] 133.5 103 13.5')
    time.sleep(3)
    g.command('forceload remove 48 0 64 24')
    g.command('forceload remove 128 0 143 15')
    g.command('tp @s -250.5 101 5.5')
    # A reopen can deduplicate the two copies again and hide the in-game failure. Prove the actual unload
    # and inspect the source while the destination's entities are still on disk.
    deadline = time.monotonic() + 270
    while time.monotonic() < deadline:
        loaded = g.command('execute if loaded 133 99 13 run say NTM_DESTINATION_LOADED')
        if not any('NTM_DESTINATION_LOADED' in line for line in loaded.get('chat', [])):
            break
        time.sleep(1)
    g.check('destination_really_unloaded', not any('NTM_DESTINATION_LOADED' in line for line in loaded.get('chat', [])), loaded)
    source = g.command('execute if loaded 46 99 13 run say NTM_SOURCE_LOADED')
    g.check('source_stays_loaded', any('NTM_SOURCE_LOADED' in line for line in source.get('chat', [])), source)
    time.sleep(2)
    g.evidence['while_away'] = ledger(g)
    copies = g.command('execute as @e[type=minecraft:item] run data get entity @s Item')
    g.check('no_copy_while_destination_is_unloaded', not any(ROD in line for line in copies.get('chat', [])), copies)
    if SCENARIO == 'crossing-kill':
        # The destination's entities went to the writer with its unload; let it write them, then die there.
        g.command('forceload add 30 0 64 24')
        time.sleep(4)
        g.save()
        return
    g.command('forceload add 30 0 64 24')
    g.command('forceload add 128 0 143 15')
    g.command('tp @s 133.5 101 15.5')


def crossing_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    if SCENARIO == 'crossing-kill' and not again:
        # Far from both, as in the action: the forced source stays, the destination can go.
        holder_away_then_save(g, ROD, (133, 99, 13), (-250.5, 101, 5.5), 'forceload remove 48 0 64 24', 'forceload remove 128 0 143 15')
    # The pile's area and the destination, both loaded to count (the action let them go).
    g.command('forceload add 30 0 64 24')
    g.command('forceload add 128 0 143 15')
    # Counted where each part is, with the player there (a block or an entity is seen only near the player): at the pile,
    # its rods (or the control's droppers') and what lies around it; at the destination, what lies there.
    g.command('tp @s 53.5 101 15.5')
    led = quiet(g, 10)
    machines = sum(pile_rods(g, 47))
    at_pile = entities(g, ROD, 40)
    at_destination = 0
    if SCENARIO in ('crossing', 'crossing-kill'):
        g.command('tp @s 133.5 101 15.5')
        time.sleep(4)
        at_destination = entities(g, ROD, 40)
    carried = inventory(g, ROD)
    held = machines + at_pile + at_destination + carried
    expected = 10 if SCENARIO == 'crossing-kill' else 12
    detail = {'machines': machines, 'entities': at_pile + at_destination, 'at_pile': at_pile, 'at_destination': at_destination,
              'inventory': carried, 'total': held, 'expected': expected, 'ledger': led}
    counted(g, 'no_rod_duplicated_by_a_chunk_that_unloaded', detail, again)


# ------------------------------------------------------------------------------------------------------ save & quit


def save_quit_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    tool(g)
    eject(g)
    led = quiet(g, 4)
    g.evidence['after_act'] = {'entities': entities(g, ROD), 'ledger': led}
    g.evidence['after_act']['inventory'] = inventory(g, ROD)
    g.check('rods_ejected_before_quit', entities(g, ROD) + inventory(g, ROD) == 12 and sum(pile_rods(g, 47)) == 0, g.evidence['after_act'])
    # The runner's quit is the first confirmed save after the undo.


def save_quit_verify(g, again=False):
    crossing_verify(g, again)
    led = ledger(g)
    g.check('save_and_quit_leaves_nothing_owed', led.get('pending', -1) == 0, led)


# ---------------------------------------------------------------------------------------------------- frozen break
# N1 of the fresh review: the core holding ten rods is broken by a command with the ticks frozen, and the save comes
# with no tick between: the save takes the chunks only once the rods are out. The control is a chest in the core's place
# holding the same ten; two loose rods lie beside both.

def frozen_break_prepare(g):
    cx = 47
    core = geometry(cx)['core']
    if VANILLA:
        g.close()
        g.command('gamemode creative @s')
        g.command('forceload add 30 0 64 24')
        time.sleep(3)
        g.command('fill 40 99 3 56 108 18 minecraft:air')
        g.command('fill 40 99 3 56 99 18 minecraft:stone')
        g.command('setblock %d %d %d minecraft:chest' % core)
        for slot in range(10):
            g.command('item replace block %d %d %d container.%d with %s 1' % (*core, slot, ROD))
    else:
        geo = build_pile(g, cx)
        for side in ('L', 'R'):
            for _ in range(5):
                load_rod(g, geo[side][0])
    for x in (46.5, 48.5):
        g.command(f'summon minecraft:item {x} 103 13.5 {{Item:{{id:"{ROD}",count:1}}}}')
    g.command('clear @s')
    g.command('tp @s 53.5 101 15.5')
    time.sleep(2)
    channels, loaders = pile_rods(g, cx)
    g.check('ten_rods_in_the_core_or_chest', channels + loaders == 10, {'channels': channels, 'containers': loaders})
    g.ctl('save')


def frozen_break_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    core = geometry(47)['core']
    # No tick between the break and the save: the mod sees the break only in the save's hook before its capture.
    g.command('tick freeze')
    g.command('setblock %d %d %d minecraft:air destroy' % core)
    g.evidence['after_act'] = {'block': block(g, core), 'entities': entities(g, ROD), 'ledger': ledger(g)}
    save_expecting_death(g)


def frozen_unload_act(g):
    """R7-2: the break is not taken by a tick (frozen), and the core's chunk leaves: the mod takes it in the unload hook,
    with the chunk still in place, so the rods leave with that chunk's entities as the chest's spilled items do."""
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    core = geometry(47)['core']
    g.command('tick freeze')
    g.command('setblock %d %d %d minecraft:air destroy' % core)
    g.command('forceload remove 30 0 64 24')
    g.command('tp @s -250.5 101 5.5')
    g.check('core_chunk_unloaded', unloaded(g, core), core)
    time.sleep(4)
    g.evidence['after_act'] = {'ledger': ledger(g)}
    save_expecting_death(g)


def frozen_break_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('tick unfreeze')
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 64 24')
    g.command('tp @s 53.5 101 15.5')
    led = quiet(g, 10)
    machines = sum(pile_rods(g, 47))
    loose = entities(g, ROD, 40)
    carried = inventory(g, ROD)
    detail = {'machines': machines, 'entities': loose, 'inventory': carried, 'total': machines + loose + carried,
              'ledger': led}
    counted(g, 'no_rod_lost_or_duplicated', detail, again)


# ---------------------------------------------------------------------------------------------------------- mining

MINE = (20, 100, 8)
DIAMONDS = 7


def chest_count(g, p):
    text = ' '.join(g.command('data get block %d %d %d Items' % p).get('chat', []))
    if 'minecraft:diamond' not in text:
        return 0
    return sum(int(n) for n in re.findall(r'(?:count|Count): ?(\d+)', text))


def mining_prepare(g):
    from native_mining import place_machine
    g.ctl('wait-ready 120')
    g.close()
    x, y, z = MINE
    g.ground(x - 4, y, z - 4, width=12, height=18)
    # The control: the excavator's core where it would stand, as a plain block; the chest it mines, broken by hand.
    core = (x, y, z) if VANILLA else place_machine(g, 'machine_excavator', x, y, z)
    cx, cy, cz = core
    g.command(f'fill {cx - 1} {cy - 4} {cz - 1} {cx + 1} {cy - 4} {cz + 1} minecraft:iron_ore')
    chest = (cx + 1, cy - 4, cz)
    g.command('setblock %d %d %d minecraft:chest' % chest)
    g.command('item replace block %d %d %d container.0 with minecraft:diamond %d' % (chest + (DIAMONDS,)))
    g.evidence['core'] = core
    g.evidence['chest'] = chest
    g.check('chest_has_the_diamonds', chest_count(g, chest) == DIAMONDS, chest_count(g, chest))
    g.ctl('save')


def mining_act(g):
    from native_mining import open_excavator
    from native_recipes import screen_json
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core, chest = prep['core'], tuple(prep['chest'])
    if VANILLA:
        g.command('setblock %d %d %d minecraft:air destroy' % chest)
        touch(g, (chest[0] - 2, chest[2] - 2), (chest[0] + 18, chest[2]), (chest[0] - 18, chest[2]))
        time.sleep(1.5)
        g.evidence['chest_gone'] = g.ctl('block %d %d %d' % chest).get('state', '')
        save_expecting_death(g)
        return
    open_excavator(g, core)
    g.put('ntm:battery_creative', 1, 0)
    g.put('ntm:drillbit__ferro', 1, 4)
    screen = screen_json('machine_excavator')
    button = next(b for b in screen['buttons'] if b['id'] == 0)
    g.button(button['rect'], screen['size'])
    time.sleep(.6)
    g.close()
    deadline = time.monotonic() + 450
    while time.monotonic() < deadline:
        state = g.ctl('block %d %d %d' % chest).get('state', '')
        if state.startswith('minecraft:air'):
            break
        time.sleep(.2)
    g.evidence['chest_gone'] = state
    save_expecting_death(g)


def mining_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core, chest = tuple(prep['core']), tuple(prep['chest'])
    g.command('tp @s %d %d %d' % (core[0] + 3, core[1], core[2] + 6))
    led = quiet(g, 14)
    total = chest_count(g, chest) + entities(g, 'minecraft:diamond') + inventory(g, 'minecraft:diamond')
    detail = {'chest': chest_count(g, chest), 'entities': entities(g, 'minecraft:diamond'), 'inventory': inventory(g, 'minecraft:diamond'), 'total': total, 'expected': DIAMONDS, 'ledger': led,
              'machine': machine_present(g, core)}
    counted(g, 'diamonds_exactly_once', detail, again)
    if not VANILLA:
        g.check('the_excavator_is_alive', machine_present(g, core), detail)


# ------------------------------------------------------------------------------------------------- break / replace

MIXER = (140, 100, 8)
# origin-merge's second mixer, in the same chunk and far enough not to fold with the first.
MIXER2 = (134, 100, 8)
MIXER3 = (128, 100, 8)
NITER = int(os.environ.get('NTM_CRASH_NITER', '16'))
# `furnace-break`: the broken machine is the brick furnace (its items are the engine's slots), against the chest control.
FURNACE = SCENARIO == 'furnace-break'
MACHINE = 'ntm:machine_furnace_brick_off' if FURNACE else 'ntm:machine_mixer'
# The mixers of a scenario (all in chunk (8,0): one ledger) and the niter each holds.
MIXERS = {'origin-merge': [(MIXER2, NITER // 2), (MIXER, NITER // 2)],
          # The smaller folds into the larger: the stack whose older copy stays in B is the one that goes.
          'merge-stale': [(MIXER2, 16), (MIXER, 48)],
          'origin-away': [(MIXER2, 16), (MIXER, 48)],
          'pickup-merge': [(MIXER3, 16), (MIXER2, 24), (MIXER, 40)]}
# Two far destinations: forced tickets keep two rings of neighbours, these unload on their own.
DEST_B, DEST_C = (503.5, 100, 8.5), (803.5, 100, 8.5)
FORCE_B, FORCE_C = '496 0 511 15', '800 0 815 15'


def mixer_items(g, p):
    text = ' '.join(g.command('data get block %d %d %d Items' % p).get('chat', []))
    return sum(int(n) for n in re.findall(r'(?:count|Count): ?(\d+)', text)) if 'niter' in text else 0


def break_prepare(g):
    from native_stations import open_machine
    from native_recipes import core_of
    g.ctl('wait-ready 120')
    g.close()
    cores = []
    mixers = MIXERS.get(SCENARIO, [(MIXER, NITER)])
    # The ground of all of them first: preparing one's ground clears what stands beside it.
    for (x, y, z), _ in mixers:
        g.ground(x - 3, y, z, width=8)
    for (x, y, z), each in mixers:
        g.command('clear @s')
        if VANILLA:
            g.command(f'setblock {x} {y} {z} minecraft:chest')
            g.command(f'item replace block {x} {y} {z} container.0 with ntm:niter {each}')
            core = (x, y, z)
        else:
            g.command(f'give @s {MACHINE} 1')
            if SCENARIO.startswith('origin') or SCENARIO in MIXERS:
                g.command(f'tp @s {x + .5} {y} {z + 3.5}')
            g.ctl(f'place {x} {y} {z} --item {MACHINE}')
            g.command('clear @s')
            open_machine(g, x, y, z)
            # The brick furnace is one block: its core is where it was put; the input is its slot 0.
            core = (x, y, z) if FURNACE else core_of(g, 'ntm:machine_mixer', x, y, z)
            g.put('ntm:niter', each, 0 if FURNACE else 1)
            g.close()
        g.check('niter_inside', mixer_items(g, core) == each, mixer_items(g, core))
        cores.append(core)
    g.evidence['core'] = cores[-1]
    g.evidence['cores'] = cores
    g.ctl('save')
    if SCENARIO == 'origin-away':
        # The base is saved with the player at B and the origin unloaded: the reopen after the death starts there.
        g.command('forceload remove all')
        g.command('gamemode creative @s')
        g.command('tp @s 503.5 102 13.5')
        g.check('source_unloaded_before_base_save', unloaded(g, (140, 100, 8)), 'A')
        g.ctl('save')



def vanilla_break_prepare(g):
    g.ctl('wait-ready 120');g.close()
    x,y,z=MIXER;g.ground(x-3,y,z,width=8)
    g.command('clear @s')
    g.command(f'setblock {x} {y} {z} minecraft:chest')
    g.command(f'item replace block {x} {y} {z} container.0 with ntm:niter {NITER}')
    g.evidence['core']=list(MIXER)
    g.check('vanilla_contents_before_break',mixer_items(g,MIXER)==NITER,mixer_items(g,MIXER))
    g.ctl('save')


def break_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode survival @s')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core = tuple(prep['core'])
    g.command('tp @s %d %d %d' % (core[0], core[1], core[2] + 4))
    g.command('give @s minecraft:diamond_pickaxe 1')
    g.ctl('select minecraft:diamond_pickaxe')
    g.ctl('break %d %d %d --timeout 30' % core)
    time.sleep(1.5)
    if SCENARIO == 'replace':
        g.command('clear @s')
        g.command('gamemode creative @s')
        g.command('tp @s %d %d %d' % (core[0], core[1], core[2] + 3))
        time.sleep(.5)
        if VANILLA:
            g.command('setblock %d %d %d minecraft:chest' % core)
        else:
            g.command('give @s ntm:machine_mixer 1')
            g.ctl('place %d %d %d --item ntm:machine_mixer' % core)
        time.sleep(1)
    g.evidence['after_act'] = {'block': g.ctl('block %d %d %d' % core).get('state', ''),
                               'entities': entities(g, 'ntm:niter'), 'inventory': inventory(g, 'ntm:niter'),
                               'ledger': ledger(g)}
    save_expecting_death(g)


def break_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core = tuple(prep['core'])
    g.command('tp @s %d %d %d' % (core[0] + 3, core[1], core[2] + 6))
    led = quiet(g, 12)
    state = g.ctl('block %d %d %d' % core).get('state', '')
    inside = mixer_items(g, core) if state.startswith(('ntm:machine_mixer', 'ntm:machine_furnace_brick', 'minecraft:chest')) else 0
    total = inside + entities(g, 'ntm:niter') + inventory(g, 'ntm:niter')
    detail = {'block': state, 'inside': inside, 'entities': entities(g, 'ntm:niter'), 'inventory': inventory(g, 'ntm:niter'), 'total': total, 'expected': NITER, 'ledger': led,
              'machine': machine_present(g, core)}
    counted(g, 'niter_exactly_once', detail, again)
    if state.startswith(('ntm:machine_mixer', 'ntm:machine_furnace_brick')):
        g.check('a_standing_mixer_is_a_live_machine', machine_present(g, core), detail)


def inside(g, cores):
    """What the mixers (or the control's chests) standing at `cores` hold."""
    total = 0
    for c in cores:
        if g.ctl('block %d %d %d' % tuple(c)).get('state', '').startswith(('ntm:machine_mixer', 'minecraft:chest')):
            total += mixer_items(g, tuple(c))
    return total


def world_items(g, item):
    lines = g.command('execute as @e[type=minecraft:item] run data get entity @s Item').get('chat', [])
    return sum(int(n) for line in lines if item in line for n in re.findall(r'(?:count|Count): ?(\d+)', line))


def unloaded(g, p):
    deadline = time.monotonic() + 270
    while True:
        loaded = g.command('execute if loaded %d %d %d run say NTM_CHUNK_LOADED' % tuple(p))
        here = any('NTM_CHUNK_LOADED' in s for s in loaded.get('chat', []))
        if not here or time.monotonic() > deadline:
            return not here
        time.sleep(1)


def holder_away_then_save(g, item, holder, stay, *release):
    """After a reopen: the chunk whose entity file holds the stack stays unloaded while the world settles, a confirmed
    save follows, and only then does the chunk come back. A stack spawned again meanwhile would be released by that
    save and meet its older copy as a second stack."""
    for line in release:
        g.command(line)
    g.command('tp @s %s %s %s' % stay)
    g.check('holder_chunk_unloaded_on_reopen', unloaded(g, holder), holder)
    led = quiet(g, 8)
    seen = {'entities': world_items(g, item), 'inventory': inventory(g, item), 'ledger': led}
    g.evidence['holder_away'] = seen
    g.check('nothing_spawned_while_its_holder_is_away', seen['entities'] == 0, seen)
    g.ctl('save')


def item_stacks(g, item):
    """How many loaded item entities hold `item`, in every loaded chunk."""
    lines = g.command('execute as @e[type=minecraft:item] run data get entity @s Item').get('chat', [])
    return sum(1 for line in lines if item in line)


def origin_kill_act(g):
    g.ctl('wait-ready 120')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core = tuple(prep['core'])
    g.command('gamemode survival @s')
    g.command('tp @s %d %d %d' % (core[0], core[1], core[2] + 4))
    g.command('give @s minecraft:diamond_pickaxe 1')
    g.ctl('select minecraft:diamond_pickaxe')
    g.ctl('break %d %d %d --timeout 30' % core)
    time.sleep(1)
    g.check('broken_contents_dropped', world_items(g, 'ntm:niter') == NITER, world_items(g, 'ntm:niter'))
    g.command('gamemode creative @s')
    g.command('forceload add 496 0 511 15')
    time.sleep(2)
    g.command('fill 496 99 0 511 99 15 minecraft:stone')
    g.command('tp @e[type=minecraft:item] 503.5 100 8.5')
    time.sleep(2)
    # The source stays (the player is there); the destination is let go and writes its entities.
    g.command('forceload remove 496 0 511 15')
    g.command('tp @s %d %d %d' % (core[0] + 3, core[1], core[2] + 6))
    g.check('destination_really_unloaded', unloaded(g, (503, 99, 8)), 'destination')
    time.sleep(4)
    g.evidence['after_act'] = {'block': g.ctl('block %d %d %d' % core).get('state', ''), 'entities': world_items(g, 'ntm:niter'),
                               'inventory': inventory(g, 'ntm:niter'), 'ledger': ledger(g)}
    g.save()


def origin_kill_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core = tuple(prep['core'])
    if not again:
        holder_away_then_save(g, 'ntm:niter', (503, 99, 8), (core[0] + 3, core[1], core[2] + 6), 'forceload remove 496 0 511 15')
    g.command('forceload add 496 0 511 15')
    g.command('forceload add %d %d' % (core[0], core[2]))
    led = quiet(g, 10)
    state = g.ctl('block %d %d %d' % core).get('state', '')
    detail = {'block': state, 'inside': inside(g, [core]), 'entities': world_items(g, 'ntm:niter'), 'inventory': inventory(g, 'ntm:niter'), 'ledger': led}
    detail['total'] = detail['inside'] + detail['entities'] + detail['inventory']
    counted(g, 'niter_exactly_once', detail, again)


def take_to(g, c, spot):
    """The drops of the mixer whose core is `c` to `spot`."""
    g.command('tp @e[type=minecraft:item,x=%d,y=%d,z=%d,distance=..5] %s %s %s' % (c[0], c[1], c[2], *spot))


def away_and_back(g, force, at):
    """The chunk is let go (its entities go to its file), then forced back: that file now lags its entities."""
    g.command('forceload remove ' + force)
    g.check('holder_unloaded', unloaded(g, at), at)
    time.sleep(4)
    g.command('forceload add ' + force)
    time.sleep(6)


def stale_act(g):
    """merge-stale: X was in B, whose file keeps it, then folds into Y in C. pickup-merge: N, D1 and D2 were in a
    file that lags them; then, in one stepped stretch, N is picked up and D1 folds into D2. Both die with no save."""
    g.ctl('wait-ready 120')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    cores = prep['cores']
    g.command('gamemode survival @s')
    g.command('give @s minecraft:diamond_pickaxe 1')
    g.ctl('select minecraft:diamond_pickaxe')
    for force in (FORCE_B, FORCE_C):
        g.command('forceload add ' + force)
    time.sleep(2)
    g.command('fill 496 99 0 511 99 15 minecraft:stone')
    g.command('fill 800 99 0 815 99 15 minecraft:stone')
    if SCENARIO in STALE:
        spots = [DEST_B, (DEST_C[0], 100, 3.5)]
    else:
        spots = [(DEST_C[0], 100, 3.5), (DEST_C[0] + 6, 100, 3.5), (DEST_C[0] + 6, 100, 12.5)]
    for c, spot in zip(cores, spots):
        g.command('tp @s %d %d %d' % (c[0], c[1], c[2] + 4))
        g.ctl('break %d %d %d --timeout 30' % tuple(c))
        take_to(g, c, spot)
        time.sleep(.5)
    g.command('gamemode creative @s')
    g.command('tp @s %d %d %d' % (cores[-1][0] + 3, cores[-1][1], cores[-1][2] + 6))
    each = sum(n for _, n in MIXERS[SCENARIO])
    g.check('broken_contents_dropped', world_items(g, 'ntm:niter') == each, world_items(g, 'ntm:niter'))
    if SCENARIO in STALE:
        away_and_back(g, FORCE_B, (503, 99, 8))
        g.check('x_back_from_its_file', item_stacks(g, 'ntm:niter') == 2, item_stacks(g, 'ntm:niter'))
        take_to(g, (503, 100, 8), spots[1])
        time.sleep(4)
    else:
        away_and_back(g, FORCE_C, (803, 99, 8))
        g.check('all_back_from_their_file', item_stacks(g, 'ntm:niter') == 3, item_stacks(g, 'ntm:niter'))
        g.command('tick freeze')
        take_to(g, (809, 100, 3), spots[2])
        g.command('tp @s %s %s %s' % spots[0])
        g.command('tick step 60')
        time.sleep(5)
        g.command('tick unfreeze')
        g.check('n_picked_up', inventory(g, 'ntm:niter') == 16, inventory(g, 'ntm:niter'))
    folded = {'stacks': item_stacks(g, 'ntm:niter'), 'entities': world_items(g, 'ntm:niter'), 'inventory': inventory(g, 'ntm:niter')}
    g.check('d_folded_into_one_stack_of_64', folded['stacks'] == 1 and folded['entities'] == 64, folded)
    time.sleep(3)
    g.evidence['after_act'] = {**folded, 'ledger': ledger(g)}
    g.save()


def stale_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core = prep['cores'][-1]
    if SCENARIO == 'origin-away' and not again:
        # Reopened at B, where the base save left the player, with the origin away: B's older copy is picked up first.
        # After a save that does not wait (`kv_committed`) the player's file is that save's, as vanilla writes it, and
        # the player opens where the action left them: the reopen is compared with the control's all the same.
        paused = json.loads((g.folder / 'native-crash-more-act.json').read_text()).get('pause_save', False)
        g.evidence['origin_away_on_reopen'] = unloaded(g, (140, 100, 8))
        if not paused:
            g.check('origin_away_on_reopen', g.evidence['origin_away_on_reopen'], 'A')
        g.command('tp @s 503.5 100 8.5')
        time.sleep(4)
        g.evidence['picked_old_copy'] = {'inventory': inventory(g, 'ntm:niter'), 'entities': world_items(g, 'ntm:niter'),
                                         'origin_away': unloaded(g, (140, 100, 8)), 'ledger': ledger(g)}
    g.command('tp @s %d %d %d' % (core[0] + 3, core[1], core[2] + 6))
    if SCENARIO == 'merge-stale' and not again:
        # B (whose file shows X) stays away while the world settles and a confirmed save prunes the ledger.
        g.command('forceload remove ' + FORCE_B)
        g.check('older_holder_away_on_reopen', unloaded(g, (503, 99, 8)), 'B')
        led = quiet(g, 8)
        g.evidence['holder_away'] = {'entities': world_items(g, 'ntm:niter'), 'ledger': led}
        g.ctl('save')
    for force in (FORCE_B, FORCE_C):
        g.command('forceload add ' + force)
    led = quiet(g, 12)
    detail = {'inside': inside(g, prep['cores']), 'entities': world_items(g, 'ntm:niter'), 'inventory': inventory(g, 'ntm:niter'), 'ledger': led}
    detail['total'] = detail['inside'] + detail['entities'] + detail['inventory']
    counted(g, 'niter_exactly_once', detail, again)


def origin_expected():
    return 0 if SCENARIO == 'origin-vanish' else NITER


def origin_pickup_act(g):
    g.ctl('wait-ready 120')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core = tuple(prep['core'])
    g.command('gamemode survival @s')
    g.command('tp @s %d %d %d' % (core[0], core[1], core[2] + 4))
    g.command('give @s minecraft:diamond_pickaxe 1')
    g.ctl('select minecraft:diamond_pickaxe')
    if SCENARIO == 'origin-merge':
        # Each mixer's contents to its own spot in the destination, right after its break: two stacks of one ledger.
        g.command('forceload add 496 0 511 15')
        time.sleep(2)
        g.command('fill 496 99 0 511 99 15 minecraft:stone')
        for c, spot in zip(prep['cores'], (498.5, 508.5)):
            g.command('tp @s %d %d %d' % (c[0], c[1], c[2] + 4))
            g.ctl('break %d %d %d --timeout 30' % tuple(c))
            g.command('tp @e[type=minecraft:item,x=%d,y=%d,z=%d,distance=..5] %s 100 3.5' % (c[0], c[1], c[2], spot))
            time.sleep(.5)
        g.check('broken_contents_dropped', world_items(g, 'ntm:niter') == NITER, world_items(g, 'ntm:niter'))
        g.evidence['spread'] = item_stacks(g, 'ntm:niter')
    else:
        g.ctl('break %d %d %d --timeout 30' % core)
        time.sleep(1)
        g.check('broken_contents_dropped', world_items(g, 'ntm:niter') == NITER, world_items(g, 'ntm:niter'))
        g.command('forceload add 496 0 511 15')
        time.sleep(2)
        g.command('fill 496 99 0 511 99 15 minecraft:stone')
        g.command('tp @e[type=minecraft:item] 503.5 100 8.5')
    time.sleep(1)
    g.command('forceload remove all')
    g.command('forceload add 496 0 511 15')
    g.command('tp @s 503.5 102 13.5')
    deadline = time.monotonic() + 270
    while True:
        loaded = g.command('execute if loaded %d %d %d run say NTM_SOURCE_LOADED' % core)
        source_loaded = any('NTM_SOURCE_LOADED' in s for s in loaded.get('chat', []))
        if not source_loaded or time.monotonic() > deadline:
            break
        time.sleep(1)
    g.check('source_really_unloaded', not source_loaded, loaded)
    g.evidence['while_away'] = {'inventory': inventory(g, 'ntm:niter'), 'entities': world_items(g, 'ntm:niter'), 'ledger': ledger(g)}
    g.check('contents_still_alive_in_destination', g.evidence['while_away']['entities'] == NITER, g.evidence['while_away'])
    if SCENARIO == 'origin-merge':
        g.command('tp @e[type=minecraft:item] 503.5 100 8.5')
        time.sleep(3)
        folded = {'stacks': item_stacks(g, 'ntm:niter'), 'entities': world_items(g, 'ntm:niter'), 'spread': g.evidence['spread']}
        g.check('contents_folded_into_one_stack_while_source_is_away', folded['stacks'] == 1 and folded['entities'] == NITER and folded['spread'] > 1, folded)
    elif SCENARIO == 'origin-vanish':
        g.command('kill @e[type=minecraft:item]')
        time.sleep(2)
        g.check('contents_gone_while_source_is_away', world_items(g, 'ntm:niter') == 0, world_items(g, 'ntm:niter'))
    else:
        g.command('tp @s 503.5 100 8.5')
        time.sleep(3)
        picked = {'inventory': inventory(g, 'ntm:niter'), 'entities': world_items(g, 'ntm:niter')}
        g.check('all_contents_picked_up_while_source_is_away', picked == {'inventory': NITER, 'entities': 0}, picked)
    if SCENARIO != 'origin-pickup-away':
        g.command('forceload add %d %d' % (core[0], core[2]))
        time.sleep(6)
    detail = {'inventory': inventory(g, 'ntm:niter'), 'entities': world_items(g, 'ntm:niter'), 'ledger': ledger(g)}
    detail['total'] = detail['inventory'] + detail['entities']
    g.evidence['after_act'] = detail
    g.check('source_reload_does_not_duplicate_picked_up_contents', detail['total'] == origin_expected(), detail)
    save_expecting_death(g)


def origin_pickup_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('forceload add 128 0 143 15')
    g.command('forceload add 496 0 511 15')
    led = quiet(g)
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    detail = {'inside': inside(g, prep['cores']), 'inventory': inventory(g, 'ntm:niter'), 'entities': world_items(g, 'ntm:niter'), 'ledger': led}
    detail['total'] = detail['inside'] + detail['inventory'] + detail['entities']
    detail['expected'] = origin_expected()
    counted(g, 'picked_up_contents_remain_exactly_once', detail, again)


# ------------------------------------------------------------------------------------------------------------ common

def counted(g, name, detail, again):
    """The count after a reopen. For a break and a transfer only recorded: the reference is the vanilla control at the
    same cut. For an emission too: the runner compares it with the files and the control (a chunk the action unloaded
    holds what it held then, in both worlds). Else exactly `expected`."""
    g.evidence['counted' + ('_again' if again else '')] = detail
    if SCENARIO in BROKEN or SCENARIO in TRANSFERS:
        detail.pop('expected', None)
        return
    if SCENARIO in EMITTING:
        return
    g.check(name, detail['total'] == detail['expected'], detail)


def pause_expecting_death(g):
    """`kv_committed`: the pause saves without waiting; the process dies right after that save's storage, its records
    still queued. The evidence is written first."""
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


def save_expecting_death(g):
    if PAUSE_SAVE:
        pause_expecting_death(g)
        return
    if NO_SAVE:
        # `kill`: no save at all; the runner ends the process where the action left the world.
        g.save()
        return
    try:
        g.ctl('save')
        g.evidence['survived_save'] = True
    except Exception as error:  # the failpoint ends the process inside the save
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


# `furnace-smelt`: a brick furnace smelting 16 raw iron on 3 coal, killed at a boundary of a save while it works: the
# slots and the burn/progress/ash state are one block-entity compound, so what the reopen holds is a whole state.
SMELT = (140, 100, 8)


def smelt_counts(g, p):
    text = ' '.join(g.command('data get block %d %d %d Items' % p).get('chat', []))
    out = {}
    for entry in re.findall(r'\{[^{}]*\}', text):
        item = re.search(r'id: ?"?([\w:]+)"?', entry)
        n = re.search(r'[cC]ount: ?(\d+)', entry)
        if item and n:
            out[item[1]] = out.get(item[1], 0) + int(n[1])
    return out


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def smelt_prepare(g):
    from native_stations import open_machine
    g.ctl('wait-ready 120')
    g.close()
    x, y, z = SMELT
    g.ground(x - 3, y, z, width=8)
    g.command('clear @s')
    g.command('give @s ntm:machine_furnace_brick_off 1')
    g.ctl(f'place {x} {y} {z} --item ntm:machine_furnace_brick_off')
    g.command('clear @s')
    open_machine(g, x, y, z)
    g.put('minecraft:raw_iron', 16, 0)
    g.put('minecraft:coal', 3, 1)
    g.close()
    sprint(g, 1100)
    got = smelt_counts(g, SMELT)
    g.check('smelting_before_the_base_save', got.get('minecraft:iron_ingot', 0) > 0 and got.get('minecraft:raw_iron', 0) > 0, got)
    g.ctl('save')


def smelt_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    sprint(g, 1000)
    g.evidence['after_act'] = smelt_counts(g, SMELT)
    save_expecting_death(g)


def smelt_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s %d %d %d' % (SMELT[0], SMELT[1], SMELT[2] + 4))
    quiet(g, 6)
    got = smelt_counts(g, SMELT)
    raw, ingots = got.get('minecraft:raw_iron', 0), got.get('minecraft:iron_ingot', 0)
    counted(g, 'raw_and_ingots_are_16', {'total': raw + ingots, 'expected': 16, 'items': got}, again)
    g.check('the_furnace_is_alive', machine_present(g, SMELT), got)
    # The burn, the progress and the ash ride with the slots (one compound): whatever cut the world reopens from, they
    # agree with the coal gone, the ingots made and the ash items. While it smelts, the progress adds one per tick and
    # the burn falls one per tick but the first (the coal is lit in that tick): progress = burned + 1.
    st = frozen_state(g, SMELT)
    got = st['_items']
    ingots = got.get('minecraft:iron_ingot', 0)
    used = 3 - got.get('minecraft:coal', 0)
    burned = 1600 * used - st['burn_time']
    work = 200 * ingots + st['progress']
    ash_items = got.get('ntm:powder_ash__coal', 0)
    if got.get('minecraft:raw_iron', 0):
        g.check('burn_and_progress_agree', work == burned + 1, {'burned': burned, 'work': work, 'state': st, 'items': got})
    else:
        # Nothing left to smelt: the burn goes on falling alone, so it is at least what the work took.
        g.check('burn_and_progress_agree', burned >= work - 1, {'burned': burned, 'work': work, 'state': st, 'items': got})
    g.check('ash_level_and_items_agree', st['ash_coal'] + 2000 * ash_items == 1600 * used, {'state': st, 'items': got})


# `welder-run`: an arc welder making motors on a creative battery, killed at a boundary of a save while it works:
# steel plates + motors stay the 40 that went in (each weld turns 2 plates and 2 wires into 2 motors).
WELD = (140, 100, 8)


def weld_prepare(g):
    from native_welders import place, open_any
    g.ctl('wait-ready 120')
    g.close()
    core = place(g, 'ntm:machine_arc_welder', '"Arc"', *WELD)
    cells = [(core[0] + dx, core[1] + dy, core[2] + dz) for dy in (1, 0) for dz in (1, 0) for dx in (0, -1, 1)]
    open_any(g, cells)
    g.put('ntm:plate_steel', 40, 0)
    g.put('ntm:wire_dense__red_copper', 40, 1)
    g.put('ntm:battery_creative', 1, 4)
    g.close()
    sprint(g, 250)
    got = smelt_counts(g, core)
    g.check('welding_before_the_base_save', got.get('ntm:motor', 0) > 0 and got.get('ntm:plate_steel', 0) > 0, got)
    g.evidence['core'] = list(core)
    g.ctl('save')


def weld_core(g):
    import json as _json
    return tuple(_json.loads((g.folder / 'native-crash-more-prepare.json').read_text())['core'])


def weld_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    sprint(g, 300)
    g.evidence['after_act'] = smelt_counts(g, weld_core(g))
    save_expecting_death(g)


def weld_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    core = weld_core(g)
    g.command('tp @s %d %d %d' % (core[0], core[1], core[2] + 4))
    quiet(g, 6)
    got = smelt_counts(g, core)
    plates, motors = got.get('ntm:plate_steel', 0), got.get('ntm:motor', 0)
    counted(g, 'plates_and_motors_are_40', {'total': plates + motors, 'expected': 40, 'items': got}, again)
    g.check('the_welder_is_alive', machine_present(g, core), got)


def frozen_state(g, core, keys=('burn_time', 'max_burn_time', 'progress', 'processing_time', 'ash_coal', 'heat', 'mb')):
    """The machine's saved fields and its slots' items, read in the same tick (the world is frozen for the reads)."""
    from native_combustion import machine_state, first_int
    g.command('tick freeze')
    try:
        text = machine_state(g, core)
        out = {k: first_int(text, r'"%s":(\d+)' % k) for k in keys}
        for key in ('bonus',):
            found = re.search(r'"%s":\[(\d+),(\d+),(\d+)\]' % key, text)
            out[key] = [int(v) for v in found.groups()] if found else None
        out['_items'] = smelt_counts(g, core)
    finally:
        g.command('tick unfreeze')
    return out


MB_BLOCK = {'iron': ('ntm:furnace_iron', '"Iron"'), 'steel': ('ntm:furnace_steel', '"Steel"'),
            'combination': ('ntm:furnace_combination', '"Combination"')}
MB_AT = (140, 100, 8)


def mb_place(g, kind, fire=True):
    """The multiblock furnace of `kind` and, for the two on a heat source, the firebox under it: its core and firebox."""
    from native_furnaces import core_of as find_core
    x, y, z = MB_AT
    g.close()
    g.ground(x - 3, y, z, width=10)
    g.command('clear @s')
    item, marker = MB_BLOCK[kind]
    g.command(f'give @s {item} 1')
    if kind == 'iron':
        g.ctl(f'place {x} {y} {z} --item {item}')
        time.sleep(1.5)
        return find_core(g, (x, y, z), marker), None
    g.command(f'setblock {x} {y} {z} minecraft:stone')
    g.ctl(f'place {x} {y + 1} {z} --item {item}')
    time.sleep(1.5)
    core = find_core(g, (x, y + 1, z), marker)
    g.command(f'setblock {x} {y} {z} minecraft:air')
    fire_at = (core[0], core[1] - 1, core[2])
    if fire:
        g.command(f'setblock {fire_at[0]} {fire_at[1]} {fire_at[2]} ntm:heater_firebox[facing=north]')
        time.sleep(1)
    return core, fire_at


def mb_open(g, kind, core):
    """The screen, through a cell of the multiblock a player can click from the south; patient, the host may be loaded."""
    first = [(0, 1, 0)] if kind == 'iron' else [(0, 0, 1)]
    rest = [(dx, dy, dz) for dy in (1, 0, 2) for dz in (0, 1, -1) for dx in (0, -1, 1) if (dx, dy, dz) not in first]
    g.close()
    tried = []
    for dx, dy, dz in first + rest:
        cell = (core[0] + dx, core[1] + dy, core[2] + dz)
        g.command('tp @s %s 100 %s' % (cell[0] + .5, cell[2] + 3.5))
        time.sleep(1.5)
        for _ in range(3):
            try:
                g.ctl('interact %d %d %d south' % cell)
            except RuntimeError:
                break  # no visible face from here: another cell
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if g.ctl('screen').get('screen') == 'container':
                    return cell
                time.sleep(.4)
            g.close()
        tried.append(cell)
    raise AssertionError('no cell opened the screen: ' + str(tried))


def mb_fill_fire(g, fire_at):
    from native_furnaces import open_fire
    open_fire(g, fire_at)
    g.put('minecraft:coal', 16, 0)
    g.close()


STATE_KIND = {'iron-state': 'iron', 'steel-state': 'steel', 'combination-state': 'combination'}


def state_core(g):
    return tuple(json.loads((g.folder / 'native-crash-more-prepare.json').read_text())['core'])


def state_prepare(g):
    kind = STATE_KIND[SCENARIO]
    g.ctl('wait-ready 120')
    core, fire_at = mb_place(g, kind)
    if fire_at:
        mb_fill_fire(g, fire_at)
    mb_open(g, kind, core)
    if kind == 'iron':
        g.put('minecraft:coal', 3, 1)
        g.put('minecraft:raw_iron', 16, 0)
        g.close()
        sprint(g, 500)
    elif kind == 'steel':
        g.put('minecraft:iron_ore', 8, 0)
        g.close()
        # Until the line holds some bonus (one to three ores done): the state a reopen must keep.
        for _ in range(60):
            sprint(g, 100)
            st = frozen_state(g, core)
            done = 8 - st['_items'].get('minecraft:iron_ore', 0)
            if done % 4:
                break
    else:
        g.put('minecraft:coal', 8, 0)
        g.close()
        sprint(g, 600)
    st = frozen_state(g, core)
    g.evidence['core'] = list(core)
    g.evidence['before_the_base_save'] = st
    g.check('working_before_the_base_save', bool(st['_items']) and (st['progress'] or any(st['bonus'] or [])), st)
    g.ctl('save')


def state_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    core = state_core(g)
    sprint(g, 700)
    g.evidence['after_act'] = frozen_state(g, core)
    save_expecting_death(g)


def state_verify(g, again=False):
    kind = STATE_KIND[SCENARIO]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    core = state_core(g)
    g.command('tp @s %d %d %d' % (core[0], core[1], core[2] + 4))
    quiet(g, 6)
    st = frozen_state(g, core)
    got = st['_items']
    g.evidence['state_after_reopen' + ('_again' if again else '')] = st
    g.check('the_furnace_is_alive', machine_present(g, core), st)
    if kind == 'iron':
        raw, ingots = got.get('minecraft:raw_iron', 0), got.get('minecraft:iron_ingot', 0)
        counted(g, 'raw_and_ingots_are_16', {'total': raw + ingots, 'expected': 16, 'items': got}, again)
        used = 3 - got.get('minecraft:coal', 0)
        burned = st['max_burn_time'] * used - st['burn_time']
        work = st['processing_time'] * ingots + st['progress']
        g.check('burn_and_progress_agree', work == burned, {'burned': burned, 'work': work, 'state': st})
    elif kind == 'steel':
        ores, ingots = got.get('minecraft:iron_ore', 0), got.get('minecraft:iron_ingot', 0)
        done = 8 - ores
        # Four ores pay one more ingot (25 % each); the bonus left is the part of the next one: the line's progress
        # and its bonus ride with the items, or one of them is lost at a reopen.
        counted(g, 'ores_and_ingots_agree', {'total': done + done // 4, 'expected': ingots, 'items': got}, again)
        g.check('bonus_agrees_with_the_ores_done', st['bonus'] is not None and st['bonus'][0] == 25 * (done % 4), {'done': done, 'state': st})
    else:
        coal, coke = got.get('minecraft:coal', 0), got.get('ntm:coke__coal', 0)
        counted(g, 'coal_and_coke_are_8', {'total': coal + coke, 'expected': 8, 'items': got}, again)
        g.check('creosote_100_mb_a_coke', st['mb'] == 100 * coke, {'mb': st['mb'], 'coke': coke})


# `iron-break`, `steel-break`, `combination-break`: the multiblock furnace holding items, broken, against the chest control
# holding the same ones; counted item by item, once in the world.
MB_BREAK = {'iron-break': ('iron', [('minecraft:raw_iron', 6, 0), ('ntm:upgrade_speed_1', 1, 4)]),
            'steel-break': ('steel', [('minecraft:raw_iron', 6, 0), ('minecraft:raw_copper', 3, 1)]),
            'combination-break': ('combination', [('minecraft:coal', 8, 0), ('minecraft:bucket', 2, 2)])}


def mb_items(g, p, ids):
    text = ' '.join(g.command('data get block %d %d %d Items' % p).get('chat', []))
    total = 0
    for entry in re.findall(r'\{[^{}]*\}', text):
        item = re.search(r'id: ?"?([\w:]+)"?', entry)
        n = re.search(r'[cC]ount: ?(\d+)', entry)
        if item and n and item[1] in ids:
            total += int(n[1])
    return total


def mb_break_prepare(g):
    kind, loads = MB_BREAK[SCENARIO]
    ids = [i for i, _, _ in loads]
    g.ctl('wait-ready 120')
    if VANILLA:
        x, y, z = MB_AT
        g.close()
        g.ground(x - 3, y, z, width=10)
        g.command('clear @s')
        g.command(f'setblock {x} {y} {z} minecraft:chest')
        for n, (item, count, _) in enumerate(loads):
            g.command(f'item replace block {x} {y} {z} container.{n} with {item} {count}')
        core = MB_AT
    else:
        # No firebox: no heat, nothing smelts, the items stay as they were put.
        core, _ = mb_place(g, kind, fire=False)
        mb_open(g, kind, core)
        for item, count, slot in loads:
            g.put(item, count, slot)
        g.close()
    expected = {i: c for i, c, _ in loads}
    g.check('items_inside', mb_items(g, core, ids) == sum(expected.values()), mb_items(g, core, ids))
    g.evidence['core'] = list(core)
    g.ctl('save')


def mb_break_verify(g, again=False):
    kind, loads = MB_BREAK[SCENARIO]
    ids = [i for i, _, _ in loads]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    core = state_core(g)
    g.command('tp @s %d %d %d' % (core[0] + 3, core[1], core[2] + 6))
    led = quiet(g, 12)
    state = g.ctl('block %d %d %d' % core).get('state', '')
    standing = state.startswith(('ntm:furnace_', 'minecraft:chest'))
    inside = mb_items(g, core, ids) if standing else 0
    ents = sum(entities(g, i) for i in ids)
    inv = sum(inventory(g, i) for i in ids)
    detail = {'block': state, 'inside': inside, 'entities': ents, 'inventory': inv, 'total': inside + ents + inv,
              'expected': sum(c for _, c, _ in loads), 'ledger': led, 'machine': machine_present(g, core)}
    counted(g, 'mb_items_exactly_once', detail, again)
    if state.startswith('ntm:furnace_'):
        g.check('a_standing_furnace_is_a_live_machine', machine_present(g, core), detail)


def drain(g):
    g.ctl('wait-ready 120')
    g.ctl('save')


STEPS = {
    'coupled': (coupled_prepare, coupled_act, coupled_verify),
    'coupled-break': (coupled_prepare, coupled_break_act, coupled_break_verify),
    'crossing': (crossing_prepare, crossing_act, crossing_verify),
    'origin-pickup': (break_prepare, origin_pickup_act, origin_pickup_verify),
    'origin-pickup-away': (break_prepare, origin_pickup_act, origin_pickup_verify),
    'origin-merge': (break_prepare, origin_pickup_act, origin_pickup_verify),
    'origin-vanish': (break_prepare, origin_pickup_act, origin_pickup_verify),
    'origin-kill': (break_prepare, origin_kill_act, origin_kill_verify),
    'crossing-kill': (crossing_prepare, crossing_act, crossing_verify),
    'merge-stale': (break_prepare, stale_act, stale_verify),
    'origin-away': (break_prepare, stale_act, stale_verify),
    'pickup-merge': (break_prepare, stale_act, stale_verify),
    'save-quit': (crossing_prepare, save_quit_act, save_quit_verify),
    'mining': (mining_prepare, mining_act, mining_verify),
    'vanilla-break': (vanilla_break_prepare, break_act, break_verify),
    'break': (break_prepare, break_act, break_verify),
    'furnace-break': (break_prepare, break_act, break_verify),
    'furnace-smelt': (smelt_prepare, smelt_act, smelt_verify),
    'welder-run': (weld_prepare, weld_act, weld_verify),
    'iron-state': (state_prepare, state_act, state_verify),
    'steel-state': (state_prepare, state_act, state_verify),
    'combination-state': (state_prepare, state_act, state_verify),
    'iron-break': (mb_break_prepare, break_act, mb_break_verify),
    'steel-break': (mb_break_prepare, break_act, mb_break_verify),
    'combination-break': (mb_break_prepare, break_act, mb_break_verify),
    'frozen-break': (frozen_break_prepare, frozen_break_act, frozen_break_verify),
    'frozen-unload': (frozen_break_prepare, frozen_unload_act, frozen_break_verify),
    'replace': (break_prepare, break_act, break_verify),
}

if __name__ == '__main__':
    mode = sys.argv[2]
    kind = SCENARIO if SCENARIO in STEPS else 'coupled' if SCENARIO.startswith('coupled') else SCENARIO
    g = Game(sys.argv[1], 'crash-more-' + mode)
    prepare, act, verify = STEPS[kind]
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    # `kv_committed`: an action that ends without a save of its own (the runner's quit saves it) ends with the pause's.
    if mode == 'act' and PAUSE_SAVE and not g.evidence.get('pause_save'):
        pause_expecting_death(g)
    g.evidence['ok'] = True
    g.save()

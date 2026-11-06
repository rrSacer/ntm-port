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
(`EMITTING`): their control has loose items put on the ground where the effect drops them (a chest broken in place of the
mined one), and the runner checks that the reopens count what the files held, that nothing is held twice, and that no
more is lost than the control loses at the same cut. The transfers without an ejection are counted exactly.
"""
import json
import os
import re
import sys
import time
from native_game import Game
from native_pile import interact, machine, tool, block
from native_crash import ledger

SCENARIO = os.environ.get('NTM_CRASH_SCENARIO', 'coupled-a')
# A chest in each mixer's place: the vanilla control of a scenario of `BROKEN`.
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
BROKEN = ('break', 'replace', 'origin-pickup', 'origin-pickup-away', 'origin-merge', 'origin-vanish', 'origin-kill',
          'merge-stale', 'origin-away', 'pickup-merge')
# The scenarios whose items an effect of the mod drops, with the items counted.
EMITTING = {'coupled-ea': ('ntm:pile_rod',), 'coupled-eb': ('ntm:pile_rod',), 'crossing': ('ntm:pile_rod',),
            'crossing-kill': ('ntm:pile_rod',), 'save-quit': ('ntm:pile_rod',), 'mining': ('minecraft:diamond',)}
# The scenarios built on an entity file that lags its stacks.
STALE = ('merge-stale', 'origin-away')
ROD = 'ntm:pile_rod__ra226be'
INCOMING = 'ntm:pile_rod__zr'

# scenario -> (core x, which loader, a full channel first)
COUPLED = {'coupled-a': (47, 'R', False), 'coupled-b': (48, 'L', False),
           'coupled-ea': (47, 'R', True), 'coupled-eb': (48, 'L', True)}


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
    return channels, loaders


def loose(g, item, n, at):
    """The control's part of an effect: `n` loose items of `item` put on the ground at `at`."""
    for _ in range(n):
        g.command('summon minecraft:item %s %s %s {Item:{id:"%s",count:1}}' % (*at, item))


def touch(g, *cells):
    """The control changes a block in each chunk the effect's save would write, so that the cut has its records."""
    for x, z in cells:
        g.command(f'setblock {x} 99 {z} minecraft:cobblestone')


def coupled_prepare(g):
    cx, side, full = COUPLED[SCENARIO]
    if VANILLA and full:
        g.close()
        g.command('gamemode creative @s')
        g.command('forceload add 30 0 64 24')
        time.sleep(3)
        g.command('fill 40 99 3 56 108 18 minecraft:air')
        g.command('fill 40 99 3 56 99 18 minecraft:stone')
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
    cx, side, full = COUPLED[SCENARIO]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    if VANILLA and full:
        # The ejected rod, loose by the core; the loader's and the core's chunks changed.
        loose(g, ROD, 1, (cx + .5, 103, 9.5))
        touch(g, (40, 4), (50, 4))
        time.sleep(1.8)
        g.evidence['after_act'] = {'entities': entities(g, ROD)}
        save_expecting_death(g)
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
    save_expecting_death(g)


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
    expected = (1 if VANILLA else 6) if full else 1
    detail = {'channels': channels, 'loaders': loaders, 'entities': outside, 'inventory': carried, 'total': held, 'expected': expected, 'ledger': led}
    counted(g, 'no_rod_lost_or_duplicated', detail, again)
    if VANILLA:
        return
    core = geometry(cx)['core']
    g.check('the_core_is_alive', machine_present(g, core), 'core machine missing')


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
        # Near the pile, as the mod's world leaves its player, out of reach of the loose rods.
        g.command('tp @s 47.5 101 1.5')
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
    """The pile gives up both channels through its loaders; in the control, as many loose rods by each loader."""
    for side in ('L', 'R'):
        p = geometry(47)[side][0]
        if VANILLA:
            loose(g, ROD, 5, (p[0] + .5, 102, p[2] - 1.5))
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
    deadline = time.monotonic() + 90
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
    if SCENARIO == 'crossing-kill':
        g.command('forceload add 30 0 64 24')
        g.command('forceload add 128 0 143 15')
    g.command('tp @s 133.5 101 15.5' if SCENARIO in ('crossing', 'crossing-kill') else 'tp @s 53.5 101 15.5')
    led = quiet(g, 10)
    held = entities(g, ROD) + inventory(g, ROD)
    expected = 10 if SCENARIO == 'crossing-kill' else 12
    detail = {'entities': entities(g, ROD), 'inventory': inventory(g, ROD), 'total': held, 'expected': expected, 'ledger': led}
    counted(g, 'no_rod_duplicated_by_a_chunk_that_unloaded', detail, again)


# ------------------------------------------------------------------------------------------------------ save & quit


def save_quit_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    tool(g)
    eject(g)
    led = quiet(g, 4)
    g.evidence['after_act'] = {'entities': entities(g, ROD), 'ledger': led}
    g.check('rods_ejected_before_quit', (VANILLA or led['applied'] > 0) and entities(g, ROD) == 12, g.evidence['after_act'])
    # The runner's quit is the first confirmed save after the undo.


def save_quit_verify(g, again=False):
    crossing_verify(g, again)
    led = ledger(g)
    g.check('save_and_quit_leaves_nothing_owed', led.get('pending', -1) == 0, led)


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
    deadline = time.monotonic() + 150
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
            g.command('give @s ntm:machine_mixer 1')
            if SCENARIO.startswith('origin') or SCENARIO in MIXERS:
                g.command(f'tp @s {x + .5} {y} {z + 3.5}')
            g.ctl(f'place {x} {y} {z} --item ntm:machine_mixer')
            g.command('clear @s')
            open_machine(g, x, y, z)
            core = core_of(g, 'ntm:machine_mixer', x, y, z)
            g.put('ntm:niter', each, 1)
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
    inside = mixer_items(g, core) if state.startswith(('ntm:machine_mixer','minecraft:chest')) else 0
    total = inside + entities(g, 'ntm:niter') + inventory(g, 'ntm:niter')
    detail = {'block': state, 'inside': inside, 'entities': entities(g, 'ntm:niter'), 'inventory': inventory(g, 'ntm:niter'), 'total': total, 'expected': NITER, 'ledger': led,
              'machine': machine_present(g, core)}
    counted(g, 'niter_exactly_once', detail, again)
    if state.startswith('ntm:machine_mixer'):
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
    deadline = time.monotonic() + 90
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
        g.check('origin_away_on_reopen', unloaded(g, (140, 100, 8)), 'A')
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
    deadline = time.monotonic() + 90
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
    """The count after a reopen. For a break only recorded: the reference is the vanilla chest at the same cut. For an
    emission never more than there was; the files and the control are the runner's to compare. Else exactly `expected`."""
    g.evidence['counted' + ('_again' if again else '')] = detail
    if SCENARIO in BROKEN:
        detail.pop('expected', None)
        return
    if SCENARIO in EMITTING:
        g.check(name.replace('lost_or_', '').replace('exactly_once', 'never_twice'), detail['total'] <= detail['expected'], detail)
        return
    g.check(name, detail['total'] == detail['expected'], detail)


def save_expecting_death(g):
    try:
        g.ctl('save')
        g.evidence['survived_save'] = True
    except Exception as error:  # the failpoint ends the process inside the save
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def drain(g):
    g.ctl('wait-ready 120')
    g.ctl('save')


STEPS = {
    'coupled': (coupled_prepare, coupled_act, coupled_verify),
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
    'replace': (break_prepare, break_act, break_verify),
}

if __name__ == '__main__':
    mode = sys.argv[2]
    kind = 'coupled' if SCENARIO.startswith('coupled') else SCENARIO
    g = Game(sys.argv[1], 'crash-more-' + mode)
    prepare, act, verify = STEPS[kind]
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    g.evidence['ok'] = True
    g.save()

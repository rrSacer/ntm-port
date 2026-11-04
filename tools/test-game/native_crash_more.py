#!/usr/bin/env python3
"""More ways to break an atomic save, one scenario per finding of the independent review (see run-crash-more.py).

Every scenario has three steps: `prepare` (a world saved cleanly), `act` (do the thing, then ask for a save that a
failpoint may end) and `verify` (after a reopen: wait for the world to settle, then count exactly). `again` is a
second reopen. Scenarios:
  coupled-a / coupled-b          a loader and its core in different chunks (core in the lower / the upper chunk),
                                 one rod pushed in: nothing is ejected, so no effect is owed
  coupled-ea / coupled-eb        the same with a full channel: the push ejects a rod
  crossing                       no crash: drops of a pile move into a chunk that then unloads
  mining                         an excavator mines a chest of diamonds
  break / replace                a mixer with items is broken (and, in replace, another put in its place)
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
ROD = 'ntm:pile_rod__ra226be'

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
    """Let the server tick: block entities are rewritten from the models and pending drops appear."""
    time.sleep(seconds)
    deadline = time.monotonic() + 60
    last = None
    while time.monotonic() < deadline:
        led = ledger(g)
        key = (led['pending'], led['spawned'], led['tagged'])
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


def coupled_prepare(g):
    cx, side, full = COUPLED[SCENARIO]
    geo = build_pile(g, cx)
    loader = geo[side][0]
    if full:
        for _ in range(5):
            load_rod(g, loader)
    channels, loaders = pile_rods(g, cx)
    g.check('prepared', channels + loaders == (5 if full else 0), (channels, loaders))
    g.command('clear @s')
    g.ctl('save')


def coupled_act(g):
    cx, side, full = COUPLED[SCENARIO]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    loader = geometry(cx)[side][0]
    load_rod(g, loader)
    g.evidence['after_act'] = {'rods': pile_rods(g, cx), 'entities': entities(g, ROD)}
    save_expecting_death(g)


def coupled_verify(g, again=False):
    cx, side, full = COUPLED[SCENARIO]
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s 53.5 101 15.5')
    led = quiet(g)
    channels, loaders = pile_rods(g, cx)
    held = channels + loaders + entities(g, ROD) + inventory(g, ROD)
    expected = 6 if full else 1
    detail = {'channels': channels, 'loaders': loaders, 'entities': entities(g, ROD), 'ledger': led}
    g.evidence['counted' + ('_again' if again else '')] = detail
    g.check('no_rod_lost_or_duplicated', held == expected, detail)
    core = geometry(cx)['core']
    g.check('the_core_is_alive', machine_present(g, core), 'core machine missing')


# -------------------------------------------------------------------------------------------------------- crossing

def crossing_prepare(g):
    # The pile of the main scenario, its drops in the lower chunk and the upper one.
    cx = 47
    geo = build_pile(g, cx)
    for side in ('L', 'R'):
        for _ in range(5):
            load_rod(g, geo[side][0])
    g.command('clear @s')
    g.ctl('save')


def crossing_act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    tool(g)
    for side in ('L', 'R'):
        p = geometry(47)[side][0]
        interact(g, p, 'north', (p[0] + .5, 101, p[2] - 3.5))
    led = quiet(g, 4)
    g.evidence['after_undo'] = {'entities': entities(g, ROD), 'ledger': led}
    # Every drop goes to the upper chunk; then that chunk is let go while the lower one stays.
    g.command('tp @e[type=minecraft:item] 53.5 103 13.5')
    time.sleep(3)
    g.command('forceload remove 48 0 64 24')
    g.command('tp @s -250.5 101 5.5')
    time.sleep(14)
    g.evidence['while_away'] = ledger(g)
    g.command('forceload add 30 0 64 24')
    g.command('tp @s 53.5 101 15.5')


def crossing_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tp @s 53.5 101 15.5')
    led = quiet(g, 10)
    held = entities(g, ROD) + inventory(g, ROD)
    detail = {'entities': entities(g, ROD), 'ledger': led}
    g.evidence['counted' + ('_again' if again else '')] = detail
    g.check('no_rod_duplicated_by_a_chunk_that_unloaded', held == 12, detail)


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
    core = place_machine(g, 'machine_excavator', x, y, z)
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
    detail = {'chest': chest_count(g, chest), 'entities': entities(g, 'minecraft:diamond'), 'ledger': led,
              'machine': machine_present(g, core)}
    g.evidence['counted' + ('_again' if again else '')] = detail
    g.check('diamonds_exactly_once', total == DIAMONDS, detail)
    g.check('the_excavator_is_alive', machine_present(g, core), detail)


# ------------------------------------------------------------------------------------------------- break / replace

MIXER = (140, 100, 8)
NITER = 16


def mixer_items(g, p):
    text = ' '.join(g.command('data get block %d %d %d Items' % p).get('chat', []))
    return sum(int(n) for n in re.findall(r'(?:count|Count): ?(\d+)', text) if True) if 'niter' in text else 0


def break_prepare(g):
    from native_stations import open_machine
    from native_recipes import core_of
    g.ctl('wait-ready 120')
    g.close()
    x, y, z = MIXER
    g.ground(x - 3, y, z, width=8)
    g.command('clear @s')
    g.command('give @s ntm:machine_mixer 1')
    g.ctl(f'place {x} {y} {z} --item ntm:machine_mixer')
    g.command('clear @s')
    open_machine(g, x, y, z)
    core = core_of(g, 'ntm:machine_mixer', x, y, z)
    g.put('ntm:niter', NITER, 1)
    g.close()
    g.evidence['core'] = core
    g.check('niter_inside', mixer_items(g, core) == NITER, mixer_items(g, core))
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
        g.command('give @s ntm:machine_mixer 1')
        g.command('gamemode creative @s')
        g.command('tp @s %d %d %d' % (core[0], core[1], core[2] + 3))
        time.sleep(.5)
        g.ctl('place %d %d %d --item ntm:machine_mixer' % core)
        time.sleep(1)
    save_expecting_death(g)


def break_verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    prep = json.loads((g.folder / 'native-crash-more-prepare.json').read_text())
    core = tuple(prep['core'])
    g.command('tp @s %d %d %d' % (core[0] + 3, core[1], core[2] + 6))
    led = quiet(g, 12)
    state = g.ctl('block %d %d %d' % core).get('state', '')
    inside = mixer_items(g, core) if state.startswith('ntm:machine_mixer') else 0
    total = inside + entities(g, 'ntm:niter') + inventory(g, 'ntm:niter')
    detail = {'block': state, 'inside': inside, 'entities': entities(g, 'ntm:niter'), 'ledger': led,
              'machine': machine_present(g, core)}
    g.evidence['counted' + ('_again' if again else '')] = detail
    g.check('niter_exactly_once', total == NITER, detail)
    if state.startswith('ntm:machine_mixer'):
        g.check('a_standing_mixer_is_a_live_machine', machine_present(g, core), detail)


# ------------------------------------------------------------------------------------------------------------ common

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
    'mining': (mining_prepare, mining_act, mining_verify),
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

#!/usr/bin/env python3
"""Stirling overspeed (Onda 2, D1+D2 fatia 5): the failure's blast through the post-phase door, on a disposable world.

The runner, run-stirling-overspeed-proof.py, owns the process and the cut, and counts the blasts each session applied
in its log (NTM_TRACE_EFFECTS=1: `effect <serial>.<n> explosion ...`, one line per application).
  prepare  an iron Stirling on a firebox, cold (no fuel): its saved record keeps the gear
  act      rocket fuel (TileEntityHeaterFirebox.java:37, heat x5: 500 TU a tick) through a hopper: the Stirling takes
           10% of the firebox's heat, over its 300 for more than 300 ticks, and loses its gear in one blast at
           [x+0.5, y+1, z+0.5], power 5, no fire, no block broken (TileEntityStirling.java:90-92); a player
           on top of the engine has its health recorded (the engine's own cells enclose
           the blast's centre); then the save the runner cuts
  verify   reopen: the gear as the files hold it, what the home record still owes, ~30 s of ticks, and the gear and the
           blocks after them
  again    the same on the second reopen
"""
import os
import re
import sys
import time
from native_game import Game
from native_combustion import start_area, machine_state
from native_solar_stirling import place
import native_rbmk_t01 as t

NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
X, Y, Z = 47, 101, 8
FUEL = 'ntm:rocket_fuel'
# The cells around the engine an explosion that broke blocks would change (the machine and its feed excluded).
LO, HI = (X - 6, Y - 1, Z - 6), (X + 8, Y + 5, Z + 6)


def core(g):
    found = g.evidence.get('core')
    if found:
        return tuple(found)
    for dy in range(0, 3):
        for dx in range(-3, 4):
            for dz in range(-3, 4):
                state = g.ctl(f'block {X + dx} {Y + dy} {Z + dz}').get('state', '')
                if state.startswith('ntm:machine_stirling['):
                    return X + dx, Y + dy, Z + dz
    raise AssertionError('no Stirling core')


def has_cog(g, at):
    state = machine_state(g, at)
    found = re.search(r'"has_cog":(true|false)', state)
    if not found:
        raise AssertionError('no Stirling state: ' + state[:300])
    return found.group(1) == 'true'


def area(g):
    """Every cell around the engine that is not air, as `x,y,z -> state`, but the machine's own cells, its firebox and
    the feed's hopper (their states are the machine's, not the blast's)."""
    out = g.ctl('blocks %d %d %d %d %d %d' % (*LO, *HI))
    own = ('ntm:machine_stirling', 'ntm:heater_firebox', 'minecraft:hopper')
    return {f'{x},{y},{z}': state for x, y, z, state in out['blocks'] if not state.startswith(own)}


def health(g):
    text = ' '.join(g.command('data get entity @s Health').get('chat', []))
    found = re.search(r'([\d.]+)f?\s*$', text)
    return float(found.group(1)) if found else None


def prepare(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    start_area(g, X - 4, Y, Z, 16)
    c = place(g, 'machine_stirling', X, Y, Z)
    g.evidence['core'] = list(c)
    g.command(f'setblock {c[0]} {c[1] - 1} {c[2]} ntm:heater_firebox[facing=south,honey_level=0]')
    time.sleep(2)
    g.check('cold_engine_with_its_gear', has_cog(g, c), machine_state(g, c))
    g.evidence['area'] = area(g)
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    c = core(g)
    g.evidence['core'] = list(c)
    before_area = area(g)
    # The blast's centre [x+0.5, y+1, z+0.5] is the face between the core and the cell above it, inside the engine's own
    # 3x3x2 cells (MachineStirling.getDimensions {1,0,1,1,1,1}): the exposure of an entity (Java getBlockDensity, rays
    # from the entity to the centre) is nil beside it and on top of it alike. The player's health is recorded, not
    # required to drop; resistance IV keeps the player alive should a ray get through.
    g.command(f'tp @s {c[0] + .5} {c[1] + 2} {c[2] + .5}')
    g.command('gamemode survival @s')
    g.command('effect give @s minecraft:resistance 120 3 true')
    g.command('effect give @s minecraft:instant_health 1 5 true')
    time.sleep(1)
    hp = health(g)
    g.evidence['health_before'] = hp
    hx = (c[0] + 1, c[1] - 1, c[2])
    g.command(f'setblock {hx[0]} {hx[1]} {hx[2]} minecraft:hopper[facing=west]')
    g.command(f'item replace block {hx[0]} {hx[1]} {hx[2]} container.0 with {FUEL} 4')
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline and has_cog(g, c):
        time.sleep(.5)
    g.check('the_gear_breaks', not has_cog(g, c), machine_state(g, c))
    time.sleep(2)
    hurt = health(g)
    g.evidence['health_after'] = hurt
    g.command('gamemode creative @s')
    g.evidence['player_hurt'] = hp is not None and hurt is not None and hurt < hp
    after_area = area(g)
    # The feed's hopper is the only cell the act itself put there (on the stone below its east side).
    feed = '%d,%d,%d' % hx
    changed = sorted(k for k in set(before_area) | set(after_area) if k != feed and before_area.get(k) != after_area.get(k))
    g.evidence['area_cells'] = len(before_area)
    g.check('no_block_broken', not changed, changed[:20])
    g.evidence['ledger_after_blast'] = t.ledger(g)
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
    except Exception as error:
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    c = core(g)
    g.command(f'tp @s {c[0] + .5} {c[1]} {c[2] + 6.5}')
    suffix = '_again' if again else ''
    start = {'has_cog': has_cog(g, c), 'ledger': t.ledger(g), 'area': area(g)}
    time.sleep(30)
    end = {'has_cog': has_cog(g, c), 'ledger': t.ledger(g), 'area': area(g)}
    g.evidence['session' + suffix] = {'start': {k: v for k, v in start.items() if k != 'area'},
                                     'end': {k: v for k, v in end.items() if k != 'area'},
                                     'failed_here': start['has_cog'] and not end['has_cog'],
                                     'area_unchanged': start['area'] == end['area']}
    g.check('a_lost_gear_never_comes_back', start['has_cog'] or not end['has_cog'], [start['has_cog'], end['has_cog']])
    g.check('no_block_broken_on_reopen', start['area'] == end['area'] == g.evidence.get('prepared_area', end['area']), 'area changed')
    g.check('nothing_left_owed', end['ledger']['obligations'] == 0, end['ledger'])
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'stirling-overspeed-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True)}[mode](g)

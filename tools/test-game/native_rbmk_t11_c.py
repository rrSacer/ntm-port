#!/usr/bin/env python3
"""T11 cut (C, `rbmk-plano` T11; coordinator's decision 4): the RBMK crane setting a rod down when the save is cut.

Modes prepare/act/verify/again/drain; the runner (`run-rbmk-t08-c-proof.py --script native_rbmk_t11_c.py --sides ntm`)
owns the process and the cut.
  prepare  a crane console in chunk (2, 0), linked to a fuel channel in chunk (1, 0); the crane holds the rod "Lot 7"
           (taken out of the channel by the crane itself), the hook up over the empty channel
  act      the host at the console presses Enter: the hook goes down (26 ticks) and sets the rod into the channel, the
           two blocks in one publication; then the save the runner cuts
  verify   the crane's held slot, the channel's slot and the items on the ground, read in one go: exactly one Lot 7,
           in the channel or held, never in both and never in neither, on both reopens
"""
import json
import os
import re
import sys
import time
from native_game import Game

Y = 101
PLACE = (36, Y, 4)
CHANNEL = (30, Y, 12)
ROD = 'ntm:rbmk_fuel_ueu[custom_data={hull:400.0d,core:450.0d,xenon:2.5d,yield:500000.0d},custom_name=\'"Lot 7"\']'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'


def machine(g, p):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))
    m = re.search(r"block data: '(.*)'$", text)
    try:
        return json.loads(m[1].replace("\\'", "'")) if m else None
    except ValueError:
        return None


def items(g, p):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]}').get('chat', []))
    m = re.search(r'Items: \[(.*)\]', text)
    return m[1] if m else ''


def core_of(g):
    cells = [(x, y, z) for x in range(PLACE[0] - 2, PLACE[0] + 3) for y in range(Y, Y + 2) for z in range(PLACE[2] - 2, PLACE[2] + 3)]
    for p in cells:
        m = machine(g, p)
        if m and m.get('block') == 'ntm:rbmk_crane_console':
            return p, m.get('facing')
    return None, None


def count(g):
    core, _ = core_of(g)
    held = items(g, core) if core else ''
    channel = items(g, CHANNEL)
    ground = ' '.join(g.command(f'data get entity @e[type=item,limit=1,sort=nearest,x={CHANNEL[0]},y={Y},z={CHANNEL[2]},distance=..40] Item').get('chat', []))
    n = lambda t: t.count('Lot 7')
    out = {'held': n(held), 'channel': n(channel), 'ground': n(ground), 'readable': core is not None}
    out['total'] = out['held'] + out['channel'] + out['ground']
    return out


def stand(g, core, facing):
    d = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}[facing]
    g.close()
    g.command(f'tp @s {core[0] + .5 + d[0] * 1.6} {Y} {core[2] + .5 + d[1] * 1.6} 0 0')
    time.sleep(.8)


def enter(g):
    g.ctl('key key.keyboard.enter down')
    time.sleep(.3)
    g.ctl('key key.keyboard.enter up')


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    g.command('forceload add 16 0 47 31')
    time.sleep(2)
    g.command(f'fill 22 {Y - 1} 0 44 {Y + 20} 24 minecraft:air')
    g.command(f'fill 22 {Y - 1} 0 44 {Y - 1} 24 minecraft:stone')
    g.command('clear @s')
    g.command('gamemode creative @s')
    # By hand: the column with its dummies up to its top, the cell the crane looks at (`setblock` places the core alone).
    g.command('give @s ntm:rbmk_rod 1')
    g.command(f'tp @s {CHANNEL[0] + .5} {Y} {CHANNEL[2] - 2.5} 0 0')
    time.sleep(.6)
    g.ctl(f'place {CHANNEL[0]} {CHANNEL[1]} {CHANNEL[2]} --item ntm:rbmk_rod')
    time.sleep(1.5)
    g.command(f'item replace block {CHANNEL[0]} {CHANNEL[1]} {CHANNEL[2]} container.0 with {ROD}')
    g.command('give @s ntm:rbmk_crane_console 1')
    g.command(f'tp @s {PLACE[0] + .5} {Y} {PLACE[2] - 3.5} 0 0')
    time.sleep(.6)
    g.ctl(f'place {PLACE[0]} {PLACE[1]} {PLACE[2]} --item ntm:rbmk_crane_console')
    time.sleep(1.5)
    core, facing = core_of(g)
    g.evidence['core'] = [core, facing]
    # The link: sneaking, the tool on the channel, then on the console.
    g.command('clear @s')
    g.command('give @s ntm:rbmk_tool 1')
    for p, at in ((CHANNEL, (CHANNEL[0] + .5, CHANNEL[2] - 2.5)), (PLACE, (PLACE[0] + .5, PLACE[2] - 2.5))):
        g.command(f'tp @s {at[0]} {Y} {at[1]} 0 0')
        time.sleep(.6)
        g.ctl('select ntm:rbmk_tool')
        g.ctl('key shift down')
        time.sleep(.3)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
        time.sleep(.5)
        g.ctl('key shift up')
        time.sleep(.5)
    g.command('clear @s')
    g.evidence['linked'] = (machine(g, core) or {}).get('data')
    # The crane takes the rod (over the channel at its centre: no travel).
    stand(g, core, facing)
    g.evidence['standing'] = g.ctl('status').get('player', {}).get('position')
    enter(g)
    # The console's model values ([4] the loading lamp, [5] the target lamp) while it goes down.
    g.evidence['lamps'] = [re.search(r'data: \[I; ([^\]]*)\]', ' '.join(g.command(f'data get block {core[0]} {core[1]} {core[2]} data').get('chat', []))) for _ in range(6)]
    g.evidence['lamps'] = [m[1] if m else None for m in g.evidence['lamps']]
    time.sleep(4)
    g.command('tp @s 24.5 101 1.5 0 0')
    time.sleep(2)
    base = count(g)
    g.evidence['base'] = base
    g.check('base', base['readable'] and base['total'] == 1 and base['held'] == 1, base)
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    core, facing = core_of(g)
    before = count(g)
    g.evidence['before_act'] = before
    g.check('held_before', before['held'] == 1 and before['total'] == 1, before)
    stand(g, core, facing)
    enter(g)
    # Down in 26 ticks and the transfer at the 27th: the save comes just after, the two chunks' records apart.
    time.sleep(1.6)
    moving = count(g)
    g.evidence['after_act'] = moving
    g.check('one_rod_at_the_save', moving['total'] == 1, [before, moving])
    g.save()
    if NO_SAVE:
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
    time.sleep(2)
    now = count(g)
    key = 'counted' + ('_again' if again else '')
    g.evidence[key] = now
    g.check('one_rod' + ('_again' if again else ''), now['readable'] and now['total'] == 1, now)


def drain(g):
    g.ctl('wait-ready 120')
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t11c-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    g.evidence['ok'] = True
    g.save()

#!/usr/bin/env python3
"""T09 cut (C, `rbmk-plano` T09): the RBMK outgasser processing coal tar when the save is cut.

Modes prepare/act/verify/again/drain; the runner (`run-rbmk-t08-c-proof.py --script native_rbmk_t09_c.py`) owns the
process and the cut.
  prepare  an outgasser in chunk (2, 0) with four coal tars in its input, an empty fuel channel beside it, its gas by a
           duct to a barrel in chunk (1, 0); `dialOutgasserSpeedMod` 5
  act      a Po-210/Be rod put into the channel by hand: the tars are processed (100 mB of coal oil each) one after the
           other; after three seconds, the save the runner cuts, the processing still going
  verify   the outgasser's slots and gas and the barrel, read in one go (`ntmtest fluids`): 400 in all, counting a tar as
           100 mB, so each processing used exactly one tar and made exactly one recipe's oil, and none was done twice
With NTM_CRASH_VANILLA=1 the vanilla control of T08's cut (hoppers moving 64 stones across the same border).
"""
import os
import re
import sys
import time
from native_game import Game
from native_stations import type_duct
import native_rbmk_t08_c as t08

Y = 101
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
ROD, OUT = (32, Y, 8), (33, Y, 8)
DUCTS, BARREL = [(33, Y - 1, 8), (32, Y - 1, 8), (31, Y - 1, 8)], (30, Y - 1, 8)
TAR = 'ntm:oil_tar__coal'


def count(g):
    if VANILLA:
        return t08.stones(g)
    cells = ' '.join(f'{p[0]} {p[1]} {p[2]}' for p in (OUT, BARREL))
    text = ' '.join(g.command(f'ntmtest fluids {cells}').get('chat', []))
    m = re.search(r'fluids=(\S+)', text)
    out = {}
    for part in (m[1].split(';') if m else []):
        at, tanks = part.split('=')
        out[at] = {} if tanks == 'none' else {f: int(n) for f, n in (t.rsplit(':', 1) for t in tanks.split('+'))}
    tars = sum(v.get(TAR, 0) for v in out.values())
    oil = sum(n for v in out.values() for f, n in v.items() if f.lower() == 'coaloil')
    out['tars'], out['oil'] = tars, oil
    out['total'] = tars * 100 + oil
    out['readable'] = bool(m)
    return out


def prepare(g):
    if VANILLA:
        return t08.prepare(g)
    g.ctl('wait-ready 120')
    g.close()
    g.command('forceload add 16 0 47 15')
    time.sleep(2)
    g.command(f'fill 26 {Y - 2} 2 38 {Y + 6} 14 minecraft:air')
    g.command(f'fill 26 {Y - 3} 2 38 {Y - 3} 14 minecraft:stone')
    g.command('clear @s')
    g.command('gamemode creative @s')
    g.command('ntmrbmk dial dialOutgasserSpeedMod 5')
    g.command(f'setblock {OUT[0]} {OUT[1]} {OUT[2]} ntm:rbmk_outgasser[facing=north,honey_level=0]')
    g.command(f'setblock {ROD[0]} {ROD[1]} {ROD[2]} ntm:rbmk_rod[facing=north,honey_level=0]')
    for p in DUCTS:
        g.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:fluid_duct_neo')
    g.command(f'setblock {BARREL[0]} {BARREL[1]} {BARREL[2]} ntm:barrel_steel[facing=south]')
    time.sleep(1.5)
    g.command(f'item replace block {OUT[0]} {OUT[1]} {OUT[2]} container.0 with {TAR} 4')
    t08.stand(g, (32, Y - 2, 6))
    type_duct(g, 'coaloil', DUCTS, (32, Y - 1, 6))
    g.command('clear @s')
    g.command('tp @s 30.5 100 3.5')
    time.sleep(3)
    base = count(g)
    g.evidence['base'] = base
    g.check('base', base['readable'] and base['total'] == 400 and base['tars'] == 4, base)
    g.ctl('save')


def act(g):
    if VANILLA:
        return t08.act(g)
    g.ctl('wait-ready 120')
    g.command('gamemode survival @s')
    before = count(g)
    g.evidence['before_act'] = before
    g.check('start_with_all', before['total'] == 400, before)
    g.command('give @s ntm:rbmk_fuel_po210be 1')
    g.command(f'tp @s {ROD[0] + .5} {Y} {ROD[2] - 2.5} 0 0')
    time.sleep(.6)
    g.ctl('select ntm:rbmk_fuel_po210be')
    g.ctl(f'interact {ROD[0]} {ROD[1]} {ROD[2]} north')
    g.command('gamemode creative @s')
    g.command('tp @s 30.5 100 3.5')
    g.command(f'setblock 28 {Y} 3 minecraft:cobblestone')
    g.command(f'setblock 35 {Y} 3 minecraft:cobblestone')
    time.sleep(3)
    moving = count(g)
    g.evidence['after_act'] = moving
    g.check('processing_at_the_save', moving['total'] == 400 and 0 < moving['tars'] < 4, [before, moving])
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
    if VANILLA:
        return t08.verify(g, again)
    g.ctl('wait-ready 120')
    time.sleep(2)
    now = count(g)
    key = 'counted' + ('_again' if again else '')
    g.evidence[key] = now
    g.check('one_tar_one_recipe' + ('_again' if again else ''), now['readable'] and now['total'] == 400, now)


def drain(g):
    g.ctl('wait-ready 120')
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t09c-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    g.evidence['ok'] = True
    g.save()

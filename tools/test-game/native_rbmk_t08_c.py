#!/usr/bin/env python3
"""T08 cut (C, `rbmk-plano` T08): fluid moving through an RBMK cooler across a chunk border when the save is cut.

Modes prepare/act/verify/again/drain; the runner (run-rbmk-t08-c-proof.py) owns the process and the cut.
  prepare  a barrel of 16 000 mB of cold cryogel in chunk (1, 0), a duct to the cooler's bottom in chunk (2, 0), the
           cooler's top back by ducts to an empty barrel in chunk (1, 0); the barrel's first duct left untyped
  act      that duct typed: cryogel flows (cold in, used out, 50 mB a tick); after three seconds, the save the runner
           cuts, the flow still going
  verify   the source barrel, the cooler's two tanks and the sink barrel, read in one go (`ntmtest fluids`): 16 000 mB
           in all, none lost, none made
With NTM_CRASH_VANILLA=1 the vanilla control: 64 stones moved by two hoppers from a chest in chunk (1, 0) (over the
first hopper) to a chest in chunk (2, 0), the hoppers unlocked in the act and locked again before counting.
"""
import os
import re
import sys
import time
from native_game import Game
from native_stations import fill_barrel, type_duct

Y = 101
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
TOTAL = 64 if VANILLA else 16_000
# NTM: source and sink barrels in chunk (1, 0), the cooler in chunk (2, 0).
SRC, D1, D2, D3, COOLER = (30, Y - 1, 8), (31, Y - 1, 8), (32, Y - 1, 8), (33, Y - 1, 8), (33, Y, 8)
TOP = [(33, Y + 4, 8), (32, Y + 4, 8), (31, Y + 4, 8)]
SINK = (30, Y + 4, 8)
# Vanilla: a chest over the first hopper (a hopper pulls from the block above it), that hopper pushing east into the
# second, which pushes into a chest in the next chunk; the lock under the first hopper.
CHEST_A, H1, H2, CHEST_B, LOCK = (31, Y, 8), (31, Y - 1, 8), (32, Y - 1, 8), (33, Y - 1, 8), (31, Y - 2, 8)


def fluids(g):
    cells = ' '.join(f'{p[0]} {p[1]} {p[2]}' for p in (SRC, COOLER, SINK))
    text = ' '.join(g.command(f'ntmtest fluids {cells}').get('chat', []))
    m = re.search(r'fluids=(\S+)', text)
    out = {}
    for part in (m[1].split(';') if m else []):
        at, tanks = part.split('=')
        out[at] = {} if tanks == 'none' else {f: int(n) for f, n in (t.rsplit(':', 1) for t in tanks.split('+'))}
    out['total'] = sum(sum(v.values()) for k, v in out.items() if isinstance(v, dict))
    out['readable'] = bool(m) and all(f'{p[0]},{p[1]},{p[2]}' in out for p in (SRC, COOLER, SINK))
    return out


def items(g, p):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    return sum(int(n) for n in re.findall(r'count: ?(\d+)', text))


def stones(g):
    out = {name: items(g, p) for name, p in (('a', CHEST_A), ('h1', H1), ('h2', H2), ('b', CHEST_B))}
    out['total'] = sum(out.values())
    out['readable'] = True
    return out


def count(g):
    return stones(g) if VANILLA else fluids(g)


def stand(g, cell):
    g.command(f'setblock {cell[0]} {cell[1]} {cell[2]} minecraft:stone')
    g.command(f'tp @s {cell[0] + .5} {cell[1] + 1} {cell[2] + .5}')
    time.sleep(1)


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    g.command('forceload add 16 0 47 15')
    time.sleep(2)
    g.command(f'fill 26 {Y - 2} 2 38 {Y + 6} 14 minecraft:air')
    g.command(f'fill 26 {Y - 3} 2 38 {Y - 3} 14 minecraft:stone')
    g.command('clear @s')
    g.command('gamemode creative @s')
    if VANILLA:
        g.command(f'setblock {LOCK[0]} {LOCK[1]} {LOCK[2]} minecraft:redstone_block')
        g.command(f'setblock {CHEST_A[0]} {CHEST_A[1]} {CHEST_A[2]} minecraft:chest')
        g.command(f'setblock {H1[0]} {H1[1]} {H1[2]} minecraft:hopper[facing=east]')
        g.command(f'setblock {H2[0]} {H2[1]} {H2[2]} minecraft:hopper[facing=east]')
        g.command(f'setblock {CHEST_B[0]} {CHEST_B[1]} {CHEST_B[2]} minecraft:chest')
        g.command(f'item replace block {CHEST_A[0]} {CHEST_A[1]} {CHEST_A[2]} container.0 with minecraft:stone 64')
    else:
        g.command(f'setblock {COOLER[0]} {COOLER[1]} {COOLER[2]} ntm:rbmk_cooler[facing=north,honey_level=0]')
        for p in (SRC, SINK):
            g.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:barrel_steel[facing=south]')
        for p in (D1, D2, D3, *TOP):
            g.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:fluid_duct_neo')
        time.sleep(1.5)
        stand(g, (SRC[0], SRC[1] - 1, SRC[2] - 2))
        fill_barrel(g, SRC, 1, 'ntm:fluid_barrel_full__perfluoromethyl_cold')
        type_duct(g, 'perfluoromethyl_cold', [D2, D3], (32, Y - 1, 6))
        stand(g, (32, Y + 3, 6))
        type_duct(g, 'perfluoromethyl', TOP, (32, Y + 4, 6))
        g.command('clear @s')
    g.command('tp @s 30.5 100 3.5')
    time.sleep(3)
    base = count(g)
    g.evidence['base'] = base
    g.check('base', base['readable'] and base['total'] == TOTAL, base)
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    before = count(g)
    g.evidence['before_act'] = before
    g.check('start_with_all', before['total'] == TOTAL, before)
    if VANILLA:
        g.command(f'setblock {LOCK[0]} {LOCK[1]} {LOCK[2]} minecraft:air')
    else:
        type_duct(g, 'perfluoromethyl_cold', [D1], (31, Y - 1, 6))
        g.command('clear @s')
    # A block of each chunk changes in both worlds, so the cuts by chunk record land alike.
    g.command(f'setblock 28 {Y} 3 minecraft:cobblestone')
    g.command(f'setblock 35 {Y} 3 minecraft:cobblestone')
    time.sleep(3)
    if VANILLA:
        # Counted with the hoppers held a moment (several reads apart, a moving stone could count twice), then let go:
        # the save below still finds them moving.
        g.command(f'setblock {LOCK[0]} {LOCK[1]} {LOCK[2]} minecraft:redstone_block')
        time.sleep(.6)
        moving = count(g)
        g.command(f'setblock {LOCK[0]} {LOCK[1]} {LOCK[2]} minecraft:air')
        time.sleep(1)
    else:
        moving = count(g)
    g.evidence['after_act'] = moving
    g.check('flowing_at_the_save', moving['total'] == TOTAL and moving != before, [before, moving])
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
    if g.evidence['survived_save']:
        try:
            g.evidence['after_save'] = count(g)
        except Exception as error:  # the game died at the cut after all
            g.evidence['after_save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    time.sleep(2)
    if VANILLA:
        # The hoppers locked again: nothing moves while they are counted.
        g.command(f'setblock {LOCK[0]} {LOCK[1]} {LOCK[2]} minecraft:redstone_block')
        time.sleep(1)
    now = count(g)
    key = 'counted' + ('_again' if again else '')
    g.evidence[key] = now
    g.check('nothing_lost_nothing_made' + ('_again' if again else ''), now['readable'] and now['total'] == TOTAL, now)


def drain(g):
    g.ctl('wait-ready 120')
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t08c-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)
    g.evidence['ok'] = True
    g.save()

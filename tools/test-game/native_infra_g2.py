#!/usr/bin/env python3
"""INFRA-G cause check: a LAN guest uses a multiblock's dummy cells with the host out of the way, then with the host
standing where the guest stands (the T10 check's setup: the host was teleported to the same front spot first and stayed
there, so the guest's eye started inside the host's body and its crosshair picked the host, never the block).

Per machine: the host uses the front cell (a dummy) and steps aside; the guest uses the front cell, a dummy on the
east side and a dummy on top; then the host goes back to the guest's front spot and the guest tries the front again.
The evidence lists each use's block, whether a screen opened and the guest's inventory around the uses.

Usage: native_infra_g2.py HOST_FOLDER GUEST_FOLDER  (both clients running with control.sock; the guest joined)
"""
import sys
import time
from pathlib import Path
from native_game import Game

HOST, GUEST = 'Host', 'Guest'
Y = 101
MACHINES = ['ntm:rbmk_console', 'ntm:machine_electrolyser', 'ntm:machine_assembly_machine', 'ntm:machine_chemical_plant']


def opened(g):
    deadline = time.monotonic() + 4
    while time.monotonic() < deadline:
        if g.ctl('screen').get('screen') == 'container':
            return True
        time.sleep(.2)
    return False


def tp(host, name, at, yaw, pitch=0, y=Y):
    host.command(f'tp {name} {at[0]} {y} {at[1]} {yaw} {pitch}')
    time.sleep(.8)


def use(host, g, name, cell, at, yaw, face, y=Y, pitch=0):
    g.close()
    time.sleep(.5)
    tp(host, name, at, yaw, pitch, y)
    try:
        r = g.ctl(f'interact {cell[0]} {cell[1]} {cell[2]} {face}')
        block = (r.get('block') or {}).get('block')
    except RuntimeError as error:
        return {'error': str(error)[:200]}
    # What the crosshair names after the aim: the block, or the host's body when the guest's eye is inside it.
    target = g.ctl('looking-at')
    out = {'cell': list(cell), 'block': block, 'looking_at': target, 'opened': opened(g),
           'inventory_change': r.get('inventory_change')}
    g.close()
    return out


def is_dummy(host, p):
    chat = ' '.join(host.command(f'execute if block {p[0]} {p[1]} {p[2]} ntm:dummy_machine').get('chat', []))
    return 'passed' in chat.lower()


def main():
    host, guest = Game(Path(sys.argv[1]), 'infra-g2-host'), Game(Path(sys.argv[2]), 'infra-g2-guest')
    host.ctl('wait-ready 120')
    host.close()
    host.command('forceload add 0 0 95 31')
    time.sleep(3)
    host.command(f'fill 0 {Y - 1} 0 95 {Y + 10} 31 minecraft:air')
    host.command(f'fill 0 {Y - 1} 0 95 {Y - 1} 31 minecraft:stone')
    for name in (HOST, GUEST):
        host.command(f'gamemode creative {name}')
        host.command(f'effect give {name} minecraft:resistance 1200 4 true')
        host.command(f'clear {name}')
    # The guest holds a stack of cobblestone: an opening never moves it.
    host.command(f'give {GUEST} minecraft:cobblestone 7')
    results = {}
    for n, item in enumerate(MACHINES):
        place = (8 + 20 * n, Y, 10)
        host.command(f'clear {HOST}')
        host.command(f'give {HOST} {item} 1')
        tp(host, HOST, (place[0] + .5, place[2] - 3.5), 0)
        host.ctl(f'place {place[0]} {place[1]} {place[2]} --item {item}')
        time.sleep(1.5)
        cells = [(x, y, z) for x in range(place[0] - 3, place[0] + 4) for y in range(Y, Y + 3) for z in range(place[2] - 1, place[2] + 6)]
        core = next((p for p in cells if 'has the following block data' in ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))), None)
        front = (place[0] + .5, place[2] - 2.5)
        aside = (place[0] - 6.5, place[2] - 6.5)
        r = {'core': core, 'host_front': use(host, host, HOST, place, front, 0, 'north')}
        tp(host, HOST, aside, 0)
        r['guest_front'] = use(host, guest, GUEST, place, front, 0, 'north')
        if core is not None:
            # The eastmost dummy on the core's row, used from the east; the topmost dummy above the core, from above.
            east = next((c for c in ((core[0] + d, Y, core[2]) for d in range(4, 0, -1)) if is_dummy(host, c)), None)
            if east is not None:
                r['guest_east'] = use(host, guest, GUEST, east, (east[0] + 2.5, east[2] + .5), 90, 'east')
            top = next((c for c in ((core[0], Y + d, core[2]) for d in range(4, 0, -1)) if is_dummy(host, c)), None)
            if top is not None:
                r['guest_top'] = use(host, guest, GUEST, top, (top[0] + .5, top[2] - 1.5), 0, 'up', y=top[1] + 1, pitch=60)
        # The T10 check's setup: the host back at the guest's spot.
        tp(host, HOST, front, 0)
        r['guest_front_host_inside'] = use(host, guest, GUEST, place, front, 0, 'north')
        tp(host, HOST, aside, 0)
        results[item] = r
        host.evidence['results'] = results
        host.save()
    host.evidence['guest_counts_end'] = guest.ctl('inventory').get('counts')
    host.evidence['ok'] = True
    host.save()
    guest.evidence['ok'] = True
    guest.save()


if __name__ == '__main__':
    main()

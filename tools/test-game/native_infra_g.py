#!/usr/bin/env python3
"""INFRA-G check (T10 review P3-1): which multiblocks a LAN guest opens through a dummy cell. Each machine is placed
by hand from the south; the host, then the guest, uses the cell they clicked (a dummy when the core sits further in),
then the guest uses the core itself. The evidence lists, per machine, the cell's block and whether a screen opened.

Usage: native_infra_g.py HOST_FOLDER GUEST_FOLDER  (both clients running with control.sock; the guest joined)
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


def use(host, g, name, cell, at, yaw, face):
    g.close()
    time.sleep(.5)
    host.command(f'clear {name}')
    host.command(f'tp {name} {at[0]} {Y} {at[1]} {yaw} 0')
    time.sleep(.8)
    try:
        r = g.ctl(f'interact {cell[0]} {cell[1]} {cell[2]} {face}')
        block = (r.get('block') or {}).get('block')
    except RuntimeError as error:
        return {'error': str(error)[:160]}
    out = {'block': block, 'opened': opened(g)}
    g.close()
    return out


def main():
    host, guest = Game(Path(sys.argv[1]), 'infra-g-host'), Game(Path(sys.argv[2]), 'infra-g-guest')
    host.ctl('wait-ready 120')
    host.close()
    host.command('forceload add 0 0 95 31')
    time.sleep(3)
    host.command(f'fill 0 {Y - 1} 0 95 {Y + 10} 31 minecraft:air')
    host.command(f'fill 0 {Y - 1} 0 95 {Y - 1} 31 minecraft:stone')
    for name in (HOST, GUEST):
        host.command(f'gamemode creative {name}')
        host.command(f'effect give {name} minecraft:resistance 1200 4 true')
    results = {}
    for n, item in enumerate(MACHINES):
        place = (8 + 20 * n, Y, 10)
        host.command(f'clear {HOST}')
        host.command(f'give {HOST} {item} 1')
        host.command(f'tp {HOST} {place[0] + .5} {Y} {place[2] - 3.5} 0 0')
        time.sleep(.6)
        host.ctl(f'place {place[0]} {place[1]} {place[2]} --item {item}')
        time.sleep(1.5)
        cells = [(x, y, z) for x in range(place[0] - 3, place[0] + 4) for y in range(Y, Y + 3) for z in range(place[2] - 1, place[2] + 6)]
        core = next((p for p in cells if 'has the following block data' in ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))), None)
        front = (place[0] + .5, place[2] - 2.5)
        r = {'core': core,
             'host_front': use(host, host, HOST, place, front, 0, 'north'),
             'guest_front': use(host, guest, GUEST, place, front, 0, 'north')}
        if core is not None:
            # The core from above (a machine's top may be a dummy: then from wherever its own face shows).
            for label, at, yaw, face in (('guest_core_south', (core[0] + .5, core[2] + 5.5), 180, 'south'),
                                          ('guest_core_west', (core[0] - 4.5, core[2] + .5), -90, 'west'),
                                          ('guest_core_east', (core[0] + 5.5, core[2] + .5), 90, 'east')):
                r[label] = use(host, guest, GUEST, core, at, yaw, face)
                if r[label].get('opened'):
                    break
        results[item] = r
        host.evidence['results'] = results
        host.save()
    host.evidence['ok'] = True
    host.save()
    guest.evidence['ok'] = True
    guest.save()


if __name__ == '__main__':
    main()

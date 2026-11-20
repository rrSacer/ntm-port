#!/usr/bin/env python3
"""T11 proof (`rbmk-plano` T11): the RBMK crane on a host and a LAN guest, as the server keeps the crane console and the
columns (`data get block ... ntm`). The host places the crane console by hand and links it with the RBMK tool (sneaking:
the channel, then the console); a hot rod (hull 1200 °C, named "Lot 7") sits in a fuel channel, a storage column two
cells beside it. The guest, standing at the console, drives the crane with the arrow keys and Enter: the rod out of
the channel, into the storage (its queue takes it down), out again and back into the channel, whole each time (its
name, its xenon); a blank under the hook takes nothing. The console is then broken with the rod held: the rod drops
whole (the Java's console loses it, T11 decision 2). On reopen the crane's link and spans are as saved.

Usage: native_rbmk_t11.py HOST_FOLDER GUEST_FOLDER  (both clients running with control.sock; the guest joined)
       native_rbmk_t11.py HOST_FOLDER - reopen
"""
import json
import math
import re
import sys
import time
from pathlib import Path
from native_game import Game

HOST, GUEST = 'Host', 'Guest'
Y = 101
PLACE = (40, Y, 4)
CHANNEL = (40, Y, 14)
STORE = (42, Y, 14)
# Where a player waits while the other works (out of the console's area and of every aim).
PARK = (34.5, Y, 1.5)
BLANK = (40, Y, 16)
ROD = 'ntm:rbmk_fuel_ueu[custom_data={hull:1200.0d,core:1300.0d,xenon:2.5d,yield:500000.0d},custom_name=\'"Lot 7"\']'


def column(id_):
    return f'ntm:{id_}[facing=north,honey_level=0]'


def raw(host, p):
    return ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]}').get('chat', []))


def machine(host, p):
    """The machine's JSON (the block entity's `ntm`), or None."""
    text = ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))
    m = re.search(r"block data: '(.*)'$", text)
    if not m:
        return None
    try:
        return json.loads(m[1].replace("\\'", "'"))
    except ValueError:
        return None


def crane(host, core):
    m = machine(host, core)
    return (m or {}).get('data')


def bits(v):
    import struct
    return struct.unpack('<d', struct.pack('<Q', int(v)))[0]


def items(host, p):
    """The block entity's items as SNBT text (`Items` of `data get block`)."""
    text = raw(host, p)
    m = re.search(r'Items: \[(.*)\]', text)
    return m[1] if m else ''


def lot7(text):
    return 'Lot 7' in text and 'xenon: 2.5d' in text


class Proof:
    def __init__(self, host, guest):
        self.host, self.guest = host, guest
        self.players = {HOST: host, GUEST: guest}
        self.core = None
        self.facing = None

    def check(self, name, ok, actual):
        self.host.check(name, ok, actual)

    def wait(self, read, predicate, timeout=30):
        deadline = time.monotonic() + timeout
        s = read()
        while time.monotonic() < deadline:
            s = read()
            if predicate(s):
                return s
            time.sleep(.3)
        return s

    def park(self, name):
        """The other player out of the way: one standing where this one goes takes this one's aim, and the crane takes
        the keys of the first player in its console's area."""
        other = GUEST if name == HOST else HOST
        if self.players.get(other) is not None:
            self.players[other].close()
            self.host.command(f'tp {other} {PARK[0]} {PARK[1]} {PARK[2]} 0 0')
            time.sleep(.4)

    def sneak_use(self, p, item, face='north', away=(0.5, -2.5)):
        h = self.host
        h.close()
        self.park(HOST)
        h.command(f'clear {HOST}')
        h.command(f'give {HOST} {item} 1')
        h.command(f'tp {HOST} {p[0] + away[0]} {Y} {p[2] + away[1]} 0 0')
        time.sleep(.6)
        h.ctl(f'select {item}')
        h.ctl('key shift down')
        time.sleep(.3)
        h.ctl(f'interact {p[0]} {p[1]} {p[2]} {face}')
        time.sleep(.5)
        h.ctl('key shift up')
        time.sleep(.5)

    def setup(self):
        h = self.host
        h.ctl('wait-ready 120')
        h.close()
        h.command('forceload add 30 0 54 31')
        time.sleep(3)
        h.command(f'fill 32 {Y - 1} 0 52 {Y + 20} 24 minecraft:air')
        h.command(f'fill 32 {Y - 1} 0 52 {Y - 1} 24 minecraft:stone')
        for name in (HOST, GUEST):
            h.command(f'gamemode creative {name}')
            h.command(f'effect give {name} minecraft:resistance 1200 4 true')
        for p, id_ in ((CHANNEL, 'rbmk_rod'), (STORE, 'rbmk_storage'), (BLANK, 'rbmk_blank')):
            h.command(f'setblock {p[0]} {p[1]} {p[2]} {column(id_)}')
        time.sleep(2)
        h.command(f'item replace block {CHANNEL[0]} {CHANNEL[1]} {CHANNEL[2]} container.0 with {ROD}')
        # The console, placed by hand from the south, facing it.
        h.command(f'give {HOST} ntm:rbmk_crane_console 1')
        h.command(f'tp {HOST} {PLACE[0] + .5} {Y} {PLACE[2] - 3.5} 0 0')
        time.sleep(.6)
        self.host.evidence['place'] = h.ctl(f'place {PLACE[0]} {PLACE[1]} {PLACE[2]} --item ntm:rbmk_crane_console')
        time.sleep(1.5)
        cells = [(x, y, z) for x in range(PLACE[0] - 2, PLACE[0] + 3) for y in range(Y, Y + 2) for z in range(PLACE[2] - 2, PLACE[2] + 3)]
        for p in cells:
            m = machine(h, p)
            if m and m.get('block') == 'ntm:rbmk_crane_console':
                self.core, self.facing = p, m.get('facing')
                break
        self.check('the_crane_console_stands', self.core is not None and lot7(items(h, CHANNEL)), [self.core, items(h, CHANNEL)])
        if self.core is None:
            raise AssertionError('no crane console core')
        self.host.evidence['core'] = self.core

    def link(self):
        self.sneak_use(CHANNEL, 'ntm:rbmk_tool')
        g = self.host
        g.close()
        self.park(HOST)
        g.command(f'tp {HOST} {PLACE[0] + .5} {Y} {PLACE[2] - 2.5} 0 0')
        time.sleep(.6)
        g.ctl('key shift down')
        time.sleep(.3)
        g.ctl(f'interact {PLACE[0]} {PLACE[1]} {PLACE[2]} north')
        time.sleep(.5)
        g.ctl('key shift up')
        s = self.wait(lambda: crane(self.host, self.core) or {}, lambda s: s.get('set_up'))
        self.check('the_tool_links_the_crane', s.get('set_up') and s.get('center', [0])[0] == CHANNEL[0] and s.get('center', [0, 0, 0])[2] == CHANNEL[2]
                   and s.get('span_f', 0) > 0 and s.get('height') == 7, s)
        self.host.evidence['linked'] = s

    def stand(self, name):
        """At the console: two cells in front of its core (its facing's side), where the Java's area reaches."""
        d = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}[self.facing]
        x, z = self.core[0] + .5 + d[0] * 1.6, self.core[2] + .5 + d[1] * 1.6
        self.players[name].close()
        self.park(name)
        self.host.command(f'clear {name}')
        self.host.command(f'tp {name} {x} {Y} {z} 0 0')
        time.sleep(.8)

    def cell(self, s):
        """`getColumnAtPos`'s cell from the crane's saved state."""
        d = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}[self.facing]
        left = (d[1], -d[0])
        pf, pl = bits(s['pos_front']), bits(s['pos_left'])
        c = s['center']
        return (math.floor(c[0] - d[0] * pf - left[0] * pl + .5), math.floor(c[2] - d[1] * pf - left[1] * pl + .5))

    def drive(self, name, target):
        """Holds the arrow keys until the hook is over `target`'s cell, then lets them go."""
        g = self.players[name]
        for _ in range(2):
            for key in ('up', 'down', 'left', 'right'):
                s = crane(self.host, self.core)
                at = self.cell(s)
                if at == (target[0], target[2]):
                    return s
                g.ctl(f'key key.keyboard.{key} down')
                deadline = time.monotonic() + 12
                best = None
                while time.monotonic() < deadline:
                    s = crane(self.host, self.core)
                    at = self.cell(s)
                    dist = abs(at[0] - target[0]) + abs(at[1] - target[2])
                    if best is not None and dist > best:
                        break
                    best = dist if best is None else min(best, dist)
                    if dist == 0:
                        break
                    time.sleep(.15)
                g.ctl(f'key key.keyboard.{key} up')
                time.sleep(.4)
        return crane(self.host, self.core)

    def load(self, name):
        g = self.players[name]
        g.ctl('key key.keyboard.enter down')
        time.sleep(.3)
        g.ctl('key key.keyboard.enter up')
        time.sleep(3.5)

    def held(self):
        return items(self.host, self.core)

    def carry(self):
        self.stand(GUEST)
        s = self.drive(GUEST, CHANNEL)
        self.check('the_hook_over_the_channel', self.cell(s) == (CHANNEL[0], CHANNEL[2]), s)
        self.load(GUEST)
        self.check('the_rod_out_of_the_channel_hot', lot7(self.held()) and not lot7(items(self.host, CHANNEL)),
                   [self.held(), items(self.host, CHANNEL)])
        s = self.drive(GUEST, STORE)
        self.load(GUEST)
        stored = self.wait(lambda: items(self.host, STORE), lot7, 10)
        self.check('the_rod_into_the_storage', lot7(stored) and not lot7(self.held()), [stored, self.held()])
        # The queue brings it to slot 0 (a pass every 10 ticks); then the crane takes it again.
        self.wait(lambda: items(self.host, STORE), lambda t: 'Slot: 0b' in t, 15)
        self.load(GUEST)
        self.check('the_rod_out_of_the_storage', lot7(self.held()) and not lot7(items(self.host, STORE)), [self.held(), items(self.host, STORE)])
        # A blank takes nothing: the rod stays held.
        self.drive(GUEST, BLANK)
        self.load(GUEST)
        self.check('a_blank_takes_nothing', lot7(self.held()), self.held())
        self.drive(GUEST, CHANNEL)
        self.load(GUEST)
        self.check('the_rod_back_into_the_channel_whole', lot7(items(self.host, CHANNEL)) and not lot7(self.held()),
                   [items(self.host, CHANNEL), self.held()])

    def break_holding(self):
        """The rod taken once more, then the console broken by the host: the rod drops whole on the ground."""
        self.load(GUEST)
        self.check('held_before_the_break', lot7(self.held()), self.held())
        self.players[GUEST].close()
        before = ' '.join(self.host.command(f'execute if entity @e[type=item,distance=..12,x={self.core[0]},y={self.core[1]},z={self.core[2]}]').get('chat', []))
        self.host.evidence['items_before_break'] = before
        self.host.command(f'setblock {self.core[0]} {self.core[1]} {self.core[2]} minecraft:air destroy')
        time.sleep(2)
        drops = ' '.join(self.host.command(f'data get entity @e[type=item,limit=1,sort=nearest,x={self.core[0]},y={self.core[1]},z={self.core[2]}] Item').get('chat', []))
        self.check('the_broken_console_drops_the_rod_whole', lot7(drops), drops)

    def run(self):
        self.setup()
        self.host.evidence['phase'] = 'link'
        self.link()
        self.host.evidence['phase'] = 'carry'
        self.carry()
        self.host.evidence['crane'] = crane(self.host, self.core)
        self.host.ctl('save')
        self.host.evidence['phase'] = 'break'
        self.break_holding()
        self.host.ctl('save')
        self.host.evidence['ok'] = True
        self.host.save()

    def reopen(self):
        """After the world opened again: the rod in the channel or on the ground as left, never twice."""
        self.host.ctl('wait-ready 120')
        self.host.evidence['phase'] = 'reopen'
        time.sleep(3)
        channel = items(self.host, CHANNEL)
        ground = ' '.join(self.host.command(f'data get entity @e[type=item,limit=1,sort=nearest,x={PLACE[0]},y={Y},z={PLACE[2]}] Item').get('chat', []))
        self.check('one_rod_after_the_reopen', lot7(ground) != lot7(channel), {'channel': channel, 'ground': ground})
        self.host.evidence['ok'] = True
        self.host.save()


if __name__ == '__main__':
    if sys.argv[3:] == ['reopen']:
        Proof(Game(Path(sys.argv[1]), 'rbmk-t11-reopen'), None).reopen()
        sys.exit(0)
    host, guest = Game(Path(sys.argv[1]), 'rbmk-t11-host'), Game(Path(sys.argv[2]), 'rbmk-t11-guest')
    Proof(host, guest).run()

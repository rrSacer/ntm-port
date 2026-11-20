#!/usr/bin/env python3
"""T07 proof (`rbmk-plano` T07): the five control rods (manual, moderated, ReaSim, automatic, automatic ReaSim) set
through their screens on a host and a LAN guest, as the server keeps them (`data get block ... ntm`): the target a
level button sets, the group a colour button toggles, the automatic rod's target from its curve over the column's
heat, the level following at `0.00277` a tick; the two ReaSim rods move on real power (a battery under them, fed by a
creative battery, through a cable) and the battery pays only while they move; a manual rod at 0 stops a ReaSim
source's flux from reaching the channel behind it, and once out lets it through.

Usage: native_rbmk_t07.py HOST_FOLDER GUEST_FOLDER  (both clients running with control.sock; the guest joined)
"""
import re
import struct
import sys
import time
from pathlib import Path
from native_game import Game

HOST, GUEST = 'Host', 'Guest'
Y = 101
SPEED = 0.00277
# The five rods in a row, two blocks apart; the ReaSim ones on a cable fed from a battery.
RODS = {'manual': (40, Y, 8), 'moderated': (42, Y, 8), 'reasim': (44, Y, 8), 'auto': (46, Y, 8), 'reasim_auto': (48, Y, 8)}
BLOCKS = {'manual': 'rbmk_control', 'moderated': 'rbmk_control_mod', 'reasim': 'rbmk_control_reasim',
          'auto': 'rbmk_control_auto', 'reasim_auto': 'rbmk_control_reasim_auto'}
BATTERY = (46, Y - 1, 9)
# The flux line: a ReaSim source, the manual rod of the line, a uranium channel.
SOURCE, LINE_ROD, TARGET = (40, Y, 14), (41, Y, 14), (42, Y, 14)
INVENTORY = (8, 104)
# GUIRBMKControl: the levels (118, 26 + 11k, 30 x 10), the groups (28, 26 + 11k, 12 x 10); GUIRBMKControlAuto: the
# fields (30, 27 + 11i), the save (28, 70).
LEVEL = [(130, 31 + 11 * k) for k in range(5)]
GROUP = [(33, 31 + 11 * k) for k in range(5)]
FIELD = [(32, 29 + 11 * i) for i in range(4)]
SAVE = (40, 75)


def column(id_):
    return f'ntm:{id_}[facing=north,honey_level=0]'


def bits(v):
    return struct.unpack('<d', struct.pack('<Q', int(v)))[0]


def state(host, p):
    """The column's model as the server keeps it: its control state (doubles by their bits) and last flux."""
    text = ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))
    out = {}
    m = re.search(r'\\?"control\\?":\{(.*?)\}', text)
    if m:
        f = dict(re.findall(r'\\?"(\w+)\\?":(-?\d+|null|true|false)', m[1]))
        out = {k: bits(f[k]) for k in ('level', 'target_level', 'starting_level') if k in f}
        out['color'] = None if f.get('color') in (None, 'null') else int(f['color'])
    lf = re.search(r'\\?"last_flux_quantity\\?":(\d+)', text)
    if lf:
        out['last_flux'] = bits(lf[1])
    return out


def power(host, p):
    """The power a battery or a control rod's buffer holds now, as the server keeps it (`ntmtest power`: the block
    entity's copy of a battery lags, a rod's buffer is never saved)."""
    text = ' '.join(host.command(f'ntmtest power {p[0]} {p[1]} {p[2]}').get('chat', []))
    m = re.search(r'power=(-?\d+)', text)
    return int(m[1]) if m else None


def battery_power(host):
    return power(host, BATTERY)


class Proof:
    def __init__(self, host, guest):
        self.host, self.guest = host, guest
        self.players = {HOST: host, GUEST: guest}

    def check(self, name, ok, actual):
        self.host.check(name, ok, actual)

    def origin(self, g):
        w = next(w for w in g.ctl('ui').get('widgets', []) if w.get('id') == 'slot-9')
        return w['bounds'][0] - INVENTORY[0], w['bounds'][1] - INVENTORY[1]

    def click(self, g, at):
        ox, oy = self.origin(g)
        g.ctl(f'click-at {ox + at[0]} {oy + at[1]}')
        time.sleep(.6)

    def open(self, name, p):
        g = self.players[name]
        g.close()
        self.host.command(f'tp {name} {p[0] + .5} {p[1]} {p[2] - 2.5} 0 0')
        time.sleep(.6)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if g.ctl('screen').get('screen') == 'container':
                time.sleep(.8)
                return g
            time.sleep(.2)
        raise AssertionError(f'{name}: the screen at {p} did not open')

    def write(self, g, field, text):
        self.click(g, FIELD[field])
        g.ctl('key key.keyboard.end tap')
        for _ in range(5):
            g.ctl('key backspace tap')
        g.ctl(f'type {text}')

    def wait_level(self, p, predicate, timeout=60):
        deadline = time.monotonic() + timeout
        s = state(self.host, p)
        while time.monotonic() < deadline:
            s = state(self.host, p)
            if predicate(s):
                return s
            time.sleep(1)
        return s

    def setup(self):
        h = self.host
        h.ctl('wait-ready 120')
        h.close()
        h.command('forceload add 30 0 64 24')
        time.sleep(3)
        h.command('fill 36 97 2 62 108 22 minecraft:air')
        h.command('fill 36 96 2 62 96 22 minecraft:stone')
        h.command('fill 36 100 2 62 100 22 minecraft:stone')
        for name in (HOST, GUEST):
            h.command(f'gamemode creative {name}')
            h.command(f'effect give {name} minecraft:resistance 1200 4 true')
        for kind, p in RODS.items():
            h.command(f'setblock {p[0]} {p[1]} {p[2]} {column(BLOCKS[kind])}')
        # The ReaSim rods' cable (under each, joined along the row) and the battery beside it, giving (a redstone block
        # under it) and kept full (a creative battery in its slot, through a hopper).
        h.command(f'fill 44 {Y - 1} 8 48 {Y - 1} 8 ntm:red_cable')
        h.command(f'setblock 46 {Y - 1} 9 ntm:machine_battery[facing=south]')
        h.command(f'setblock 46 {Y - 2} 9 minecraft:redstone_block')
        h.command(f'setblock 46 {Y} 9 minecraft:hopper[facing=down]')
        h.command(f'item replace block 46 {Y} 9 container.0 with ntm:battery_creative 1')
        # The flux line.
        h.command(f'setblock {SOURCE[0]} {Y} {SOURCE[2]} {column("rbmk_rod_reasim")}')
        h.command(f'setblock {LINE_ROD[0]} {Y} {LINE_ROD[2]} {column("rbmk_control")}')
        h.command(f'setblock {TARGET[0]} {Y} {TARGET[2]} {column("rbmk_rod")}')
        time.sleep(2)
        for p, rod in ((SOURCE, 'ntm:rbmk_fuel_po210be'), (TARGET, 'ntm:rbmk_fuel_ueu')):
            h.command(f'clear {HOST}')
            h.command(f'give {HOST} {rod} 1')
            h.command(f'tp {HOST} {p[0] + .5} {p[1]} {p[2] - 2.5} 0 0')
            time.sleep(.6)
            h.ctl(f'select {rod}')
            h.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
            time.sleep(.6)
        h.command(f'clear {HOST}')
        time.sleep(2)

    def manual(self, name, kind, k, expect_target):
        """A level button: the target, the start where the rod was; the rod on its way at the Java's speed."""
        p = RODS[kind]
        g = self.open(name, p)
        before = state(self.host, p)
        self.click(g, LEVEL[k])
        s = self.wait_level(p, lambda s: s.get('target_level') == expect_target)
        self.check(f'{name}:{kind}_target', s.get('target_level') == expect_target and s.get('starting_level') == before.get('level'), [before, s])
        s = self.wait_level(p, lambda s: s.get('level', 0) > 0)
        self.check(f'{name}:{kind}_moves', 0 < s.get('level', 0) <= expect_target, s)
        # Its group: the button sets it, the same again takes it out.
        self.click(g, GROUP[1])
        self.check(f'{name}:{kind}_group_set', self.wait_level(p, lambda s: s.get('color') == 1).get('color') == 1, state(self.host, p))
        self.click(g, GROUP[1])
        self.check(f'{name}:{kind}_group_cleared', self.wait_level(p, lambda s: s.get('color') is None).get('color') is None, state(self.host, p))
        g.close()

    def auto(self, name, kind):
        """The automatic rod's fields: the column at 20 °C is below the lower bound, so its target is the level there."""
        p = RODS[kind]
        g = self.open(name, p)
        for i, text in enumerate(['10', '60', '500', '100']):
            self.write(g, i, text)
        self.click(g, SAVE)
        s = self.wait_level(p, lambda s: s.get('target_level') == 0.6)
        self.check(f'{name}:{kind}_target_from_the_curve', s.get('target_level') == 0.6, s)
        s = self.wait_level(p, lambda s: s.get('level', 0) > 0)
        self.check(f'{name}:{kind}_moves', 0 < s.get('level', 0) <= 0.6, s)
        g.close()

    def power(self):
        """The ReaSim rods move on the battery's power and stop paying once both are at their targets."""
        s1 = self.wait_level(RODS['reasim'], lambda s: s.get('level') == s.get('target_level'), 400)
        s2 = self.wait_level(RODS['reasim_auto'], lambda s: s.get('level') == s.get('target_level'), 400)
        self.check('reasim_rods_reach_their_targets', s1.get('level') == s1.get('target_level') and s2.get('level') == s2.get('target_level'), [s1, s2])
        # The creative cell out (from the hopper and from the battery's two slots, where the hopper put it): from here
        # the battery holds what it has. Still rods (their buffers full) draw nothing.
        self.host.command(f'setblock 46 {Y} 9 minecraft:air')
        for slot in (0, 1):
            self.host.command(f'item replace block {BATTERY[0]} {BATTERY[1]} {BATTERY[2]} container.{slot} with minecraft:air')
        time.sleep(3)
        a = battery_power(self.host)
        time.sleep(5)
        b = battery_power(self.host)
        buffers = [power(self.host, RODS['reasim']), power(self.host, RODS['reasim_auto'])]
        self.check('still_rods_draw_nothing', a is not None and a == b and buffers == [50000, 50000], {'battery': [a, b], 'buffers': buffers})
        # The ReaSim rod sent back to 0.5 (90 moves, some 450 000 HE): the battery pays while it moves (5000 a tick
        # moved, through the buffer), and nothing once it is there.
        g = self.open(GUEST, RODS['reasim'])
        self.click(g, LEVEL[2])
        g.close()
        time.sleep(3)
        moving = (battery_power(self.host), state(self.host, RODS['reasim']))
        time.sleep(3)
        later = (battery_power(self.host), state(self.host, RODS['reasim']))
        self.check('a_moving_rod_draws', later[0] < moving[0] and later[1]['level'] < moving[1]['level'], [moving, later])
        s = self.wait_level(RODS['reasim'], lambda s: s.get('level') == 0.5, 400)
        time.sleep(3)
        a = battery_power(self.host)
        time.sleep(5)
        b = battery_power(self.host)
        self.check('back_at_its_target_it_draws_nothing', s.get('level') == 0.5 and a == b, [s, a, b])
        # Down to 0 is more than the battery holds (another 180 moves): once it is empty the rod stops where it is
        # (`hasPower` false under 5000), as the Java's does.
        g = self.open(GUEST, RODS['reasim'])
        self.click(g, LEVEL[4])
        g.close()
        deadline = time.monotonic() + 400
        while time.monotonic() < deadline and battery_power(self.host) != 0:
            time.sleep(2)
        time.sleep(3)
        stopped = state(self.host, RODS['reasim'])
        time.sleep(5)
        later = state(self.host, RODS['reasim'])
        self.check('an_empty_battery_stops_the_rod', battery_power(self.host) == 0 and 0.0 < later.get('level', 0) < 0.5
                   and later.get('level') == stopped.get('level'), [stopped, later, battery_power(self.host)])

    def flux(self):
        # Ten readings over five seconds: the channel behind the rod at 0 takes nothing.
        readings = []
        for _ in range(10):
            readings.append(state(self.host, TARGET).get('last_flux', 0.0))
            time.sleep(.5)
        # The source works: its own channel heats (its rod burns), so the zeros are the rod's.
        source = ' '.join(self.host.command(f'data get block {SOURCE[0]} {SOURCE[1]} {SOURCE[2]} ntm').get('chat', []))
        heat = re.search(r'\\?"heat\\?":(\d+)', source)
        self.check('a_rod_at_zero_stops_the_stream', max(readings) == 0.0 and heat and bits(heat[1]) > 20.0,
                   {'behind_the_rod': readings, 'source_heat': bits(heat[1]) if heat else None})
        g = self.open(HOST, LINE_ROD)
        self.click(g, LEVEL[0])
        g.close()
        # 1 / 0.00277 ticks out, then the stream passes.
        self.wait_level(LINE_ROD, lambda s: s.get('level') == 1.0, 60)
        deadline = time.monotonic() + 30
        got = 0.0
        while time.monotonic() < deadline and got == 0.0:
            got = state(self.host, TARGET).get('last_flux', 0.0)
            time.sleep(.5)
        self.check('a_rod_out_lets_the_stream_through', got > 0.0, {'last_flux_read': got, 'rod': state(self.host, LINE_ROD)})

    def reopen(self):
        """After the world opened again: each rod where it was and where it was going (`level`, `targetLevel` and a
        manual rod's start are saved; the ReaSim rods' buffers are not, a unit test's matter: the network refills them
        at once)."""
        self.host.ctl('wait-ready 120')
        self.host.evidence['phase'] = 'reopen'
        want = {'manual': (0.5, 0.5), 'moderated': (1.0, 1.0), 'auto': (0.6, 0.6), 'reasim_auto': (0.6, 0.6)}
        got = {}
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            got = {k: state(self.host, RODS[k]) for k in want}
            if all(got[k].get('level') is not None for k in want):
                break
            time.sleep(1)
        # The ReaSim rod stopped where its battery ran out, its target 0: both come back as saved.
        reasim = state(self.host, RODS['reasim'])
        got['reasim'] = reasim
        self.check('rods_survive_a_world_reopen', all((got[k].get('level'), got[k].get('target_level')) == want[k] for k in want)
                   and reasim.get('target_level') == 0.0 and 0.0 < reasim.get('level', 0) < 0.5, got)
        self.host.evidence['ok'] = True
        self.host.save()

    def run(self):
        self.setup()
        self.host.evidence['phase'] = 'rods'
        self.manual(HOST, 'manual', 2, 0.5)
        self.manual(GUEST, 'moderated', 0, 1.0)
        self.manual(GUEST, 'reasim', 1, 0.75)
        self.auto(HOST, 'auto')
        self.auto(GUEST, 'reasim_auto')
        self.power()
        self.host.evidence['phase'] = 'flux'
        self.flux()
        self.host.ctl('save')
        self.host.evidence['ok'] = True
        self.host.save()


if __name__ == '__main__':
    if sys.argv[3:] == ['reopen']:
        Proof(Game(Path(sys.argv[1]), 'rbmk-t07-reopen'), None).reopen()
        sys.exit(0)
    host, guest = Game(Path(sys.argv[1]), 'rbmk-t07-host'), Game(Path(sys.argv[2]), 'rbmk-t07-guest')
    Proof(host, guest).run()
    guest.evidence['ok'] = True
    guest.save()

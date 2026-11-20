#!/usr/bin/env python3
"""T10 proof (`rbmk-plano` T10): the RBMK console on a host and a LAN guest, as the server keeps the console and the
rods (`data get block ... ntm`). The RBMK tool, sneaking, takes a column's position and gives it to the console; the
guest opens the console's screen (no inventory) and, through it, picks the manual rods ("select all control rods")
and saves 100 for them, picks two columns on the map and saves 25, assigns them a group (right click), switches a
screen and gives it the picked columns, and cycles a picked boiler's compressor; AZ-5 (the lid first, then the
button) sets the three manual rods' targets to 0 from where each is, the automatic rod's target untouched; the
screwdriver, sneaking, turns the console's window. On reopen the console's target, rotation and screens are as saved.

Usage: native_rbmk_t10.py HOST_FOLDER GUEST_FOLDER  (both clients running with control.sock; the guest joined)
       native_rbmk_t10.py HOST_FOLDER - reopen
"""
import re
import struct
import sys
import time
from pathlib import Path
from native_game import Game

HOST, GUEST = 'Host', 'Guest'
Y = 101
PLACE = (40, Y, 4)  # the console's front cell (its core is one further south): what a player clicks
TARGET = (40, Y, 14)
RODS = {'manual': (40, Y, 14), 'moderated': (41, Y, 14), 'reasim': (42, Y, 14), 'auto': (43, Y, 14)}
BLOCKS = {'manual': 'rbmk_control', 'moderated': 'rbmk_control_mod', 'reasim': 'rbmk_control_reasim', 'auto': 'rbmk_control_auto'}
BOILER = (40, Y, 15)
MANUAL = ('manual', 'moderated', 'reasim')
SIZE = (244, 172)
# GUIRBMKConsole: the field (9, 84, 35 x 9), its save (48, 82), the compressor (70, 82), "select all control rods"
# (61, 70), the groups (6 + 11k, 70), AZ-5 (30, 138, 28 x 28), screen j's type (6 + 40(j % 2), 8 + 21(j / 2)) and its
# columns (24 + ...); a press is inside past the top edge.
FIELD, SAVE, COMPRESSOR, ALL, AZ5 = (20, 88), (54, 88), (76, 88), (66, 75), (44, 152)
GROUP = [(11 + 11 * k, 75) for k in range(5)]
SCREEN_TYPE = [(15 + 40 * (j % 2), 17 + 21 * (j // 2)) for j in range(6)]
SCREEN_COLUMNS = [(33 + 40 * (j % 2), 17 + 21 * (j // 2)) for j in range(6)]


def column(id_):
    return f'ntm:{id_}[facing=north,honey_level=0]'


def bits(v):
    return struct.unpack('<d', struct.pack('<Q', int(v)))[0]


def index(p, rotation=0):
    """The window index of cell `p` (`getXFromIndex`/`getZFromIndex` turned back), the console aimed at TARGET."""
    dx, dz = p[0] - TARGET[0], p[2] - TARGET[2]
    i, j = {0: (dx, dz), 1: (dz, -dx), 2: (-dx, -dz), 3: (-dz, dx)}[rotation]
    return (j + 7) * 15 + (i + 7)


def cell(i):
    return 86 + 10 * (i % 15) + 5, 11 + 10 * (i // 15) + 5


def raw(host, p):
    return ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))


def rod(host, p):
    """A control rod's state as the server keeps it (doubles by their bits)."""
    m = re.search(r'\\?"control\\?":\{(.*?)\}', raw(host, p))
    if not m:
        return {}
    f = dict(re.findall(r'\\?"(\w+)\\?":(-?\d+|null|true|false)', m[1]))
    out = {k: bits(f[k]) for k in ('level', 'target_level', 'starting_level') if k in f}
    out['color'] = None if f.get('color') in (None, 'null') else int(f['color'])
    return out


def console(host, p):
    """The console's saved state: target, rotation, the screens' types and columns."""
    text = raw(host, p)
    t = re.search(r'\\?"target\\?":\[(-?\d+),(-?\d+),(-?\d+)\]', text)
    r = re.search(r'\\?"rotation\\?":(\d+)', text)
    if not (t and r):
        return None
    screens = [(int(k), [int(c) for c in cols.split(',') if c]) for k, cols in re.findall(r'\\?"kind\\?":(\d+),\\?"columns\\?":\[([-\d,]*)\]', text)]
    return {'target': [int(v) for v in t.groups()], 'rotation': int(r[1]), 'screens': screens}


class Proof:
    def __init__(self, host, guest):
        self.host, self.guest = host, guest
        self.players = {HOST: host, GUEST: guest}
        self.core = None

    def check(self, name, ok, actual):
        self.host.check(name, ok, actual)

    def origin(self, g):
        w, h = g.ctl('ui')['gui_size']
        return (int(w) - SIZE[0]) // 2, (int(h) - SIZE[1]) // 2

    def click(self, g, at, right=False):
        ox, oy = self.origin(g)
        g.ctl(f'click-at {ox + at[0]} {oy + at[1]}' + (' right' if right else ''))
        time.sleep(.6)

    def wait(self, read, predicate, timeout=30):
        deadline = time.monotonic() + timeout
        s = read()
        while time.monotonic() < deadline:
            s = read()
            if predicate(s):
                return s
            time.sleep(.5)
        return s

    def sneak_use(self, name, p, item, face='north'):
        g = self.players[name]
        g.close()
        self.host.command(f'clear {name}')
        self.host.command(f'give {name} {item} 1')
        self.host.command(f'tp {name} {p[0] + .5} {p[1]} {p[2] - 2.5} 0 0')
        time.sleep(.6)
        g.ctl(f'select {item}')
        g.ctl('key shift down')
        time.sleep(.3)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} {face}')
        time.sleep(.5)
        g.ctl('key shift up')
        time.sleep(.5)

    def open(self, name, carry=None):
        g = self.players[name]
        g.close()
        self.host.command(f'clear {name}')
        if carry:
            # In the main inventory, out of the hotbar: the hand stays empty.
            self.host.command(f'item replace entity {name} container.20 with {carry}')
        if name == HOST:
            # The front cell, a dummy: its core's screen.
            self.host.command(f'tp {name} {PLACE[0] + .5} {PLACE[1]} {PLACE[2] - 2.5} 0 0')
            time.sleep(.6)
            g.ctl(f'interact {PLACE[0]} {PLACE[1]} {PLACE[2]} north')
        else:
            # The front cell first; a LAN guest's use of a dummy cell has opened nothing (recorded in the T10 doc): then
            # the core itself, from behind.
            self.host.command(f'tp {name} {PLACE[0] + .5} {PLACE[1]} {PLACE[2] - 2.5} 0 0')
            time.sleep(.6)
            g.ctl(f'interact {PLACE[0]} {PLACE[1]} {PLACE[2]} north')
            time.sleep(2)
            through = g.ctl('screen').get('screen') == 'container'
            self.host.evidence.setdefault('guest_through_the_dummy', []).append(through)
            if not through:
                self.host.command(f'tp {name} {self.core[0] + .5} {Y} {self.core[2] + 3.5} 180 0')
                time.sleep(.6)
                g.ctl(f'interact {self.core[0]} {self.core[1]} {self.core[2]} south')
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if g.ctl('screen').get('screen') == 'container':
                time.sleep(1.5)
                return g
            time.sleep(.2)
        self.host.evidence.setdefault('not_opened', []).append({'who': name, 'screen': g.ctl('screen'), 'ui': g.ctl('ui').get('screen')})
        self.diagnose(name)
        self.host.save()
        raise AssertionError(f'{name}: the console did not open')

    def diagnose(self, name):
        """Which uses open a screen for `name`: the console's core from the south, a top cell, a rod's core (T07's)."""
        g = self.players[name]
        tries = [('core_south', self.core, (self.core[0] + .5, self.core[2] + 3.5, 180), 'south'),
                 ('top_cell', (PLACE[0], Y + 2, PLACE[2]), (PLACE[0] + .5, PLACE[2] - 2.5, 0), 'north'),
                 ('rod_core', RODS['auto'], (RODS['auto'][0] + .5, RODS['auto'][2] - 2.5, 0), 'north')]
        out = {}
        for label, cell, (x, z, yaw), face in tries:
            g.close()
            time.sleep(.5)
            self.host.command(f'tp {name} {x} {Y} {z} {yaw} 0')
            time.sleep(.8)
            try:
                r = g.ctl(f'interact {cell[0]} {cell[1]} {cell[2]} {face}')
            except RuntimeError as error:
                r = {'error': str(error)}
            time.sleep(2)
            out[label] = {'interact': {k: r.get(k) for k in ('ok', 'error', 'block')} if isinstance(r, dict) else str(r), 'screen': g.ctl('screen').get('screen')}
        self.host.evidence['diagnose'] = out
        g.close()

    def setup(self):
        h = self.host
        h.ctl('wait-ready 120')
        h.close()
        h.command('forceload add 30 0 54 24')
        time.sleep(3)
        h.command(f'fill 32 {Y - 1} 0 52 {Y + 18} 22 minecraft:air')
        h.command(f'fill 32 {Y - 1} 0 52 {Y - 1} 22 minecraft:stone')
        for name in (HOST, GUEST):
            h.command(f'gamemode creative {name}')
            h.command(f'effect give {name} minecraft:resistance 1200 4 true')
        for kind, p in RODS.items():
            h.command(f'setblock {p[0]} {p[1]} {p[2]} {column(BLOCKS[kind])}')
        h.command(f'setblock {BOILER[0]} {BOILER[1]} {BOILER[2]} {column("rbmk_boiler")}')
        # The console, placed by hand from the south, facing it.
        h.command(f'give {HOST} ntm:rbmk_console 1')
        h.command(f'tp {HOST} {PLACE[0] + .5} {Y} {PLACE[2] - 3.5} 0 0')
        time.sleep(.6)
        self.host.evidence['place'] = h.ctl(f'place {PLACE[0]} {PLACE[1]} {PLACE[2]} --item ntm:rbmk_console')
        time.sleep(1.5)
        cells = [(x, y, z) for x in range(PLACE[0] - 3, PLACE[0] + 4) for y in range(Y, Y + 2) for z in range(PLACE[2] - 3, PLACE[2] + 4)]
        self.core = next((p for p in cells if console(h, p) is not None), None)
        self.check('the_console_stands', self.core is not None, self.host.evidence['place'])
        if self.core is None:
            raise AssertionError('no console core found')
        self.host.evidence['core'] = self.core

    def link(self):
        """Sneaking, the tool on a column keeps its position; on the console it gives it, at the console's next tick."""
        self.check('no_target_before', console(self.host, self.core)['target'] == [0, 0, 0], console(self.host, self.core))
        self.sneak_use(HOST, TARGET, 'ntm:rbmk_tool')
        held = ' '.join(self.host.command(f'data get entity {HOST} SelectedItem').get('chat', []))
        kept = re.search(r'posX:\s*(-?\d+).*?posY:\s*(-?\d+).*?posZ:\s*(-?\d+)', held)
        self.check('the_tool_keeps_the_column', kept is not None and [int(v) for v in kept.groups()] == list(TARGET), held[-300:])
        g = self.host
        g.close()
        g.command(f'tp {HOST} {PLACE[0] + .5} {PLACE[1]} {PLACE[2] - 2.5} 0 0')
        time.sleep(.6)
        g.ctl('key shift down')
        time.sleep(.3)
        g.ctl(f'interact {PLACE[0]} {PLACE[1]} {PLACE[2]} north')
        time.sleep(.5)
        g.ctl('key shift up')
        s = self.wait(lambda: console(self.host, self.core), lambda s: s['target'] == list(TARGET))
        self.check('the_console_takes_the_target', s['target'] == list(TARGET), s)

    def count(self, name, item):
        text = ' '.join(self.host.command(f'clear {name} {item} 0').get('chat', []))
        m = re.search(r'(\d+)', text)
        return int(m[1]) if m else 0

    def host_opens(self):
        try:
            self.open(HOST)
            self.check('the_host_opens_the_console', True, self.host.ctl('screen').get('screen'))
        except AssertionError as e:
            self.check('the_host_opens_the_console', False, str(e))
        self.host.close()

    def shot(self, g, name):
        try:
            g.ctl(f'screenshot {g.folder / (name + ".png")}')
            time.sleep(1)
        except RuntimeError:
            pass

    def commands(self):
        # C: the guest carries 7 cobblestone through the screen; the console has no slot, and none is copied.
        g = self.open(GUEST, 'minecraft:cobblestone 7')
        self.shot(g, 'console-opened')
        # All the manual rods to 100: "select all control rods", the field, the save.
        self.click(g, ALL)
        self.click(g, FIELD)
        self.shot(g, 'console-field')
        g.ctl('key key.keyboard.end tap')
        for _ in range(4):
            g.ctl('key backspace tap')
        g.ctl('type 100')
        self.click(g, SAVE)
        got = {k: self.wait(lambda k=k: rod(self.host, RODS[k]), lambda s: s.get('target_level') == 1.0) for k in MANUAL}
        auto = rod(self.host, RODS['auto'])
        self.check('select_all_and_save_reach_the_manual_rods', all(got[k].get('target_level') == 1.0 for k in MANUAL) and auto.get('target_level') != 1.0, [got, auto])
        # Two columns picked on the map (the selection cleared first), 25, then the green group (right click).
        self.click(g, (77, 75))
        for k in ('manual', 'moderated'):
            self.click(g, cell(index(RODS[k])))
        self.click(g, FIELD)
        g.ctl('key key.keyboard.end tap')
        for _ in range(4):
            g.ctl('key backspace tap')
        g.ctl('type 25')
        self.click(g, SAVE)
        got = {k: self.wait(lambda k=k: rod(self.host, RODS[k]), lambda s: s.get('target_level') == 0.25) for k in ('manual', 'moderated')}
        reasim = rod(self.host, RODS['reasim'])
        self.check('a_level_for_the_picked_columns', all(got[k].get('target_level') == 0.25 for k in got) and reasim.get('target_level') == 1.0, [got, reasim])
        self.click(g, GROUP[2], right=True)
        got = {k: self.wait(lambda k=k: rod(self.host, RODS[k]), lambda s: s.get('color') == 2) for k in ('manual', 'moderated')}
        self.check('a_group_for_the_picked_rods', all(got[k].get('color') == 2 for k in got) and rod(self.host, RODS['reasim']).get('color') is None, got)
        # Screen 1 monitors column temperature over the two picked columns.
        self.click(g, SCREEN_TYPE[0])
        self.click(g, SCREEN_COLUMNS[0])
        s = self.wait(lambda: console(self.host, self.core), lambda s: s['screens'] and s['screens'][0][0] == 1 and s['screens'][0][1])
        self.check('a_screen_and_its_columns', s['screens'][0] == (1, sorted(index(RODS[k]) for k in ('manual', 'moderated'))), s)
        # The boiler picked alone, its compressor cycled once: dense steam.
        self.click(g, (77, 75))
        self.click(g, cell(index(BOILER)))
        self.click(g, COMPRESSOR)
        text = self.wait(lambda: raw(self.host, BOILER), lambda t: 'HOTSTEAM' in t)
        self.check('the_compressor_of_the_picked_boiler', 'HOTSTEAM' in text, text[-300:])
        g.close()
        time.sleep(1)
        kept = self.count(GUEST, 'minecraft:cobblestone')
        self.check('the_screen_copies_no_inventory', kept == 7, kept)

    def auto_curve(self):
        """The automatic rod's own screen (T07's): a curve whose target at 20 °C is 0.6, for AZ-5 to leave alone."""
        g = self.guest
        g.close()
        p = RODS['auto']
        self.host.command(f'tp {GUEST} {p[0] + .5} {Y} {p[2] - 2.5} 0 0')
        time.sleep(.6)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
        time.sleep(1.5)
        w = next(w for w in g.ctl('ui').get('widgets', []) if w.get('id') == 'slot-9')
        ox, oy = w['bounds'][0] - 8, w['bounds'][1] - 104
        for i, text in enumerate(['10', '60', '500', '100']):
            g.ctl(f'click-at {ox + 32} {oy + 29 + 11 * i}')
            time.sleep(.4)
            g.ctl('key key.keyboard.end tap')
            for _ in range(5):
                g.ctl('key backspace tap')
            g.ctl(f'type {text}')
        g.ctl(f'click-at {ox + 40} {oy + 75}')
        time.sleep(.6)
        g.close()
        s = self.wait(lambda: rod(self.host, p), lambda s: s.get('target_level') == 0.6)
        self.check('the_automatic_rod_has_a_target', s.get('target_level') == 0.6, s)

    def az5(self):
        """The rods out (100 % saved above); then AZ-5 from the guest: the lid, the button."""
        self.auto_curve()
        s = self.wait(lambda: rod(self.host, RODS['moderated']), lambda s: s.get('level', 0) >= 0.2, 120)
        g = self.open(GUEST)
        auto_before = rod(self.host, RODS['auto'])
        self.click(g, AZ5)
        lid_only = {k: rod(self.host, RODS[k]) for k in MANUAL}
        self.check('the_lid_opens_first', all(lid_only[k].get('target_level') != 0.0 for k in ('reasim',)), lid_only)
        before = {k: rod(self.host, RODS[k]) for k in MANUAL}
        self.click(g, AZ5)
        got = {k: self.wait(lambda k=k: rod(self.host, RODS[k]), lambda s: s.get('target_level') == 0.0) for k in MANUAL}
        auto = rod(self.host, RODS['auto'])
        # The start is where each was then (between the reading before and the press).
        starts = all(got[k].get('target_level') == 0.0 and got[k].get('starting_level', -1) >= before[k].get('level', 0) - 0.03 for k in MANUAL)
        self.check('az5_drops_the_three_manual_rods', starts, [before, got, s])
        self.check('az5_leaves_the_automatic_rod', auto.get('target_level') == auto_before.get('target_level') == 0.6, [auto_before, auto])
        time.sleep(2)
        later = rod(self.host, RODS['manual'])
        self.check('the_manual_rod_goes_down', later.get('level', 1) < got['manual'].get('level', 0) or later.get('level') == 0.0, [got['manual'], later])
        g.close()

    def turn(self):
        self.sneak_use(HOST, PLACE, 'ntm:screwdriver')
        s = self.wait(lambda: console(self.host, self.core), lambda s: s['rotation'] == 1)
        self.check('the_screwdriver_turns_the_console', s['rotation'] == 1, s)

    def run(self):
        self.setup()
        self.host.evidence['phase'] = 'link'
        self.link()
        self.host.evidence['phase'] = 'commands'
        self.host_opens()
        self.commands()
        self.host.evidence['phase'] = 'az5'
        self.az5()
        self.host.evidence['phase'] = 'turn'
        self.turn()
        self.host.evidence['console'] = console(self.host, self.core)
        self.host.evidence['rods'] = {k: rod(self.host, p) for k, p in RODS.items()}
        self.host.ctl('save')
        self.host.evidence['ok'] = True
        self.host.save()

    def reopen(self):
        """After the world opened again: the console's target, rotation and screen as saved; the rods' targets."""
        self.host.ctl('wait-ready 120')
        self.host.evidence['phase'] = 'reopen'
        cells = [(x, y, z) for x in range(PLACE[0] - 3, PLACE[0] + 4) for y in range(Y, Y + 2) for z in range(PLACE[2] - 3, PLACE[2] + 4)]
        found = None
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and found is None:
            found = next((console(self.host, p) for p in cells if console(self.host, p) is not None), None)
            time.sleep(1)
        rods = {k: rod(self.host, RODS[k]) for k in RODS}
        ok = (found is not None and found['target'] == list(TARGET) and found['rotation'] == 1 and found['screens'][0] == (1, sorted(index(RODS[k]) for k in ('manual', 'moderated')))
              and all(rods[k].get('target_level') == 0.0 for k in MANUAL))
        self.check('the_console_survives_a_reopen', ok, {'console': found, 'rods': rods})
        self.host.evidence['ok'] = True
        self.host.save()


if __name__ == '__main__':
    if sys.argv[3:] == ['reopen']:
        Proof(Game(Path(sys.argv[1]), 'rbmk-t10-reopen'), None).reopen()
        sys.exit(0)
    host, guest = Game(Path(sys.argv[1]), 'rbmk-t10-host'), Game(Path(sys.argv[2]), 'rbmk-t10-guest')
    Proof(host, guest).run()

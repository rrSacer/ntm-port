#!/usr/bin/env python3
"""T05 proof (`rbmk-plano` T05): a fuel channel's menu (`ContainerRBMKRod`) refuses every survival gesture on its slot
while the rod in it is hotter than 200 °C, and lets the same gestures through when it is cold or the player is in
creative; the hand puts a rod (hot or cold) into an empty channel by itself (`RBMKRod.onBlockActivated`), with its
sound only then; automation (hoppers) sees no slot. Played by the host and by a LAN guest, each in its own client.

Usage: native_rbmk_t05.py HOST_FOLDER GUEST_FOLDER  (both clients running with control.sock; the guest joined)

The rods' hull stays where the test puts it: `dialHeatProvision` 0 (the channel draws no heat from the rod) and core =
hull (no diffusion), with no yield (a tag without the key reads 0: nothing burns).
"""
import re
import sys
import time
from pathlib import Path
from native_game import Game

HOST, GUEST = 'Host', 'Guest'
Y = 101
FUEL = 'ntm:rbmk_fuel_ueu'
HOT, COLD = 1200.0, 150.0
# One row of channels per player: hot, cold, empty (and a spare cold one for the full inventory).
ROWS = {HOST: 8, GUEST: 16}
KINDS = {'hot': 40, 'cold': 43, 'empty': 46, 'full': 49}
# Automation: a hopper above an empty channel holding a rod, and a hopper under a cold channel.
HOP_IN = (54, Y, 8)
HOP_OUT = (57, Y, 8)
CHANNEL = 'ntm:rbmk_rod[facing=north,honey_level=0]'
SLOT = 300


def at(name, kind):
    return (KINDS[kind], Y, ROWS[name])


def rod(hull):
    return f'{FUEL}[minecraft:custom_data={{hull:{hull}d,core:{hull}d}}]'


def items(host, p):
    """What the channel's block entity holds, as the host's server keeps it: (id, count, hull) per stack."""
    text = ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    if 'Found no elements matching Items' in text:
        return []  # an empty block entity writes no `Items`
    if 'block data: [' not in text:
        raise AssertionError(f'no Items at {p}: {text}')
    found, depth, start = [], 0, None
    for i, c in enumerate(text[text.index('block data: [') + 13:]):
        if c == '{':
            depth += 1
            start = i if depth == 1 else start
        elif c == '}':
            depth -= 1
            if depth == 0:
                entry = text[text.index('block data: [') + 13:][start:i + 1]
                count, item = re.search(r'count: ?(\d+)', entry), re.search(r'id: ?"([\w:]+)"', entry)
                hull = re.search(r'hull: ?([-\d.E]+)d', entry)
                found.append((item[1], int(count[1]), float(hull[1]) if hull else None))
    return found


def hull_of(stack):
    m = re.search(r'hull: ?([-\d.E]+)d', str((stack or {}).get('components') or ''))
    return float(m[1]) if m else None


def inventory(p):
    inv = p.ctl('inventory')
    stacks = [s for part in ('hotbar', 'main') for s in inv.get(part, []) or []]
    rods = [(s['item'], s['count'], hull_of(s)) for s in stacks if isinstance(s, dict) and str(s.get('item') or '').startswith('ntm:rbmk_fuel_')]
    return {'rods': sorted(rods, key=str), 'carried': inv.get('carried'), 'hotbar0': (inv.get('hotbar') or [None])[0]}


def rod_entities(p):
    return sum(e['item']['count'] for e in p.ctl('entities 12')['entities']
               if e.get('kind') == 'item' and e['item']['item'].startswith('ntm:rbmk_fuel_'))


def upgrade_plugs(p):
    """`hbm:item.upgradePlug` as this client resolved it (`HORIZON_TRACE_SOUNDS`)."""
    return sum(f.read_text(errors='replace').count('sound resolved event=ntm:item.upgradeplug ')
               for f in p.folder.glob('client*.log'))


class Proof:
    def __init__(self, host, guest):
        self.host, self.guest = host, guest
        self.players = {HOST: host, GUEST: guest}

    def check(self, name, ok, actual):
        self.host.check(name, ok, actual)

    def state(self, name, p):
        g = self.players[name]
        return {'be': items(self.host, p), 'inv': inventory(g), 'entities': rod_entities(g)}

    def stand(self, name, p):
        g = self.players[name]
        g.close()
        self.host.command(f'tp {name} {p[0] + .5} {p[1]} {p[2] - 2.5} 0 0')
        time.sleep(.6)

    def open(self, name, p):
        g = self.players[name]
        self.stand(name, p)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            screen = g.ctl('screen')
            if screen.get('screen') == 'container' and any(s['slot'] == SLOT for s in screen.get('slots', [])):
                return screen
            time.sleep(.2)
        raise AssertionError(f'{name}: the channel at {p} did not open: {screen}')

    def hover(self, g, slot=SLOT):
        """The mouse over a slot with nothing pressed on it: pressed outside the window (nothing in the cursor: no
        click), released over the slot."""
        widget = next((w for w in g.ctl('ui').get('widgets', []) if w.get('id') == f'slot-{slot}'), None)
        if widget is None:
            raise AssertionError(f'no slot-{slot} widget')
        x, y, w, h = widget['bounds']
        g.ctl(f'drag-at 1 1 {x + w / 2} {y + h / 2}')

    def settle(self):
        time.sleep(.8)

    def setup(self):
        h = self.host
        h.ctl('wait-ready 120')
        h.close()
        h.command('gamemode creative @s')
        h.command('forceload add 30 0 64 24')
        time.sleep(3)
        h.command('fill 36 99 2 62 108 22 minecraft:air')
        h.command('fill 36 100 2 62 100 22 minecraft:stone')
        h.command('ntmrbmk dial dialHeatProvision 0')
        for name in (HOST, GUEST):
            for kind in KINDS:
                p = at(name, kind)
                h.command(f'setblock {p[0]} {p[1]} {p[2]} {CHANNEL}')
            h.command(f'effect give {name} minecraft:fire_resistance 1200 0 true')
            h.command(f'effect give {name} minecraft:resistance 1200 4 true')
            h.command(f'gamemode survival {name}')
            h.command(f'clear {name}')
        for p in (HOP_IN, HOP_OUT):
            h.command(f'setblock {p[0]} {p[1]} {p[2]} {CHANNEL}')
        h.command(f'setblock {HOP_IN[0]} {Y + 1} {HOP_IN[2]} minecraft:hopper[facing=down]')
        h.command(f'setblock {HOP_OUT[0]} {Y - 1} {HOP_OUT[2]} minecraft:hopper[facing=down]')
        time.sleep(2)

    def by_hand(self, name, p, hull):
        """A rod put into the empty channel by the player's hand in survival: one of two in hand, the sound once."""
        g = self.players[name]
        self.stand(name, p)
        self.host.command(f'clear {name}')
        self.host.command(f'give {name} {rod(hull)} 2')
        time.sleep(.6)
        g.ctl(f'select {FUEL}')
        sounds = upgrade_plugs(g)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
        self.settle()
        tag = f'{name}:hand_{int(hull)}_{p[0]}'
        be, inv = items(self.host, p), inventory(g)
        self.check(tag + '_in', be == [(FUEL, 1, hull)] and inv['rods'] == [(FUEL, 1, hull)], [be, inv])
        heard = upgrade_plugs(g) - sounds
        self.check(tag + '_sound_once', heard == 1, heard)
        # The channel is full: the same click opens the menu, silent, the hand kept.
        sounds = upgrade_plugs(g)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
        self.settle()
        screen = g.ctl('screen')
        self.check(tag + '_full_opens_the_menu', screen.get('screen') == 'container' and inventory(g)['rods'] == [(FUEL, 1, hull)],
                   [screen.get('screen'), inventory(g)])
        self.check(tag + '_full_is_silent', upgrade_plugs(g) == sounds, upgrade_plugs(g) - sounds)
        g.close()
        self.host.command(f'clear {name}')

    def refused(self, name, p, gesture, act):
        """A gesture on the hot channel's slot: nothing moves, neither the rod nor the hand nor the cursor."""
        g = self.players[name]
        self.open(name, p)
        before = self.state(name, p)
        act(g)
        self.settle()
        after = self.state(name, p)
        screen = g.ctl('screen')
        self.check(f'{name}:hot_{gesture}_refused', after == before and not screen.get('carried'), [before, after, screen.get('carried')])
        g.close()

    def hot(self, name):
        p = at(name, 'hot')
        self.by_hand(name, p, HOT)
        self.host.command(f'give {name} minecraft:stone 3')
        time.sleep(.5)
        self.refused(name, p, 'click', lambda g: g.ctl(f'click-slot {SLOT}'))
        self.refused(name, p, 'right_click', lambda g: g.ctl(f'click-slot {SLOT} right'))
        self.refused(name, p, 'shift_click', lambda g: g.ctl(f'click-slot {SLOT} shift'))
        self.refused(name, p, 'number_swap', lambda g: (self.hover(g), g.ctl('key 1 tap')))
        self.refused(name, p, 'drop', lambda g: (self.hover(g), g.ctl('key q tap')))
        self.host.command(f'clear {name}')

    def cold(self, name):
        p = at(name, 'cold')
        g = self.players[name]
        self.by_hand(name, p, COLD)
        cold = [(FUEL, 1, COLD)]
        # Click: the rod into the cursor, whole, and back.
        self.open(name, p)
        g.ctl(f'click-slot {SLOT}')
        self.settle()
        carried = g.ctl('screen').get('carried')
        self.check(f'{name}:cold_click_takes', items(self.host, p) == [] and hull_of(carried) == COLD and carried.get('count') == 1,
                   [items(self.host, p), carried])
        g.ctl(f'click-slot {SLOT}')
        self.settle()
        self.check(f'{name}:cold_click_puts_back', items(self.host, p) == cold and not g.ctl('screen').get('carried'), items(self.host, p))
        # Shift: into the inventory once, and back from it.
        g.ctl(f'click-slot {SLOT} shift')
        self.settle()
        inv = inventory(g)
        self.check(f'{name}:cold_shift_out', items(self.host, p) == [] and inv['rods'] == cold, [items(self.host, p), inv])
        source = next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot'] < 36 and str(s.get('item')).startswith(FUEL))
        g.ctl(f'click-slot {source} shift')
        self.settle()
        self.check(f'{name}:cold_shift_in', items(self.host, p) == cold and inventory(g)['rods'] == [], [items(self.host, p), inventory(g)])
        # Number key: swapped with the hotbar's first slot (a stone), then back.
        self.host.command(f'give {name} minecraft:stone 1')
        time.sleep(.5)
        self.hover(g)
        g.ctl('key 1 tap')
        self.settle()
        be, inv = items(self.host, p), inventory(g)
        self.check(f'{name}:cold_number_swap', be == [('minecraft:stone', 1, None)] and inv['rods'] == cold, [be, inv])
        g.ctl('key 1 tap')
        self.settle()
        self.check(f'{name}:cold_number_swap_back', items(self.host, p) == cold, items(self.host, p))
        self.host.command(f'clear {name}')
        # Full inventory: shift-click finds no room, nothing moves.
        self.host.command(f'give {name} minecraft:stone {36 * 64}')
        time.sleep(.8)
        before = self.state(name, p)
        g.ctl(f'click-slot {SLOT} shift')
        self.settle()
        self.check(f'{name}:cold_shift_full_inventory', self.state(name, p) == before and before['be'] == cold, [before, self.state(name, p)])
        self.host.command(f'clear {name}')
        time.sleep(.5)
        # Drop (Q over the slot): the rod thrown, once.
        self.hover(g)
        g.ctl('key q tap')
        self.settle()
        self.check(f'{name}:cold_drop', items(self.host, p) == [] and rod_entities(g) == 1, [items(self.host, p), rod_entities(g)])
        g.close()
        self.host.command('kill @e[type=minecraft:item]')
        time.sleep(.5)

    def drag(self, name):
        """Two stones dragged over the empty channel's slot and an empty inventory slot: one each (a vanilla `Slot`)."""
        p = at(name, 'empty')
        g = self.players[name]
        self.host.command(f'clear {name}')
        self.host.command(f'give {name} minecraft:stone 2')
        time.sleep(.6)
        self.open(name, p)
        source = next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot'] < 36 and s.get('item') == 'minecraft:stone')
        g.ctl(f'click-slot {source}')
        widgets = {w['id']: w['bounds'] for w in g.ctl('ui').get('widgets', []) if str(w.get('id', '')).startswith('slot-')}
        empty = next(i for i in range(9, 36) if f'slot-{i}' in widgets and i != source)
        pts = [widgets[f'slot-{SLOT}'], widgets[f'slot-{empty}']]
        g.ctl('drag-at ' + ' '.join(f'{x + w / 2} {y + h / 2}' for x, y, w, h in pts))
        self.settle()
        be = items(self.host, p)
        self.check(f'{name}:empty_drag_splits', be == [('minecraft:stone', 1, None)], be)
        g.ctl(f'click-slot {SLOT}')
        g.close()
        self.host.command(f'clear {name}')

    def creative(self, name):
        """In creative the hot rod comes out and goes back by click and shift (`!isCreativeMode`)."""
        p = at(name, 'hot')
        g = self.players[name]
        self.host.command(f'gamemode creative {name}')
        time.sleep(.5)
        self.open(name, p)
        g.ctl(f'click-slot {SLOT}')
        self.settle()
        carried = g.ctl('screen').get('carried')
        self.check(f'{name}:creative_hot_click_takes', items(self.host, p) == [] and hull_of(carried) == HOT, [items(self.host, p), carried])
        g.ctl(f'click-slot {SLOT}')
        self.settle()
        self.check(f'{name}:creative_hot_puts_back', items(self.host, p) == [(FUEL, 1, HOT)], items(self.host, p))
        g.ctl(f'click-slot {SLOT} shift')
        self.settle()
        self.check(f'{name}:creative_hot_shift_out', items(self.host, p) == [] and inventory(g)['rods'] == [(FUEL, 1, HOT)],
                   [items(self.host, p), inventory(g)])
        g.close()
        self.host.command(f'gamemode survival {name}')
        self.host.command(f'clear {name}')

    def hoppers(self):
        h = self.host
        self.by_hand(HOST, HOP_OUT, COLD)
        h.command(f'item replace block {HOP_IN[0]} {Y + 1} {HOP_IN[2]} container.0 with {rod(COLD)} 1')
        time.sleep(4)
        above = ' '.join(h.command(f'data get block {HOP_IN[0]} {Y + 1} {HOP_IN[2]} Items').get('chat', []))
        below = ' '.join(h.command(f'data get block {HOP_OUT[0]} {Y - 1} {HOP_OUT[2]} Items').get('chat', []))
        self.check('hopper_puts_nothing_in', items(h, HOP_IN) == [] and FUEL in above, [items(h, HOP_IN), above])
        self.check('hopper_takes_nothing_out', items(h, HOP_OUT) == [(FUEL, 1, COLD)] and FUEL not in below, [items(h, HOP_OUT), below])

    def run(self):
        self.setup()
        self.hoppers()
        for name in (HOST, GUEST):
            self.host.evidence['phase'] = name
            self.hot(name)
            self.cold(name)
            self.drag(name)
            self.creative(name)
        self.host.evidence['ok'] = True
        self.host.save()


if __name__ == '__main__':
    host, guest = Game(Path(sys.argv[1]), 'rbmk-t05-host'), Game(Path(sys.argv[2]), 'rbmk-t05-guest')
    Proof(host, guest).run()
    guest.evidence['ok'] = True
    guest.save()

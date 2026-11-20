#!/usr/bin/env python3
"""T06 proof (`rbmk-plano` T06): the automatic control rod's screen (GUIRBMKControlAuto, no machine slot) on a host and
a LAN guest. Its four text fields are edited with the keyboard and saved with a click; what the server keeps (the
column's block entity, read with `data get block`) is the Java save rule applied to what was typed: the maximum length,
the clamp, `0` for text that is not a number, nothing at all for text `isNumber` passes and `parseDouble` throws on.
Reopened, the fields start from the server's values (a save with nothing typed keeps them). The curve buttons pick the
function. A command of a stale session is refused and changes nothing (`ntmtest screen replay`: a repeat while the
screen is open, an old session once it opened again, another generation once the column was replaced).

Usage: native_rbmk_t06.py HOST_FOLDER GUEST_FOLDER  (both clients running with control.sock; the guest joined)
"""
import re
import struct
import sys
import time
from pathlib import Path
from native_game import Game

HOST, GUEST = 'Host', 'Guest'
Y = 101
AUTO = 'ntm:rbmk_control_auto[facing=north,honey_level=0]'
AT = {HOST: (40, Y, 8), GUEST: (44, Y, 8)}
# A column the host sets and leaves: its settings must come back when the world opens again.
KEPT = (48, Y, 8)
KEPT_VALUES = {'level_upper': 33.0, 'level_lower': 66.0, 'heat_upper': 777.0, 'heat_lower': 222.0, 'function': 1}
# The screen's inventory (`horizon_machine`: [8, 104]) gives its origin; the fields (30, 27 + 11i, 26 x 6), the save
# (28, 70, 30 x 10) and the curves (61, 48 + 11k, 22 x 10).
INVENTORY = (8, 104)
FIELD = [(32, 29 + 11 * i) for i in range(4)]
SAVE = (40, 75)
CURVE = [(70, 53 + 11 * k) for k in range(3)]


def auto(host, p):
    """The column's settings as the server keeps them (`"auto"` of its model, doubles by their bits)."""
    text = ' '.join(host.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))
    m = re.search(r'\\?"auto\\?":\{(.*?)\}', text)
    if not m:
        return None
    fields = dict(re.findall(r'\\?"(\w+)\\?":(\d+)', m[1]))
    d = lambda k: struct.unpack('<d', struct.pack('<Q', int(fields[k])))[0]
    return {'level_upper': d('level_upper'), 'level_lower': d('level_lower'), 'heat_upper': d('heat_upper'),
            'heat_lower': d('heat_lower'), 'function': int(fields['function'])}


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
        time.sleep(.3)

    def open(self, name):
        g, p = self.players[name], AT[name]
        g.close()
        self.host.command(f'tp {name} {p[0] + .5} {p[1]} {p[2] - 2.5} 0 0')
        time.sleep(.6)
        g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if g.ctl('screen').get('screen') == 'container':
                time.sleep(.5)
                return
            time.sleep(.2)
        raise AssertionError(f'{name}: the screen did not open')

    def write(self, g, field, text):
        """Focus a field, empty it (End, then Backspace past its four characters), type."""
        self.click(g, FIELD[field])
        g.ctl('key key.keyboard.end tap')
        for _ in range(5):
            g.ctl('key backspace tap')
        if text:
            g.ctl(f'type {text}')
        time.sleep(.2)

    def save(self, name):
        self.click(self.players[name], SAVE)
        time.sleep(1.0)

    def screen_hook(self, name, what='replay'):
        # Asked by the host for the player (the reply goes to whoever runs the command): the hook replays that
        # player's last command, from where they stand.
        text = ' '.join(self.host.command(f'ntmtest screen {what} {name}').get('chat', []))
        m = re.search(r'screen=(\S+)', text)
        return m[1] if m else text

    def play(self, name):
        p, g = AT[name], self.players[name]
        self.host.evidence['phase'] = name
        self.open(name)
        self.check(f'{name}:opens_with_the_defaults', auto(self.host, p) == {'level_upper': 0.0, 'level_lower': 0.0,
                   'heat_upper': 0.0, 'heat_lower': 0.0, 'function': 0}, auto(self.host, p))
        # Typed values, saved: the screen's rule, the server's values.
        for i, text in enumerate(['75', '20', '1500', '300']):
            self.write(g, i, text)
        self.save(name)
        got = auto(self.host, p)
        self.check(f'{name}:saved_as_typed', got and (got['level_upper'], got['level_lower'], got['heat_upper'], got['heat_lower']) == (75, 20, 1500, 300), got)
        # The maximum length (3 for a level: `12345` is `123`) and the clamp (to 100), not a number (`abc`: 0), a
        # decimal cast down (`9.9`: 9).
        self.write(g, 0, '12345')
        self.write(g, 1, 'abc')
        self.write(g, 3, '9.9')
        self.save(name)
        got = auto(self.host, p)
        self.check(f'{name}:the_java_save_rule', got and (got['level_upper'], got['level_lower'], got['heat_upper'], got['heat_lower']) == (100, 0, 1500, 9), got)
        before = got
        # `1L`: isNumber passes, parseDouble throws (a 1.7.10 client dies there): nothing is sent.
        self.write(g, 2, '1L')
        self.save(name)
        self.check(f'{name}:a_throwing_text_sends_nothing', auto(self.host, p) == before, auto(self.host, p))
        self.write(g, 2, '1500')
        # The curves.
        self.click(g, CURVE[2])
        time.sleep(1.0)
        got = auto(self.host, p)
        self.check(f'{name}:the_curve_is_picked', got and got['function'] == 2 and got['level_upper'] == 100, got)
        # A repeat of the last command, the screen still open: refused, nothing changes.
        before = auto(self.host, p)
        self.check(f'{name}:a_repeat_is_refused', self.screen_hook(name) == 'refused:Repeated', self.screen_hook(name))
        # Reopened: the fields start from the server's values, so a save with nothing typed keeps them.
        g.close()
        self.open(name)
        self.save(name)
        self.check(f'{name}:reopened_fields_show_the_server', auto(self.host, p) == before, [auto(self.host, p), before])
        # The last command taken was that save's; once the screen opens again, it is an old session's.
        g.close()
        self.open(name)
        self.check(f'{name}:an_old_session_is_refused', self.screen_hook(name) == 'refused:OldSession', self.screen_hook(name))
        self.check(f'{name}:nothing_changed', auto(self.host, p) == before, auto(self.host, p))
        # Too far (T06 review P3(d)): a command of the current session, sent again with the next number by a player
        # 25 blocks away, is refused at `hasPermission`'s reach.
        self.save(name)
        self.host.command(f'tp {name} {p[0] + .5} {p[1]} {p[2] - 25.5} 0 0')
        time.sleep(1.0)
        got = self.screen_hook(name, 'next')
        self.check(f'{name}:too_far_is_refused', got == 'refused:TooFar', got)
        self.check(f'{name}:nothing_changed_from_afar', auto(self.host, p) == before, auto(self.host, p))
        # The column replaced under an open screen: its commands are another generation's.
        self.open(name)
        self.save(name)
        self.host.command(f'setblock {p[0]} {p[1]} {p[2]} minecraft:air')
        time.sleep(1.0)
        self.host.command(f'setblock {p[0]} {p[1]} {p[2]} {AUTO}')
        time.sleep(1.5)
        self.check(f'{name}:a_replaced_column_is_new', auto(self.host, p) == {'level_upper': 0.0, 'level_lower': 0.0,
                   'heat_upper': 0.0, 'heat_lower': 0.0, 'function': 0}, auto(self.host, p))
        self.check(f'{name}:another_generation_is_refused', self.screen_hook(name) == 'refused:Generation', self.screen_hook(name))
        self.check(f'{name}:the_new_column_is_untouched', auto(self.host, p)['function'] == 0, auto(self.host, p))
        g.close()

    def keep(self):
        """The host sets the kept column through its screen and leaves it."""
        g = self.players[HOST]
        self.host.command(f'tp {HOST} {KEPT[0] + .5} {KEPT[1]} {KEPT[2] - 2.5} 0 0')
        time.sleep(.6)
        g.ctl(f'interact {KEPT[0]} {KEPT[1]} {KEPT[2]} north')
        time.sleep(1.5)
        for i, v in enumerate(['33', '66', '777', '222']):
            self.write(g, i, v)
        self.save(HOST)
        self.click(g, CURVE[1])
        time.sleep(1.0)
        self.check('kept_column_set', auto(self.host, KEPT) == KEPT_VALUES, auto(self.host, KEPT))
        g.close()
        self.host.ctl('save')

    def reopen(self):
        """After the world opened again: the kept column's settings, as the save kept them (T06 review P3(d))."""
        self.host.ctl('wait-ready 120')
        self.host.evidence['phase'] = 'reopen'
        deadline = time.monotonic() + 60
        got = auto(self.host, KEPT)
        while time.monotonic() < deadline and got is None:
            time.sleep(1)
            got = auto(self.host, KEPT)
        self.check('settings_survive_a_world_reopen', got == KEPT_VALUES, got)
        self.host.evidence['ok'] = True
        self.host.save()

    def run(self):
        h = self.host
        h.ctl('wait-ready 120')
        h.close()
        h.command('forceload add 30 0 64 24')
        time.sleep(3)
        h.command('fill 36 99 2 62 108 22 minecraft:air')
        h.command('fill 36 100 2 62 100 22 minecraft:stone')
        h.command(f'setblock {KEPT[0]} {KEPT[1]} {KEPT[2]} {AUTO}')
        for name in (HOST, GUEST):
            p = AT[name]
            h.command(f'setblock {p[0]} {p[1]} {p[2]} {AUTO}')
            h.command(f'gamemode survival {name}')
            h.command(f'clear {name}')
        time.sleep(2)
        for name in (HOST, GUEST):
            self.play(name)
        self.keep()
        h.evidence['ok'] = True
        h.save()


if __name__ == '__main__':
    if sys.argv[3:] == ['reopen']:
        host = Game(Path(sys.argv[1]), 'rbmk-t06-reopen')
        Proof(host, None).reopen()
        sys.exit(0)
    host, guest = Game(Path(sys.argv[1]), 'rbmk-t06-host'), Game(Path(sys.argv[2]), 'rbmk-t06-guest')
    Proof(host, guest).run()
    guest.evidence['ok'] = True
    guest.save()

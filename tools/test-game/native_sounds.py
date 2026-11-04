#!/usr/bin/env python3
"""Prove mod-file resolution/decoding and a real client-owned machine loop.

Run with run-native-reactors.py sounds. HORIZON_NO_AUDIO remains enabled: no
physical output is claimed; the client diagnostic uses the real Ogg decoder.
"""
import re
import sys
import time
from pathlib import Path
from native_game import Game
from native_combustion import diesel


def log_lines(g):
    return (g.folder / 'client-first.log').read_text(errors='replace').splitlines()


def resolved(g, event, offset=0):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        lines = [line for line in log_lines(g)[offset:]
                 if 'sound resolved ' in line and 'event=' + event + ' ' in line
                 and '/assets/ntm/sounds/' in line
                 and re.search(r'decoded=Some\(\([1-9]\d*, [1-9]\d*\)\)', line)]
        if lines:
            return lines
        time.sleep(.2)
    return []


def setup(g):
    g.evidence['phase'] = 'samples'
    g.ctl('wait-ready 90')
    g.close()
    g.command('time set noon')
    g.ground(20, 100, 8, width=12, depth=8)
    for event, source in [('ntm:block.rbmk_explosion', 'block'),
                          ('ntm:block.pressoperate', 'block'),
                          ('ntm:item.geiger1', 'player'),
                          ('ntm:item.geiger6', 'player'),
                          ('ntm:weapon.nuclearexplosion', 'block')]:
        offset = len(log_lines(g))
        g.command(f'playsound {event} {source} @s ~ ~ ~ 1 1')
        lines = resolved(g, event, offset)
        g.check('resolved_' + event, bool(lines), lines)
    # Use the actual door interaction, twice. Do not substitute /playsound for
    # the content declaration's open/close event and category.
    g.command('give @s ntm:door_metal 1')
    g.ctl('place 20 100 8 --item ntm:door_metal')
    time.sleep(1)
    for action in ['open', 'close']:
        offset = len(log_lines(g))
        g.ctl('interact 20 100 8 south')
        lines = resolved(g, 'ntm:block.opendoor', offset)
        g.check('door_' + action, bool(lines) and all('source=4' in line for line in lines), lines)
    # The regular diesel acceptance fills it through its real slots and turns
    # its ignition through the real menu. The resulting TE audio tag drives the
    # loop in the window, independently of the server's transient `was_on`.
    offset = len(log_lines(g))
    diesel(g)
    lines = resolved(g, 'ntm:block.dieseloperate', offset)
    g.check('machine_loop_resolved', bool(lines), lines)
    before = g.ctl('status')['sounds']['instances']
    g.command('setblock 260 100 8 minecraft:air')
    deadline = time.monotonic() + 15
    after = before
    while time.monotonic() < deadline:
        after = g.ctl('status')['sounds']['instances']
        if after < before:
            break
        time.sleep(.25)
    g.check('removed_machine_releases_its_instance', after < before, [before, after])
    g.evidence['source'] = 'real client SoundRegistry and Ogg decoder; output intentionally disabled'
    g.evidence['ok'] = True
    g.save()


if __name__ == '__main__':
    setup(Game(Path(sys.argv[1]), 'sounds'))

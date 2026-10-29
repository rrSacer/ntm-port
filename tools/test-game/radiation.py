#!/usr/bin/env python3
"""Exercise the real M1 radiation flow in an already-open throwaway world.

Usage: radiation.py GAME DIST [--reenter]
Launch with run.sh first; use a NEW creative world for the first phase.
No player-data paths outside tools/test-game's isolated folder are touched.
"""
import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
p = argparse.ArgumentParser()
p.add_argument('game')
p.add_argument('dist', type=Path)
p.add_argument('--reenter', action='store_true')
a = p.parse_args()
assert a.game and all(c.isalnum() or c in '-_' for c in a.game)
game = ROOT / 'target/test-game' / a.game
storage = a.dist.resolve() / '.horizon-mod-data/ntmlogic'
report_path = game / 'radiation-evidence.json'
report = json.loads(report_path.read_text()) if report_path.exists() else {'steps': {}}


def ctl(*args):
    raw = subprocess.check_output([str(ROOT / 'tools/test-game/ctl.sh'), a.game, *map(str, args)], text=True)
    result = json.loads(raw)
    if 'error' in result:
        raise RuntimeError(result)
    return result


def cmd(line):
    return ctl('command', line)


def saved():
    for path in storage.glob('w.*'):
        try:
            data = json.loads(path.read_bytes().split(b'\n', 1)[1])
            doses = [d['millirad'] for d in data.get('radiation', {}).values()]
            if doses:
                return max(doses)
        except (ValueError, IndexError, KeyError):
            pass
    return 0


def capture(name):
    report['steps'][name] = {
        'state': ctl('state'), 'inventory': ctl('inventory'),
        'effects': ctl('effects'), 'saved_millirad': saved(),
    }
    ctl('screenshot', str(game / f'{name}.png'))
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    return report['steps'][name]


def wait_dose(minimum, timeout=50):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if saved() >= minimum:
            return
        time.sleep(.1)
    raise AssertionError(f'dose did not reach {minimum}: {saved()}')


def drop_stack():
    # ctl's convenience `drop` mutates the client window in server-thread mode;
    # the actual keyboard path sends the authoritative PlayerAction packet.
    ctl('key', 'ctrl', 'down')
    ctl('key', 'q', 'tap')
    ctl('wait', 3)
    ctl('key', 'ctrl', 'up')
    ctl('wait', 5)
    cmd('kill @e[type=minecraft:item]')  # prevent picking the test item back up


if a.reenter:
    ctl('wait', 240)
    state = capture('reentered')
    expected = report['steps']['saved']['saved_millirad']
    assert state['saved_millirad'] == expected, (state['saved_millirad'], expected)
    assert not state['inventory']['counts']
else:
    ctl('wait', 120)  # integrated-server guest/inventory handshake
    state = ctl('state')
    assert state['gamemode'] == 'creative', 'start a NEW creative test world'
    assert saved() == 0, 'use a fresh DIST storage to avoid another world dose'
    report['packages_sha256'] = {f.name: hashlib.sha256(f.read_bytes()).hexdigest() for f in a.dist.glob('*.hmod')}
    cmd('clear @a')
    cmd('gamerule minecraft:natural_health_regeneration false')
    cmd('gamemode survival')
    cmd('give @a ntm:ingot_uranium 1')
    ctl('wait', 260)
    single = capture('single')
    assert single['inventory']['counts'] == {'ntm:ingot_uranium': 1}
    assert single['saved_millirad'] > 0
    drop_stack()
    ctl('wait', 120)
    empty = capture('dropped-single')
    assert not empty['inventory']['counts']
    stable = saved()
    ctl('wait', 60)
    assert saved() == stable, 'dose still accumulates after the real Q drop'
    cmd('give @a ntm:ingot_uranium 64')
    wait_dose(210000)
    mild = capture('sickness-200')
    assert any(e['id'] == 'weakness' for e in mild['effects']['effects']['active'])
    assert mild['inventory']['counts']['ntm:ingot_uranium'] == 64
    wait_dose(610000)
    capture('sickness-600')
    drop_stack()
    ctl('wait', 120)
    severe = capture('dropped-stack')
    assert not severe['inventory']['counts']
    assert severe['effects']['health'] < 20, 'expected poison damage'
    stable = saved()
    ctl('wait', 60)
    assert saved() == stable
    # No /ntmlogic save: this verifies automatic player-dose persistence.
    capture('saved')
    ctl('save')
print(json.dumps({'ok': True, 'report': str(report_path), 'dose_millirad': saved()}))

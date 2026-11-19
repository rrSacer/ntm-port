#!/usr/bin/env python3
"""T04 strict cut (P3(d) of the T04 review): the T03 scenario C (native_rbmk_t03.py: a rod taken out of its channel
beside a ReaSim source that keeps burning) with the state T04 added in play at the cut: the satellite events the
burning channels report (`rbmk_rays`) and a dial rule rewritten just before the act (`/ntmrbmk dial dialFluxRange 7`;
the vanilla control rewrites its own rule, `/gamerule random_tick_speed 7`, kept in level.dat).

Modes prepare/act/verify/again/drain, as T03's; the runner (run-rbmk-t04-c-proof.py) cuts the save at the points
between kv_committed and chunks_written. Verify adds, to T03's checks on the rods: the rule reads back as one of its two
values (never a third, never unreadable), and NTM's satellite events read back as each reading allows
(`rays_as_expected`: the tick of the reading and the channels with a rod recorded with it).
"""
import re
import sys
import time
import native_rbmk_t03 as t03
from native_game import Game

NTM_RULE, VANILLA_RULE = 'dialFluxRange', 'random_tick_speed'
OLD, NEW = '5', '7'


def rule(g):
    if t03.VANILLA:
        text = ' '.join(g.command(f'gamerule {VANILLA_RULE}').get('chat', []))
    else:
        text = ' '.join(g.command(f'ntmrbmk dial {NTM_RULE}').get('chat', []))
    m = re.search(r'(?:=|to:?)\s*(\S+)\s*$', text.strip())
    return m[1] if m else text


def rays(g):
    if t03.VANILLA:
        return None
    text = ' '.join(g.command('ntmtest rays').get('chat', []))
    m = re.search(r'tick=(\d+) rays=(\S*)', text)
    if not m:
        return {'readable': False, 'text': text[:200]}
    events = [e.split(':') for e in filter(None, m[2].split(';'))]
    return {'readable': True, 'tick': int(m[1]), 'count': len(events), 'expires': [int(e[2]) for e in events],
            'at': [[int(v) for v in e[0].split(',')] for e in events]}


def channels_with_rod(g):
    """The scenario's channels (P, S, Q) that hold a rod as the world reads now (`data get block ... Items`)."""
    return [list(p) for p in (t03.CP, t03.CS, t03.CQ) if 'rbmk_fuel_' in t03.block_items(g, p)]


def rays_as_expected(r, rods):
    """What a reading may hold (T04 re-review P3(a)): an event only where a channel holds a rod (a channel reports
    while it has a rod and flux, `TileEntityRBMKRod:122-124`), each on its phase (reported on a tick multiple of 200:
    `expiresOn = tick + 300`), none past its expiry by more than the 20 ticks `updateSystem` may take to remove it.
    None at all is expected when no channel took flux on the last multiple of 200, as after a cut that lost P's rod
    (`chunks_written`: S's partner gone, S and Q starve)."""
    return (r['readable'] and all(e % 200 == 100 for e in r['expires'])
            and all(e > r['tick'] - 20 for e in r['expires']) and all(a in rods for a in r['at']))


def set_rule(g, value):
    g.command(f'gamerule {VANILLA_RULE} {value}' if t03.VANILLA else f'ntmrbmk dial {NTM_RULE} {value}')


def prepare(g):
    t03.prepare(g)
    # The rule at its old value in the save every cut starts from (vanilla's default is 3, the dial's 5).
    set_rule(g, OLD)
    g.evidence['rule_prepared'] = rule(g)
    g.ctl('save')
    g.save()


def act(g):
    g.ctl('wait-ready 120')
    # The new value just before the act and its save: a cut leaves the old one (the last save's) or the new one.
    set_rule(g, NEW)
    g.evidence['rule_before_save'] = rule(g)
    g.evidence['rays_before_save'] = rays(g)
    t03.act(g)


def verify(g, again=False):
    t03.verify(g, again)
    suffix = '_again' if again else ''
    got = rule(g)
    g.evidence['rule' + suffix] = got
    g.check('rule_is_one_of_its_values' + suffix, got in (OLD, NEW), got)
    if not t03.VANILLA:
        rods = channels_with_rod(g)
        r = rays(g)
        r['channels_with_rod'] = rods
        g.evidence['rays' + suffix] = r
        g.check('rays_read_back' + suffix, rays_as_expected(r, rods), r)


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t04c-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': t03.drain}[mode](g)
    g.evidence['ok'] = True
    g.save()

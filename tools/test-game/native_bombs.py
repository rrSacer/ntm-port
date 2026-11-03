#!/usr/bin/env python3
"""Real components/UI, manual detonator and redstone/fuses in a fresh world.

Only the existing administrative radius cap is changed through its native
command. No machine state, components or fuse values are injected.
"""
import json
import re
import sys
import time
from native_game import Game
from native_solar_stirling import place
from native_zirnox import machine, pos_text
from native_pile import block

PARTS = ['boy_shielding', 'boy_target', 'boy_bullet', 'boy_propellant', 'boy_igniter']


def log(g):
    return (g.folder / 'client-first.log').read_text(errors='replace')


def wait(g, name, predicate, actual, timeout=120):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            g.check(name, True, actual())
            return
        time.sleep(.3)
    g.check(name, False, actual())


def prepare(g, x, bomb):
    g.ground(x, 100, 0, width=16, height=8)
    g.command(f'forceload add {x-32} -32 {x+32} 32')
    g.ctl('wait-ready 120')
    g.command(f'fill {x-10} 96 -10 {x+10} 99 10 minecraft:stone')
    core = tuple(place(g, bomb, x, 100, 0))
    g.check('item_placed', block(g, core) == 'ntm:' + bomb, core)
    return core


def assemble(g, x):
    core = prepare(g, x, 'nuke_boy')
    g.open(*core)
    for i, part in enumerate(PARTS):
        g.put('ntm:' + part, 1, i)
    slots = g.slots()
    g.check('five_real_components',
            [s.get('item') for s in slots] == ['ntm:' + p for p in PARTS], slots)
    g.close()
    state = machine(g, core)['data']
    g.check('components_in_confirmed_state', state['slots'] == ['ntm:' + p for p in PARTS], state)
    return core


def detonator(g, core):
    g.close()
    g.command('clear @s')
    g.command('give @s ntm:detonator 1')
    g.ctl('select ntm:detonator')
    g.command(f'tp @s {core[0]+.5} {core[1]} {core[2]+4.5}')
    start = len(log(g))
    g.ctl('key shift down')
    time.sleep(.2)
    try:
        g.ctl('interact ' + pos_text(core))
    finally:
        g.ctl('key shift up')
    wait(g, 'detonator_linked', lambda: 'Detonador ligado' in log(g)[start:],
         lambda: log(g)[start:][-1000:], 15)
    g.check('link_does_not_consume_bomb', block(g, core).startswith('ntm:'), block(g, core))
    g.ctl('look 0 -90')
    g.ctl('use-item')
    return start


def lever(g, core):
    p = (core[0]+1, core[1], core[2])
    g.close()
    g.command('clear @s')
    g.command(f'setblock {pos_text(p)} minecraft:lever[face=floor,facing=north,powered=false]')
    g.command(f'tp @s {core[0]+3.5} {core[1]} {core[2]+.5}')
    start = len(log(g))
    g.ctl('interact ' + pos_text(p))
    time.sleep(.1)
    observed = g.ctl('block ' + pos_text(p))
    g.check('physical_lever_powered', 'powered=true' in str(observed), observed)
    return start


def no_parts(g):
    counts = g.ctl('inventory')['counts']
    entities = g.ctl('entities')['entities']
    ids = {'ntm:' + p for p in PARTS}
    found = {p: counts.get(p, 0) + sum((e.get('item') or {}).get('count', 0)
             for e in entities if (e.get('item') or {}).get('item') == p) for p in ids}
    g.check('spent_components_not_in_player_or_world', not any(found.values()), found)


def nuke(g, x, manual):
    g.evidence['phase'] = 'little-boy-detonator' if manual else 'little-boy-redstone'
    core = assemble(g, x)
    start = detonator(g, core) if manual else lever(g, core)
    cause = 'detonador' if manual else 'redstone'
    wait(g, 'real_nuke_admitted',
         lambda: re.search(r'ntm:nuke_boy at .*\(' + cause + r'\): explosion .*radius 12\n', log(g)[start:]) is not None,
         lambda: [l for l in log(g)[start:].splitlines() if 'Little Boy at' in l], 60)
    wait(g, 'bomb_and_components_consumed', lambda: block(g, core) == 'minecraft:air', lambda: block(g, core))
    below = (core[0], core[1]-1, core[2])
    wait(g, 'real_crater_removed_stone', lambda: block(g, below) in ('minecraft:air', 'minecraft:fire'), lambda: block(g, below))
    if manual:
        text = log(g)[start:]
        g.check('intent_precedes_confirmation',
                'Detonação solicitada.' in text and 'Detonada.' in text and text.index('Detonação solicitada.') < text.index('Detonada.'),
                [l for l in text.splitlines() if 'solicitada' in l or 'Detonada.' in l])
    no_parts(g)
    g.snapshot('bombs-' + cause)


def charge(g, x, manual):
    g.evidence['phase'] = 'fuse-detonator' if manual else 'fuse-redstone'
    core = prepare(g, x, 'dynamite')
    start = detonator(g, core) if manual else lever(g, core)
    fuse = machine(g, core)['data']['fuse']
    g.check('confirmed_fuse_before_explosion', isinstance(fuse, int) and 0 < fuse <= 80, fuse)
    g.check('charge_still_present_during_fuse', block(g, core) == 'ntm:dynamite', block(g, core))
    wait(g, 'fuse_caused_real_explosion',
         lambda: re.search(r'ntm:dynamite at .*\(pavio\): power 8', log(g)[start:]) is not None,
         lambda: [l for l in log(g)[start:].splitlines() if 'Dinamite at' in l], 30)
    g.check('charge_removed', block(g, core) == 'minecraft:air', block(g, core))
    if manual:
        g.check('fuse_confirmed_once', log(g)[start:].count('Pavio aceso.') == 1,
                [l for l in log(g)[start:].splitlines() if 'Pavio aceso.' in l])


def main():
    g = Game(sys.argv[1], 'bombs')
    g.command('gamemode creative')
    result = g.command('ntmbomba config mundo raiomax 12')
    chat = result.get('chat', [])
    g.check('radius_cap_configured', any('raiomax 12' in s for s in chat) and not any('Unknown' in s or 'Erro:' in s for s in chat), result)
    confirmed = g.command('ntmbomba config')
    g.check('radius_cap_read_back', any('raiomax 12' in s and '(do mundo)' in s for s in confirmed.get('chat', [])), confirmed)
    nuke(g, 0, True)
    nuke(g, 80, False)
    charge(g, 160, True)
    charge(g, 240, False)
    g.command('forceload remove all')
    g.snapshot('bombs-final')
    worlds = list(g.folder.glob('**/data/ntm/world'))
    saved = json.loads(worlds[0].read_text())
    g.check('four_confirmed_detonations', saved['bombs']['next'] == 4, saved['bombs']['next'])
    g.evidence['ok'] = True
    g.save()


if __name__ == '__main__':
    main()

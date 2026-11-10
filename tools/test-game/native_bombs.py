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
import ntm_store

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
    time.sleep(.25)
    linked = machine(g, core)['data']
    g.check('link_alone_does_not_light_fuse', linked.get('fuse') is None, linked)
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
    observed = g.ctl('interact ' + pos_text(p))
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
    wait(g, 'charge_removed', lambda: block(g, core) == 'minecraft:air', lambda: block(g, core), 15)
    if manual:
        g.check('fuse_confirmed_once', log(g)[start:].count('Pavio aceso.') == 1,
                [l for l in log(g)[start:].splitlines() if 'Pavio aceso.' in l])


# A nuke (EntityNukeExplosionMK5) and a balefire (EntityBalefire), set off and left digging when the world closes.
RESTART = {'nuke': (400, ''), 'balefire': (720, ' balefire')}


def explosion_id(text, x):
    """The id of the explosion the test command set off at x (`... at [x, y, z] (teste): explosion <id>, radius R`)."""
    found = re.findall(r'at \[%d, \d+, -?\d+\] \(teste\): explosion (\S+), radius' % x, text)
    return found[-1] if found else None


def dug(g, x):
    """Air under the blast's centre, a column down from y=99: how far its crater got."""
    return sum(block(g, (x, y, 0)) in ('minecraft:air', 'minecraft:fire') for y in range(60, 100))


def restart_arm(g):
    """The second session's start: a nuke and a balefire of radius 80 set off, their craters still being dug when the
    world is saved and closed (the runner quits): at the least fuel a tick allows, the nuke's crater takes 917 ticks."""
    g.evidence['phase'] = 'restart-arm'
    g.command('gamemode creative')
    g.command('ntmbomba config mundo raiomax 80')
    g.command('ntmbomba config mundo fuel 4')
    # Both areas are loaded before either goes off: the nuke's crater takes a fixed 917 ticks (~46 s), and a
    # forceload wait under load, between the two, ate them before the save.
    for x, _ in RESTART.values():
        g.ground(x, 100, 0, width=4, height=4)
        g.command(f'forceload add {x-80} -80 {x+80} 80')
    time.sleep(3)
    start = len(log(g))
    for kind, (x, suffix) in RESTART.items():
        g.command(f'tp @s {x + .5} 100 0.5')
        result = g.command('ntmbomba teste 80' + suffix)
        g.check(f'{kind}_set_off', any('detonada' in line for line in result.get('chat', [])), result)
    wait(g, 'both_handed_to_the_engine', lambda: all(explosion_id(log(g)[start:], x) for x, _ in RESTART.values()),
         lambda: [l for l in log(g)[start:].splitlines() if '(teste)' in l], 60)
    ids = {kind: explosion_id(log(g)[start:], x) for kind, (x, _) in RESTART.items()}
    digging = lambda: ' '.join(g.command('ntmbomba status').get('chat', []))
    wait(g, 'both_digging', lambda: all(i in digging() for i in ids.values()), digging, 30)
    g.ctl('save')
    # The save holds both digging: they are still at it once it is done.
    status = digging()
    g.evidence['restart_arm'] = {'ids': ids, 'status': status, 'dug': {k: dug(g, x) for k, (x, _) in RESTART.items()}}
    g.check('both_still_digging_after_the_save', all(i in status for i in ids.values()), g.evidence['restart_arm'])
    # Still in its crater (not yet in its fallout, which comes back read): what the reopened world checks.
    phase = re.search(r'%s (\w+) R=' % ids['nuke'], status)
    g.check('the_nuke_still_in_its_crater_at_the_save', phase and phase.group(1) in ('Planning', 'Writing', 'Waiting'),
            g.evidence['restart_arm'])
    (g.folder / 'bombs-restart.json').write_text(json.dumps(g.evidence['restart_arm']))


def restart_check(g):
    """Reopened: the nuke's crater stops with the world (`EntityNukeExplosionMK5` keeps `ticksExisted` alone and dies
    read back); the balefire's comes back and is finished (`EntityBalefire` keeps its state)."""
    g.evidence['phase'] = 'restart-check'
    g.ctl('wait-ready 120')
    armed = json.loads((g.folder / 'bombs-restart.json').read_text())
    ids = armed['ids']
    for x, _ in RESTART.values():
        g.command(f'forceload add {x-80} -80 {x+80} 80')
    text = lambda: (g.folder / 'client-reopen.log').read_text(errors='replace')
    wait(g, 'the_nukes_crater_stops_with_the_world',
         lambda: f"{ids['nuke']}: a nuke's crater stops with the world" in text(), lambda: [l for l in text().splitlines() if ids['nuke'] in l], 60)
    wait(g, 'the_balefire_comes_back', lambda: f'explosion resumed: Ok("{ids["balefire"]}")' in text(),
         lambda: [l for l in text().splitlines() if ids['balefire'] in l], 60)
    nuke_dug = dug(g, RESTART['nuke'][0])
    wait(g, 'the_balefire_is_finished', lambda: f'explosion done: {ids["balefire"]}' in text(),
         lambda: [l for l in text().splitlines() if ids['balefire'] in l][-3:], 600)
    g.check('the_nukes_crater_did_not_go_on', dug(g, RESTART['nuke'][0]) == nuke_dug, {'then': nuke_dug, 'now': dug(g, RESTART['nuke'][0])})
    g.check('the_balefire_dug_on', dug(g, RESTART['balefire'][0]) >= armed['dug']['balefire'], {'armed': armed['dug'], 'now': dug(g, RESTART['balefire'][0])})
    status = ' '.join(g.command('ntmbomba status').get('chat', []))
    g.check('nothing_left_digging', ids['nuke'] not in status and ids['balefire'] not in status, status)
    g.command('ntmbomba config mundo fuel 22')
    g.ctl('save')
    saved = ntm_store.value(g.folder)
    g.check('the_balefire_is_forgotten', not saved['bombs'].get('balefires'), saved['bombs'].get('balefires'))


def main():
    mode = sys.argv[2] if len(sys.argv) > 2 else 'setup'
    if mode != 'setup':
        g = Game(sys.argv[1], 'bombs-' + mode)
        {'restart-arm': restart_arm, 'restart-check': restart_check}[mode](g)
        g.evidence['ok'] = True
        g.save()
        return
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
    # The storage is on disk as of a save: ask for one (it waits, storage included), then read what it wrote.
    g.ctl('save')
    saved = ntm_store.value(g.folder)
    g.check('four_confirmed_detonations', saved['bombs']['next'] == 4, saved['bombs']['next'])
    g.evidence['ok'] = True
    g.save()


if __name__ == '__main__':
    main()

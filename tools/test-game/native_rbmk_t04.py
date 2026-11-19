#!/usr/bin/env python3
"""T04 proof on a disposable world (`rbmk-plano` T04): the reactor's scheduler, streams, heat, occlusion, dials,
radiation and satellite events in the native game, NTM only.

Each reactor step can be dumped (`ntmtest rbmkdump`, NTM_TEST_HOOKS=1): the exact input the server built for that step
(dials, random, queued streams, occlusion, columns, by their bits), written once the step is accepted. Two dumps of the
same reactor with nothing touched by hand between them must replay one into the other with the pure core, the ticks
between them stepped once each (`ntm_core::rbmk::dump`, run by the runner after the game): that is the check "against
the fixture" (the core is the one `tests/rbmk_flux_parity.rs` holds to the Java oracle) and, across a reopen, "no
round lost or repeated". Chains: `a-*` (the 15x15 reactor warming), `b-*` (the same, across the reopen).

Mode `play`:
  - a 15x15 reactor placed and fuelled by the player, row by row: reflector and blank rings with an absorber, a boiler,
    a storage, a cooler and a heater in them; inside, uranium channels (plain and moderated) alternating with
    moderators, four Po-210/Be sources and a ReaSim Po-210/Be source at the centre, a manual and an automatic control
    rod; no lids (as Java places them): it steps as one reactor, all its column kinds in it, flux and heat spreading;
  - dials: `/ntmrbmk dial dialFluxRange 3` reaches the next step's input (the overworld's cache), and back to 5;
  - occlusion: beside the reactor, one small reactor per block between a source and a channel; the step's occlusion of
    that cell is Java's `isOpaqueCube` over the engine's blocks: stone, dirt and a blank's dummies count, glass,
    leaves, a slab, stairs, a pane, iron bars and a fuel channel's dummies do not; four stones block it whole;
  - radiation: two equal pairs in their own chunks, one with lids: the open pair's chunk gets more radiation;
  - satellite events: the fuel channels that took flux report every 200 ticks for 300.
Mode `reopen`: the reactor's state right after the reopen replays from the last dump before it; the satellite events
are kept; a column set into the Nether (a world without a dial cache) writes `dialColumnHeight` back to its default
while the overworld's cache keeps its own (3A), so the player's next column is as tall as the overworld reads.
"""
import json
import re
import sys
import time
from pathlib import Path
from native_game import Game

Y = 101
SRC = 'ntm:rbmk_fuel_po210be'
UEU = 'ntm:rbmk_fuel_ueu'
DUMMY = 'ntm:dummy_rbmk'
CENTRE = (7, Y, 7)
OCC_Z = (40, 42, 43)  # source, probed cell, channel
# The open pair the farther from the 15x15 reactor and the occlusion row (their radiation spreads chunk to chunk): any
# spread favours the lidded pair, against the check.
RAD = {'open': (200, Y, 8), 'lid': (100, Y, 8)}


def layout(x, z):
    """The 15x15 reactor's column at (x, z) and the rod it takes."""
    ring = min(x, 14 - x, z, 14 - z)
    if ring == 0:
        return ('rbmk_absorber', None) if (x, z) in ((0, 7), (14, 7)) else ('rbmk_reflector', None)
    if ring == 1:
        return {(1, 7): ('rbmk_boiler', None), (13, 7): ('rbmk_storage', None), (7, 1): ('rbmk_cooler', None),
                (7, 13): ('rbmk_heater', None)}.get((x, z), ('rbmk_blank', None))
    if (x, z) == (7, 7):
        return ('rbmk_rod_reasim', SRC)
    if (x, z) in ((4, 4), (10, 10), (4, 10), (10, 4)):
        return ('rbmk_rod', SRC)
    if (x, z) == (7, 5):
        return ('rbmk_control', None)
    if (x, z) == (5, 7):
        return ('rbmk_control_auto', None)
    if (x + z) % 2 == 0:
        return ('rbmk_rod_mod', UEU) if x % 4 == 2 else ('rbmk_rod', UEU)
    return ('rbmk_moderator', None)


def check(g, name, passed, actual):
    """A check kept without stopping the run: every one is reported, the run fails at its end if any did."""
    name = g.evidence.get('phase', 'setup') + ':' + name
    g.evidence['checks'][name] = {'ok': bool(passed), 'actual': actual}
    g.save(True)


def floor(g, x0, x1, z0, z1):
    g.command(f'forceload add {x0} {z0} {x1} {z1}')
    time.sleep(2)
    g.command(f'fill {x0} {Y} {z0} {x1} {Y + 20} {z1} minecraft:air')
    g.command(f'fill {x0} {Y - 1} {z0} {x1} {Y - 1} {z1} minecraft:stone')


def place(g, p, block):
    """The column `block` placed by the player at `p`, standing south of it (its dummies with it)."""
    g.command(f'give @s {block} 1')
    g.command(f'tp @s {p[0] + .5} {p[1]} {p[2] + 3.5} 180 45')
    time.sleep(.2)
    g.ctl(f'place {p[0]} {p[1]} {p[2]} --item {block}')
    time.sleep(.3)


def use_top(g, p, item):
    """`item` from the hand on the column's top cell, from the south (a rod into a channel, a lid onto a column)."""
    g.command(f'tp @s {p[0] + .5} {Y} {p[2] + 2.5}')
    time.sleep(.2)
    g.ctl(f'select {item}')
    g.ctl(f'interact {p[0]} {p[1] + 3} {p[2]} south')
    time.sleep(.4)
    g.close()


def column_json(g, p):
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', []))
    return text


def slot_has_rod(g, p):
    return 'rbmk_fuel_' in ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))


def dump(g, name, p=CENTRE, timeout=20):
    """The input of the next step of the reactor of the column at `p`, read back from its file."""
    folder = Path(g.folder) / 'dumps'
    folder.mkdir(exist_ok=True)
    path = folder / f'{name}.json'
    path.unlink(missing_ok=True)
    reply = ' '.join(g.command(f'ntmtest rbmkdump {p[0]} {p[1]} {p[2]} {path}').get('chat', []))
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists() and path.stat().st_size > 0:
            time.sleep(.2)
            return json.loads(path.read_text())
        time.sleep(.2)
    g.note(f'dump_{name}', f'no dump: {reply}')
    return None


def rays(g):
    text = ' '.join(g.command('ntmtest rays').get('chat', []))
    m = re.search(r'tick=(\d+) rays=(\S*)', text)
    if not m:
        return None, []
    out = []
    for e in filter(None, m[2].split(';')):
        pos, info, exp = e.split(':')
        out.append({'pos': [int(v) for v in pos.split(',')], 'info': info, 'expires_on': int(exp)})
    return int(m[1]), out


def rad(g, chunk):
    text = ' '.join(g.command(f'ntmtest rad {chunk[0]} {chunk[1]}').get('chat', []))
    m = re.search(rf'(?:^|[=;]){chunk[0]},{chunk[1]}:(\w+):', text)
    if not m or m[1] == 'none':
        return 0.0
    import struct
    return struct.unpack('<f', struct.pack('<I', int(m[1])))[0]


def flux_of(d):
    return [__import__('struct').unpack('<d', __import__('struct').pack('<Q', c['rod']['flux'][2]))[0] for c in d['columns'] if c.get('rod')]


def heat_of(d):
    return [__import__('struct').unpack('<d', __import__('struct').pack('<Q', c['heat']))[0] for c in d['columns']]


def build_reactor(g):
    floor(g, -4, 18, -4, 22)
    g.command('give @s ntm:rbmk_fuel_ueu 64')
    g.command('give @s ntm:rbmk_fuel_po210be 8')
    for z in range(15):
        for x in range(15):
            block, _ = layout(x, z)
            place(g, (x, Y, z), 'ntm:' + block)
        for x in range(15):
            _, rod = layout(x, z)
            if rod:
                use_top(g, (x, Y, z), rod)
    missing = [(x, z) for z in range(15) for x in range(15) if layout(x, z)[1] and not slot_has_rod(g, (x, Y, z))]
    for x, z in missing:
        use_top(g, (x, Y, z), layout(x, z)[1])
    missing = [(x, z) for z in range(15) for x in range(15) if layout(x, z)[1] and not slot_has_rod(g, (x, Y, z))]
    check(g, 'every_channel_fuelled_by_hand', not missing, missing)


def occlusion(g):
    """One small reactor per probe, 8 blocks apart: a source at z 40, the probed cell at 42, a channel at 43."""
    variants = [
        ('open', [], []),
        ('glass', [(1, 'minecraft:glass')], []),
        ('stone', [(1, 'minecraft:stone')], [1]),
        ('dirt', [(0, 'minecraft:dirt')], [0]),
        ('full', [(h, 'minecraft:stone') for h in range(4)], [0, 1, 2, 3]),
        ('leaves', [(1, 'minecraft:oak_leaves[persistent=true]')], []),
        ('slab', [(1, 'minecraft:smooth_stone_slab')], []),
        ('stairs', [(1, 'minecraft:oak_stairs')], []),
        ('pane', [(1, 'minecraft:glass_pane')], []),
        ('bars', [(1, 'minecraft:iron_bars')], []),
        ('rod_dummies', [(-2, 'ntm:rbmk_rod'), (-1, DUMMY), (0, DUMMY), (1, DUMMY)], []),
        ('blank_dummies', [(-2, 'ntm:rbmk_blank'), (-1, DUMMY), (0, DUMMY), (1, DUMMY)], [0, 1]),
    ]
    floor(g, 30, 30 + 8 * len(variants), 36, 48)
    g.command('give @s ntm:rbmk_fuel_ueu 16')
    g.command('give @s ntm:rbmk_fuel_po210be 16')
    found = {}
    for i, (name, blocks, want) in enumerate(variants):
        x = 32 + 8 * i
        src, cell, chan = (x, Y, OCC_Z[0]), (x, OCC_Z[1]), (x, Y, OCC_Z[2])
        # From the north row to the south one: the player stands south of what it places.
        place(g, src, 'ntm:rbmk_rod')
        place(g, chan, 'ntm:rbmk_rod')
        use_top(g, src, SRC)
        use_top(g, chan, UEU)
        for dy, b in blocks:
            if dy < 0:
                g.command(f'fill {x} {Y + dy} {cell[1]} {x} {Y - 1} {cell[1]} minecraft:air')
        for dy, b in blocks:
            g.command(f'setblock {x} {Y + dy} {cell[1]} {b}')
        time.sleep(.5)
        d = dump(g, f'occ_{name}-1', src)
        got = sorted(o[2] for o in (d or {}).get('obstacles', []) if o[0] == x and o[1] == cell[1]) if d else None
        found[name] = {'obstacles': got, 'expected': want}
        check(g, f'occlusion_{name}', got == want, found[name])
    g.evidence['occlusion'] = found


def radiation(g):
    for key, p in RAD.items():
        floor(g, p[0] - 3, p[0] + 3, p[2] - 3, p[2] + 5)
    g.command('give @s ntm:rbmk_fuel_ueu 4')
    g.command('give @s ntm:rbmk_fuel_po210be 4')
    g.command('give @s ntm:rbmk_lid 4')
    for key, p in RAD.items():
        chan = (p[0], Y, p[2] + 1)
        place(g, p, 'ntm:rbmk_rod')
        place(g, chan, 'ntm:rbmk_rod')
        use_top(g, p, SRC)
        use_top(g, chan, UEU)
        if key == 'lid':
            use_top(g, chan, 'ntm:rbmk_lid')
            use_top(g, p, 'ntm:rbmk_lid')
    lids = {k: [c['lid'] for c in (dump(g, f'rad_{k}-1', p) or {}).get('columns', [])] for k, p in RAD.items()}
    check(g, 'lids_as_placed', lids == {'open': [False, False], 'lid': [True, True]}, lids)
    time.sleep(20)
    got = {k: rad(g, (p[0] >> 4, p[2] >> 4)) for k, p in RAD.items()}
    g.evidence['chunk_radiation'] = got
    check(g, 'open_columns_leak_more', got['open'] > got['lid'] >= 0, got)


def play(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('ntmrbmk dial dialFluxRange 5')
    build_reactor(g)
    time.sleep(15)
    a1 = dump(g, 'a-1')
    check(g, 'one_reactor_of_225', a1 is not None and len(a1['columns']) == 225, a1 and len(a1['columns']))
    if a1:
        kinds = sorted({c['kind'] for c in a1['columns']})
        g.evidence['kinds'] = kinds
        # Reflector, absorber, blank, boiler, storage, cooler, heater, moderator, the two fuel channels and the two
        # control rods (a moderated channel is a fuel channel, moderated).
        check(g, 'every_kind_steps', len(kinds) == 12, kinds)
        check(g, 'flux_spreads', sum(1 for f in flux_of(a1) if f > 0) >= 40, sorted(flux_of(a1))[:5])
    time.sleep(25)
    a2 = dump(g, 'a-2')
    if a1 and a2:
        g.evidence['heat'] = {'a1_max': max(heat_of(a1)), 'a2_max': max(heat_of(a2)), 'a2_min': min(heat_of(a2))}
        check(g, 'it_heats', max(heat_of(a2)) > max(heat_of(a1)) > 20, g.evidence['heat'])
        check(g, 'steps_between', a2['tick'] - a1['tick'] > 200, [a1['tick'], a2['tick']])
    # A dial reaches the next step (the overworld's cache, refreshed by the command), then back.
    g.command('ntmrbmk dial dialFluxRange 3')
    d = dump(g, 'dial-1')
    check(g, 'the_dial_reaches_the_step', d is not None and d['dials'][12] == 3, d and d['dials'][12])
    g.command('ntmrbmk dial dialFluxRange 5')
    b1 = dump(g, 'b-1')
    check(g, 'back_to_5', b1 is not None and b1['dials'][12] == 5, b1 and b1['dials'][12])
    occlusion(g)
    radiation(g)
    tick, events = rays(g)
    g.evidence['rays'] = {'tick': tick, 'events': events[:8], 'count': len(events)}
    ok = tick is not None and events and all(e['info'] == 'NEUTRON_EMISSION' and e['expires_on'] % 200 == 100
                                             and 0 < e['expires_on'] - tick <= 300 for e in events)
    check(g, 'satellite_events', bool(ok), g.evidence['rays'])
    g.command(f'tp @s {CENTRE[0] + .5} {Y + 6} {CENTRE[2] + 20.5}')
    time.sleep(2)
    g.ctl('save')
    b2 = dump(g, 'b-2')
    check(g, 'dump_before_quit', b2 is not None, None)


def reopen(g):
    g.ctl('wait-ready 120')
    b3 = dump(g, 'b-3')
    check(g, 'dump_after_reopen', b3 is not None, None)
    time.sleep(15)
    b4 = dump(g, 'b-4')
    check(g, 'dump_later', b4 is not None, None)
    tick, events = rays(g)
    g.evidence['rays_after'] = {'tick': tick, 'count': len(events), 'events': events[:8]}
    check(g, 'satellite_events_kept', bool(events), g.evidence['rays_after'])
    # 3A: the Nether has no dial cache after the start; a column set there reads the height first, writing the rule
    # back to its default, while the overworld's cache keeps the 6 the player set.
    g.command('ntmrbmk dial dialColumnHeight 6 confirmar')
    g.command('execute in minecraft:the_nether run forceload add 0 0')
    time.sleep(2)
    g.command('execute in minecraft:the_nether run fill 0 100 0 8 110 8 minecraft:air')
    g.command('execute in minecraft:the_nether run setblock 4 101 4 ntm:rbmk_blank')
    time.sleep(1.5)
    rule = ' '.join(g.command('ntmrbmk dial dialColumnHeight').get('chat', []))
    g.evidence['height_rule_after_nether'] = rule
    check(g, 'the_nether_first_read_writes_the_default', '= 4' in rule, rule)
    p = (60, Y, 20)
    floor(g, 56, 64, 16, 26)
    place(g, p, 'ntm:rbmk_blank')
    cells = [g.ctl(f'block {p[0]} {p[1] + dy} {p[2]}')['block'].split('[')[0] for dy in range(8)]
    height = next((i for i, b in enumerate(cells) if i > 0 and b != DUMMY), len(cells))
    g.evidence['overworld_column_height'] = height
    check(g, 'the_overworld_cache_keeps_its_height', height == 6, cells)
    g.command('ntmrbmk dial dialColumnHeight 4')
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'rbmk-t04-' + mode)
    g.evidence['phase'] = mode
    {'play': play, 'reopen': reopen}[mode](g)
    failed = [k for k, c in g.evidence['checks'].items() if not c['ok']]
    g.evidence['failed'] = failed
    g.evidence['ok'] = not failed
    g.save()
    sys.exit(1 if failed else 0)

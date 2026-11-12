#!/usr/bin/env python3
"""The radiolysis chamber (docs/onda-2-sublote-2.md) on a disposable native world.

setup          four polonium pellets (200 heat), a water identifier and a contaminated sword in the screen; a barrel of water on a
               duct at its east port and an empty one on a duct at its west port: the identifier types the input and goes to its out
               slot, the water cracks into hydrogen peroxide and hydrogen (4 to 1, a hundred mB at a time) and the peroxide flows to
               the west barrel, the sword comes out of the sterilization slot without its contagion, the pellets lose life;
capture-state  a chamber with the same pellets, identifier and sword (no network), saved once the identifier has moved and the sword
               is clean;
check-state    after reopening, the slots, the types of the three tanks and the pellets' life are as saved and the chamber still runs.
No machine state is injected; time passes by `tick sprint`.
"""
import json
import re
import sys
import time
from native_game import Game
from native_furnaces import core_of
from native_orbus import open_through
from native_recipes import screen_json

X, Z = 40, 8
SAVED = 'radiolysis-saved.json'
IDENTIFIER = 'ntm:fluid_identifier_multi'


def sprint(g, ticks):
    g.command(f'tick sprint {ticks}')
    deadline = time.monotonic() + 900
    while time.monotonic() < deadline:
        if 'sprint' not in ' '.join(g.command('tick query').get('chat', [])).lower():
            return
        time.sleep(.5)


def state_text(g, p):
    return ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} ntm').get('chat', [])).replace('\\"', '"')


def state(g, core):
    """The chamber's own saved state: the dict holding its `clock`, inside the machine's JSON (the `ntm` string of the block entity)."""
    found = re.search(r'\{.*\}', state_text(g, core))
    data = json.loads(found[0]) if found else {}

    def find(node):
        if isinstance(node, dict):
            if 'clock' in node and 'tanks' in node:
                return node
            for v in node.values():
                r = find(v)
                if r is not None:
                    return r
        return None
    return find(data) or {}


def tanks(g, core):
    """[(fluid, mb)] of the input and the two outputs."""
    return [(t.get('fluid', ''), t['mb']) for t in state(g, core).get('tanks', [])]


def number(g, core, key):
    return state(g, core).get(key)


def items(g, p):
    """slot -> (id, count, custom data text) from the block entity's items."""
    text = ' '.join(g.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    out, depth, start, entries = {}, 0, None, []
    for i, c in enumerate(text):
        if c == '{':
            if depth == 1:
                start = i
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 1 and start is not None:
                entries.append(text[start:i + 1])
                start = None
        elif c == '[' and depth == 0:
            depth = 1
    for entry in entries:
        slot = re.search(r'[sS]lot: ?(\d+)', entry)
        item = re.search(r'(?:^|[,{] ?)id: ?"([\w:]+)"(?=[,}])', entry)
        count = re.search(r'[cC]ount: ?(\d+)', entry)
        if slot and item:
            out[int(slot[1])] = (item[1], int(count[1]) if count else 1, 'custom_data' in entry or 'ntmContagion' in entry or 'PELLET' in entry)
    return out


def put(g, item, slot, component='', count=1):
    g.command(f'give @s {item}{component} {count}')
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        screen = g.ctl('screen')
        src = next((s['slot'] for s in screen.get('slots', []) if s['slot'] < 36 and s.get('item') == item), None)
        if src is not None:
            break
        time.sleep(.2)
    else:
        raise AssertionError(f'item not received: {item}')
    g.ctl(f'click-slot {src}')
    g.ctl(f'click-slot {300 + slot}')
    time.sleep(.4)


def build(g):
    g.close()
    g.ground(X, 100, Z, width=12)
    g.command('clear @s')
    g.command('give @s ntm:machine_radiolysis 1')
    g.command(f'tp @s {X + 5 + .5} 100 {Z + 3.5}')
    time.sleep(1)
    g.ctl(f'place {X + 5} 100 {Z} --item ntm:machine_radiolysis')
    time.sleep(1.5)
    g.command('clear @s')
    return core_of(g, (X + 5, 100, Z), '"clock"')


def open_chamber(g, core):
    from native_orbus import dummies
    cells = dummies(g, core)
    g.evidence['dummies'] = len(cells)
    return open_through(g, cells)


def fill_chamber(g, core):
    open_chamber(g, core)
    for slot in range(4):
        put(g, 'ntm:pellet_rtg_polonium', slot)
    put(g, IDENTIFIER, 10, '[minecraft:custom_data={ntm_fluid:"water"}]')
    put(g, 'minecraft:iron_sword', 12, '[minecraft:custom_data={ntmContagion:1b}]')
    g.close()


def duct_and_barrel(g, x, core, fluid):
    """A duct at `x` typed by the identifier in hand and a barrel beside it."""
    y, z = core[1], core[2]
    g.command(f'setblock {x} {y} {z} ntm:fluid_duct_neo')
    g.command(f'setblock {x + (1 if x > core[0] else -1)} {y} {z} ntm:barrel_steel[facing=south]')
    g.command(f'tp @s {x + .5} {y} {z + 2.5} facing {x + .5} {y} {z + .5}')
    time.sleep(1.5)
    g.command('clear @s')
    g.command(f'give @s {IDENTIFIER}[minecraft:custom_data={{ntm_fluid:"{fluid}"}}] 1')
    time.sleep(1)
    g.ctl(f'select {IDENTIFIER}')
    g.ctl(f'interact {x} {y} {z}')
    time.sleep(.6)
    g.command('clear @s')


def barrel_mb(g, p):
    text = state_text(g, p)
    f = re.search(r'"fluid":"(\w*)"', text)
    mb = re.search(r'"mb":(\d+)', text)
    return (f[1] if f else '', int(mb[1]) if mb else 0)


def water_barrel(g, p, buckets):
    """Buckets of water in through the barrel's screen, then its mode to output (two presses)."""
    g.command('clear @s')
    for n in range(buckets):
        g.command('give @s minecraft:water_bucket 1')
        time.sleep(.8)
        g.command(f'tp @s {p[0] + .5} {p[1]} {p[2] + 2.5} facing {p[0] + .5} {p[1]} {p[2] + .5}')
        if n == 0:
            time.sleep(1)
            g.ctl(f'interact {p[0]} {p[1]} {p[2]}')
            time.sleep(2)
        screen = g.ctl('screen')
        hot = [s['slot'] for s in screen.get('slots', []) if s.get('item') == 'minecraft:water_bucket' and s['slot'] < 36]
        if hot:
            g.ctl(f'click-slot {hot[0]}')
            time.sleep(.4)
            g.ctl('click-slot 302')
            time.sleep(1)
            # The empty bucket comes out on slot 303: take it away for the next one.
            g.ctl('click-slot 303 shift')
            time.sleep(.4)
    slot0 = next(w for w in g.ctl('ui').get('widgets', []) if w.get('id') == 'slot-0')['bounds']
    ox, oy = slot0[0] - 8, slot0[1] - (84 + 58)
    for _ in range(2):
        g.ctl(f'click-at {ox + 151 + 9} {oy + 35 + 9}')
        time.sleep(.8)
    g.close()


def setup(g):
    core = build(g)
    g.evidence['core'] = list(core)
    east, west = (core[0] + 3, core[1], core[2]), (core[0] - 3, core[1], core[2])
    duct_and_barrel(g, core[0] + 2, core, 'water')
    duct_and_barrel(g, core[0] - 2, core, 'peroxide')
    water_barrel(g, east, 4)
    fill_chamber(g, core)
    sprint(g, 60)
    s = items(g, core)
    g.check('the_identifier_went_to_its_out_slot', 10 not in s and s.get(11, ('', 0))[0] == IDENTIFIER, s)
    g.check('the_sword_came_out_clean', 12 not in s and s.get(13, ('', 0, True))[:2] == ('minecraft:iron_sword', 1) and not s[13][2], s)
    g.check('the_pellets_lost_life', all(s.get(i, ('', 0, False))[2] for i in range(4)), s)
    sprint(g, 900)
    t = tanks(g, core)
    peroxide, hydrogen = barrel_mb(g, west), (t[2][1] if len(t) > 2 else None)
    g.evidence['after'] = {'tanks': t, 'west': peroxide, 'east': barrel_mb(g, east), 'heat': number(g, core, 'heat'), 'power': number(g, core, 'power')}
    g.check('the_tanks_are_typed_by_the_recipe', len(t) == 3 and t[0][0] == 'water' and t[1][0] == 'peroxide' and t[2][0] == 'hydrogen', t)
    g.check('the_heat_is_the_four_pellets', number(g, core, 'heat') == 200, number(g, core, 'heat'))
    g.check('the_chamber_makes_power', (number(g, core, 'power') or 0) > 0, number(g, core, 'power'))
    # 200 heat cracks a hundred mB every 20 ticks: peroxide 80 and hydrogen 20 each, the peroxide on to the west barrel.
    total_peroxide = peroxide[1] + (t[1][1] if len(t) > 2 else 0)
    g.check('peroxide_and_hydrogen_four_to_one', hydrogen is not None and hydrogen > 0 and total_peroxide == 4 * hydrogen, [t, peroxide])
    g.check('the_peroxide_flows_out_to_the_west_barrel', peroxide[0] == 'peroxide' and peroxide[1] > 0, peroxide)
    g.check('the_input_comes_from_the_east_barrel', barrel_mb(g, east)[1] < 4000, barrel_mb(g, east))


def capture(g):
    core = build(g)
    g.evidence['core'] = list(core)
    fill_chamber(g, core)
    sprint(g, 150)
    s, t = items(g, core), tanks(g, core)
    g.check('identifier_moved_and_sword_clean_before_the_save', 10 not in s and 11 in s and s.get(13, ('', 0, True))[:2] == ('minecraft:iron_sword', 1) and not s[13][2], s)
    g.check('the_tanks_are_typed_before_the_save', [f for f, _ in t] == ['water', 'peroxide', 'hydrogen'], t)
    (g.folder / SAVED).write_text(json.dumps({'core': list(core), 'items': {str(k): list(v) for k, v in s.items()}, 'tanks': t}))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    core = tuple(saved['core'])
    sprint(g, 5)
    s, t = items(g, core), tanks(g, core)
    now = {str(k): list(v) for k, v in s.items()}
    g.check('the_slots_are_as_saved', set(now) == set(saved['items']) and all(now[k][:2] == saved['items'][k][:2] for k in now), [saved['items'], now])
    g.check('the_pellets_kept_their_life', all(now[str(i)][2] for i in range(4)), now)
    g.check('the_tanks_are_typed_as_saved', t == [tuple(x) for x in saved['tanks']] or [f for f, _ in t] == [f for f, _ in saved['tanks']], [saved['tanks'], t])
    g.check('the_chamber_still_runs', number(g, core, 'heat') == 200, number(g, core, 'heat'))


if __name__ == '__main__':
    g = Game(sys.argv[1], 'radiolysis-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

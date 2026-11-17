#!/usr/bin/env python3
"""The free-electron laser and the SILEX (docs/onda-2-sublote-2.md) on a disposable native world.

setup          a FEL with a creative battery and a bismuth crystal (visible), switched on, and a SILEX along its beam, seven
               cells on, of 16 tanks of peroxide and two uranium ingots: the beam goes through the SILEX (24 cells, a valid
               SILEX found); the tanks load, an ingot loads 900 mB of fill, and the visible laser runs it 100 ticks a
               nugget, the first a U-235 (index 0), the next U-238 (index 137 % 12 = 5); with the digamma crystal the SILEX
               runs eight times as fast, a stone block further on burns to digamma fire on digamma ash, and a survival
               player in the beam takes 0.1 digamma a tick;
capture-state  the world is saved with the SILEX running (its fill, item, progress and queue) and the FEL on;
check-state    after reopening, the FEL is still on and the SILEX's state as saved, and it goes on running; a second SILEX
               across the beam further on is broken (all its parts), spilling the ingot in it and dropping a SILEX item.
No machine state is injected; time passes by `wait`.
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state, facing_of, FACING
from native_furnaces import core_of
from native_orbus import dummies, open_through
from native_recipes import screen_json

X, Z, Y = 60, 60, 100
SAVED = 'fel-saved.json'
# The SILEX's core this far along the beam: its first part at 7 - 1 = 6, past the FEL's 5.
SILEX_AT = 7
STONE_AT = 20


def state(g, core):
    text = machine_state(g, core)
    found = re.search(r'\{.*\}', text)
    data = json.loads(found[0]) if found else {}

    def find(node, key):
        if isinstance(node, dict):
            if key in node:
                return node
            for v in node.values():
                r = find(v, key)
                if r is not None:
                    return r
        return None
    return data, find


def fel(g, core):
    data, find = state(g, core)
    return find(data, 'missing_valid_silex') or {}


def silex(g, core):
    data, find = state(g, core)
    return find(data, 'recipe_index') or {}


def index_after(runs, start=0, weights=12):
    """`recipeIndex` after `runs` runs: `%= total` before the pick, `+= 137` after."""
    i = start
    for _ in range(runs):
        i = i % weights + 137
    return i


def nuggets_of(node):
    out = {}
    for i in range(4, 11):
        q = slot_items(node).get(i)
        if q:
            out[q[0]] = out.get(q[0], 0) + q[1]
    return out


def slot_items(node):
    out = {}
    for i, s in enumerate(node.get('slots', [])):
        if s:
            out[i] = (s['id'], s['count'])
    return out


def is_(g, p, block):
    return 'passed' in ' '.join(g.command(f'execute if block {p[0]} {p[1]} {p[2]} {block}').get('chat', [])).lower()


def place(g, item, at):
    """`item` placed at `at` by a player south of it looking north."""
    g.close()
    g.command('clear @s')
    g.command(f'give @s {item} 1')
    g.command(f'tp @s {at[0] + .5} {Y} {at[2] + 3.5} facing {at[0] + .5} {Y} {at[2]}')
    time.sleep(1)
    g.ctl(f'place {at[0]} {at[1]} {at[2]} --item {item}')
    time.sleep(1.5)
    g.command('clear @s')


def place_across(g, item, at):
    """`item` placed at `at` by a player west of it looking east."""
    g.close()
    g.command('clear @s')
    g.command(f'give @s {item} 1')
    g.command(f'tp @s {at[0] - 3 + .5} {Y} {at[2] + .5} facing {at[0] + .5} {Y} {at[2] + .5}')
    time.sleep(1)
    g.ctl(f'place {at[0]} {at[1]} {at[2]} --item {item}')
    time.sleep(1.5)
    g.command('clear @s')


def gametime(g):
    """The server's game time in ticks (`time query gametime`)."""
    return int(re.search(r'(\d+)', ' '.join(g.command('time query gametime').get('chat', [])))[1])


def clear_box(g, core):
    g.command(f'fill {core[0] - 1} {core[1]} {core[2] - 1} {core[0] + 1} {core[1] + 2} {core[2] + 1} minecraft:air')
    time.sleep(1)


def away(g, core):
    """Out of the beam: beside the FEL."""
    g.command(f'tp @s {core[0] + 4.5} {Y} {core[2] + .5}')


def build(g):
    g.close()
    g.command('gamemode creative @s')
    # The beam's 24 cells either way, on stone: the DRX's ash falls like sand.
    g.ground(X - 6, Y, Z - 30, width=14, height=6, depth=60)
    place(g, 'ntm:machine_fel', (X, Y, Z))
    core = core_of(g, (X, Y, Z), '"missing_valid_silex"')
    facing = facing_of(g, core)
    d = FACING[facing]
    # Where a SILEX placed like that has its core: found by placing one aside, then taken away.
    probe = (X + 5, Y, Z + 6)
    place(g, 'ntm:machine_silex', probe)
    probe_core = core_of(g, probe, '"recipe_index"')
    offset = [probe_core[i] - probe[i] for i in range(3)]
    clear_box(g, probe_core)
    target = (core[0] + d[0] * SILEX_AT, core[1], core[2] + d[2] * SILEX_AT)
    place(g, 'ntm:machine_silex', (target[0] - offset[0], target[1] - offset[1], target[2] - offset[2]))
    s_core = core_of(g, target, '"recipe_index"')
    g.evidence.update(fel=list(core), facing=facing, silex=list(s_core), silex_facing=facing_of(g, s_core), offset=offset)
    along = FACING[facing_of(g, s_core)] in (d, tuple(-v for v in d))
    g.check('the_silex_is_on_the_beam_line_and_along_it', tuple(s_core) == tuple(target) and along, [s_core, target, facing_of(g, s_core)])
    return core, d, s_core


def open_fel(g, core):
    cells = dummies(g, core) or [core]
    open_through(g, cells)


def open_silex(g, core):
    cells = [c for c in dummies(g, core) if c[1] == core[1]] or [core]
    open_through(g, cells)


def press_on(g):
    screen = screen_json('machine_fel')
    button = next(b for b in screen['buttons'] if b['id'] == 0)
    g.button(button['rect'], screen['size'])
    time.sleep(.6)


def setup(g):
    core, d, s_core = build(g)
    open_fel(g, core)
    g.put('ntm:battery_creative', 1, 0)
    g.put('ntm:laser_crystal_bismuth', 1, 1)
    press_on(g)
    g.close()
    open_silex(g, s_core)
    g.put('ntm:fluid_tank_full__peroxide', 16, 2)
    g.put('ntm:ingot_uranium', 2, 0)
    g.close()
    away(g, core)
    g.ticks(30)
    f = fel(g, core)
    g.evidence['fel_after_30'] = f
    g.check('the_fel_is_on_with_the_visible_laser', f.get('on') is True and f.get('mode') == 'Visible', f)
    g.check('the_beam_finds_a_valid_silex_and_goes_on', f.get('missing_valid_silex') is False and f.get('distance') == 24, f)
    s = silex(g, s_core)
    g.check('the_tanks_fill_the_silex', s.get('tank', {}).get('mb', 0) + s.get('fill', 0) >= 15_000 and slot_items(s).get(3, ('', 0))[0] == 'ntm:fluid_tank_empty', [s.get('tank'), slot_items(s)])
    g.ticks(260)
    s = silex(g, s_core)
    g.evidence['silex_after_290'] = s
    queue = [slot_items(s).get(i) for i in range(5, 11)]
    nuggets = {k: n for k, n in (q for q in queue if q)}
    # Both ingots load, 900 each, 21 ticks apart (the loader's delay), the 16 tanks having filled the tank first; each run
    # takes 100 of the fill.
    runs = (1800 - s.get('fill', 0)) // 100
    nuggets = nuggets_of(s)
    g.check('the_ingots_load_900_of_fill_each', s.get('current') == 'ntm:ingot_uranium' and 0 not in slot_items(s)
            and s.get('fill', 0) % 100 == 0 and s.get('tank', {}).get('mb') == 16_000 - 1800, [s.get('current'), s.get('fill'), s.get('tank'), runs])
    g.check('the_visible_laser_runs_it_into_nuggets_by_the_index', runs >= 2 and sum(nuggets.values()) == runs and nuggets.get('ntm:nugget_u235') == 1
            and s.get('recipe_index') == index_after(runs), [queue, runs, s.get('recipe_index'), index_after(runs)])
    # DRX: eight times as fast, and the stone further on burns to digamma fire on digamma ash.
    stone = (core[0] + d[0] * STONE_AT, core[1] + 1, core[2] + d[2] * STONE_AT)
    open_fel(g, core)
    g.ctl('click-slot 301 shift')
    g.put('ntm:laser_crystal_digamma', 1, 1)
    g.close()
    away(g, core)
    # Only now: the visible laser would burn it to plain fire first.
    g.command(f'setblock {stone[0]} {stone[1]} {stone[2]} minecraft:stone')
    g.evidence['stone_before'] = [g.ctl(f'block {stone[0]} {stone[1] + dy} {stone[2]}').get('state') for dy in (-1, 0)]
    before = silex(g, s_core)
    g.ticks(40)
    after = silex(g, s_core)
    f = fel(g, core)
    g.evidence['drx'] = {'fel': f, 'before': {k: before.get(k) for k in ('progress', 'recipe_index', 'fill')}, 'after': {k: after.get(k) for k in ('progress', 'recipe_index', 'fill')}}
    runs = (before.get('fill', 0) - after.get('fill', 0)) // 100
    # 40 ticks at 8 a tick: 3 runs (2 at 1 a tick would take 200).
    g.check('the_drx_laser_runs_the_silex_eight_times_as_fast', f.get('mode') == 'Drx' and runs >= 3, [runs, before.get('fill'), after.get('fill')])
    g.evidence['drx_cells'] = {f'{dz}:{dy}': g.ctl(f'block {stone[0]} {stone[1] + dy} {stone[2] + dz}').get('state') for dz in (-1, 0, 1) for dy in (-1, 0, 1)}
    g.check('the_drx_beam_leaves_digamma_fire_on_digamma_ash',
            is_(g, stone, 'ntm:fire_digamma') and is_(g, (stone[0], stone[1] - 1, stone[2]), 'ntm:ash_digamma'), stone)
    # A survival player in the beam, between the FEL and the SILEX: 0.1 a tick.
    import native_digamma as D
    g.command('gamemode survival @s')
    g.ticks(220)
    spot = (core[0] + d[0] * 4, core[1], core[2] + d[2] * 4)
    # The ticks the player stood in the beam, by the server's clock: the dose is 0.1 for each (one tick either way for
    # the commands that put it there and take it away).
    g.command(f'tp @s {spot[0] + .5} {Y} {spot[2] + .5}')
    entered = gametime(g)
    g.ticks(10)
    away(g, core)
    left = gametime(g)
    g.ticks(2)
    dose, _, raw = D.diagnostic(g)
    g.evidence['dose'] = {'dose': dose, 'entered': entered, 'left': left}
    g.check('the_drx_beam_doses_a_survival_player', dose is not None and dose > 0 and abs(dose - 0.1 * (left - entered)) <= 0.2
            and D.consistent(dose, D.modifier(g)), [dose, left - entered, raw])
    g.command('gamemode creative @s')
    g.command('effect clear @s')
    # Back to the visible laser for the save, and an ingot more for it to run on (the DRX ran the fill out).
    open_fel(g, core)
    g.ctl('click-slot 301 shift')
    g.put('ntm:laser_crystal_bismuth', 1, 1)
    g.close()
    open_silex(g, s_core)
    g.put('ntm:ingot_uranium', 1, 0)
    g.close()
    away(g, core)
    g.ticks(30)


def capture(g):
    core = tuple(json.loads((g.folder / 'native-fel-setup.json').read_text())['fel'])
    s_core = tuple(json.loads((g.folder / 'native-fel-setup.json').read_text())['silex'])
    g.ticks(20)
    s, f = silex(g, s_core), fel(g, core)
    keep = {'fill': s.get('fill'), 'current': s.get('current'), 'recipe_index': s.get('recipe_index'), 'slots': slot_items(s), 'tank': s.get('tank'),
            'on': f.get('on'), 'fel': list(core), 'silex': list(s_core)}
    g.check('the_silex_is_running_before_the_save', s.get('current') is not None and f.get('on') is True, keep)
    # The crystal out: with no laser the SILEX stands still (its progress back to 0), so the reopened world must hold this
    # state whatever ticks it runs before the check.
    open_fel(g, core)
    g.ctl('click-slot 301 shift')
    g.close()
    away(g, core)
    g.ticks(4)
    s = silex(g, s_core)
    keep.update(fill=s.get('fill'), recipe_index=s.get('recipe_index'), slots=slot_items(s), progress=s.get('progress'), tank=s.get('tank'))
    (g.folder / SAVED).write_text(json.dumps(keep))
    g.ctl('save')


def check(g):
    saved = json.loads((g.folder / SAVED).read_text())
    core, s_core = tuple(saved['fel']), tuple(saved['silex'])
    g.ticks(20)
    s, f = silex(g, s_core), fel(g, core)
    got = {'fill': s.get('fill'), 'current': s.get('current'), 'recipe_index': s.get('recipe_index'), 'slots': {str(k): list(v) for k, v in slot_items(s).items()},
           'progress': s.get('progress'), 'tank': s.get('tank'), 'on': f.get('on')}
    want = {'fill': saved['fill'], 'current': saved['current'], 'recipe_index': saved['recipe_index'], 'slots': {k: list(v) for k, v in saved['slots'].items()},
            'progress': saved['progress'], 'tank': saved['tank'], 'on': saved['on']}
    g.check('the_silex_and_the_fel_are_as_saved', got == want, [want, got])
    open_fel(g, core)
    g.put('ntm:laser_crystal_bismuth', 1, 1)
    g.close()
    away(g, core)
    g.ticks(240)
    s = silex(g, s_core)
    g.check('the_silex_goes_on_running', s.get('fill', 0) < saved['fill'], [saved['fill'], s.get('fill')])
    # A SILEX across the beam further on (`rotationIsValid` false): broken, its items spilled, a SILEX item dropped.
    facing = json.loads((g.folder / 'native-fel-setup.json').read_text())['facing']
    d = FACING[facing]
    probe = (X + 5, Y, Z + 6)
    place_across(g, 'ntm:machine_silex', probe)
    probe_core = core_of(g, probe, '"recipe_index"')
    offset = [probe_core[i] - probe[i] for i in range(3)]
    clear_box(g, probe_core)
    g.command('kill @e[type=minecraft:item]')
    target = (core[0] + d[0] * 14, core[1], core[2] + d[2] * 14)
    open_fel(g, core)
    g.ctl('click-slot 301 shift')
    g.close()
    place_across(g, 'ntm:machine_silex', (target[0] - offset[0], target[1] - offset[1], target[2] - offset[2]))
    across = core_of(g, target, '"recipe_index"')
    across_facing = facing_of(g, across)
    open_silex(g, across)
    g.put('ntm:ingot_uranium', 1, 0)
    g.close()
    away(g, core)
    open_fel(g, core)
    g.put('ntm:laser_crystal_bismuth', 1, 1)
    g.close()
    away(g, core)
    g.ticks(20)
    gone = not any(is_(g, (across[0] + dx, across[1] + dy, across[2] + dz), b) for dx in (-1, 0, 1) for dy in (0, 1, 2) for dz in (-1, 0, 1)
                   for b in ('ntm:machine_silex', 'ntm:dummy_machine'))
    # Exactly one SILEX and the one ingot it held: no copy, no loss.
    silex_drops, ingot_drops = g.dropped('ntm:machine_silex'), g.dropped('ntm:ingot_uranium')
    g.check('a_silex_across_the_beam_breaks_and_drops', tuple(across) == tuple(target) and FACING[across_facing][2] == 0 and gone
            and silex_drops == 1 and ingot_drops == 1, [across, target, across_facing, gone, silex_drops, ingot_drops])
    g.check('the_silex_along_it_is_untouched', bool(silex(g, s_core)) and is_(g, s_core, 'ntm:machine_silex'), s_core)


if __name__ == '__main__':
    g = Game(sys.argv[1], 'fel-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

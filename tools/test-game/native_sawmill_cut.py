#!/usr/bin/env python3
"""The sawmill's cut scenarios (Onda 2, D3 and the sawmill, step 4), on a disposable world; the runner,
run-sawmill-proof.py, owns the process and the cut. NTM_SAW_SCENARIO picks one:

  overspeed  a sawmill with its blade on a firebox (cold in the base), its outputs in its slots (6 planks, 1 sawdust: a
             log sawn, not taken; a log in its input would be sawn at that heat long before the blade breaks); the act
             feeds rocket fuel (x5, 500 TU a tick) and the blade breaks: the blast, then one `ntm:sawblade` one up in
             front of the core, in the core's chunk; the outputs stay; then the save the runner cuts. Control: a
             barrel at the core holds the blade and the same outputs; the act takes the blade out and puts it as an
             item where the mod's lies (`summon item`)
  border     the same with the core at z=16 facing north: the blade lands in the chunk north of the core's (R2)
  break      a sawmill without its blade (its item's `ntm:no_blade`) holding a log, 6 planks and 1 sawdust (put in by
             command: no heat saws them) and a chest holding one blade; the act breaks the sawmill as a player in
             survival, picks up what it drops (its item and its slots) and places it again: it has no blade (R1, D3).
             Control: a barrel with the same stacks, broken, picked up and placed again
  hand       a sawmill that sawed a log in the base (6 planks and its sawdust out) and one without its blade; in hand a
             blade and 2 logs; the act takes the outputs, puts a log in and the blade back, each by hand (R5). Control:
             barrels, the same stacks moved by hand (`item replace`)
  hopper     step 3c: a sawmill that sawed a log fed by the hopper above it (6 planks and its sawdust out, a log left in
             that hopper, blocked: the three slots must be empty); the act puts a hopper below it into a chest: the
             outputs leave, then the last log goes in and stays. Control: a barrel with the same stacks under the same
             hoppers (vanilla's hoppers move the log too: the totals are compared)

The atoms counted (`disk_count.py` by the same rule): blades loose, in a player, in a chest, installed (`has_blade`)
or in a sawmill item without `ntm:no_blade`; and, but for `border`, the planks, logs and sawdust. A sawmill's
slots are its block entity's since step 3c (its model's before: read as a fallback).
"""
import os
import re
import sys
import time
from native_game import Game
from native_combustion import start_area, machine_state
from native_recipes import core_of

SCENARIO = os.environ.get('NTM_SAW_SCENARIO', 'overspeed')
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
SAWMILL, BLADE = 'ntm:machine_sawmill', 'ntm:sawblade'
VARIANT = SAWMILL + '[minecraft:custom_data={"ntm:no_blade":1b}]'
WOOD = ('minecraft:oak_planks', 'minecraft:oak_log', 'ntm:powder_sawdust')
# Where the sawmill's core stands, by scenario: x=47 in chunk 2 (x=40 well inside it: the blade appears in the air one
# up in front, with a dropped item's random motion, and must stay in the core's chunk for `overspeed`); z=8 in chunk 0,
# z=16 in chunk 1.
# `border` at x=40 too: at x=47 the blade could drift east into chunk 3 as well, and the cut then wrote another chunk's
# entities than the control's (`same_cut=false` with the same counts, the review's rerun).
CORE = {'overspeed': (40, 101, 8), 'border': (40, 101, 16), 'break': (47, 101, 8), 'hand': (47, 101, 8),
        'hopper': (40, 101, 8)}[SCENARIO]
OTHER = (55, 101, 8)
CHEST = (34, 101, 8)
# `hopper`: the hopper above the core, the one below it, the chest it fills; all in the core's chunk (2, 0).
ABOVE, BELOW, OUT = (CORE[0], CORE[1] + 1, CORE[2]), (CORE[0], CORE[1] - 1, CORE[2]), (CORE[0] + 1, CORE[1] - 1, CORE[2])


def front(core):
    """One up in front of a north-facing core: where the blade lies."""
    return (core[0], core[1] + 1, core[2] - 1)


def items_in(g, p):
    text = ' '.join(g.command('data get block %d %d %d Items' % p).get('chat', []))
    return re.findall(r'count: ?(\d+)[^{}]*?id: ?"([a-z_:]+)"', text)


def held_in(g, p, item):
    return sum(int(n) for n, i in items_in(g, p) if i == item)


def has_blade(g, core):
    found = re.search(r'"has_blade":(true|false)', machine_state(g, core))
    return bool(found) and found.group(1) == 'true'


def model_slots(g, core):
    """The sawmill's slots: its block entity's (step 3c), else its model's (a binary from before)."""
    return [(i, n) for n, i in items_in(g, core)] or re.findall(r'"id":"([a-z_:]+)","count":(\d+)', machine_state(g, core))


def inventory(g):
    """Each item the player holds, by id, with sawmill items without their blade apart."""
    out = {}
    text = ' '.join(g.command('data get entity @s Inventory').get('chat', []))
    for entry in re.findall(r'\{[^{}]*(?:\{[^{}]*\{[^{}]*\}[^{}]*\}[^{}]*)?\}', text):
        item = re.search(r'id: ?"([a-z_:]+)"', entry)
        count = re.search(r'count: ?(\d+)', entry)
        if item and count:
            key = item.group(1) + ('#no_blade' if 'ntm:no_blade' in entry else '')
            out[key] = out.get(key, 0) + int(count.group(1))
    return out


def loose(g):
    out = {}
    for e in g.ctl('entities 60')['entities']:
        if e.get('kind') == 'item':
            out[e['item']['item']] = out.get(e['item']['item'], 0) + e['item']['count']
    return out


def count(g):
    """The atoms in the world now, by the oracle's rule."""
    inv, ground = inventory(g), loose(g)
    blades = inv.get(BLADE, 0) + inv.get(SAWMILL, 0) + ground.get(BLADE, 0) + held_in(g, CHEST, BLADE)
    machines = []
    for core in (CORE, OTHER):
        if VANILLA:
            blades += held_in(g, core, BLADE)
        elif g.ctl('block %d %d %d' % core).get('state', '').startswith(SAWMILL):
            machines.append({'core': core, 'has_blade': has_blade(g, core), 'slots': model_slots(g, core)})
            blades += int(has_blade(g, core))
    out = {'blades': blades, 'inventory': inv, 'ground': ground, 'machines': machines}
    if SCENARIO != 'border':
        wood = {w: inv.get(w, 0) + ground.get(w, 0) for w in WOOD}
        if SCENARIO == 'hopper':
            for p in (ABOVE, BELOW, OUT):
                for w in WOOD:
                    wood[w] += held_in(g, p, w)
        for core in (CORE, OTHER):
            if VANILLA:
                for w in WOOD:
                    wood[w] += held_in(g, core, w)
            for m in machines:
                if tuple(m['core']) == core:
                    for i, n in m['slots']:
                        if i in wood:
                            wood[i] += int(n)
        out['wood'] = wood
    # `hopper` moves wood only: its atoms are the wood's, as its disk count (the installed blade is not one of them).
    out['total'] = (0 if SCENARIO == 'hopper' else blades) + sum((out.get('wood') or {}).values())
    return out


def place(g, item, spot):
    x, y, z = spot
    g.command('gamemode creative @s')
    g.command('clear @s')
    g.command(f'give @s {item} 1')
    g.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(1)
    g.ctl(f'place {x} {y} {z} --item {SAWMILL}')
    time.sleep(1)
    # Creative keeps the stack placed: it is not part of the world being counted.
    g.command('clear @s')
    return core_of(g, SAWMILL, x, y, z, reach=(3, 3))


def click(g, core, item=None):
    """A player south of the sawmill (or of the control's barrel) clicks the first cell of it it sees."""
    g.command('gamemode survival @s')
    if item:
        g.ctl(f'select {item}')
    g.command(f'tp @s {core[0] + .5} {core[1]} {core[2] + 4.5}')
    time.sleep(.5)
    for dx, dy, dz in [(0, 1, 1), (0, 0, 1), (-1, 1, 1), (1, 1, 1), (0, 1, 0), (0, 0, 0)]:
        try:
            g.ctl('interact %d %d %d south' % (core[0] + dx, core[1] + dy, core[2] + dz))
            time.sleep(1)
            return
        except RuntimeError:
            continue
    raise AssertionError('no cell seen from the south')


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    g.command('gamemode creative @s')
    start_area(g, 36, 101, CORE[2], 28)
    if CORE[2] != 8:
        start_area(g, 36, 101, 8, 28)
    g.command('clear @s')
    g.command('setblock %d %d %d minecraft:chest' % CHEST)
    if SCENARIO in ('overspeed', 'border'):
        if VANILLA:
            g.command('setblock %d %d %d minecraft:barrel' % CORE)
            g.command('item replace block %d %d %d container.0 with %s 1' % (*CORE, BLADE))
        else:
            core = place(g, SAWMILL, CORE)
            g.check('core_where_planned', tuple(core) == CORE, core)
            g.command('setblock %d %d %d ntm:heater_firebox[facing=south,honey_level=0]' % (CORE[0], CORE[1] - 1, CORE[2]))
            g.command('setblock %d %d %d minecraft:hopper[facing=west]' % (CORE[0] + 1, CORE[1] - 1, CORE[2]))
        if SCENARIO == 'overspeed':
            # Its outputs, a log sawn and not taken (the barrel's slots 1 and 2 alike).
            g.command('item replace block %d %d %d container.1 with minecraft:oak_planks 6' % CORE)
            g.command('item replace block %d %d %d container.2 with ntm:powder_sawdust 1' % CORE)
            time.sleep(1)
            g.check('outputs_in', held_in(g, CORE, 'minecraft:oak_planks') == 6 and held_in(g, CORE, 'ntm:powder_sawdust') == 1,
                    items_in(g, CORE))
    elif SCENARIO == 'break':
        g.command('item replace block %d %d %d container.0 with %s 1' % (*CHEST, BLADE))
        if VANILLA:
            g.command('setblock %d %d %d minecraft:barrel' % CORE)
        else:
            core = place(g, VARIANT, CORE)
            g.check('core_where_planned', tuple(core) == CORE, core)
        # A log, 6 planks and 1 sawdust in its slots, by command: without its blade (and cold) nothing saws them.
        g.command('item replace block %d %d %d container.0 with minecraft:oak_log 1' % CORE)
        g.command('item replace block %d %d %d container.1 with minecraft:oak_planks 6' % CORE)
        g.command('item replace block %d %d %d container.2 with ntm:powder_sawdust 1' % CORE)
        time.sleep(1)
        g.check('slots_in', [held_in(g, CORE, w) for w in WOOD] == [6, 1, 1], items_in(g, CORE))
    elif SCENARIO == 'hand':
        if VANILLA:
            g.command('setblock %d %d %d minecraft:barrel' % CORE)
            g.command('setblock %d %d %d minecraft:barrel' % OTHER)
            g.command('item replace block %d %d %d container.1 with minecraft:oak_planks 6' % CORE)
            g.command('item replace block %d %d %d container.2 with ntm:powder_sawdust 1' % CORE)
            # The mod's first sawmill has its blade installed.
            g.command('item replace block %d %d %d container.3 with %s 1' % (*CORE, BLADE))
        else:
            core = place(g, SAWMILL, CORE)
            g.check('core_where_planned', tuple(core) == CORE, core)
            other = place(g, VARIANT, OTHER)
            g.check('other_where_planned', tuple(other) == OTHER, other)
            firebox = (CORE[0], CORE[1] - 1, CORE[2])
            g.command('setblock %d %d %d ntm:heater_firebox[facing=south,honey_level=0]' % firebox)
            g.command('setblock %d %d %d minecraft:hopper[facing=west]' % (CORE[0] + 1, CORE[1] - 1, CORE[2]))
            g.command('item replace block %d %d %d container.0 with minecraft:coal 1' % (CORE[0] + 1, CORE[1] - 1, CORE[2]))
            time.sleep(8)
            g.command('clear @s')
            g.command('give @s minecraft:oak_log 1')
            click(g, CORE, 'minecraft:oak_log')
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline and not any(i == 'minecraft:oak_planks' for i, _ in model_slots(g, CORE)):
                time.sleep(1)
            # Cold from here: nothing changes on its own after the base.
            g.command('setblock %d %d %d minecraft:stone' % firebox)
            g.command('setblock %d %d %d minecraft:stone' % (CORE[0] + 1, CORE[1] - 1, CORE[2]))
            g.check('sawn_in_the_base', ('minecraft:oak_planks', '6') in model_slots(g, CORE), model_slots(g, CORE))
        g.command('gamemode creative @s')
        g.command('clear @s')
        g.command(f'give @s {BLADE} 1')
        g.command('give @s minecraft:oak_log 2')
    elif SCENARIO == 'hopper':
        g.command('setblock %d %d %d minecraft:chest' % OUT)
        if VANILLA:
            g.command('setblock %d %d %d minecraft:barrel' % CORE)
            g.command('item replace block %d %d %d container.1 with minecraft:oak_planks 6' % CORE)
            g.command('item replace block %d %d %d container.2 with ntm:powder_sawdust 1' % CORE)
            g.command('setblock %d %d %d minecraft:hopper[facing=down]' % ABOVE)
            g.command('item replace block %d %d %d container.0 with minecraft:oak_log 1' % ABOVE)
            # The barrel takes the log at once (vanilla has no rule): the base then holds it in the barrel.
        else:
            core = place(g, SAWMILL, CORE)
            g.check('core_where_planned', tuple(core) == CORE, core)
            feeder = (CORE[0] - 1, CORE[1] - 1, CORE[2])
            g.command('setblock %d %d %d ntm:heater_firebox[facing=south,honey_level=0]' % BELOW)
            g.command('setblock %d %d %d minecraft:hopper[facing=east]' % feeder)
            g.command('item replace block %d %d %d container.0 with minecraft:coal 1' % feeder)
            time.sleep(8)
            g.command('setblock %d %d %d minecraft:hopper[facing=down]' % ABOVE)
            g.command('item replace block %d %d %d container.0 with minecraft:oak_log 2' % ABOVE)
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline and not any(i == 'minecraft:oak_planks' for i, _ in model_slots(g, CORE)):
                time.sleep(1)
            # Cold from here; the last log waits above, blocked by the outputs.
            g.command('setblock %d %d %d minecraft:stone' % BELOW)
            g.command('setblock %d %d %d minecraft:stone' % feeder)
            time.sleep(3)
            g.check('sawn_in_the_base', ('minecraft:oak_planks', '6') in model_slots(g, CORE), model_slots(g, CORE))
            g.check('the_log_waits_above', held_in(g, ABOVE, 'minecraft:oak_log') == 1, items_in(g, ABOVE))
        g.command('clear @s')
    g.command('kill @e[type=minecraft:item]')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 4.5}')
    time.sleep(2)
    base = count(g)
    g.evidence['base'] = base
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 4.5}')
    time.sleep(2)
    before = count(g)
    g.evidence['before'] = before
    if SCENARIO in ('overspeed', 'border'):
        b = front(CORE)
        if VANILLA:
            # Where the mod's blade came to lie in the same point's play (its runner passes it on), else in front: a
            # dropped item's random motion may carry it over a chunk's edge, and the cut must write the same chunks.
            at = os.environ.get('NTM_SAW_BLADE_AT')
            x, y, z = [float(v) for v in at.split(',')] if at else (b[0] + .5, b[1] + .5, b[2] + .5)
            g.evidence['blade_summoned_at'] = [x, y, z]
            g.command('item replace block %d %d %d container.0 with minecraft:air' % CORE)
            g.command(f'summon minecraft:item {x} {y} {z} {{Item:{{id:"{BLADE}",count:1}},Motion:[0d,0d,0d]}}')
        else:
            hopper = (CORE[0] + 1, CORE[1] - 1, CORE[2])
            g.command('item replace block %d %d %d container.0 with ntm:rocket_fuel 4' % hopper)
            # The hopper gone once the fuel is in the firebox: a hopper sucks the item entities near it, and the blade
            # the failure throws must lie where it falls (the act's own count holds it).
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline and held_in(g, hopper, 'ntm:rocket_fuel'):
                time.sleep(.5)
            g.check('the_fuel_went_in', not held_in(g, hopper, 'ntm:rocket_fuel'), items_in(g, hopper))
            g.command('setblock %d %d %d minecraft:stone' % hopper)
            deadline = time.monotonic() + 120
            while time.monotonic() < deadline and has_blade(g, CORE):
                time.sleep(.5)
            g.check('the_blade_breaks', not has_blade(g, CORE), machine_state(g, CORE)[:300])
            # Cold again, before the save: the cut takes the failure, not a fuel still burning.
            g.command('setblock %d %d %d minecraft:stone' % (CORE[0], CORE[1] - 1, CORE[2]))
            g.command('setblock %d %d %d minecraft:stone' % (CORE[0] + 1, CORE[1] - 1, CORE[2]))
        time.sleep(3)
        if not VANILLA:
            lying = [e['pos'] for e in g.ctl('entities 30')['entities'] if e.get('kind') == 'item' and e['item']['item'] == BLADE]
            g.evidence['blade_at'] = lying[0] if lying else None
    elif SCENARIO == 'break':
        g.command('clear @s')
        g.command('gamemode survival @s')
        g.command('give @s minecraft:diamond_pickaxe 1')
        g.ctl('select minecraft:diamond_pickaxe')
        broken = None
        for dx, dy, dz in [(0, 0, 1), (0, 1, 1), (-1, 0, 1), (1, 0, 1), (0, 1, 0), (0, 0, 0)]:
            cell = (CORE[0] + dx, CORE[1] + dy, CORE[2] + dz)
            try:
                g.ctl('break %d %d %d --timeout 30' % cell)
                broken = cell
                break
            except RuntimeError:
                continue
        g.check('broken', broken is not None, broken)
        time.sleep(1.5)
        drops = [e['pos'] for e in g.ctl('entities 20')['entities'] if e.get('kind') == 'item']
        g.check('it_dropped_its_item', bool(drops), drops)
        # Each of them picked up: a full block entity's spill lies around, its own item maybe out of one reach.
        for d in drops:
            g.command(f'tp @s {d[0]} {d[1]} {d[2]}')
            time.sleep(1)
        time.sleep(1)
        g.evidence['picked'] = inventory(g)
        g.command('gamemode creative @s')
        g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 3.5}')
        time.sleep(1)
        g.ctl('place %d %d %d --item %s' % (*CORE, 'minecraft:barrel' if VANILLA else SAWMILL))
        time.sleep(1.5)
        if not VANILLA:
            g.check('placed_again_without_its_blade', not has_blade(g, CORE), machine_state(g, CORE)[:300])
    elif SCENARIO == 'hand':
        if VANILLA:
            # The barrels' stacks moved by hand, as the mod's transfers: the outputs to the player, a log in, the blade.
            g.command('item replace block %d %d %d container.1 with minecraft:air' % CORE)
            g.command('item replace block %d %d %d container.2 with minecraft:air' % CORE)
            g.command('give @s minecraft:oak_planks 6')
            g.command('give @s ntm:powder_sawdust 1')
            g.command('clear @s minecraft:oak_log 1')
            g.command('item replace block %d %d %d container.0 with minecraft:oak_log 1' % CORE)
            g.command(f'clear @s {BLADE} 1')
            g.command('item replace block %d %d %d container.0 with %s 1' % (*OTHER, BLADE))
        else:
            click(g, CORE)
            click(g, CORE, 'minecraft:oak_log')
            click(g, OTHER, BLADE)
            g.check('blade_back', has_blade(g, OTHER), machine_state(g, OTHER)[:300])
        g.command('gamemode creative @s')
    elif SCENARIO == 'hopper':
        # A hopper below into the chest: the outputs leave by `canExtractItem`, then the log above goes in and stays.
        g.command('setblock %d %d %d minecraft:hopper[facing=east]' % BELOW)
        time.sleep(10)
        if not VANILLA:
            slots = model_slots(g, CORE)
            g.evidence['slots_after'] = slots
            g.check('the_input_stays', slots == [('minecraft:oak_log', '1')] and held_in(g, OUT, 'minecraft:oak_log') == 0,
                    [slots, items_in(g, OUT), items_in(g, BELOW)])
    # Both chunks change a block too, in both worlds (the cut then has their records).
    for z in (CORE[2] - 6, CORE[2] + 6):
        g.command(f'setblock 37 100 {z} minecraft:cobblestone')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 4.5}')
    time.sleep(1.5)
    after = count(g)
    g.evidence['after_act'] = after
    g.check('nothing_lost_or_made_in_the_act', after['total'] == before['total'], [before, after])
    g.save()
    if NO_SAVE:
        return
    if PAUSE_SAVE:
        g.evidence['pause_save'] = True
        g.save()
        try:
            g.ctl('pause')
        except Exception:
            return
        deadline = time.monotonic() + 270
        while time.monotonic() < deadline:
            try:
                g.ctl('status')
            except Exception:
                return
            time.sleep(.5)
        return
    try:
        g.ctl('save')
        g.evidence['survived_save'] = True
    except Exception as error:
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 4.5}')
    time.sleep(6)
    now = count(g)
    # Counted only: the verdict is the runner's, against the base and the vanilla control (a check here compared the
    # count with itself, the review's P3).
    g.evidence['counted' + ('_again' if again else '')] = now
    g.ctl('save')


def drain(g):
    g.ctl('wait-ready 120')
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'sawmill-cut-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)

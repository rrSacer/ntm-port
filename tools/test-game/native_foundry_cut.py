#!/usr/bin/env python3
"""The foundry's cut scenarios (Onda 2, D4 and the strand caster), on a disposable world; the runner,
run-foundry-proof.py, owns the process and the cut. NTM_FOUNDRY_SCENARIO picks one:

  scrap   a crucible on a hot firebox that melted an iron ingot and 3 nuggets (96 quanta, a pool no ingot count
          holds); the act breaks it as a player in survival, picks up what it drops (one scrap with all 96:
          `ntm:scraps__iron` with `ntm_amount:96`, D4) and puts that scrap into a crucible slot again, by the screen.
          Whatever time passes after, the 96 are the crucible's slot or its melted pool. Control: a barrel with the
          ingot and the 3 nuggets, broken, picked up and put back by the screen
  caster  a strand caster whose funnel cell a crucible's stream falls on (F1): 2 ingots (144) melt, pour and wait in
          its pool (no water: nothing is cast); the act breaks the caster as a player in survival and picks up its
          drops: its mold and one scrap with all 144. Control: a barrel with the 2 ingots and the mold, broken and
          picked up

The atoms counted are the quanta of iron (`foundry_quanta.py` on disk, by the same rule): ingots 72, nuggets 8, a scrap
its `ntm_amount` (72 without it), and the molten metal of the models (a crucible's stacks, a pool).
"""
import json
import os
import re
import sys
import time
from native_game import Game
from native_combustion import start_area, machine_state

SCENARIO = os.environ.get('NTM_FOUNDRY_SCENARIO', 'scrap')
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
CRUCIBLE, FIREBOX, CASTER = 'ntm:machine_crucible', 'ntm:heater_firebox', 'ntm:machine_strand_caster'
SCRAP = 'ntm:scraps__iron'
WEIGHT = {'minecraft:iron_ingot': 72, 'minecraft:iron_nugget': 8, 'minecraft:iron_block': 648}
# `scrap`: the crucible's core (or the control's barrel); `caster`: where the caster is placed (its core is searched near
# it) and the player stands; both in chunk (2, 0) and within 6 blocks of x=40, z=8.
CORE = (40, 101, 8)
OTHER = CORE


def quanta_of(item, tail):
    """The weight of a stack (id, the text of its entry): a scrap its `ntm_amount`."""
    if item == SCRAP:
        found = re.search(r'ntm_amount: ?(\d+)', tail)
        return int(found.group(1)) if found else 72
    return WEIGHT.get(item, 0)


def entries(text):
    """Each `{...}` stack of a `data get ... Items|Inventory` reply: (id, count, the entry's text)."""
    out = []
    for entry in re.findall(r'\{[^{}]*(?:\{[^{}]*(?:\{[^{}]*\}[^{}]*)?\}[^{}]*)?\}', text):
        item, count = re.search(r'id: ?"([a-z_:0-9]+)"', entry), re.search(r'count: ?(\d+)', entry)
        if item and count:
            out.append((item.group(1), int(count.group(1)), entry))
    return out


def stack_quanta(text):
    return sum(quanta_of(i, e) * n for i, n, e in entries(text))


def block_items(g, p):
    return ' '.join(g.command('data get block %d %d %d Items' % tuple(p)).get('chat', []))


def inventory_text(g):
    return ' '.join(g.command('data get entity @s Inventory').get('chat', []))


def model(g, core):
    """The machine's model (the `ntm` string of its block entity) as a dict, or None."""
    text = machine_state(g, core)
    found = re.search(r'\{.*\}', text)
    try:
        return json.loads(found.group(0)) if found else None
    except ValueError:
        return None


def molten(m):
    """The molten iron of a model: a pool (`mat`, `amount`) or a crucible's stacks."""
    if not isinstance(m, dict):
        return 0
    total = int(m['amount']) if m.get('mat') == 'iron' and isinstance(m.get('amount'), int) else 0
    for key in ('recipe_stack', 'waste_stack'):
        total += sum(int(q) for mat, q in m.get(key, []) or [] if mat == 'iron')
    return total


def loose(g):
    out = 0
    for e in g.ctl('entities 60')['entities']:
        if e.get('kind') == 'item':
            out += quanta_of(e['item']['item'], json.dumps(e['item'])) * e['item']['count']
    return out


def machines(g):
    """(the machines' molten quanta, the quanta of their slots) at the places the scenario uses."""
    mol, slots = 0, 0
    for p in places(g):
        if VANILLA:
            slots += stack_quanta(block_items(g, p))
        elif g.ctl('block %d %d %d' % tuple(p)).get('state', '').startswith(('ntm:machine_crucible', CASTER)):
            mol += molten(model(g, p))
            slots += stack_quanta(block_items(g, p))
    return mol, slots


def places(g):
    if SCENARIO != 'caster':
        return [CORE]
    return [caster_core(g)] + ([] if VANILLA else [tuple(placed(g)['crucible'])])


def placed(g):
    """Where the caster and its crucible are (`caster-core.json`, kept by the prepare), else the planned place."""
    path = g.folder / 'caster-core.json'
    return json.loads(path.read_text()) if path.exists() else {'core': list(OTHER), 'crucible': list(OTHER)}


def caster_core(g):
    return tuple(placed(g)['core'])


def count(g):
    mol, slots = machines(g)
    inv = stack_quanta(inventory_text(g))
    ground = loose(g)
    return {'molten': mol, 'slots': slots, 'inventory': inv, 'ground': ground, 'total': mol + slots + inv + ground}


def wait_for(predicate, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return True
        time.sleep(1)
    return False


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    g.command('gamemode creative @s')
    start_area(g, 36, 101, 8, 28)
    g.command('clear @s')
    if SCENARIO == 'scrap':
        if VANILLA:
            g.command('setblock %d %d %d minecraft:barrel' % CORE)
            g.command('item replace block %d %d %d container.1 with minecraft:iron_ingot 1' % CORE)
            g.command('item replace block %d %d %d container.2 with minecraft:iron_nugget 3' % CORE)
        else:
            g.command('setblock %d %d %d %s[facing=south,honey_level=0]' % (CORE[0], CORE[1] - 1, CORE[2], FIREBOX))
            g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*CORE, CRUCIBLE))
            time.sleep(2)
            g.command('item replace block %d %d %d container.0 with minecraft:coal 16' % (CORE[0], CORE[1] - 1, CORE[2]))
            g.command('item replace block %d %d %d container.1 with minecraft:iron_ingot 1' % CORE)
            g.command('item replace block %d %d %d container.2 with minecraft:iron_nugget 3' % CORE)
            ok = wait_for(lambda: molten(model(g, CORE)) == 96, 180)
            g.check('melted_96', ok, molten(model(g, CORE)))
            g.check('the_slots_are_empty', stack_quanta(block_items(g, CORE)) == 0, block_items(g, CORE))
            # Cold from here: nothing changes on its own after the base.
            g.command('setblock %d %d %d minecraft:stone' % (CORE[0], CORE[1] - 1, CORE[2]))
    elif SCENARIO == 'caster':
        place_caster(g)
    g.command('kill @e[type=minecraft:item]')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 4.5}')
    time.sleep(2)
    base = count(g)
    g.evidence['base'] = base
    g.ctl('save')


def place_caster(g):
    """The caster, with its pour cell under the crucible's waste stream, 144 quanta in its pool."""
    if VANILLA:
        g.command('setblock %d %d %d minecraft:barrel' % OTHER)
        g.command('item replace block %d %d %d container.0 with ntm:mold__ingot 1' % OTHER)
        g.command('item replace block %d %d %d container.1 with minecraft:iron_ingot 2' % OTHER)
        return
    g.command('gamemode creative @s')
    g.command(f'give @s {CASTER} 1')
    g.command(f'tp @s {OTHER[0] + .5} {OTHER[1]} {OTHER[2] + 3.5}')
    time.sleep(1)
    g.ctl(f'place {OTHER[0]} {OTHER[1]} {OTHER[2]} --item {CASTER}')
    time.sleep(1.5)
    g.command('clear @s')
    core = None
    for dx in range(-3, 4):
        for dz in range(-3, 4):
            p = (OTHER[0] + dx, OTHER[1], OTHER[2] + dz)
            if g.ctl('block %d %d %d' % p).get('state', '').startswith(CASTER + '['):
                core = core or p
    # The core is the cell that holds the model (the dummies have none).
    for dx in range(-3, 4):
        for dz in range(-3, 4):
            p = (OTHER[0] + dx, OTHER[1], OTHER[2] + dz)
            if g.ctl('block %d %d %d' % p).get('state', '').startswith(CASTER + '[') and model(g, p):
                core = p
    g.check('caster_placed', core is not None, core)
    state = g.ctl('block %d %d %d' % core)['state']
    facing = re.search(r'facing=(north|south|east|west)', state).group(1)
    d = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}[facing]
    g.evidence['caster'] = {'core': core, 'facing': facing}
    cell = (core[0], core[1] + 2, core[2])  # the cell over the core is one of the four funnel cells
    g.command('item replace block %d %d %d container.0 with ntm:mold__ingot 1' % core)
    # The crucible two blocks south of the cell facing north: its waste stream starts two blocks north of it.
    crucible = (cell[0], cell[1] + 1, cell[2] - 2)
    fire = (crucible[0], crucible[1] - 1, crucible[2])
    g.command('setblock %d %d %d %s[facing=south,honey_level=0]' % (*fire, FIREBOX))
    g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*crucible, CRUCIBLE))
    g.evidence['crucible'] = crucible
    (g.folder / 'caster-core.json').write_text(json.dumps({'core': core, 'crucible': crucible}))
    time.sleep(2)
    g.command('item replace block %d %d %d container.0 with minecraft:coal 16' % fire)
    g.command('item replace block %d %d %d container.1 with minecraft:iron_ingot 2' % crucible)
    ok = wait_for(lambda: molten(model(g, core)) == 144, 240)
    g.check('poured_144_into_the_caster', ok, [molten(model(g, core)), molten(model(g, crucible))])
    g.check('the_crucible_is_empty', molten(model(g, crucible)) == 0, molten(model(g, crucible)))
    # Cold from here.
    g.command('setblock %d %d %d minecraft:stone' % fire)
    g.command('setblock %d %d %d minecraft:stone' % crucible)


def break_it(g, cells, label):
    """A player in survival breaks the first of `cells` it can, as the sawmill's break scenario does."""
    g.command('clear @s')
    g.command('gamemode survival @s')
    g.command('give @s minecraft:diamond_pickaxe 1')
    g.ctl('select minecraft:diamond_pickaxe')
    broken = None
    for cell in cells:
        try:
            g.ctl('break %d %d %d --timeout 30' % cell)
            broken = cell
            break
        except RuntimeError:
            continue
    g.check('broken_' + label, broken is not None, broken)
    time.sleep(1.5)


def pick_up_all(g):
    drops = [e['pos'] for e in g.ctl('entities 20')['entities'] if e.get('kind') == 'item']
    g.check('it_dropped_something', bool(drops), drops)
    for d in drops:
        g.command(f'tp @s {d[0]} {d[1]} {d[2]}')
        time.sleep(1)
    time.sleep(1)


def put_back(g, item):
    """The item in the inventory into slot 1 of the machine at CORE, by its screen."""
    g.command('gamemode creative @s')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 3.5}')
    time.sleep(1)
    g.close()
    g.ctl(f'interact {CORE[0]} {CORE[1]} {CORE[2]} south')
    time.sleep(1)
    screen = g.ctl('screen')
    source = next((s['slot'] for s in screen.get('slots', []) if s['slot'] < 36 and s.get('item') == item), None)
    g.check('the_item_is_in_the_inventory', source is not None, [s for s in screen.get('slots', []) if s.get('item')])
    g.ctl(f'click-slot {source}')
    g.ctl('click-slot 301')
    time.sleep(.5)
    g.check('put_in_slot_1', not g.ctl('screen').get('carried'), g.ctl('screen'))
    g.close()


def act(g):
    g.ctl('wait-ready 120')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + 4.5}')
    time.sleep(2)
    before = count(g)
    g.evidence['before'] = before
    if SCENARIO == 'scrap':
        break_it(g, [CORE], 'crucible')
        pick_up_all(g)
        inv = inventory_text(g)
        g.evidence['picked'] = inv[:600]
        if VANILLA:
            g.command('gamemode creative @s')
            g.command('setblock %d %d %d minecraft:barrel' % CORE)
            time.sleep(1)
            g.command('item replace block %d %d %d container.1 with minecraft:iron_ingot 1' % CORE)
            g.command('item replace block %d %d %d container.2 with minecraft:iron_nugget 3' % CORE)
            g.command('clear @s minecraft:iron_ingot')
            g.command('clear @s minecraft:iron_nugget')
        else:
            g.check('one_exact_scrap', stack_quanta(inv) == 96 and 'ntm_amount:96' in inv.replace(' ', '').replace('"', ''), inv[:400])
            g.command('gamemode creative @s')
            g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*CORE, CRUCIBLE))
            time.sleep(2)
            put_back(g, SCRAP)
    elif SCENARIO == 'caster':
        core = caster_core(g)
        cells = [(core[0], core[1] + dy, core[2]) for dy in (0, 1, 2)]
        break_it(g, cells if not VANILLA else [OTHER], 'caster')
        pick_up_all(g)
        g.evidence['picked'] = inventory_text(g)[:600]
        g.command('gamemode creative @s')
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
    # Counted only: the verdict is the runner's, against the base and the vanilla control.
    g.evidence['counted' + ('_again' if again else '')] = now
    g.ctl('save')


def drain(g):
    g.ctl('wait-ready 120')
    g.ctl('save')


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], 'foundry-cut-' + mode)
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True), 'drain': drain}[mode](g)

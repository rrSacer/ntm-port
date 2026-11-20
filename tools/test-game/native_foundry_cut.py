#!/usr/bin/env python3
"""The foundry's cut scenarios (Onda 2, D4 and the strand caster), on a disposable world; the runner,
run-foundry-proof.py, owns the process and the cut. NTM_FOUNDRY_SCENARIO picks one:

  scrap   a crucible on a hot firebox that melted an gold ingot and 3 nuggets (96 quanta, a pool no ingot count
          holds); the act breaks it as a player in survival, picks up what it drops (one scrap with all 96:
          `ntm:scraps__gold` with `ntm_amount:96`, D4) and puts that scrap into a crucible slot again, by the screen.
          Whatever time passes after, the 96 are the crucible's slot or its melted pool. Control: a barrel with the
          ingot and the 3 nuggets, broken, picked up and put back by the screen
  excess  the caster's pool of 144 and, by a lighter mold put in by command (capacity 80), the excess of 64 that goes out as
          one exact scrap above the machine (F5: a critical effect, journaled with the model): picked up, the atoms are
          the pool's 80 and the scrap's 64. The cut `kv_committed` reopens the chunk with the model from the journal
          (the review's P1-1: the mold and the outputs of the block entity must stay). Control: a barrel with an ingot
          and a nugget and the player's 8 nuggets
  batch   the caster with water: a steel barrel at its first port, filled with a bucket of water through its screen and
          set to output; the crucible's 2 ingots (144) melt, pour and are cast by the caster (5 water a quantum): 2
          ingots in its outputs, none in the pool (F4 in game); the act breaks it and picks up what it drops. Control:
          as `caster`
  clicks  the foundry's blocks by hand: a crucible with 144 quanta (2 gold ingots) whose stream falls on a mold block with no
          mold (nothing pours); as a player in survival the act clicks the mold block with a mold in hand (it goes in, one of
          the stack), waits for the stream to fill it and the cast to cool (an ingot in its slot), clicks it with an empty hand
          (the ingot comes out) and clicks the crucible with a shovel (its 72 left come as one scrap). Control: a barrel with
          the 2 ingots, which the player takes by command
  hand    the caster by hand (`MachineStrandCaster.onBlockActivated`): the pool of 144 and, as a player in survival, the
          mold taken out by command (there is no screwdriver) and put back by a click with a mold in hand (one of the
          stack, into slot 0), then a click with a shovel: the whole pool as one exact scrap in the inventory. Control: a
          barrel with the 2 ingots, which the player takes by command
  slagtap-pour    a crucible's 144 quanta (2 gold ingots) melted into a channel under its waste stream, a stone pit three
          blocks deep behind the channel; the act puts the slag tap (facing south, fed from the channel) by command and goes on
          straight to the save, so the pour (the channel's loss, the slag block and its record, one commit) races the save and
          its cuts: wherever the process dies, the 144 are in the channel or in the slag block, never in both and never in
          neither. Control: a barrel with the 2 ingots whose ingots move by command to a barrel in the pit
  slagtap-border  the same with the channel on one side of a chunk border and the slag tap and the pit on the other (z 15 | 16): a
          pour would leave the channel's chunk, whose record is the only one the world writes atomically, so the tap pours nothing
          and the 144 stay in the channel at every cut (the metal is never moved across two chunks' records)
  slagtap-break   the pour done in the prepare; the act breaks the slag block as a player in survival and picks up what it drops:
          one scrap with all 144 (D4). Control: as `slagtap-pour`, the ingots taken by command
  caster  a strand caster whose funnel cell a crucible's stream falls on (F1): 2 ingots (144) melt, pour and wait in
          its pool (no water: nothing is cast); the act breaks the caster as a player in survival and picks up its
          drops: its mold and one scrap with all 144. Control: a barrel with the 2 ingots and the mold, broken and
          picked up

The atoms counted are the quanta of gold (`foundry_quanta.py` on disk, by the same rule): ingots 72, nuggets 8, a scrap
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
SCRAP = 'ntm:scraps__gold'
WEIGHT = {'minecraft:gold_ingot': 72, 'minecraft:gold_nugget': 8, 'minecraft:gold_block': 648}
# `scrap`: the crucible's core (or the control's barrel); `caster`: where the caster is placed (its core is searched near
# it) and the player stands; both in chunk (2, 0) and within 6 blocks of x=40, z=8.
SLAGTAP = SCENARIO.startswith('slagtap')
BORDER = SCENARIO == 'slagtap-border'
# The slag tap's chain stands one chunk south in the border variant: its channel at z 15, the tap and the slag at z 16.
CORE = (40, 101, 13 if BORDER else 8)
# The player stands north of the crucible in the slag tap's scenarios (the pit is south of it), south of it in the others.
PZ = -3.5 if SLAGTAP else 4.5
OTHER = CORE
# The slag tap's chain: the channel under the crucible's waste stream, the tap behind which it stands (facing south), the pit the
# pour lands in (stone at y 97, the slag block at y 98).
CHANNEL_AT = (CORE[0], CORE[1], CORE[2] + 2)
TAP_AT = (CORE[0], CORE[1], CORE[2] + 3)
SLAG_AT = (CORE[0], 98, CORE[2] + 3)
# `clicks`: the mold block under the crucible's waste stream (two blocks south of its core).
MOLD_AT = (CORE[0], CORE[1] - 1, CORE[2] + 2)


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


def server_block(g, p):
    """The block of the machine at `p` as the server's model says (`data get block ... ntm`: the block entity's), not as the
    client sees it: '' where there is no machine."""
    found = re.search(r'"block":"([a-z_:0-9]+)"', machine_state(g, p))
    return found.group(1) if found else ''


def block_items(g, p):
    return ' '.join(g.command('data get block %d %d %d Items' % tuple(p)).get('chat', []))


def inventory_text(g):
    return ' '.join(g.command('data get entity @s Inventory').get('chat', []))


def model(g, core):
    """The machine's model (the `ntm` string of its block entity) as a dict, or None."""
    text = machine_state(g, core)
    found = re.search(r'\{.*\}', text)
    try:
        whole = json.loads(found.group(0)) if found else None
    except ValueError:
        return None
    # The block entity's string: {generation, block, facing, data: the model}.
    return whole.get('data', whole) if isinstance(whole, dict) else None


def molten(m):
    """The molten gold of a model: a pool (`mat`, `amount`) or a crucible's stacks."""
    if not isinstance(m, dict):
        return 0
    total = int(m['amount']) if m.get('mat') == 'gold' and isinstance(m.get('amount'), int) else 0
    for key in ('recipe_stack', 'waste_stack'):
        total += sum(int(q) for mat, q in m.get(key, []) or [] if mat == 'gold')
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
        elif server_block(g, p) in ('ntm:machine_crucible', CASTER, 'ntm:foundry_mold', 'ntm:foundry_channel', 'ntm:slag'):
            mol += molten(model(g, p))
            slots += stack_quanta(block_items(g, p))
    return mol, slots


def places(g):
    if SLAGTAP:
        return [CORE, CHANNEL_AT, SLAG_AT]
    if SCENARIO == 'clicks':
        return [CORE, MOLD_AT]
    if SCENARIO not in ('caster', 'excess', 'batch', 'hand'):
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
            g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 1' % CORE)
            g.command('item replace block %d %d %d container.2 with minecraft:gold_nugget 3' % CORE)
        else:
            g.command('setblock %d %d %d %s[facing=south,honey_level=0]' % (CORE[0], CORE[1] - 1, CORE[2], FIREBOX))
            g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*CORE, CRUCIBLE))
            time.sleep(2)
            g.command('item replace block %d %d %d container.0 with minecraft:coal 16' % (CORE[0], CORE[1] - 1, CORE[2]))
            g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 1' % CORE)
            g.command('item replace block %d %d %d container.2 with minecraft:gold_nugget 3' % CORE)
            ok = wait_for(lambda: molten(model(g, CORE)) == 96, 180)
            g.check('melted_96', ok, molten(model(g, CORE)))
            g.check('the_slots_are_empty', stack_quanta(block_items(g, CORE)) == 0, block_items(g, CORE))
            # Cold from here: nothing changes on its own after the base.
            g.command('setblock %d %d %d minecraft:stone' % (CORE[0], CORE[1] - 1, CORE[2]))
    elif SCENARIO == 'clicks':
        if VANILLA:
            g.command('setblock %d %d %d minecraft:barrel' % CORE)
            g.command('setblock %d %d %d minecraft:barrel' % MOLD_AT)
            g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 2' % CORE)
        else:
            g.command('setblock %d %d %d %s[facing=south,honey_level=0]' % (CORE[0], CORE[1] - 1, CORE[2], FIREBOX))
            g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*CORE, CRUCIBLE))
            g.command('setblock %d %d %d ntm:foundry_mold[facing=north,honey_level=0]' % MOLD_AT)
            time.sleep(2)
            g.command('item replace block %d %d %d container.0 with minecraft:coal 16' % (CORE[0], CORE[1] - 1, CORE[2]))
            g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 2' % CORE)
            ok = wait_for(lambda: molten(model(g, CORE)) == 144, 240)
            g.check('melted_144', ok, molten(model(g, CORE)))
            g.check('nothing_pours_without_a_mold', molten(model(g, MOLD_AT)) == 0, model(g, MOLD_AT))
            g.command('item replace block %d %d %d container.0 with minecraft:air' % (CORE[0], CORE[1] - 1, CORE[2]))
            g.command('setblock %d %d %d minecraft:stone' % (CORE[0], CORE[1] - 1, CORE[2]))
    elif SLAGTAP:
        slagtap_world(g)
    elif SCENARIO in ('caster', 'excess', 'batch', 'hand'):
        place_caster(g)
    g.command('kill @e[type=minecraft:item]')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + PZ}')
    time.sleep(2)
    base = count(g)
    g.evidence['base'] = base
    g.ctl('save')


def slagtap_world(g):
    """The chain up to the tap: a crucible whose waste stream falls on a channel (the channel fills to its 144), cold after; a
    pit behind it, down to stone at y 97 (and, for the break, a corridor south for the player). In `slagtap-break` the tap is
    placed here and the slag is made; in the others the act places it."""
    pit_end = SLAG_AT[2] + (0 if BORDER else 3)
    g.command('fill %d %d %d %d %d %d minecraft:air' % (SLAG_AT[0], SLAG_AT[1], SLAG_AT[2], SLAG_AT[0], CORE[1] - 1, pit_end))
    g.command('fill %d %d %d %d %d %d minecraft:stone' % (SLAG_AT[0], SLAG_AT[1] - 1, SLAG_AT[2], SLAG_AT[0], SLAG_AT[1] - 1, pit_end))
    if VANILLA:
        g.command('setblock %d %d %d minecraft:barrel' % CHANNEL_AT)
        g.command('setblock %d %d %d minecraft:barrel' % SLAG_AT)
        g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 2' % CHANNEL_AT)
        return
    g.command('setblock %d %d %d %s[facing=south,honey_level=0]' % (CORE[0], CORE[1] - 1, CORE[2], FIREBOX))
    g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*CORE, CRUCIBLE))
    g.command('setblock %d %d %d ntm:foundry_channel[facing=north,honey_level=0]' % CHANNEL_AT)
    time.sleep(2)
    g.command('item replace block %d %d %d container.0 with minecraft:coal 16' % (CORE[0], CORE[1] - 1, CORE[2]))
    g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 2' % CORE)
    ok = wait_for(lambda: molten(model(g, CHANNEL_AT)) == 144, 300)
    g.check('the_channel_holds_144', ok, [molten(model(g, CHANNEL_AT)), molten(model(g, CORE))])
    g.check('the_crucible_is_empty', molten(model(g, CORE)) == 0, molten(model(g, CORE)))
    g.command('item replace block %d %d %d container.0 with minecraft:air' % (CORE[0], CORE[1] - 1, CORE[2]))
    g.command('setblock %d %d %d minecraft:stone' % (CORE[0], CORE[1] - 1, CORE[2]))
    g.check('nothing_pours_without_a_tap', molten(model(g, CHANNEL_AT)) == 144, model(g, CHANNEL_AT))
    if SCENARIO == 'slagtap-break':
        put_tap(g)


def put_tap(g, wait=True):
    """The slag tap behind the channel, facing south; the pour follows. Waited for: the slag holds all 144 and the channel none."""
    g.command('setblock %d %d %d ntm:foundry_slagtap[facing=south,honey_level=0]' % TAP_AT)
    if not wait:
        return
    ok = wait_for(lambda: molten(model(g, SLAG_AT)) == 144, 120)
    g.check('the_slag_holds_144', ok, [molten(model(g, SLAG_AT)), molten(model(g, CHANNEL_AT)), server_block(g, SLAG_AT)])
    g.check('the_channel_is_empty', molten(model(g, CHANNEL_AT)) == 0, model(g, CHANNEL_AT))
    g.check('it_is_a_slag_block', server_block(g, SLAG_AT) == 'ntm:slag', server_block(g, SLAG_AT))


def place_caster(g):
    """The caster, with its pour cell under the crucible's waste stream, 144 quanta in its pool."""
    if VANILLA:
        g.command('setblock %d %d %d minecraft:barrel' % OTHER)
        g.command('item replace block %d %d %d container.0 with ntm:mold__ingot 1' % OTHER)
        g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 2' % OTHER)
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
            if server_block(g, p) == CASTER:
                core = core or p
    # The core is the cell that holds the model (the dummies have none).
    for dx in range(-3, 4):
        for dz in range(-3, 4):
            p = (OTHER[0] + dx, OTHER[1], OTHER[2] + dz)
            if server_block(g, p) == CASTER and model(g, p):
                core = p
    g.check('caster_placed', core is not None, core)
    facing = re.search(r'"facing":"(north|south|east|west)"', machine_state(g, core)).group(1)
    d = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}[facing]
    g.evidence['caster'] = {'core': core, 'facing': facing}
    cell = (core[0], core[1] + 2, core[2])  # the cell over the core is one of the four funnel cells
    g.command('item replace block %d %d %d container.0 with ntm:mold__ingot 1' % core)
    if SCENARIO == 'batch':
        water_barrel(g, core, d)
    # The crucible two blocks south of the cell facing north: its waste stream starts two blocks north of it.
    crucible = (cell[0], cell[1] + 1, cell[2] - 2)
    fire = (crucible[0], crucible[1] - 1, crucible[2])
    g.command('setblock %d %d %d %s[facing=south,honey_level=0]' % (*fire, FIREBOX))
    g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*crucible, CRUCIBLE))
    g.evidence['crucible'] = crucible
    (g.folder / 'caster-core.json').write_text(json.dumps({'core': core, 'crucible': crucible}))
    time.sleep(2)
    g.command('item replace block %d %d %d container.0 with minecraft:coal 16' % fire)
    g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 2' % crucible)
    if SCENARIO == 'batch':
        ok = wait_for(lambda: held_gold(g, core) == 2 and molten(model(g, core)) == 0, 300)
        g.check('cast_two_ingots', ok, [held_gold(g, core), molten(model(g, core)), molten(model(g, crucible)), block_items(g, core)[:300]])
        m = model(g, core)
        g.evidence['water_after'] = (m or {}).get('water')
        g.check('water_was_used', ((m or {}).get('water') or {}).get('mb', 64000) < 64000 and ((m or {}).get('steam') or {}).get('mb', 0) > 0, (m or {}).get('water'))
    else:
        ok = wait_for(lambda: molten(model(g, core)) == 144, 240)
        g.check('poured_144_into_the_caster', ok, [molten(model(g, core)), molten(model(g, crucible))])
    g.check('the_crucible_is_empty', molten(model(g, crucible)) == 0, molten(model(g, crucible)))
    # Cold from here.
    g.command('setblock %d %d %d minecraft:stone' % fire)
    g.command('setblock %d %d %d minecraft:stone' % crucible)


def held_gold(g, p):
    """The gold ingots in the slots of the machine at `p`."""
    return sum(n for i, n, _ in entries(block_items(g, p)) if i == 'minecraft:gold_ingot')


def water_barrel(g, core, d):
    """A steel barrel at the caster's first port (core + rot - dir, open to rot: `getPorts`), with a bucket of water in
    through its screen and set to output, as native_redes.py does."""
    rot = (-d[1], d[0])
    barrel = (core[0] + 2 * rot[0] - d[0], core[1], core[2] + 2 * rot[1] - d[1])
    g.command('setblock %d %d %d ntm:barrel_steel[facing=south,honey_level=0]' % barrel)
    g.command('gamemode creative @s')
    g.command('clear @s')
    g.command('give @s minecraft:water_bucket 1')
    g.command(f'tp @s {barrel[0] + .5} {barrel[1]} {barrel[2] + 3.5}')
    time.sleep(2)
    g.close()
    g.ctl('interact %d %d %d south' % barrel)
    time.sleep(2)
    screen = g.ctl('screen')
    hot = [s['slot'] for s in screen.get('slots', []) if s.get('item') == 'minecraft:water_bucket' and s['slot'] < 36]
    g.check('the_bucket_is_there', bool(hot), [s for s in screen.get('slots', []) if s.get('item')])
    g.ctl(f'click-slot {hot[0]}')
    time.sleep(.4)
    g.ctl('click-slot 302')
    time.sleep(1)
    slot0 = next(w for w in g.ctl('ui').get('widgets', []) if w.get('id') == 'slot-0')['bounds']
    ox, oy = slot0[0] - 8, slot0[1] - (84 + 58)
    for _ in range(2):
        g.ctl(f'click-at {ox + 151 + 9} {oy + 35 + 9}')
        time.sleep(.8)
    g.close()
    time.sleep(3)
    state = machine_state(g, barrel)
    g.evidence['barrel'] = {'at': barrel, 'state': state[:300]}
    g.check('the_barrel_holds_water', '"fluid":"water"' in state.replace(' ', ''), state[:300])


def break_it(g, cells, label):
    """A player in survival breaks the first of `cells` it can, as the sawmill's break scenario does."""
    g.command('clear @s')
    g.command('gamemode survival @s')
    g.command('give @s minecraft:diamond_pickaxe 1')
    g.ctl('select minecraft:diamond_pickaxe')
    broken = None
    queue = list(cells)
    while queue:
        cell = queue.pop(0)
        try:
            g.ctl('break %d %d %d --timeout 30' % cell)
            broken = cell
            break
        except RuntimeError as error:
            # A multiblock's other cells hide the core's faces from where the player stands: breaking any of its
            # dummies breaks the machine, so the one in the way is tried (the control's barrel has none).
            blocker = re.search(r'ntm:dummy_machine at \[(-?\d+), (-?\d+), (-?\d+)\] is in the way', str(error))
            if blocker:
                queue.append(tuple(int(v) for v in blocker.groups()))
    g.check('broken_' + label, broken is not None, broken)
    time.sleep(1.5)


def use_machine(g, cells, label, face='south'):
    """A right click on the first of `cells` that the player can reach; a multiblock's other cells hide the core's faces, so
    the dummy in the way is clicked (it is the same machine)."""
    queue = list(cells)
    while queue:
        cell = queue.pop(0)
        try:
            g.ctl('interact %d %d %d %s' % (*cell, face))
            time.sleep(1.5)
            return cell
        except RuntimeError as error:
            blocker = re.search(r'ntm:dummy_machine at \[(-?\d+), (-?\d+), (-?\d+)\] is in the way', str(error))
            if blocker:
                queue.append(tuple(int(v) for v in blocker.groups()))
    g.check('used_' + label, False, cells)


def pick_up_all(g, required=True):
    drops = [e['pos'] for e in g.ctl('entities 20')['entities'] if e.get('kind') == 'item']
    if required:
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
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + PZ}')
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
            g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 1' % CORE)
            g.command('item replace block %d %d %d container.2 with minecraft:gold_nugget 3' % CORE)
            g.command('clear @s minecraft:gold_ingot')
            g.command('clear @s minecraft:gold_nugget')
        else:
            g.check('one_exact_scrap', stack_quanta(inv) == 96 and 'ntm_amount:96' in inv.replace(' ', '').replace('"', ''), inv[:400])
            g.command('gamemode creative @s')
            g.command('setblock %d %d %d %s[facing=north,honey_level=0]' % (*CORE, CRUCIBLE))
            time.sleep(2)
            put_back(g, SCRAP)
    elif SCENARIO == 'clicks':
        if VANILLA:
            g.command('item replace block %d %d %d container.1 with minecraft:air' % CORE)
            g.command('give @s minecraft:gold_ingot 2')
        else:
            g.command('clear @s')
            g.command('gamemode survival @s')
            g.command('give @s ntm:mold__ingot 2')
            # From above: the mold block is in the floor and a side face is hidden by the floor's blocks.
            g.command(f'tp @s {MOLD_AT[0] + .5} {MOLD_AT[1] + 2} {MOLD_AT[2] + 2.5}')
            time.sleep(1)
            g.ctl('select ntm:mold__ingot')
            use_machine(g, [MOLD_AT], 'mold_block_with_a_mold', 'up')
            g.check('the_mold_went_in', 'ntm:mold__ingot' in block_items(g, MOLD_AT), block_items(g, MOLD_AT)[:200])
            held = [n for i, n, _ in entries(inventory_text(g)) if i == 'ntm:mold__ingot']
            g.check('one_mold_of_the_stack', held == [1], held)
            # The stream fills the mold (72) and the cast cools for 200 ticks: an ingot in its slot, 72 left in the crucible.
            g.check('the_cast_cooled', wait_for(lambda: held_gold(g, MOLD_AT) == 1, 120), [block_items(g, MOLD_AT)[:200], molten(model(g, CORE))])
            # The ingot waits in the mold's slot, so nothing more pours (a mold takes metal only with its cast slot empty): the
            # 72 left stay in the crucible for the shovel. Then the ingot comes out by a click with an empty hand.
            g.command('clear @s')
            g.command('give @s minecraft:iron_shovel 1')
            g.ctl('select minecraft:iron_shovel')
            g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + PZ}')
            time.sleep(1)
            g.check('72_wait_in_the_crucible', molten(model(g, CORE)) == 72, molten(model(g, CORE)))
            use_machine(g, [(CORE[0], CORE[1] + 1, CORE[2]), CORE], 'crucible_with_a_shovel')
            g.check('the_crucible_is_empty', molten(model(g, CORE)) == 0, molten(model(g, CORE)))
            inv = inventory_text(g)
            g.check('the_rest_is_one_scrap_of_72', 'ntm:scraps__gold' in inv and stack_quanta(inv) == 72, inv[:600])
            g.command('clear @s minecraft:iron_shovel')
            g.command(f'tp @s {MOLD_AT[0] + .5} {MOLD_AT[1] + 2} {MOLD_AT[2] + 2.5}')
            time.sleep(1)
            use_machine(g, [MOLD_AT], 'mold_block_with_an_empty_hand', 'up')
            g.check('the_ingot_came_out', held_gold(g, MOLD_AT) == 0 and 'minecraft:gold_ingot' in inventory_text(g), [block_items(g, MOLD_AT)[:200], inventory_text(g)[:300]])
        g.evidence['picked'] = inventory_text(g)[:600]
        g.command('gamemode creative @s')
    elif SLAGTAP and SCENARIO != 'slagtap-break':
        if VANILLA:
            g.command('item replace block %d %d %d container.1 with minecraft:air' % CHANNEL_AT)
            g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 2' % SLAG_AT)
        else:
            # The tap, and straight to the save: the pour is one game step racing it, whichever side of the cut it falls on.
            put_tap(g, wait=False)
            try:
                g.ctl('save')
            except Exception as error:
                g.evidence['early_save_error'] = repr(error)[:200]
            if BORDER:
                time.sleep(15)
                g.check('the_border_tap_pours_nothing', server_block(g, SLAG_AT) == '' and molten(model(g, CHANNEL_AT)) == 144,
                        [server_block(g, SLAG_AT), molten(model(g, CHANNEL_AT))])
            else:
                ok = wait_for(lambda: molten(model(g, SLAG_AT)) == 144, 120)
                g.check('the_slag_holds_144', ok, [molten(model(g, SLAG_AT)), molten(model(g, CHANNEL_AT)), server_block(g, SLAG_AT)])
                g.check('the_channel_is_empty', molten(model(g, CHANNEL_AT)) == 0, model(g, CHANNEL_AT))
    elif SCENARIO == 'slagtap-break':
        if VANILLA:
            g.command('item replace block %d %d %d container.1 with minecraft:air' % SLAG_AT)
            g.command('give @s minecraft:gold_ingot 2')
        else:
            g.command('clear @s')
            g.command('gamemode survival @s')
            g.command('give @s minecraft:diamond_pickaxe 1')
            g.ctl('select minecraft:diamond_pickaxe')
            g.command(f'tp @s {SLAG_AT[0] + .5} {SLAG_AT[1]} {SLAG_AT[2] + 2.5}')
            time.sleep(1)
            break_it(g, [SLAG_AT], 'slag')
            pick_up_all(g)
            inv = inventory_text(g)
            g.check('one_exact_scrap_of_144', stack_quanta(inv) == 144 and 'ntm_amount:144' in inv.replace(' ', '').replace('"', ''), inv[:400])
            g.check('the_slag_is_gone', server_block(g, SLAG_AT) == '', server_block(g, SLAG_AT))
        g.evidence['picked'] = inventory_text(g)[:600]
        g.command('gamemode creative @s')
    elif SCENARIO == 'excess':
        core = caster_core(g)
        if VANILLA:
            # The 144 become what a barrel and a hand hold: an ingot and a nugget, and 8 nuggets for the player.
            g.command('item replace block %d %d %d container.1 with minecraft:gold_ingot 1' % tuple(core))
            g.command('item replace block %d %d %d container.2 with minecraft:gold_nugget 1' % tuple(core))
            g.command('give @s minecraft:gold_nugget 8')
        else:
            g.command('item replace block %d %d %d container.0 with ntm:mold__nugget 1' % tuple(core))
            g.check('the_excess_went_out', wait_for(lambda: molten(model(g, core)) == 80, 120), molten(model(g, core)))
            time.sleep(2)
            # A player near the machine may have picked the scrap up already: the inventory is what is checked.
            pick_up_all(g, required=False)
            g.check('one_exact_scrap_of_64', 'ntm_amount:64' in inventory_text(g).replace(' ', '').replace('"', ''), inventory_text(g)[:400])
        g.evidence['picked'] = inventory_text(g)[:600]
        g.command('gamemode creative @s')
    elif SCENARIO == 'hand':
        core = caster_core(g)
        if VANILLA:
            g.command('item replace block %d %d %d container.1 with minecraft:air' % tuple(core))
            g.command('give @s minecraft:gold_ingot 2')
        else:
            # The mold out by hand through the screen (no screwdriver): the review's P3 (c), the mold is not stuck.
            g.command('clear @s')
            g.command('gamemode survival @s')
            g.command(f'tp @s {core[0] + .5} {core[1]} {core[2] + 4.5}')
            time.sleep(1)
            use_machine(g, [(core[0], core[1] + 1, core[2]), core], 'caster_screen')
            screen = g.ctl('screen')
            g.check('the_screen_shows_the_mold', any(sl['slot'] == 300 and sl.get('item') == 'ntm:mold__ingot' for sl in screen.get('slots', [])), [sl for sl in screen.get('slots', []) if sl.get('item')])
            free = next(sl['slot'] for sl in screen.get('slots', []) if sl['slot'] < 36 and not sl.get('item'))
            g.ctl('click-slot 300')
            g.ctl(f'click-slot {free}')
            time.sleep(.5)
            g.close()
            g.check('the_mold_is_out', 'ntm:mold__' not in block_items(g, core), block_items(g, core)[:200])
            g.check('the_mold_is_in_the_inventory', 'ntm:mold__ingot' in inventory_text(g), inventory_text(g)[:200])
            g.command('clear @s')
            g.command('give @s ntm:mold__ingot 2')
            g.command('gamemode survival @s')
            g.command(f'tp @s {core[0] + .5} {core[1]} {core[2] + 4.5}')
            time.sleep(1)
            g.ctl('select ntm:mold__ingot')
            use_machine(g, [(core[0], core[1] + 1, core[2]), core], 'caster_with_a_mold')
            g.check('the_mold_went_in', 'ntm:mold__ingot' in block_items(g, core), block_items(g, core)[:200])
            held = [n for i, n, _ in entries(inventory_text(g)) if i == 'ntm:mold__ingot']
            g.check('one_mold_of_the_stack', held == [1], held)
            g.command('give @s minecraft:iron_shovel 1')
            g.ctl('select minecraft:iron_shovel')
            use_machine(g, [(core[0], core[1] + 1, core[2]), core], 'caster_with_a_shovel')
            g.check('the_pool_is_empty', molten(model(g, core)) == 0, molten(model(g, core)))
            inv = inventory_text(g)
            g.check('one_exact_scrap_of_144', 'ntm_amount:144' in inv.replace(' ', '').replace('"', '') and stack_quanta(inv) == 144, inv[:500])
        g.evidence['picked'] = inventory_text(g)[:600]
        g.command('gamemode creative @s')
    elif SCENARIO in ('caster', 'batch'):
        core = caster_core(g)
        cells = [(core[0], core[1] + dy, core[2]) for dy in (0, 1, 2)]
        break_it(g, cells if not VANILLA else [OTHER], 'caster')
        pick_up_all(g)
        g.evidence['picked'] = inventory_text(g)[:600]
        g.command('gamemode creative @s')
    for z in (CORE[2] - 6, CORE[2] + 6):
        g.command(f'setblock 37 100 {z} minecraft:cobblestone')
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + PZ}')
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
    g.command(f'tp @s {CORE[0] + .5} {CORE[1]} {CORE[2] + PZ}')
    time.sleep(6)
    now = count(g)
    if SCENARIO == 'excess' and not VANILLA:
        # The review's P1-1: a caster restored from the journal keeps the block entity's mold (the older or the newer one:
        # never none) and its model has the pool and the slots.
        items = block_items(g, caster_core(g))
        g.evidence['caster_items'] = items[:400]
        g.check('the_mold_survives', 'ntm:mold__' in items, items[:400])
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

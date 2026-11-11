#!/usr/bin/env python3
"""The game side of run-player-cut-proof.py: a player's state that lives in a player attachment, cut by a save.

Scenarios (NTM_CUT_SCENARIO):
  table    a hazmat hood and a lead cladding are clicked into the armor table (slots 9 and 5) and the table is left
           open: they live in the `ntm:armor_table` attachment. Control (NTM_CRASH_VANILLA=1): the same two stacks
           moved into the Ender chest, which lives in the player's file (EnderItems).
  radaway  one RadAway used on a 300 RAD dose: the item leaves the inventory, the treatment goes to `ntm:item_state`.
  herbal   one herbal pill: the item leaves, the sickness timer (`herbal`) goes to `ntm:item_state`.
  irradiation  (slice 5c) a step on glowing mycelium: the radiation potion (`irradiated`, `irradiated_level`) and its
           dose go to `ntm:item_state`, the `ntm:radiation` effect shows it. Control: the table's two stacks moved
           into the Ender chest at the same moment.
  migration    (slice 5c) prepared by the binary before 5c, whose potion is in the world's storage
           (`zirnox.irradiated`); the act is the first minute of the new binary, which moves it to the attachment.
Modes: prepare (the world saved with the starting state), act (do it, then a save the failpoint may end; NTM_CRASH_NO_SAVE,
NTM_CRASH_PAUSE_SAVE as run-crash-proof), verify / again (after a reopen: count).
"""
import os
import re
import sys
import time
from native_game import Game
import ntm_store

SCENARIO = os.environ.get('NTM_CUT_SCENARIO', 'table')
VANILLA = os.environ.get('NTM_CRASH_VANILLA') == '1'
NO_SAVE = os.environ.get('NTM_CRASH_NO_SAVE') == '1'
PAUSE_SAVE = os.environ.get('NTM_CRASH_PAUSE_SAVE') == '1'
TABLE = (24, 101, 8)
ITEMS = ['ntm:hazmat_helmet_grey', 'ntm:cladding_lead']
USED = {'radaway': 'ntm:radaway', 'herbal': 'ntm:pill_herbal'}


def held_items(g):
    """Every stack of the scenario's items the player and the ground near them hold, nested ones included."""
    inv = g.ctl('inventory')
    total = {}
    blob = []
    for group in ('hotbar', 'main', 'armor', 'offhand', 'enderchest', 'ender'):
        stacks = inv.get(group) or []
        stacks = stacks if isinstance(stacks, list) else [stacks]
        for s in stacks:
            if isinstance(s, dict) and s.get('item'):
                total[s['item']] = total.get(s['item'], 0) + s.get('count', 0)
                blob.append(s.get('components') or '')
    for e in g.ctl('entities 30').get('entities', []):
        item = e.get('item') or {}
        if item.get('item'):
            total[item['item']] = total.get(item['item'], 0) + item.get('count', 0)
            blob.append(item.get('components') or '')
    for name in ITEMS:
        total[name] = total.get(name, 0) + sum(len(re.findall(r'id:"' + re.escape(name) + '"', b)) for b in blob)
    return total


def ender(g):
    """The Ender chest's stacks, read through `/data`: the inventory snapshot has no Ender group."""
    text = ' '.join(g.command('data get entity @s EnderItems').get('chat', []))
    return {name: len(re.findall(re.escape(name), text)) for name in ITEMS}


def item_state(g):
    text = ' '.join(g.command('ntmitem inspect').get('chat', []))
    out = {k: int(v) for k, v in re.findall(r'(dose|radaway|herbal|irradiated|irradiated_level):\s*(\d+)[lL]?', text)}
    out['effects'] = sorted(e['id'].split(':')[-1] for e in g.ctl('state')['player'].get('effects', []))
    return out


def count(g):
    if SCENARIO == 'table' or VANILLA:
        found = held_items(g)
        e = ender(g)
        return {'items': {n: found.get(n, 0) + e.get(n, 0) for n in ITEMS}, 'ender': e}
    if SCENARIO in ('irradiation', 'migration'):
        st = item_state(g)
        # The world's storage as the cut left it (the potion's home before 5c): read before this session saves.
        try:
            st['kv_irradiated'] = sorted((ntm_store.value(g.folder / 'saves').get('zirnox') or {}).get('irradiated', {}).values())
        except (KeyError, RuntimeError, OSError) as error:
            st['kv_error'] = repr(error)[:120]
        return {'items': {}, 'state': st}
    item = USED[SCENARIO]
    return {'items': {item: held_items(g).get(item, 0)}, 'state': item_state(g)}


MYCELIUM = (22, 100, 11)


def step_on_mycelium(g):
    """`WasteEarth.onEntityWalking`: a moment on glowing mycelium gives the radiation potion (600 ticks, amplifier 3)."""
    x, y, z = MYCELIUM
    g.command(f'setblock {x} {y} {z} ntm:waste_mycelium')
    g.command(f'tp @s {x + .5} {y + 1} {z + .5}')
    time.sleep(1.0)
    g.command('tp @s 20.5 101 11.5')
    g.command(f'setblock {x} {y} {z} minecraft:stone')
    time.sleep(.3)


def table_slot(screen, n):
    return next((x.get('item') for x in screen.get('slots', []) if x['slot'] == 300 + n), None)


def prepare(g):
    g.ctl('wait-ready 120')
    g.close()
    g.ground(20, 101, 8, width=12, height=8)
    g.command('gamemode survival @s')
    g.command('clear @s')
    g.command('ntmitem cure')
    g.command('effect clear @s')
    if SCENARIO == 'table' or VANILLA:
        if not VANILLA:
            g.command(f'setblock {TABLE[0]} {TABLE[1]} {TABLE[2]} ntm:machine_armor_table')
        g.command('tp @s 24.5 101 11.5')
        for slot, item in enumerate(ITEMS):
            g.command(f'item replace entity @s hotbar.{slot} with {item}')
    elif SCENARIO in ('irradiation', 'migration'):
        g.command('ntmitem dose 300000')
        g.command('tp @s 20.5 101 11.5')
        if SCENARIO == 'migration':
            # The binary before 5c keeps this potion in the world's storage.
            time.sleep(12)  # past the first 200 ticks, so the potion also doses
            step_on_mycelium(g)
    else:
        g.command('ntmitem dose 300000')
        g.command(f'item replace entity @s hotbar.0 with {USED[SCENARIO]} 1')
        g.ctl('select 1')
    time.sleep(1.5)
    g.evidence['base'] = count(g)
    g.ctl('save')


def act(g):
    g.ctl('wait-ready 120')
    g.close()
    before = count(g)
    g.evidence['before'] = before
    if SCENARIO == 'irradiation' and not VANILLA:
        time.sleep(11)  # past the first 200 ticks of this session: the potion doses
        step_on_mycelium(g)
    elif SCENARIO == 'migration':
        time.sleep(3)
    elif SCENARIO == 'table' or VANILLA:
        if VANILLA:
            for slot in range(len(ITEMS)):
                g.command(f'item replace entity @s enderchest.{slot} from entity @s hotbar.{slot}')
                g.command(f'item replace entity @s hotbar.{slot} with minecraft:air')
        else:
            g.ctl(f'interact {TABLE[0]} {TABLE[1]} {TABLE[2]} south')
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline and len([s for s in g.ctl('screen').get('slots', []) if s['slot'] >= 300]) != 14:
                time.sleep(.3)
            for item, target in zip(ITEMS, (9, 5)):
                screen = g.ctl('screen')
                source = next(s['slot'] for s in screen['slots'] if s['slot'] < 36 and s.get('item') == item)
                g.ctl(f'click-slot {source}')
                g.ctl(f'click-slot {300 + target}')
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline and table_slot(g.ctl('screen'), target) != item:
                    time.sleep(.3)
            g.check('both_in_the_table', table_slot(g.ctl('screen'), 9) == ITEMS[0] and table_slot(g.ctl('screen'), 5) == ITEMS[1],
                    g.ctl('screen').get('slots'))
    else:
        g.ctl('look-at 20 160 8')
        g.ctl('mouse right down')
        time.sleep(2.5 if SCENARIO == 'herbal' else .3)
        g.ctl('mouse right up')
        time.sleep(1.0)
    g.evidence['after_act'] = count(g)
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
    except Exception as error:  # the failpoint ends the process inside the save
        g.evidence['survived_save'] = False
        g.evidence['save_error'] = repr(error)[:200]
    g.save()


def verify(g, again=False):
    g.ctl('wait-ready 120')
    g.close()
    # A table left open without its menu gives its pieces back once, by the closing commit (armor_table::tick).
    time.sleep(3)
    g.evidence['counted' + ('_again' if again else '')] = count(g)


if __name__ == '__main__':
    mode = sys.argv[2]
    g = Game(sys.argv[1], f'player-cut-{SCENARIO}-{mode}')
    {'prepare': prepare, 'act': act, 'verify': verify, 'again': lambda g: verify(g, True)}[mode](g)
    g.evidence['ok'] = True
    g.save()

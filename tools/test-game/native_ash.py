#!/usr/bin/env python3
"""Onda 1, slice 6 (docs/onda-1-protecao.md): the ash on a disposable native world.

setup          an ashpit under a firebox and one under a heating oven, their cores set by command: a coal in each
               fire leaves the coal's base time (the burn module's 2000, the oven's burn being 250) in the pit under
               it, made into coal ash the next tick with the threshold taken off the wood's level as well
               (`TileEntityAshpit.processAsh`); the oven's second coal, lit when the first is out (250 ticks), grows
               the stack and leaves the wood alone (the firebox's coal stays lit: with nothing taking its heat it
               fills up and its burn stops, as the Java's `heatEnergy < getMaxHeat()`).
               A wood burner turned off still lights a coal block (`burnTime <= 0`, on or off): 16 000 of coal ash,
               eight ashes at once in its slot 1 (`while processAsh`), and its burn stays while it is off.
capture-state  the pits' and the burner's states written down, the world saved.
check-state    reopened: the same levels, ashes and burn.
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import first_int, machine_state
from native_furnaces import count, open_core, open_fire, sprint

X, Z = 40, 8
PITS = {'firebox': ((X, 100, Z), 'ntm:heater_firebox', 1), 'oven': ((X + 8, 100, Z), 'ntm:heater_oven', 2)}
BURNER = (X + 16, 100, Z)
SAVED = 'ash-capture.json'


def pit(g, at):
    """The ashpit's levels and slots from its saved state."""
    text = machine_state(g, at)
    levels = {k: int(v) for k, v in re.findall(r'"(wood|coal|misc|fly|soot)":(-?\d+)', text)}
    ashes = re.findall(r'"id":"(ntm:powder_ash__\w+)","count":(\d+)', text)
    # Lists, not tuples: the state comes back from the saved JSON as lists.
    return {'levels': levels, 'ashes': [[i, int(n)] for i, n in ashes], 'raw': text[:400]}


def burner(g):
    text = machine_state(g, BURNER)
    get = lambda k: first_int(text, r'"%s":(-?\d+)' % k)
    on = re.search(r'"on":(true|false)', text)
    open_core(g, BURNER)
    ash = count(g.slots(), 'ntm:powder_ash__coal')
    fuel = count(g.slots(), 'minecraft:coal_block')
    g.close()
    return {'burn': get('burn'), 'ash_coal': get('ash_coal'), 'on': on and on.group(1), 'ash_in_slot_1': ash, 'coal_blocks': fuel}


def setup(g):
    for name, (at, fire_block, coal) in PITS.items():
        x, y, z = at
        g.close()
        g.ground(x, y, z, width=4)
        g.command(f'setblock {x} {y} {z} ntm:machine_ashpit[facing=north]')
        g.command(f'setblock {x} {y + 1} {z} {fire_block}[facing=north]')
        time.sleep(1.5)
        g.check(f'{name}_pit_is_a_machine', 'Ashpit' in machine_state(g, at), machine_state(g, at)[:200])
        open_fire(g, (x, y + 1, z))
        g.put('minecraft:coal', coal, 0)
        g.close()
        sprint(g, 5)
        p = pit(g, at)
        g.check(f'{name}_coal_leaves_one_coal_ash_and_takes_the_wood_too',
                p['ashes'] == [['ntm:powder_ash__coal', 1]] and p['levels'].get('coal') == 0 and p['levels'].get('wood') == -2000, p)
        g.evidence[f'{name}_fire'] = machine_state(g, (x, y + 1, z))[:300]
    oven = machine_state(g, (X + 8, 101, Z))
    g.check('the_ovens_burn_is_an_eighth', '"max_burn":250' in oven, oven[:300])
    # The oven's second coal, once the first is out (250 ticks): the same stack grows, the wood stays.
    at = PITS['oven'][0]
    sprint(g, 300)
    p = pit(g, at)
    g.check('a_second_coal_grows_the_stack_and_leaves_the_wood', p['ashes'] == [['ntm:powder_ash__coal', 2]]
            and p['levels'].get('coal') == 0 and p['levels'].get('wood') == -2000, p)
    # The wood burner, off: it lights the coal block anyway.
    x, y, z = BURNER
    g.close()
    g.ground(x, y, z, width=4)
    g.command(f'setblock {x} {y} {z} ntm:machine_wood_burner[facing=north]')
    time.sleep(1.5)
    open_core(g, BURNER)
    g.button([53, 17, 16, 15], [176, 186])
    time.sleep(.5)
    g.put('minecraft:coal_block', 2, 0)
    g.close()
    sprint(g, 40)
    b = burner(g)
    g.check('an_off_burner_lights_a_coal_block_into_eight_ashes', b['on'] == 'false' and b['burn'] == 16000 and b['ash_in_slot_1'] == 8
            and b['ash_coal'] == 0 and b['coal_blocks'] == 1, b)


def capture(g):
    states = {name: pit(g, at) for name, (at, _, _) in PITS.items()}
    states['burner'] = burner(g)
    g.evidence['captured'] = states
    (g.folder / SAVED).write_text(json.dumps(states))
    g.ctl('save')


def check(g):
    old = json.loads((g.folder / SAVED).read_text())
    for name, (at, _, _) in PITS.items():
        now = pit(g, at)
        g.check(f'{name}_pit_survived_the_reopen', now['levels'] == old[name]['levels'] and now['ashes'] == old[name]['ashes'], [old[name], now])
    now = burner(g)
    g.check('burner_survived_the_reopen', now == old['burner'], [old['burner'], now])


if __name__ == '__main__':
    g = Game(sys.argv[1], 'ash-' + sys.argv[2])
    mode = sys.argv[2]
    g.evidence['phase'] = mode
    g.ctl('wait-ready 120')
    {'setup': setup, 'capture-state': capture, 'check-state': check}[mode](g)
    g.evidence['ok'] = True
    g.save()

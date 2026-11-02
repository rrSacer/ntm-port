#!/usr/bin/env python3
"""Machines in a level that is not the last one ticked (the player in the Nether, a forced
Nether level ticking after the Overworld): their block entities must keep the state, and a
machine placed meanwhile must be adopted after the world is reopened.
Usage: native_dims.py GAME [setup|capture-state|check-state]
"""
import json
import re
import sys
import time
from native_game import Game

OLD=(40,101,10)
NEW=(44,101,10)


def state(g,p):
    """The `ntm` state the Overworld block entity at p keeps, wherever the player is."""
    result=g.command(f'execute in minecraft:overworld run data get block {p[0]} {p[1]} {p[2]} ntm')
    text=' '.join(result.get('chat',[])).replace('\\"','"')
    return text


def power(g,p):
    found=re.search(r'"power":(\d+)',state(g,p))
    return int(found.group(1)) if found else None


def feed(g,p):
    g.command(f'execute in minecraft:overworld run setblock {p[0]} {p[1]+1} {p[2]} minecraft:hopper[facing=down]')
    g.command(f'execute in minecraft:overworld run item replace block {p[0]} {p[1]+1} {p[2]} container.0 with ntm:battery_creative 1')


def setup(g):
    g.close()
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 50 24')
    time.sleep(3)
    g.command('fill 32 99 3 48 99 18 minecraft:stone')
    g.command('fill 32 100 3 48 108 18 minecraft:air')
    g.command(f'tp @s 38.5 100 8.5')
    # An old machine: it exists before the level stops being the last one ticked.
    g.command(f'setblock {OLD[0]} {OLD[1]} {OLD[2]} ntm:machine_battery[facing=south]')
    time.sleep(3)
    g.check('old_machine_has_state',power(g,OLD)==0,state(g,OLD))
    # The Nether, forced, ticks after the Overworld from now on, and the player is in it.
    g.command('execute in minecraft:the_nether run forceload add 0 0')
    time.sleep(3)
    g.command('execute in minecraft:the_nether run fill -3 100 -3 3 104 3 minecraft:air')
    g.command('execute in minecraft:the_nether run fill -3 99 -3 3 99 3 minecraft:netherrack')
    g.command('execute in minecraft:the_nether run tp @s 0.5 100 0.5')
    time.sleep(3)
    here=g.ctl('block 0 99 0')['block']
    g.check('player_in_nether',here=='minecraft:netherrack',here)
    # The old machine changes in the Overworld (a creative battery fills it), and a new one is placed.
    feed(g,OLD)
    g.command(f'execute in minecraft:overworld run setblock {NEW[0]} {NEW[1]} {NEW[2]} ntm:machine_battery[facing=south]')
    feed(g,NEW)
    deadline=time.monotonic()+30
    while time.monotonic()<deadline and not ((power(g,OLD) or 0)>0 and (power(g,NEW) or 0)>0):time.sleep(1)
    old,new=power(g,OLD),power(g,NEW)
    g.check('old_machine_state_follows_while_not_last',(old or 0)>0,old)
    g.check('new_machine_is_given_state_while_not_last',(new or 0)>0,new)
    g.evidence['powers']={'old':old,'new':new}


def persistence(g,capture):
    path=g.folder/'native-dims-state.json'
    if capture:
        time.sleep(2)
        path.write_text(json.dumps({'old':power(g,OLD),'new':power(g,NEW)})+'\n')
        g.ctl('save')
        return
    before=json.loads(path.read_text())
    # The Overworld is a level of its own: it opens when the player goes back (the world
    # reopens with them in the Nether), and its machines are adopted from their block entities.
    g.command('execute in minecraft:overworld run tp @s 38.5 100 8.5')
    time.sleep(8)
    old,new=power(g,OLD),power(g,NEW)
    g.check('reopened_old_machine_state',old is not None and old>=before['old'],{'before':before['old'],'after':old})
    g.check('reopened_new_machine_adopted',new is not None and new>=before['new'],{'before':before['new'],'after':new})
    # Adopted means running: a machine that was only a dead block would not still follow its feeder.
    time.sleep(3)
    g.check('reopened_new_machine_still_runs',power(g,NEW)>=new,{'now':power(g,NEW),'then':new})


if __name__=='__main__':
    mode=sys.argv[2] if len(sys.argv)>2 else 'setup'
    g=Game(sys.argv[1],'dims'+('' if mode=='setup' else '-'+mode))
    g.ctl('wait-ready 120')
    if mode=='setup':setup(g)
    elif mode in ('capture-state','check-state'):persistence(g,mode=='capture-state')
    else:raise SystemExit('unknown mode: '+mode)
    g.evidence['ok']=True;g.save()

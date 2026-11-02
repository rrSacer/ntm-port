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
CHEST=(46,101,16)
COMPARATOR=(47,101,16)
LOADER=(48,101,16)


def state(g,p,dim='minecraft:overworld'):
    """The `ntm` state the block entity at p keeps in `dim`, wherever the player is."""
    result=g.command(f'execute in {dim} run data get block {p[0]} {p[1]} {p[2]} ntm')
    text=' '.join(result.get('chat',[])).replace('\\"','"')
    return text


def power(g,p,dim='minecraft:overworld'):
    found=re.search(r'"power":(\d+)',state(g,p,dim))
    return int(found.group(1)) if found else None


def red(g):
    """Whether the loader beside the comparator has seen the redstone signal."""
    return '"was_redstone":true' in state(g,LOADER).replace(' ','')


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
    # A comparator reading a chest feeds a loader (a redstone reader) in the Overworld. Its output
    # lives in the Overworld's own block ticks: the read must find it also when another level is in place.
    g.command(f'setblock {CHEST[0]} {CHEST[1]} {CHEST[2]} minecraft:chest')
    g.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.0 with minecraft:stone 64')
    g.command(f'setblock {COMPARATOR[0]} {COMPARATOR[1]-1} {COMPARATOR[2]} minecraft:stone')
    g.command(f'setblock {COMPARATOR[0]} {COMPARATOR[1]} {COMPARATOR[2]} minecraft:comparator[facing=west]')
    g.command(f'setblock {LOADER[0]} {LOADER[1]} {LOADER[2]} ntm:pile_device__loader[facing=north]')
    deadline=time.monotonic()+20
    while time.monotonic()<deadline and not red(g):time.sleep(1)
    g.check('comparator_feeds_the_loader_with_the_overworld_in_place',red(g),{'loader':state(g,LOADER),'comparator':g.ctl('block %d %d %d'%COMPARATOR)['state'],'chest':' '.join(g.command('data get block %d %d %d Items'%CHEST).get('chat',[]))})
    # The Nether, forced, ticks after the Overworld from now on, and the player is in it.
    g.command('execute in minecraft:the_nether run forceload add 0 0')
    time.sleep(3)
    g.command('execute in minecraft:the_nether run fill -3 100 -3 3 104 3 minecraft:air')
    g.command('execute in minecraft:the_nether run fill -3 99 -3 3 99 3 minecraft:netherrack')
    g.command('execute in minecraft:the_nether run tp @s 0.5 100 0.5')
    time.sleep(3)
    here=g.ctl('block 0 99 0')['block']
    g.check('player_in_nether',here=='minecraft:netherrack',here)
    # The Overworld is parked: the comparator's output (kept in its own block ticks) still reaches the loader.
    time.sleep(5)
    g.check('comparator_still_feeds_the_loader_while_not_last',red(g),state(g,LOADER))
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
    # Persistence only: the state came back with the block entities.
    g.check('reopened_old_machine_state',old is not None and old>=before['old'],{'before':before['old'],'after':old})
    g.check('reopened_new_machine_state',new is not None and new>=before['new'],{'before':before['new'],'after':new})
    # Functional resume: a dead block that only kept its NBT would pass the checks above. Each
    # (full, creative-fed) battery is joined by a red cable to a new electric furnace (a consumer
    # that fills its power buffer), which must strictly charge: only a running machine sends power.
    for source in (OLD,NEW):
        sink=(source[0]+2,source[1],source[2])
        g.command(f'execute in minecraft:overworld run setblock {source[0]+1} {source[1]} {source[2]} ntm:red_cable')
        g.command(f'execute in minecraft:overworld run setblock {sink[0]} {sink[1]} {sink[2]} ntm:machine_electric_furnace_off[facing=north]')
    deadline=time.monotonic()+40
    now={'old':0,'new':0}
    while time.monotonic()<deadline and not all((v or 0)>0 for v in now.values()):
        time.sleep(1)
        now={n:power(g,(s[0]+2,s[1],s[2])) for n,s in (('old',OLD),('new',NEW))}
    g.check('reopened_old_machine_charges_a_new_furnace',(now['old'] or 0)>0,now)
    g.check('reopened_new_machine_charges_a_new_furnace',(now['new'] or 0)>0,now)


def idle(g,reader_on=True):
    """A level with a redstone reader, no player and no forced chunk closes by idleness.
    Closing saves the level, so its region files are written (nothing else writes them here)."""
    nether='minecraft:the_nether'
    g.close()
    g.command('gamemode creative @s')
    g.command('fill 32 99 3 48 99 18 minecraft:stone')
    g.command('fill 32 100 3 48 108 18 minecraft:air')
    g.command(f'execute in {nether} run tp @s 0.5 110 0.5')
    time.sleep(4)
    g.command(f'execute in {nether} run fill -3 99 -3 3 99 3 minecraft:netherrack')
    g.command(f'execute in {nether} run tp @s 0.5 100 0.5')
    reader=(2,100,2)
    # A battery reads the redstone signal of its cell every tick.
    if reader_on:
        g.command(f'execute in {nether} run setblock {reader[0]} {reader[1]} {reader[2]} ntm:machine_battery[facing=north]')
        time.sleep(3)
        g.check('nether_reader_runs',power(g,reader,nether) is not None,state(g,reader,nether))
    else:
        time.sleep(3)
    regions=g.folder/'saves'/'DimsTest'/'dimensions'/'minecraft'/'the_nether'/'region'
    g.command('execute in minecraft:overworld run tp @s 38.5 100 8.5')
    left=time.time()
    # Long-lived particles of the level in place survive the redstone reads of the parked Nether.
    g.command('particle minecraft:campfire_cosy_smoke 38.5 102 8.5 0.3 0.3 0.3 0 60 force')
    time.sleep(1)
    first=g.ctl('status')['particles']
    time.sleep(4)
    later=g.ctl('status')['particles']
    g.evidence['particles']={'first':first,'later':later}
    count=lambda v: v if isinstance(v,int) else (sum(v.values()) if isinstance(v,dict) else len(v))
    g.check('particles_survive_the_parked_reader',count(first)>0 and count(later)>0,{'first':first,'later':later})
    deadline=left+90
    closed=False
    while time.time()<deadline and not closed:
        closed=any(f.stat().st_mtime>left for f in regions.glob('*.mca')) if regions.exists() else False
        time.sleep(2)
    g.check('idle_level_with_reader_closes_and_saves' if reader_on else 'idle_level_without_reader_closes_and_saves',closed,{'waited':round(time.time()-left),'files':sorted(f.name for f in regions.glob('*.mca')) if regions.exists() else []})


if __name__=='__main__':
    mode=sys.argv[2] if len(sys.argv)>2 else 'setup'
    g=Game(sys.argv[1],'dims'+('' if mode=='setup' else '-'+mode))
    g.ctl('wait-ready 120')
    if mode=='setup':setup(g)
    elif mode in ('capture-state','check-state'):persistence(g,mode=='capture-state')
    elif mode=='idle':idle(g)
    elif mode=='idle-clean':idle(g,False)
    else:raise SystemExit('unknown mode: '+mode)
    g.evidence['ok']=True;g.save()

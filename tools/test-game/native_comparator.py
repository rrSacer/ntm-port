#!/usr/bin/env python3
"""Comparators beside a battery, a fluid tank and a barrel read their charge and fill
(`hasComparatorInputOverride`; API 64), and the signal follows them strictly.
A comparator reads the block it faces (west); the redstone wire east of it holds its output as its `power`.
Usage: native_comparator.py GAME
"""
import re
import sys
import time
from native_game import Game

BATTERY=(34,101,6)
SINKS=((33,101,7),(33,101,8),(33,101,9))
SECOND=(34,101,10)
TANK=(40,101,6)
BARREL=(40,101,10)
CREATIVE='ntm:battery_creative'


def wire_power(g,reader):
    """The strength a comparator at `reader` outputs: the `power` of the wire east of it."""
    state=g.ctl('block %d %d %d'%(reader[0]+1,reader[1],reader[2])).get('state','')
    found=re.search(r'power=(\d+)',state)
    return int(found.group(1)) if found else None


def use(g,p):
    """Right-click the machine at `p` from the north, standing two blocks away."""
    g.command('tp @s %.1f 101 %.1f'%(p[0]+.5,p[2]-2.5))
    time.sleep(.5)
    g.ctl('interact %d %d %d north'%p)


def reader_of(p):
    return (p[0]+1,p[1],p[2])


def build(g,p):
    r=reader_of(p)
    g.command('setblock %d %d %d minecraft:stone'%(r[0],r[1]-1,r[2]))
    g.command('setblock %d %d %d minecraft:stone'%(r[0]+1,r[1]-1,r[2]))
    g.command('setblock %d %d %d minecraft:comparator[facing=west]'%r)
    g.command('setblock %d %d %d minecraft:redstone_wire'%(r[0]+1,r[1],r[2]))


def poll(g,reader,seconds,every=2):
    values=[];deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        values.append(wire_power(g,reader));time.sleep(every)
    return values


def setup(g):
    g.close()
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 50 24')
    time.sleep(3)
    g.command('fill 32 99 3 48 108 18 minecraft:air')
    g.command('fill 32 99 3 48 99 18 minecraft:stone')
    g.command('tp @s 38.5 100 8.5')
    time.sleep(2)
    a=BATTERY
    g.command('setblock %d %d %d ntm:machine_battery[facing=south]'%a)
    for z in (a[2]+1,a[2]+2,a[2]+3):g.command('setblock %d %d %d ntm:red_cable'%(a[0],a[1],z))
    for p in (a,TANK,BARREL):
        build(g,p)
    g.command('setblock %d %d %d ntm:machine_fluidtank[facing=north]'%TANK)
    g.command('setblock %d %d %d ntm:barrel_steel[facing=north]'%BARREL)
    time.sleep(3)
    start={name:wire_power(g,reader_of(p)) for name,p in (('battery',a),('tank',TANK),('barrel',BARREL))}
    g.check('empty_machines_read_zero',all(v==0 for v in start.values()),start)

    # A creative battery in the battery fills it at once; the comparator beside it follows.
    g.command('setblock %d %d %d minecraft:hopper[facing=down]'%(a[0],a[1]+1,a[2]))
    g.command('item replace block %d %d %d container.0 with %s 1'%(a[0],a[1]+1,a[2],CREATIVE))
    charged=[]
    deadline=time.monotonic()+60
    while time.monotonic()<deadline:
        charged.append(wire_power(g,reader_of(a)))
        if charged[-1]:break
        time.sleep(1)
    g.evidence['charging']=charged
    g.check('battery_comparator_rises_when_charged',start['battery']==0 and charged[-1] is not None and charged[-1]>0,{'before':start['battery'],'after':charged})

    # The source leaves it (its creative battery goes back to the player) and three electric furnaces join
    # the cable: each fills its own buffer (100 000 HE) from the battery, whose signal falls strictly.
    use(g,a)
    time.sleep(1.5)
    slot=next((s['slot'] for s in g.ctl('screen')['slots'] if s['slot']>=300 and s.get('item')==CREATIVE),None)
    g.evidence['creative_slot']=slot
    if slot is not None:g.ctl('click-slot %d shift'%slot)
    time.sleep(1)
    g.close()
    full=wire_power(g,reader_of(a))
    for p in SINKS:g.command('setblock %d %d %d ntm:machine_electric_furnace_off[facing=north]'%p)
    drained=[full]
    for _ in range(30):
        time.sleep(2)
        drained.append(wire_power(g,reader_of(a)))
    g.evidence['discharging']=drained
    seen=[x for x in drained if x is not None]
    g.check('battery_comparator_falls_while_discharging',seen[-1]<seen[0] and all(y<=x for x,y in zip(seen,seen[1:])),drained)

    # A second battery joins the cable at its end. Two batteries in buffer mode are both providers and
    # receivers of the pooled Java net (`PowerNetMK2`) and settle at once, with equal weights, so the first
    # is put in output mode by a redstone signal (`redHigh`): it only gives, the second takes. The first one's
    # signal keeps falling, the new one's rises from 0.
    g.command('setblock %d %d %d ntm:machine_battery[facing=north]'%SECOND)
    build(g,SECOND)
    time.sleep(2)
    g.command('setblock %d %d %d minecraft:redstone_block'%(a[0]-1,a[1],a[2]))
    first=[wire_power(g,reader_of(a))];second=[wire_power(g,reader_of(SECOND))]
    for _ in range(30):
        time.sleep(2)
        first.append(wire_power(g,reader_of(a)));second.append(wire_power(g,reader_of(SECOND)))
    g.evidence['battery_to_battery']={'first':first,'second':second}
    f=[x for x in first if x is not None];t=[x for x in second if x is not None]
    g.check('second_battery_comparator_rises_from_the_first',t[0]<=1 and t[-1]>=t[0]+5 and all(y>=x for x,y in zip(t,t[1:])),second)
    g.check('first_battery_comparator_falls_into_the_second',f[-1]<=f[0]-5 and all(y<=x for x,y in zip(f,f[1:])),first)

    # A tank fills with water barrels: each one lifts the signal strictly.
    tank_values=[wire_power(g,reader_of(TANK))]
    for _ in range(3):
        g.command('give @s ntm:fluid_barrel_full__water 1')
        time.sleep(1)
        use(g,TANK)
        time.sleep(1.5)
        hot=next(s['slot'] for s in g.ctl('screen')['slots'] if s.get('item')=='ntm:fluid_barrel_full__water' and s['slot']<36)
        g.ctl('click-slot %d'%hot)
        g.ctl('click-slot 302')
        time.sleep(1)
        g.close()
        time.sleep(1.5)
        tank_values.append(wire_power(g,reader_of(TANK)))
    g.evidence['tank']=tank_values
    g.check('tank_comparator_rises_strictly_while_filling',all(x is not None for x in tank_values) and all(x<y for x,y in zip(tank_values,tank_values[1:])),tank_values)

    # A steel barrel is a store too, with its own capacity.
    g.command('give @s ntm:fluid_barrel_full__water 1')
    time.sleep(1)
    use(g,BARREL)
    time.sleep(1.5)
    hot=next(s['slot'] for s in g.ctl('screen')['slots'] if s.get('item')=='ntm:fluid_barrel_full__water' and s['slot']<36)
    g.ctl('click-slot %d'%hot);g.ctl('click-slot 302')
    time.sleep(1);g.close();time.sleep(1.5)
    barrel=wire_power(g,reader_of(BARREL))
    g.check('barrel_comparator_reads_its_fill',barrel is not None and barrel>0,barrel)


def parked_power(g,reader):
    """`wire_power` for the Overworld while the player is elsewhere: the wire's block states are tested by command."""
    wire=(reader[0]+1,reader[1],reader[2])
    for n in range(16):
        text=' '.join(g.command('execute in minecraft:overworld if block %d %d %d minecraft:redstone_wire[power=%d]'%(wire+(n,))).get('chat',[])).lower()
        if 'test passed' in text:return n
    return None


def parked(g):
    """The same battery and comparator in the Overworld while the player is in the Nether: the Overworld is
    parked when the machine's state commits at the end of the tick, and the glue tells the comparator in
    that level (a passing visit) all the same."""
    p=(44,101,6)
    ow=lambda line:g.command('execute in minecraft:overworld run '+line)
    ow('setblock %d %d %d ntm:machine_battery[facing=south]'%p)
    r=reader_of(p)
    ow('setblock %d %d %d minecraft:stone'%(r[0],r[1]-1,r[2]))
    ow('setblock %d %d %d minecraft:stone'%(r[0]+1,r[1]-1,r[2]))
    ow('setblock %d %d %d minecraft:comparator[facing=west]'%r)
    ow('setblock %d %d %d minecraft:redstone_wire'%(r[0]+1,r[1],r[2]))
    time.sleep(2)
    g.command('execute in minecraft:the_nether run forceload add 0 0')
    time.sleep(3)
    g.command('execute in minecraft:the_nether run fill -3 100 -3 3 104 3 minecraft:air')
    g.command('execute in minecraft:the_nether run fill -3 99 -3 3 99 3 minecraft:netherrack')
    g.command('execute in minecraft:the_nether run tp @s 0.5 100 0.5')
    time.sleep(4)
    here=g.ctl('status').get('dimension')
    g.check('player_in_the_nether',here=='minecraft:the_nether',here)
    before=parked_power(g,r)
    ow('setblock %d %d %d minecraft:hopper[facing=down]'%(p[0],p[1]+1,p[2]))
    ow('item replace block %d %d %d container.0 with %s 1'%(p[0],p[1]+1,p[2],CREATIVE))
    time.sleep(6)
    after=parked_power(g,r)
    g.evidence['parked']={'before':before,'after':after}
    g.check('parked_battery_comparator_rises_when_charged',before==0 and after is not None and after>0,{'before':before,'after':after})


if __name__=='__main__':
    g=Game(sys.argv[1],'comparator')
    g.ctl('wait-ready 120')
    setup(g)
    parked(g)
    g.evidence['ok']=True;g.save()

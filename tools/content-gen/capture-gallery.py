#!/usr/bin/env python3
"""Capture reproducible block, state and item evidence from the isolated content client."""
import json,time
from pathlib import Path
from ctl import ctl,base
root=base.parent
checks=[]
def run(command):
 reply=ctl(command);time.sleep(.12)
 if 'error' in reply:raise RuntimeError((command,reply))
 checks.append({'command':command,'reply':reply})
 return reply

def cmd(command):
 reply=run('command '+command)
 if any(x in str(reply.get('chat',[])).lower() for x in ('unknown','incorrect','error')):raise RuntimeError((command,reply))
 return reply

def shot(name):
 run('key f1 tap');time.sleep(.3);run('screenshot '+str(base/name));run('key f1 tap')

rows=json.loads((root/'content/ids.json').read_text())['entries']
cmd('gamemode creative @s');cmd('time set noon');cmd('weather clear')
cmd('fill -14 100 -14 14 100 14 minecraft:stone')
cmd('fill -12 101 -12 12 110 12 minecraft:air')
cmd('tp @s 0.5 101 -7.5')
positions=[]
for i,row in enumerate(r for r in rows if r['type']=='block'):
 x=-10+(i%11)*2;z=(i//11)*3;y=101
 state=row['id']+('[facing=north,honey_level=3]' if row['category']=='machine' else '')
 cmd(f'setblock {x} {y} {z} {state}')
 positions.append({'id':row['id'],'pos':[x,y,z]})
# A tall camera pedestal makes all three rows visible without floating/flying hooks.
cmd('setblock 0 105 -11 minecraft:stone');cmd('tp @s 0.5 106 -10.5')
run('look 0 27');shot('01-block-gallery.png')
# Five machine fronts, off/on with a numeric bar, and all six visual levels of the battery.
cmd('fill -12 101 -2 12 104 10 minecraft:air')
machines=[r for r in rows if r['category']=='machine']
for i,row in enumerate(machines):
 x=-4+2*i
 for y,level in [(101,0),(103,5)]:cmd(f'setblock {x} {y} 0 {row["id"]}[facing=north,honey_level={level}]')
cmd('tp @s 0.5 101 -7.5');run('look 0 -1');shot('02-machines-off-on.png')
cmd('fill -12 101 -2 12 104 10 minecraft:air')
for level in range(6):
 x=-5+2*level;state=f'ntm:machine_battery[facing=north,honey_level={level}]'
 cmd(f'setblock {x} 101 0 {state}')
 assert run(f'block {x} 101 0')['state']==state
cmd('tp @s 0.5 101 -7.5');run('look 0 8');shot('03-battery-levels.png')
# /give real custom items, then open inventory; 4 metadata items verify the separated subtypes.
cmd('clear @s')
sample=['ntm:ingot_uranium','ntm:ingot_th232','ntm:ingot_titanium','ntm:ingot_red_copper','ntm:wire_fine__lead','ntm:wire_fine__red_copper','ntm:nugget_raw__titanium','ntm:plate_cast__titanium','ntm:stamp_iron_plate','ntm:powder_lithium','ntm:sulfur','ntm:niter','ntm:fluorite','ntm:machine_epress','ntm:red_cable']
for item in sample:cmd('give @s '+item+' 1')
inv=run('inventory')
assert all(inv['counts'].get(item,0)==1 for item in sample)
cmd('gamemode survival @s');run('open-inventory');time.sleep(.3);run('screenshot '+str(base/'04-item-inventory.png'));run('close');cmd('gamemode creative @s')
# Verify all orientations as server-written states, and the internal token without creating an item.
for facing in ['north','east','south','west']:
 state=f'ntm:machine_epress[facing={facing},honey_level=4]'
 cmd('setblock 8 101 0 '+state); assert run('block 8 101 0')['state']==state
cmd('setblock 8 101 0 ntm:logic_token[level=8]');assert run('block 8 101 0')['state']=='ntm:logic_token[level=8]'
(base/'gallery.json').write_text(json.dumps({'blocks':positions,'item_sample':sample,'checks':checks},ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'blocks':len(positions),'items_given':len(sample),'commands':len(checks),'screenshots':4}))

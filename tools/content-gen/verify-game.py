#!/usr/bin/env python3
"""Empirical content/template verification against the running isolated muted client."""
import json,time
from pathlib import Path
from ctl import ctl,base
results=[]
def check(label,command,predicate=None):
 reply=ctl(command)
 time.sleep(.25)
 ok=('error' not in reply) if predicate is None else bool(predicate(reply))
 results.append({'label':label,'command':command,'reply':reply,'pass':ok})
 print(label, json.dumps(reply,ensure_ascii=False)[:500],flush=True)
 if not ok: print('FAILED:',label,flush=True)
 return reply

def cmd(command):return check(command,'command '+command,lambda r:r.get('ok') and not any(t in str(r.get('chat',[])).lower() for t in ['unknown','incorrect','error']))

cmd('fill -12 100 -12 12 100 12 minecraft:stone')
cmd('fill -10 101 -10 10 110 10 minecraft:air')
cmd('tp @s 0.5 101 -2.5')
cmd('time set noon');cmd('weather clear');cmd('gamerule spawn_mobs false')
cmd('setblock 0 101 0 ntm:machine_battery[facing=north,honey_level=5]')
check('face battery','look-at block 0 101 0 north')
check('empty hand','select 9')
check('empty interaction','interact 0 101 0 north')
check('no vanilla UI','state',lambda r:r['screen']=='playing' and not r.get('menu'))
expected='ntm:machine_battery[facing=north,honey_level=5]'
for item in ['minecraft:shears','minecraft:glass_bottle','minecraft:honeycomb','minecraft:bone_meal','minecraft:wheat_seeds','minecraft:glowstone','minecraft:flint_and_steel']:
 cmd('item replace entity @s hotbar.0 with '+item)
 check('select '+item,'select 1')
 check('use '+item,'interact 0 101 0 north')
 check('unchanged after '+item,'block 0 101 0',lambda r:r.get('state')==expected)
 check('no menu after '+item,'state',lambda r:r['screen']=='playing' and not r.get('menu'))
 cmd('setblock 0 101 -1 minecraft:air')
# Any fire from flint-and-steel is ordinary held-item use; remove it before the next probe.
cmd('setblock 0 101 -1 minecraft:air')
cmd('setblock 1 101 0 minecraft:redstone_block')
cmd('setblock -1 101 0 minecraft:stone')
cmd('setblock -1 101 0 minecraft:air')
cmd('summon minecraft:bee 0.5 103 0.5')
cmd('tick sprint 1000')
time.sleep(2)
check('neighbors redstone bees and ticks preserve server state','block 0 101 0',lambda r:r.get('state')==expected)
# Full cube collision: fall on it, then try walking through its front.
cmd('setblock 1 101 0 minecraft:air')
cmd('tp @s 0.5 104 0.5')
check('fall to top','wait 30')
check('cube top y=102','state',lambda r:abs(r['player']['position'][1]-102)<.03)
cmd('tp @s 0.5 101 -1.5');check('look south','look 0 0');check('walk into front','move forward 1')
check('cube front collision z<=-0.299','state',lambda r:r['player']['position'][2]<=-.299)
# Block item placement must orient with the player.
cmd('tp @s 3.5 101 -2.5');check('look south placement','look 0 0')
cmd('item replace entity @s hotbar.0 with ntm:machine_battery');check('select battery','select 1')
check('place battery','place 3 101 0')
check('placement facing and zero level','block 3 101 0',lambda r:r.get('state')=='ntm:machine_battery[facing=north,honey_level=0]')
# Survival mining and own loot, using a real tool and item entity/pickup observation.
cmd('gamemode survival @s');cmd('item replace entity @s hotbar.0 with minecraft:netherite_pickaxe');check('pickaxe','select 1')
check('break machine','break 3 101 0',lambda r:bool(r.get('ok')))
check('machine removed','block 3 101 0',lambda r:r.get('block')=='minecraft:air')
check('drop machine','entities 10 minecraft:item')
cmd('tp @s 3.5 101 0.5');check('pickup wait','wait 20')
check('self drop in inventory','inventory',lambda r:'ntm:machine_battery' in json.dumps(r))
cmd('setblock 3 101 0 ntm:ore_sulfur');cmd('tp @s 3.5 101 -2.5')
check('break sulfur','break 3 101 0');cmd('tp @s 3.5 101 0.5');check('pickup sulfur wait','wait 20')
check('sulfur drop in inventory','inventory',lambda r:'ntm:sulfur' in json.dumps(r))
cmd('gamemode creative @s')
write={'checks':len(results),'failed':[r['label'] for r in results if not r['pass']],'results':results}
(base/'verification.json').write_text(json.dumps(write,ensure_ascii=False,indent=2)+'\n')
print('SUMMARY',json.dumps({k:v for k,v in write.items() if k!='results'}),flush=True)

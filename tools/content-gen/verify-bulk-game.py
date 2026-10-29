#!/usr/bin/env python3
"""Sample registry/template/loot/craft checks in the isolated muted content client."""
import json
import socket
import time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];E=ROOT/'content-gen-evidence/bulk'
scratch=Path((E/'run-path.txt').read_text().strip());results=[];failures=[]
def ctl(line):
    with socket.socket(socket.AF_UNIX) as sock:
        sock.settimeout(60);sock.connect(str(scratch/'control.sock'));sock.sendall((line+'\n').encode());reply=json.loads(sock.makefile().readline())
    results.append({'command':line,'reply':reply})
    # Command acknowledgements precede server tick/state replication.
    time.sleep(.2)
    if 'error' in reply:raise RuntimeError((line,reply))
    return reply
def cmd(line):
    reply=ctl('command '+line)
    if not reply.get('ok') or any(word in str(reply.get('chat','')).lower() for word in ['unknown','incorrect','error']):raise RuntimeError((line,reply))
    return reply
def expect(name,condition):
    if not condition:failures.append(name)
    print(name,condition,flush=True)

def craft(table,ingredient,count,slots,output,out_count):
    ctl('close');cmd('clear @s');ctl('give '+ingredient+' '+str(count));ctl('interact '+table);time.sleep(.25)
    menu=ctl('menu');slot=next(s['slot'] for s in menu['slots'] if s['stack']['item']==ingredient and s['slot']<36)
    ctl('click-slot '+str(slot))
    for cell in slots:ctl('click-slot '+str(100+cell)+' right')
    ctl('click-slot '+str(slot));menu=ctl('menu')
    result=next(s['stack'] for s in menu['slots'] if s['slot']==200)
    expect('craft '+output,result.get('item')==output and result.get('count')==out_count)
    ctl('click-slot 200 shift');ctl('close')
    expect('take craft '+output,ctl('inventory')['counts'].get(output,0)==out_count)
try:
    deadline=time.monotonic()+240
    while not (scratch/'control.sock').exists():
        if time.monotonic()>deadline:raise RuntimeError('Client did not create control socket in 240s')
        time.sleep(1)
    for attempt in range(120):
        if ctl('state').get('screen')=='playing':break
        time.sleep(1)
    else:raise RuntimeError('Client has not entered world')
    cmd('gamemode creative @s');cmd('time set noon');cmd('weather clear')
    cmd('fill -14 100 -14 14 100 14 minecraft:stone');cmd('fill -12 101 -12 12 108 12 minecraft:air');cmd('tp @s 0.5 101 -7.5')
    blocks=['ore_nether_plutonium','ore_tikite','ore_meteor__iron','ore_basalt__sulfur','block_steel','block_pu239','concrete','concrete_colored__blue','concrete_colored_ext__machine_stripe','reinforced_glass','concrete_stairs','concrete_slab__concrete','concrete_pillar','deco_steel','nuke_boy','nuke_fstbmb','tnt_ntm','fallout','sellafield_slaked','waste_log','waste_leaves']
    rows={r['id']:r for r in json.loads((ROOT/'content/ids.json').read_text())['entries']}
    positions=[]
    for i,name in enumerate(blocks):
        identity='ntm:'+name
        if identity not in rows:failures.append('missing sample '+identity);continue
        x=-10+(i%11)*2;z=(i//11)*3
        state=identity+('[persistent=true]' if name=='waste_leaves' else '')
        cmd(f'setblock {x} 101 {z} {state}');reply=ctl(f'block {x} 101 {z}');expect('setblock '+name,reply.get('block')==identity);positions.append((x,101,z))
    cmd('setblock 0 105 -11 minecraft:stone');cmd('tp @s 0.5 106 -10.5');ctl('look 0 27');ctl('key f1 tap');ctl('screenshot '+str(E/'01-expanded-blocks.png'));ctl('key f1 tap')
    # Template redstone contract: content only, no bomb logic installed.
    cmd('setblock 10 101 8 ntm:tnt_ntm[lit=false]');cmd('setblock 11 101 8 minecraft:redstone_block');ctl('wait 10')
    powered=ctl('block 10 101 8');expect('bomb lamp inherits redstone powered state','lit=true' in powered.get('state',''))
    cmd('setblock 11 101 8 minecraft:air');ctl('wait 10');unpowered=ctl('block 10 101 8');expect('bomb lamp unpowers','lit=false' in unpowered.get('state',''))
    # Representative material metadata, process grades, machinery parts and equipment.
    sample=['ingot_pu239','powder_schrabidium','bedrock_ore_fragment__uranium','scraps__plutonium','wire_fine__copper','plate_cast__steel','circuit__basic','part_generic__piston_hydraulic','ore_enriched__uranium','bedrock_ore__base_actinide','ams_catalyst_dineutronium','titanium_pickaxe','steel_helmet','boy_bullet','detonator','fallout__item','upgrade_speed_1','blades_steel','nuclear_waste_vitrified','circuit_star_piece__cpu_clock']
    cmd('clear @s')
    actual=[]
    for name in sample:
        identity='ntm:'+name
        if identity not in rows:failures.append('missing sample '+identity);continue
        cmd('give @s '+identity+' 1');actual.append(identity)
    inv=ctl('inventory');expect('all sample items received',all(inv['counts'].get(identity)==1 for identity in actual))
    cmd('gamemode survival @s');ctl('open-inventory');ctl('screenshot '+str(E/'02-expanded-inventory.png'));ctl('close');cmd('gamemode creative @s')
    # Mine a new ore with a real vanilla tool and verify its NTM fragment range.
    cmd('clear @s');cmd('setblock 8 101 -3 ntm:ore_nether_cobalt');cmd('tp @s 8.5 101 -5.5');cmd('gamemode survival @s');cmd('give @s minecraft:netherite_pickaxe');ctl('select 1');ctl('break 8 101 -3');cmd('tp @s 8.5 101 -2.5');ctl('wait 30')
    count=ctl('inventory')['counts'].get('ntm:fragment_cobalt',0);expect('nether cobalt loot 5..12',5<=count<=12)
    cmd('gamemode creative @s');cmd('tp @s 0.5 101 -7.5');cmd('setblock 1 101 -7 minecraft:crafting_table')
    craft('1 101 -7','ntm:ingot_steel',9,list(range(9)),'ntm:block_steel',1)
    craft('1 101 -7','minecraft:brick',2,[0,1],'ntm:plate_polymer',4)
    print('RESULT',len(results),'commands;',len(failures),'failures',flush=True)
finally:
    report={'commands':len(results),'failures':failures,'results':results,'scratch':str(scratch),'audio':'HORIZON_NO_AUDIO=1, temporary master/music=0, tools/test-game/mute.py','screenshots':['01-expanded-blocks.png','02-expanded-inventory.png']}
    (E/'game-validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
if failures:raise SystemExit(1)

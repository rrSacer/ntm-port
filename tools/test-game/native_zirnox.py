#!/usr/bin/env python3
"""Disposable native ZIRNOX acceptance through placement, slots and real fluids.

No reactor state or rod life is injected. /tick sprint advances ordinary server
simulation for the source's 20,000/50,000-life breeding/depletion checks.
Usage: native_zirnox.py GAME setup|inactive|convert|capture-state|check-state|failure
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state
from native_solar_stirling import place


def machine(g,p,dim=None):
    text=(machine_state(g,p) if dim is None else ' '.join(g.command(f'execute in {dim} run data get block {pos_text(p)} ntm').get('chat',[])).replace('\\"','"'))
    return json.JSONDecoder().raw_decode(text[text.index('{'):])[0]


def state(g,p,dim=None):
    return machine(g,p,dim)['data']


def config(g):
    return json.loads((g.folder/'zirnox-config.json').read_text())


def pos_text(p):
    return ' '.join(map(str,p))


def add(p,d):
    return tuple(p[i]+d[i] for i in range(3))


def block(g,p):
    b=g.ctl('block '+pos_text(p))
    return b.get('state',b['block'])


def life(g,p,slot=0,dim=None):
    prefix=f"execute in {dim} run " if dim else ""
    text=' '.join(g.command(prefix+f'data get block {pos_text(p)} Items[{{Slot:{slot}b}}].components."minecraft:custom_data".life').get('chat',[]))
    n=re.search(r'(-?\d+)\s*$',text)
    if not n:
        raise AssertionError('missing life: '+text)
    return int(n.group(1))


def game_time(g):
    text=' '.join(g.command('time query gametime').get('chat',[]))
    n=re.search(r'game time is (\d+)',text)
    if not n:
        raise AssertionError('missing game time: '+text)
    return int(n.group(1))


def sprint(g,n):
    start=game_time(g)
    g.command(f'tick sprint {n}')
    deadline=time.monotonic()+240
    while time.monotonic()<deadline:
        if game_time(g)>=start+n:
            return
        time.sleep(.5)
    raise TimeoutError(f'sprint {n}')


def lever(g,c,on):
    p=tuple(c['lever'])
    if ('powered=true' in block(g,p)) != on:
        g.close();g.command('clear @s')
        g.command(f'tp @s {p[0]+2.5} {p[1]} {p[2]+.5}')
        g.ctl('interact '+pos_text(p)+' east')
        time.sleep(.4)
    g.check('lever_'+str(on)+'_'+str(len(g.evidence['steps'])),('powered=true' in block(g,p))==on,block(g,p))


def open_reactor(g,core):
    g.close();g.command(f'tp @s {core[0]+.5} {core[1]} {core[2]+4.5}')
    p=add(core,(0,1,2));g.ctl('interact '+pos_text(p)+' south');time.sleep(.7)
    g.check('reactor_menu_'+str(len(g.evidence['steps'])),g.ctl('screen').get('screen')=='container',g.ctl('screen'))


def open_tank(g,p):
    g.close();g.command(f'tp @s {p[0]-3.5} {p[1]-1} {p[2]+.5}')
    g.ctl('interact '+pos_text(p)+' west');time.sleep(.7)
    g.check('tank_menu_'+str(len(g.evidence['steps'])),g.ctl('screen').get('screen')=='container',g.ctl('screen'))


def type_ports(g,fluid,positions):
    g.close();g.command('clear @s')
    g.command(f'give @s ntm:fluid_identifier_multi[minecraft:custom_data={{ntm_fluid:"{fluid}"}}] 1')
    g.ctl('select ntm:fluid_identifier_multi')
    for p in positions:
        g.command(f'tp @s {p[0]+.5} {p[1]+2} {p[2]+.5}')
        g.ctl('interact '+pos_text(p)+' up');time.sleep(.2)


def setup(g):
    g.close()
    g.command('gamemode creative @s');g.command('clear @s')
    g.ground(40,101,8,width=30,height=8)
    g.command('forceload add 25 -10 75 30')
    g.command('fill 25 100 -6 70 100 25 minecraft:stone')
    core=tuple(place(g,'machine_zirnox',45,101,8))
    g.check('placed_multiblock',block(g,add(core,(2,1,0))).startswith('ntm:dummy_'),{'core':core,'facing':machine(g,core)['facing']})
    # The four lateral ports are at +/-2 and heights +1/+3. Water uses
    # west lower; steam east lower. Three full-sized source/sink tanks share
    # each bus, with original containers loaded through their screens.
    water=[add(core,(-6,1,z)) for z in (-2,0,2)]
    steam=[add(core,(6,1,z)) for z in (-2,0,2)]
    for p in water+steam:
        g.command('setblock '+pos_text(p)+' ntm:machine_fluidtank[facing=north]')
    wd=[add(core,(-x,1,0)) for x in (3,4,5)] + [add(core,(-5,1,z)) for z in (-2,-1,1,2)]
    sd=[add(core,(x,1,0)) for x in (3,4,5)] + [add(core,(5,1,z)) for z in (-2,-1,1,2)]
    for p in wd+sd:g.command('setblock '+pos_text(p)+' ntm:fluid_duct_neo')
    for p in water:
        open_tank(g,p);g.put('ntm:fluid_barrel_full__water',16,2)
        time.sleep(1)
        g.check('water_tank_filled_'+pos_text(p),state(g,p)['tank']['mb']==256000,state(g,p))
        for _ in range(2):g.button([151,35,18,18],[176,166]);time.sleep(.25)
        g.close()
    open_reactor(g,core);g.check('28_slots',len(g.slots())==28,g.ctl('screen'))
    g.put('ntm:fluid_barrel_full__carbondioxide',1,24)
    g.put('ntm:fluid_barrel_full__water',2,25)
    # Natural uranium in 0 survives long enough for the strict reopen check;
    # Th and Li breed in two four-neighbor central cells, the rest are ZFB.
    kinds={0:'natural_uranium_fuel',8:'th232',11:'lithium'}
    for slot in range(24):g.put('ntm:rod_zirnox__'+kinds.get(slot,'zfb_mox'),1,slot)
    a=state(g,core)
    g.check('co2_loaded',a['co2']['mb']==16000,a)
    g.check('off_until_control',not a['on'] and a['heat']==0,a)
    g.snapshot('zirnox-loaded');g.close()
    type_ports(g,'water',wd)
    type_ports(g,'superhotsteam',sd)
    lp=add(core,(3,0,1))
    g.command('setblock '+pos_text(lp)+' minecraft:lever[face=wall,facing=east,powered=false]')
    c={'core':core,'water':water,'steam':steam,'lever':lp}
    (g.folder/'zirnox-config.json').write_text(json.dumps(c))
    lever(g,c,True);time.sleep(2)
    a=state(g,core);before=life(g,core);time.sleep(1);after=life(g,core)
    g.check('lever_enables_reaction',a['on'] and a['powered'] and after>before,{'state':a,'life':[before,after]})
    open_reactor(g,core);g.button([144,35,14,14],[203,256]);time.sleep(.4)
    g.check('redstone_overrides_gui',state(g,core)['on'],state(g,core));g.close()
    lever(g,c,False);time.sleep(.5)
    before=life(g,core);time.sleep(.8);after=life(g,core)
    g.check('falling_edge_stops',not state(g,core)['on'] and before==after,[before,after])
    open_reactor(g,core);g.button([151,51,36,36],[203,256]);time.sleep(.4)
    g.check('vent_releases_1000',state(g,core)['co2']['mb']==15000,state(g,core))
    g.button([144,35,14,14],[203,256]);time.sleep(.4)
    g.check('gui_manual_control',state(g,core)['on'],state(g,core));g.close()
    lever(g,c,True)
    sprint(g,500)
    stored=sum(state(g,p)['tank']['mb'] for p in steam)
    g.check('steam_enters_real_output_tanks',stored>0,{'steam_mb':stored,'reactor':state(g,core)})
    g.check('water_consumed',sum(state(g,p)['tank']['mb'] for p in water)<3*256000, [state(g,p) for p in water])
    g.snapshot('zirnox-running')


def inactive(g):
    c=config(g);core=c['core'];before=life(g,core)
    g.close()
    g.command('execute in minecraft:the_nether run forceload add 0 0 16 16')
    g.command('execute in minecraft:the_nether run tp @s 8 140 8')
    time.sleep(3)
    current=g.ctl('status')['dimension']
    g.check('player_in_forced_nether',current=='minecraft:the_nether',current)
    sprint(g,500)
    after=life(g,core,dim='minecraft:overworld')
    a=state(g,core,dim='minecraft:overworld')
    g.check('parked_overworld_keeps_reacting_and_writing',after>before and a['on'],{'life_before':before,'life_after':after,'state':a})
    g.command('execute in minecraft:overworld run tp @s '+pos_text(add(core,(0,0,5))))
    g.snapshot('zirnox-parked-dimension')


def convert(g):
    c=config(g);core=tuple(c['core'])
    g.command('tp @s '+pos_text(add(core,(0,1,5))))
    sprint(g,6500)
    open_reactor(g,core)
    items={s['slot']-300:s.get('item') for s in g.slots()}
    g.check('lithium_to_tritium',items.get(11)=='ntm:rod_zirnox_tritium',items)
    g.check('thorium_to_fuel',items.get(8)=='ntm:rod_zirnox__thorium_fuel',items)
    g.close();sprint(g,7000);open_reactor(g,core)
    items={s['slot']-300:s.get('item') for s in g.slots()}
    g.check('zfb_natural_depletion',any(v=='ntm:rod_zirnox_zfb_mox_depleted' for v in items.values()),items)
    g.check('reactor_survives_conversion',not state(g,core)['destroyed'],state(g,core))
    g.snapshot('zirnox-converted');g.close()


def capture(g):
    c=config(g);core=c['core'];a=state(g,core)
    saved={'life':life(g,core),'state':a,'steam':sum(state(g,p)['tank']['mb'] for p in c['steam'])}
    g.check('still_running_at_close',a['on'] and not a['destroyed'],a)
    (g.folder/'zirnox-before-reopen.json').write_text(json.dumps(saved,indent=2))
    g.ctl('save')


def check(g):
    c=config(g);core=c['core'];saved=json.loads((g.folder/'zirnox-before-reopen.json').read_text())
    g.command('tp @s '+pos_text(add(core,(0,0,5))));time.sleep(2)
    a=state(g,core);before=life(g,core)
    g.check('persisted_life_and_tanks',before>=saved['life'] and a['co2']['mb']==saved['state']['co2']['mb'],{'before':saved,'reopened':a,'life':before})
    sprint(g,500);after=life(g,core)
    g.check('strictly_reacts_after_reopen',after>before,{'before':before,'after':after})
    # Move the live rod through the real cursor twice; life must follow it.
    lever(g,c,False);open_reactor(g,core);before=life(g,core)
    empty=next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and not s.get('item'))
    g.ctl('click-slot 300');g.ctl(f'click-slot {empty}');g.ctl(f'click-slot {empty}');g.ctl('click-slot 300')
    g.check('rod_life_survives_manual_moves',life(g,core)==before,{'before':before,'after':life(g,core)})
    import os
    from xinput import XInput
    ui=g.ctl('ui');bounds=next(w['bounds'] for w in ui['widgets'] if w.get('id')=='slot-300')
    scale=g.ctl('status')['display']['width']/ui['gui_size'][0]
    keys=XInput(os.environ['DISPLAY']);keys.focus();keys.pointer((bounds[0]+bounds[2]/2)*scale,(bounds[1]+bounds[3]/2)*scale)
    time.sleep(.5);g.snapshot('zirnox-life-tooltip')
    # The output reservoirs are finite. Leave the proven reactor stopped while
    # building a separate uncooled reactor for the intentional failure phase.
    g.close()
    g.check('controlled_stop_after_reopen',not state(g,core)['on'],state(g,core))
    g.snapshot('zirnox-reopened')


def failure(g):
    g.close()
    g.ground(110,101,8,width=20,height=8)
    core=tuple(place(g,'machine_zirnox',115,101,8))
    markers=[add(core,(x,3,z)) for x,z in [(4,0),(-4,0),(0,4),(0,-4)]]
    for p in markers:g.command('setblock '+pos_text(p)+' minecraft:stone')
    grass=add(core,(15,-1,0));g.command('setblock '+pos_text(grass)+' minecraft:grass_block')
    open_reactor(g,core)
    for slot in range(24):g.put('ntm:rod_zirnox__les_fuel',1,slot)
    g.check('cold_uncooled_loaded',state(g,core)['heat']==0,state(g,core))
    g.button([144,35,14,14],[203,256]);g.close();time.sleep(2)
    b=block(g,core);a=state(g,core)
    g.check('destroyed_reactor_replaces_core',b.startswith('ntm:zirnox_destroyed') and a['destroyed'],{'block':b,'state':a})
    remaining=[block(g,p) for p in markers]
    g.check('native_explosion_breaks_surroundings',any(x=='minecraft:air' for x in remaining),remaining)
    sprint(g,120)
    g.check('waste_transforms_grass',block(g,grass)=='ntm:waste_earth',block(g,grass))
    g.ctl('save')
    worlds=list(g.folder.glob('saves/*/data/ntm/world'))
    world=json.loads(worlds[0].read_text());debris=world['zirnox']['debris']
    g.check('simulated_debris_alive',0<len(debris)<=72,{'count':len(debris),'kinds':sorted(set(d['kind'] for d in debris))})
    g.command(f'tp @s {core[0]+18} {core[1]+8} {core[2]+18} 135 20')
    g.snapshot('zirnox-failure')


PHASES={'setup':setup,'inactive':inactive,'convert':convert,'capture-state':capture,'check-state':check,'failure':failure}
if __name__=='__main__':
    g=Game(sys.argv[1],'zirnox-'+sys.argv[2]);g.ctl('wait-ready 120')
    g.evidence['phase']=sys.argv[2]
    PHASES[sys.argv[2]](g)
    g.evidence['ok']=True;g.save()
    print(g.output)

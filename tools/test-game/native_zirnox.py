#!/usr/bin/env python3
"""Disposable native ZIRNOX acceptance through placement, slots and real fluids.

No reactor state or rod life is injected. /tick sprint advances ordinary server
simulation for the source's 20,000/50,000-life breeding/depletion checks.
Usage: native_zirnox.py GAME setup|inactive|convert|capture-state|check-state|automation|failure|soils
"""
import json
import os
import re
import sys
import time
from collections import Counter
from native_game import Game
import ntm_store
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


def menu_open(g,timeout=20):
    """The screen once a menu is open (coordinator, 10/11: wait on the state, not a fixed 0.7 s; a loaded
    machine answers the click late), or the last screen seen when `timeout` seconds pass without one."""
    deadline=time.monotonic()+timeout
    while True:
        # A deliberate failure for the mutation run (a menu that never opens); never set otherwise.
        if os.environ.get('NTM_ZIRNOX_MUTANT')=='menu_never':g.close()
        screen=g.ctl('screen')
        if screen.get('screen')=='container' or time.monotonic()>=deadline:return screen
        time.sleep(.1)


def open_reactor(g,core):
    g.close();g.command(f'tp @s {core[0]+.5} {core[1]} {core[2]+4.5}')
    p=add(core,(0,1,2));g.ctl('interact '+pos_text(p)+' south');screen=menu_open(g)
    g.check('reactor_menu_'+str(len(g.evidence['steps'])),screen.get('screen')=='container',screen)


def open_tank(g,p):
    g.close();g.command(f'tp @s {p[0]-3.5} {p[1]-1} {p[2]+.5}')
    g.ctl('interact '+pos_text(p)+' west');screen=menu_open(g)
    g.check('tank_menu_'+str(len(g.evidence['steps'])),screen.get('screen')=='container',screen)


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
    # Java scans this bounding-box corner even though it is outside the casing.
    corner=add(core,(2,3,2));upper_lever=add(core,(3,3,2))
    g.command('setblock '+pos_text(corner)+' minecraft:stone')
    g.command('setblock '+pos_text(upper_lever)+' minecraft:lever[face=wall,facing=east,powered=true]')
    g.check('powered_box_corner_outside_casing',state(g,core)['on'] and state(g,core)['powered'],state(g,core))
    g.command('setblock '+pos_text(upper_lever)+' minecraft:lever[face=wall,facing=east,powered=false]')
    g.check('box_corner_falling_edge',not state(g,core)['on'],state(g,core))
    for p in (upper_lever,corner):g.command('setblock '+pos_text(p)+' minecraft:air')
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


def manual_non_fuel(g,core):
    """ContainerReactorZirnox builds the rod and fluid-input slots as vanilla `Slot`s: a hand places any item there."""
    g.close();g.command('give @s minecraft:dirt 1');time.sleep(1);open_reactor(g,core)
    dirt=next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and s.get('item')=='minecraft:dirt')
    def cell(slot):return next(s for s in g.ctl('screen')['slots'] if s['slot']==slot).get('item')
    def settle(want_slot,want_item,want_carried):
        deadline=time.monotonic()+10
        while True:
            carried=(g.ctl('screen').get('carried') or {}).get('item')
            if (cell(want_slot),carried)==(want_item,want_carried) or time.monotonic()>deadline:return cell(want_slot),carried
            time.sleep(.1)
    for name,slot in [('rod',300),('co2_input',324),('water_input',325)]:
        held=cell(slot)
        g.ctl(f'click-slot {dirt}');settle(dirt,None,'minecraft:dirt')
        # The cursor's dirt takes the slot's place (swap, or a plain put if it is empty).
        g.ctl(f'click-slot {slot}');placed=settle(slot,'minecraft:dirt',held)
        g.check('manual_non_fuel_enters_'+name+'_slot',placed==('minecraft:dirt',held),placed)
        # And the original item goes back, bringing the dirt to the cursor again.
        g.ctl(f'click-slot {slot}');back=settle(slot,held,'minecraft:dirt')
        g.check('manual_original_returns_to_'+name+'_slot',back==(held,'minecraft:dirt'),back)
        g.ctl(f'click-slot {dirt}');home=settle(dirt,'minecraft:dirt',None)
        g.check('manual_non_fuel_returns_from_'+name,home==('minecraft:dirt',None),home)
    # 1.7.10's `mergeItemStack(24, 26)` reaches the take-only slot 26 too: with slot 24 taken, a carbon
    # dioxide container moved by shift-click overflows into it (and the water one into 27 after 25).
    barrel='ntm:fluid_barrel_full__carbondioxide'
    taken=cell(324)
    if taken is None:
        g.ctl(f'click-slot {dirt}');settle(dirt,None,'minecraft:dirt')
        g.ctl('click-slot 324');settle(324,'minecraft:dirt',None)
    g.command('give @s '+barrel+' 1');time.sleep(1)
    spare=next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and s.get('item')==barrel)
    if cell(326) is not None:
        # The converted reactor left an empty barrel in the take-only slot: take it out first.
        g.ctl('click-slot 326 shift')
        deadline=time.monotonic()+8
        while time.monotonic()<deadline and cell(326) is not None:time.sleep(.2)
    before=cell(326)
    g.ctl(f'click-slot {spare} shift')
    deadline=time.monotonic()+8
    while time.monotonic()<deadline and cell(326)==before:time.sleep(.2)
    g.check('shift_click_overflows_into_the_take_only_slot',before is None and cell(326)==barrel,{'before':before,'after':cell(326),'slot_24':cell(324)})
    g.ctl('click-slot 326 shift')
    deadline=time.monotonic()+8
    while time.monotonic()<deadline and cell(326) is not None:time.sleep(.2)
    if taken is None:
        g.ctl('click-slot 324');settle(324,None,'minecraft:dirt')
        g.ctl(f'click-slot {dirt}');settle(dirt,'minecraft:dirt',None)
    # `mergeItemStack(0, 24, true)`: a rod moved in by shift-click takes the LAST free rod slot. Two rods
    # go out to the inventory (slots 300 and 301 are free); one shift-clicked back lands in 301, not 300.
    rods=[cell(300),cell(301)]
    for rod_slot in (300,301):
        g.ctl(f'click-slot {rod_slot} shift')
        deadline=time.monotonic()+8
        while time.monotonic()<deadline and cell(rod_slot) is not None:time.sleep(.2)
    def inventory_rod(item,skip=()):
        return next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and s['slot'] not in skip and s.get('item')==item)
    first=inventory_rod(rods[0])
    g.ctl(f'click-slot {first} shift')
    deadline=time.monotonic()+8
    while time.monotonic()<deadline and cell(301) is None and cell(300) is None:time.sleep(.2)
    landed={300:cell(300),301:cell(301)}
    g.check('shift_click_fills_the_rod_slots_back_to_front',landed[301] is not None and landed[300] is None,{'landed':landed,'moved_out':rods})
    # Put the rods back where they were: the one in 301 goes to 300, the other (in the inventory) to 301.
    g.ctl('click-slot 301');settle(301,None,landed[301])
    g.ctl('click-slot 300');settle(300,landed[301],None)
    other=inventory_rod(rods[1])
    g.ctl(f'click-slot {other}');settle(other,None,rods[1])
    g.ctl('click-slot 301');settle(301,rods[1],None)
    g.check('rods_are_back_in_their_slots',(cell(300),cell(301))==(rods[0],rods[1]),{'slots':(cell(300),cell(301)),'expected':rods})


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
    # click-slot queues a server action; wait for both cursor and destination
    # acknowledgement before the next move, rather than racing stale UI slots.
    rod = 'ntm:rod_zirnox__natural_uranium_fuel'
    for source, destination, carried in [(300,300,True),(empty,empty,False),(empty,empty,True),(300,300,False)]:
        g.ctl(f'click-slot {source}')
        deadline=time.monotonic()+10
        while True:
            screen=g.ctl('screen')
            cursor=screen.get('carried')
            cell=next(s for s in screen['slots'] if s['slot']==destination)
            accepted=(cursor is not None and cursor.get('item')==rod and not cell.get('item')) if carried else (not cursor and cell.get('item')==rod)
            if accepted or time.monotonic()>deadline:break
            time.sleep(.1)
        g.check('cursor_move_ack_'+str(len(g.evidence['steps'])),accepted,{'cursor':cursor,'slot':cell})
    g.check('rod_life_survives_manual_moves',life(g,core)==before,{'before':before,'after':life(g,core)})
    manual_non_fuel(g,core)
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



def automation(g):
    """A real hopper drains naturally converted rods and leaves active fuel intact."""
    c=config(g);core=tuple(c['core'])
    lever(g,c,False)
    g.check('automation_reactor_stopped',not state(g,core)['on'],state(g,core))
    open_reactor(g,core)
    before={s['slot']-300:(s.get('item'),s.get('count',0)) for s in g.slots()}
    active={i:v for i,v in before.items() if i<24 and (v[0] or '').startswith('ntm:rod_zirnox__')}
    converted=Counter()
    for i,(item,count) in before.items():
        if i<24 and item and i not in active:converted[item]+=count
    g.check('automation_has_active_and_naturally_converted_rods',bool(active) and bool(converted),
            {'active':active,'converted':dict(converted)})
    g.close()
    hopper=add(core,(0,-1,0))
    # This is below the reactor's base, not one of its structural cells.
    low=add(core,(-1,-3,-1));high=add(core,(1,-1,2))
    g.command('fill '+pos_text(add(core,(-1,-4,-1)))+' '+pos_text(add(core,(1,-4,3)))+' minecraft:stone')
    g.command('fill '+pos_text(low)+' '+pos_text(high)+' minecraft:air')
    g.command('setblock '+pos_text(hopper)+' minecraft:hopper[facing=down]')
    # A hopper moves one item per eight ticks. Give all 24 slots ample time, counted in game ticks (the
    # 12 s it was, at 20 ticks a second).
    start=game_time(g)
    while game_time(g)<start+240:time.sleep(.2)
    g.command('clear @s')
    g.command(f'tp @s {hopper[0]+.5} {hopper[1]-2} {hopper[2]+2.5}')
    g.ctl('interact '+pos_text(hopper)+' south');menu_open(g)
    g.check('automation_hopper_menu',len(g.slots())==5,g.ctl('screen'))
    received=Counter()
    for s in g.slots():
        if s.get('item'):received[s['item']]+=s.get('count',0)
    g.check('automation_only_converted_rods_extracted',received==converted,
            {'expected':dict(converted),'received':dict(received)})
    g.close();open_reactor(g,core)
    after={s['slot']-300:(s.get('item'),s.get('count',0)) for s in g.slots()}
    g.check('automation_active_rods_and_container_slots_unchanged',
            all(after.get(i)==v for i,v in before.items() if i in active or i>=24),
            {'before':before,'after':after})
    remaining=Counter()
    for i,(item,count) in after.items():
        if i<24 and item:remaining[item]+=count
    original=Counter()
    for i,(item,count) in before.items():
        if i<24 and item:original[item]+=count
    g.check('automation_rod_conservation',remaining+received==original,
            {'before':dict(original),'reactor':dict(remaining),'hopper':dict(received)})
    g.close()
    top=add(core,(0,5,0));nonport=add(core,(1,5,0))
    for p in (top,nonport):g.command('setblock '+pos_text(p)+' minecraft:hopper[facing=down]')
    added='ntm:rod_zirnox__u235_fuel'
    for p in (top,nonport):
        g.command('item replace block '+pos_text(p)+' container.0 with '+added+' 1')
    time.sleep(2)
    open_reactor(g,core)
    final={s['slot']-300:(s.get('item'),s.get('count',0)) for s in g.slots()}
    new_count=sum(v[1] for i,v in final.items() if i<24 and v[0]==added)
    g.check('automation_top_inventory_port_inserts_into_core',new_count==1,final)
    g.close()
    nonport_data=' '.join(g.command('data get block '+pos_text(nonport)+' Items[{Slot:0b}].count').get('chat',[]))
    g.check('automation_non_port_keeps_its_supplied_rod',re.search(r'\b1\s*$',nonport_data) is not None,nonport_data)
    # One supplied rod entered; the other remains in its hopper. The lower
    # hopper must not extract the newly inserted active fuel.
    g.check('automation_port_conservation',sum(v[1] for i,v in final.items() if i<24 and v[0])+sum(received.values())+1==sum(original.values())+2,
            {'reactor':final,'extracted':dict(received),'nonport':1,'supplied':2})
    g.snapshot('zirnox-automation');g.close()


def failure(g):
    g.close()
    g.ground(110,101,8,width=20,height=8)
    # Keep the diagnostic and full scene in the same placement frame. A fresh
    # client otherwise retains its spawn yaw and places the core on the other side.
    g.command('tp @s 115.5 101 11.5 180 0')
    core=tuple(place(g,'machine_zirnox',115,101,8))
    g.check('failure_core_frame',core==(115,101,6),{'core':core,'facing':machine(g,core)['facing']})
    markers=[add(core,(x,3,z)) for x,z in [(4,0),(-4,0),(0,4),(0,-4)]]
    for p in markers:g.command('setblock '+pos_text(p)+' minecraft:stone')
    # Waste rules are independent of the physical debris trajectories. Shield
    # these samples beyond the blast so a random exchanger cannot remove them.
    g.command('fill '+pos_text(add(core,(18,-3,-8)))+' '+pos_text(add(core,(20,12,12)))+' minecraft:obsidian')
    grass=add(core,(24,-1,0));g.command('setblock '+pos_text(grass)+' minecraft:grass_block')
    wood_cases=[('oak_fence','minecraft:oak_fence'),('chest','minecraft:chest'),('oak_log','ntm:waste_log'),('oak_planks','ntm:waste_planks')]
    for i,(name,_) in enumerate(wood_cases):
        g.command('setblock '+pos_text(add(core,(24,0,i+1)))+' minecraft:'+name)
    open_reactor(g,core)
    for slot in range(24):g.put('ntm:rod_zirnox__les_fuel',1,slot)
    g.check('cold_uncooled_loaded',state(g,core)['heat']==0,state(g,core))
    def remnants():
        g.ctl('save')
        return ntm_store.value(g.folder/'saves')['zirnox']
    baseline=remnants()
    g.button([144,35,14,14],[203,256]);g.close();time.sleep(2)
    b=block(g,core);a=state(g,core)
    g.check('destroyed_reactor_replaces_core',b.startswith('ntm:zirnox_destroyed') and a['destroyed'],{'block':b,'state':a})
    remaining=[block(g,p) for p in markers]
    g.check('native_explosion_breaks_surroundings',any(x=='minecraft:air' for x in remaining),remaining)
    sprint(g,120)
    g.check('waste_transforms_grass',block(g,grass)=='ntm:waste_earth',block(g,grass))
    for i,(name,expected) in enumerate(wood_cases):
        actual=block(g,add(core,(24,0,i+1))).split('[')[0]
        g.check('waste_wood_'+name,actual==expected,actual)
    after=remnants()
    created=[a-b for a,b in zip(after['debris_created'],baseline.get('debris_created',[0]*6))]
    expected=[20,10,10,10,20,2]
    g.check('debris_created_exactly_by_type',created==expected,{'baseline':baseline.get('debris_created',[0]*6),'created_delta':created})
    before_counts=Counter(d['kind'] for d in baseline['debris'])
    after_counts=Counter(d['kind'] for d in after['debris'])
    survivor_delta=[after_counts[k]-before_counts[k] for k in range(6)]
    g.check('non_destructive_debris_survive_by_type',survivor_delta[:4]==expected[:4],{'survivor_delta':survivor_delta,'created_delta':created})
    g.command(f'tp @s {core[0]+18} {core[1]+8} {core[2]+18} 135 20')
    g.snapshot('zirnox-failure')


def soils(g):
    g.close();g.command('clear @s')
    g.ground(160,101,8,width=20,height=12)
    for x,soil in [(160,'waste_earth'),(170,'waste_mycelium')]:
        g.command(f'fill {x} 101 8 {x+7} 101 15 ntm:{soil}')
        g.command(f'fill {x} 102 8 {x+7} 102 15 minecraft:stone')
    g.command('gamerule minecraft:random_tick_speed 512')
    rule=' '.join(g.command('gamerule minecraft:random_tick_speed').get('chat',[]))
    g.check('random_tick_speed_is_512',re.search(r'\b512\b',rule) is not None,rule)
    g.command('tp @s 160.5 104 11.5')
    sprint(g,120)
    for x,soil in [(160,'waste_earth'),(170,'waste_mycelium')]:
        samples=[block(g,(x+i,101,8)) for i in range(8)]
        g.check('covered_dark_'+soil+'_decays',all(b=='minecraft:dirt' for b in samples),samples)
    g.command('gamerule minecraft:random_tick_speed 3')
    g.snapshot('zirnox-soils')


PHASES={'setup':setup,'inactive':inactive,'convert':convert,'capture-state':capture,'check-state':check,'failure':failure,'soils':soils,'automation':automation}
if __name__=='__main__':
    g=Game(sys.argv[1],'zirnox-'+sys.argv[2]);g.ctl('wait-ready 120')
    g.evidence['phase']=sys.argv[2]
    PHASES[sys.argv[2]](g)
    g.evidence['ok']=True;g.save()
    print(g.output)

#!/usr/bin/env python3
"""MK2 pile acceptance on a disposable native world, with real tool/rod/fluid use.
No machine state is injected. capture-state/check-state check geometry, inventory and continuing numeric state
across a clean client restart (including heat, flux, rod depletion and devices).
"""
import json
import re
import sys
import time
from native_game import Game
from native_combustion import machine_state
from native_stations import fill_barrel, type_duct

CORE=(40,102,8)
LOADERS=[(39,102,7),(41,102,7)]
CONTROLS=[(40,105,9),(40,105,11)]
VENT=(37,101,10)
BARREL=(34,101,10)


def machine(g,p):
    text=machine_state(g,p)
    return json.JSONDecoder().raw_decode(text[text.index('{'):])[0]


def pile(g):
    return machine(g,CORE)['data']['Core']


def fuel(c,index):
    return [v for v in c['channels'] if v['kind']=='Fuel'][index]


def interact(g,p,face,standing=None):
    if standing:
        g.command('tp @s '+' '.join(map(str,standing)))
    g.ctl('interact '+' '.join(map(str,p))+' '+face)
    time.sleep(.25)


def tool(g):
    g.command('clear @s')
    g.command('give @s ntm:hand_drill_desh 1')
    g.ctl('select ntm:hand_drill_desh')


def block(g,p):
    return g.ctl('block '+' '.join(map(str,p)))['block']


def lever(g,p,on):
    # A real lever on the wall block west of p's west neighbour: it feeds the cell next to it.
    g.command(f'setblock {p[0]-1} {p[1]} {p[2]} minecraft:lever[face=wall,facing=east,powered={"true" if on else "false"}]')


def load(g,p,kind):
    g.command('clear @s')
    g.command('give @s ntm:pile_rod__'+kind+' 1')
    g.ctl('select ntm:pile_rod__'+kind)
    interact(g,p,'north',(p[0]+.5,101,p[2]-2.5))
    pending=machine(g,p)['data']['Loader']
    g.check('rod_in_loader_'+kind+'_'+str(len(g.evidence['steps'])),pending['slots'][0]['count']==1,pending)
    interact(g,p,'north')
    time.sleep(1.6)


def hand_damage(g):
    """The damage of the stack in the selected hotbar slot (`minecraft:damage`)."""
    text=' '.join(g.command('data get entity @s SelectedItem.components."minecraft:damage"').get('chat',[]))
    found=re.findall(r'-?\d+',text)
    return int(found[-1]) if found else 0


def formation_wear(g):
    """A plain drill forming a pile in survival: the pile, its blocks' first state and exactly one
    point of wear arrive together (the creative player of the rest of the scenario wears nothing)."""
    second=(46,102,8)
    g.command('gamemode survival @s')
    g.command('fill 44 99 3 48 99 18 minecraft:stone')
    g.command('fill 44 100 8 48 104 12 ntm:pile_brick')
    g.command('clear @s');g.command('give @s ntm:hand_drill 1');g.ctl('select ntm:hand_drill')
    before=hand_damage(g)
    interact(g,second,'north',(46.5,101,5.5))
    formed=machine(g,second)['data']['Core']
    after=hand_damage(g)
    g.check('formation_wears_the_drill_once',after-before==1,{'before':before,'after':after})
    g.check('formation_cells_hold_their_first_state',formed['shape']['at']==list(second) and machine(g,(46,100,8))['data'] is not None,formed['shape'])
    g.command('gamemode creative @s')
    g.command('fill 44 100 8 48 104 12 minecraft:air')
    time.sleep(2)


def setup(g):
    g.close()
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 50 24')
    time.sleep(3)
    g.command('fill 32 99 3 48 108 18 minecraft:air')
    g.command('fill 32 99 3 48 99 18 minecraft:stone')
    formation_wear(g)
    g.command('fill 38 100 8 42 104 12 ntm:pile_brick')
    tool(g)
    interact(g,CORE,'north',(40.5,101,5.5))
    formed=pile(g)
    g.check('formed_shape',formed['shape']=={'at':list(CORE),'inward':[0,0,1],'up':2,'down':2,'left':2,'right':2,'depth':5},formed['shape'])
    # Two stone ledges beside the pile top: a creative player teleported into air falls.
    g.command('fill 39 104 6 41 104 7 minecraft:stone');g.command('fill 39 104 13 41 104 14 minecraft:stone')
    # Fuel, ventilation, and two independent top-down controls. No intersection.
    for p,face,standing in [((39,102,8),'north',(39.5,101,5.5)),((41,102,8),'north',(41.5,101,5.5)),((38,101,10),'west',(35.5,100,10.5)),((40,104,9),'up',(40.5,105,7.5)),((40,104,11),'up',(40.5,105,13.5))]:
        interact(g,p,face,standing)
    c=pile(g)
    g.check('five_channels',len(c['channels'])==5,[x['kind'] for x in c['channels']])
    # Actual block placement: the placement hook retains clicked-face IO.
    for p in LOADERS:
        g.command('clear @s');g.command('give @s ntm:pile_device__loader 1')
        g.command(f'tp @s {p[0]+.5} 101 {p[2]-2.5}')
        g.ctl(f'place {p[0]} {p[1]} {p[2]} --item ntm:pile_device__loader')
    g.command(f'setblock {VENT[0]} {VENT[1]} {VENT[2]} ntm:pile_device__vent[facing=west]')
    for p in CONTROLS:
        g.command('setblock '+' '.join(map(str,p))+' ntm:pile_device__control[facing=west]')
        # The control reads the signal its own cell receives: a lever against it.
        g.command(f'setblock {p[0]-2} {p[1]} {p[2]} minecraft:stone')
        lever(g,p,True)
    # A full original compressed-air barrel (packs are not pourable), sent through AIR ducts.
    g.command(f'setblock {BARREL[0]} {BARREL[1]} {BARREL[2]} ntm:barrel_steel[facing=north]')
    for x in (35,36):g.command(f'setblock {x} 101 10 ntm:fluid_duct_neo')
    type_duct(g,'air',[(35,101,10),(36,101,10)],(34,100,7))
    g.command('clear @s');g.command('tp @s 34.5 100 7.5')
    fill_barrel(g,BARREL,1,'ntm:fluid_barrel_full__air')
    time.sleep(4)
    c=pile(g)
    g.check('compressed_air_reaches_channel',next(x for x in c['channels'] if x['kind']=='Ventilation')['air']>0,{'vent':machine(g,VENT)['data'],'barrel':machine(g,BARREL)['data'],'channels':c['channels']})
    g.check('controls_extracted',all(x['control']>.99 for x in c['channels'] if x['kind']=='Control'),c['channels'])
    for _ in range(5):load(g,LOADERS[0],'ra226be')
    for _ in range(5):load(g,LOADERS[1],'pu239')
    c=pile(g)
    g.check('real_rods_loaded',all(len([r for r in fuel(c,i)['rods'] if r])==5 for i in (0,1)),c['channels'])
    g.check('reactor_heats',fuel(c,1)['heat']>20,c['channels'])
    # Close the controls and measure the loss of cross-segment input.
    before=fuel(c,1)['incoming']
    for p in CONTROLS:lever(g,p,False)
    time.sleep(5)
    closed=pile(g)
    after=fuel(closed,1)['incoming']
    g.check('control_reduces_reaction',after < before*.8,{'open':before,'closed':after})
    for p in CONTROLS:lever(g,p,True)
    # Follow the natural 500-neutron lifetime; no NBT/state shortcut.
    deadline=time.monotonic()+150
    while time.monotonic()<deadline:
        c=pile(g)
        if any(r and r['kind']==5 for r in fuel(c,1)['rods']):break
        time.sleep(2)
    g.check('pu239_transmuted_to_rgp',any(r and r['kind']==5 for r in fuel(c,1)['rods']),fuel(c,1))
    g.evidence['transmutation']=c
    # Ventilation response: stop supply and wait for the residual channel air
    # to dissipate before comparing heating slopes, then reconnect.
    g.command('setblock 36 101 10 minecraft:air')
    # Insert the existing controls while residual vent air dissipates; an
    # uncontrolled pile can cross 800 before its last air reaches zero.
    for p in CONTROLS:lever(g,p,False)
    # Prepare the real identifier before heating so reconnecting does not
    # depend on the latency of typing/giving an item near the failure limit.
    g.command('clear @s')
    g.command('give @s ntm:fluid_identifier_multi[minecraft:custom_data={ntm_fluid:"air"}] 1')
    g.ctl('select ntm:fluid_identifier_multi')
    g.command('tp @s 34.5 100 7.5')
    started=time.monotonic();deadline=started+120
    while True:
        c=pile(g)
        air=next(x for x in c['channels'] if x['kind']=='Ventilation')['air']
        heat=fuel(c,1)['heat']
        if air==0 or time.monotonic()>=deadline:break
        time.sleep(.25)
    g.check('unventilated_zero_air_precondition',air==0,
            {'air':air,'heat':heat,'waited':round(time.monotonic()-started,2),'timeout':120})
    for p in CONTROLS:lever(g,p,True)
    a=fuel(pile(g),1)['heat']
    started=time.monotonic();deadline=started+120
    while True:
        c=pile(g);b=fuel(c,1)['heat']
        air=next(x for x in c['channels'] if x['kind']=='Ventilation')['air']
        if b>=150 or time.monotonic()>=deadline:break
        time.sleep(.1)
    g.check('unventilated_hot_precondition',air==0 and 150<=b<=400,
            {'air':air,'heat':b,'required_window':[150,400],'failure_above':800,'waited':round(time.monotonic()-started,2),'timeout':120})
    g.command('setblock 36 101 10 ntm:fluid_duct_neo')
    g.ctl('interact 36 101 10')
    deadline=time.monotonic()+60
    while True:
        c=pile(g);h1=fuel(c,1)['heat']
        air=next(x for x in c['channels'] if x['kind']=='Ventilation')['air']
        if h1>400 or air>=500 or time.monotonic()>=deadline:break
        time.sleep(.1)
    g.check('air_returns_before_upper_heat_bound',air>=500 and h1<=400,
            {'air':air,'heat':h1,'upper_bound':400,'timeout':60})
    time.sleep(8);d=fuel(pile(g),1)['heat']
    g.check('vent_cools_heated_channel',d < h1 and b>a,{'no_air_before':a,'no_air_after':b,'air_back':h1,'air_later':d})
    # A lever against the second loader starts the stroke (rising edge on its own cell).
    g.command('clear @s');g.command('give @s ntm:pile_rod__nu 1');g.ctl('select ntm:pile_rod__nu')
    interact(g,LOADERS[1],'north',(LOADERS[1][0]+.5,101,LOADERS[1][2]-2.5))
    g.check('lever_loader_rod_waiting',machine(g,LOADERS[1])['data']['Loader']['slots'][0]['count']==1,machine(g,LOADERS[1])['data'])
    g.command(f'setblock {LOADERS[1][0]-1} {LOADERS[1][1]} {LOADERS[1][2]} minecraft:lever[face=wall,facing=north,powered=false]')
    time.sleep(2)
    g.check('lever_off_does_not_load',fuel(pile(g),1)['rods'][0]['kind']!=3,fuel(pile(g),1)['rods'])
    g.command(f'setblock {LOADERS[1][0]-1} {LOADERS[1][1]} {LOADERS[1][2]} minecraft:lever[face=wall,facing=north,powered=true]')
    time.sleep(3)
    c=pile(g)
    g.check('lever_loads_rod',fuel(c,1)['rods'][0]['kind']==3 and machine(g,LOADERS[1])['data']['Loader']['slots'][0] is None,{'rods':fuel(c,1)['rods'],'loader':machine(g,LOADERS[1])['data']})
    g.command(f'setblock {LOADERS[1][0]-1} {LOADERS[1][1]} {LOADERS[1][2]} minecraft:air')
    # Undo a fuel channel using the drill through its loader: real item ejection.
    tool(g)
    interact(g,LOADERS[0],'north',(39.5,101,4.5))
    g.check('loader_drill_undo',len([x for x in pile(g)['channels'] if x['kind']=='Fuel'])==1,pile(g)['channels'])
    g.evidence['core']=CORE
    g.evidence['positions']=[CORE,*LOADERS,*CONTROLS,VENT,BARREL]
    g.snapshot('pile')


def persistence(g,capture):
    proof=json.loads((g.folder/'native-pile.json').read_text())
    positions=proof['positions']
    # Freeze each process for a coherent read; a reopened server can advance
    # before this command, so verify continuity instead of byte equality.
    g.command('tick freeze')
    if capture:
        time.sleep(.5)
        expected={','.join(map(str,p)):machine(g,p) for p in positions}
        (g.folder/'native-pile-state.json').write_text(json.dumps(expected,indent=2)+'\n')
        g.ctl('save')
    else:
        expected=json.loads((g.folder/'native-pile-state.json').read_text())
        for p in positions:
            key=','.join(map(str,p));actual=machine(g,p)
            old=expected[key]
            g.check('saved_identity_'+key,actual['block']==old['block'],actual)
            if tuple(p)==CORE:
                before=old['data']['Core'];after=actual['data']['Core']
                g.check('saved_geometry',after['shape']==before['shape'],after['shape'])
                g.check('saved_channels',[(c['entry'],c['inward'],c['kind']) for c in after['channels']]==[(c['entry'],c['inward'],c['kind']) for c in before['channels']],after['channels'])
                for i,(a,b) in enumerate(zip(before['channels'],after['channels'])):
                    g.check('saved_rods_'+str(i),[(r['kind'] if r else None) for r in a['rods']]==[(r['kind'] if r else None) for r in b['rods']],b['rods'])
                    if a['kind']=='Fuel':
                        g.check('saved_nonzero_heat_'+str(i),b['heat']>=20 and (a['heat']<=20 or b['heat']>20),[a['heat'],b['heat']])
                        g.check('saved_nonzero_flux_'+str(i),a['incoming']==0 or b['incoming']>0,[a['incoming'],b['incoming']])
                        g.check('saved_depletion_'+str(i),all(rb is None or rb['depletion']>=ra['depletion'] for ra,rb in zip(a['rods'],b['rods'])),b['rods'])
                    elif a['kind']=='Control':g.check('saved_control_'+str(i),a['control']==b['control'],b['control'])
                    else:g.check('saved_air_'+str(i),0<b['air']<=1000,[a['air'],b['air']])
            elif 'Loader' in actual['data']:
                g.check('saved_loader_'+key,actual['data']==old['data'],actual['data'])
            elif 'Control' in actual['data']:
                g.check('saved_controller_'+key,actual['data']==old['data'],actual['data'])
            elif 'Vent' in actual['data']:
                a=old['data']['Vent'];b=actual['data']['Vent']
                g.check('saved_vent_'+key,b['fluid']==a['fluid'] and b['cap']==a['cap'] and b['mb']>0,[a,b])
            else:
                a=old['data'];b=actual['data']
                # The barrel may have supplied the vent during startup.
                g.check('saved_barrel_'+key,str(a.get('tank',{}).get('fluid'))==str(b.get('tank',{}).get('fluid')),b)


CORE_B=(40,102,20)
LOADERS_B=[(39,102,19),(41,102,19)]


def form_b(g):
    g.command('fill 32 99 17 48 99 26 minecraft:stone')
    g.command('fill 32 100 17 48 108 26 minecraft:air')
    g.command('fill 38 100 20 42 104 24 ntm:pile_brick')
    tool(g)
    interact(g,CORE_B,'north',(40.5,101,17.5))
    for _ in range(40):
        try:
            data=machine(g,CORE_B)['data'];break
        except ValueError:time.sleep(.5)
    g.check('b_formed',data['Core']['shape']['depth']==5,data)


def destroy(g):
    """Player break and a thermal failure, on a second pile (the first one is persisted)."""
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    g.command('tick unfreeze')
    form_b(g)
    for p in [(39,102,20),(41,102,20)]:interact(g,p,'north',(p[0]+.5,101,17.5))
    cells=[(x,y,z) for x in range(38,43) for y in range(100,105) for z in range(20,25)]
    # Break one exposed member by hand: the whole structure reverts, the cell too.
    broken=(38,101,22)
    g.command('tp @s 35.5 101 22.5')
    g.ctl('break 38 101 22 --timeout 20')
    time.sleep(2)
    names=[block(g,c) for c in cells]
    g.check('player_break_reverts_all_to_brick',all(n=='ntm:pile_brick' for n in names),sorted(set(names)))
    g.check('player_break_cell_is_brick',block(g,broken)=='ntm:pile_brick',block(g,broken))
    # Thermal failure: rebuild with no controls and no air; sources + plutonium melt it.
    form_b(g)
    for p in [(39,102,20),(41,102,20)]:interact(g,p,'north',(p[0]+.5,101,17.5))
    for p in LOADERS_B:
        g.command('clear @s');g.command('give @s ntm:pile_device__loader 1')
        g.command(f'tp @s {p[0]+.5} 101 {p[2]-2.5}')
        g.ctl(f'place {p[0]} {p[1]} {p[2]} --item ntm:pile_device__loader')
    # The failure can come mid-loading and the blast takes the loaders with it.
    try:
        for kind,p in [('ra226be',LOADERS_B[0])]*5+[('pu239',LOADERS_B[1])]*5:
            if block(g,CORE_B)!='ntm:pile_block__core':break
            load(g,p,kind)
    except (RuntimeError,KeyError,TypeError) as error:
        g.evidence['load_interrupted']=str(error)
    deadline=time.monotonic()+120
    while time.monotonic()<deadline and block(g,CORE_B)=='ntm:pile_block__core':time.sleep(1)
    time.sleep(3)
    after=[block(g,c) for c in cells]
    leftovers=[n for n in after if n.startswith('ntm:pile_block__')]
    g.check('meltdown_dismantles_pile',block(g,CORE_B)!='ntm:pile_block__core' and not leftovers,{'core':block(g,CORE_B),'leftovers':len(leftovers)})
    g.check('meltdown_explosion_damaged_blocks',sum(n in ('minecraft:air','minecraft:fire') for n in after)>0,sorted(set(after)))
    log=(g.folder/'client-reopen.log').read_text(errors='replace')
    g.check('meltdown_logged',('pile meltdown' in log),log[-400:])
    items=g.ctl('entities')
    g.evidence['entities_after_meltdown']=items
    nether_player(g)


def overworld_is(g,p,name):
    """Whether the Overworld block at p is `name`, wherever the player is."""
    text=' '.join(g.command(f'execute in minecraft:overworld if block {p[0]} {p[1]} {p[2]} {name}').get('chat',[]))
    return 'passed' in text.lower()


def nether_player(g):
    """A pile keeps simulating, and melts down, in the Overworld while the player is in the Nether."""
    form_b(g)
    for p in [(39,102,20),(41,102,20)]:interact(g,p,'north',(p[0]+.5,101,17.5))
    for p in LOADERS_B:
        g.command('clear @s');g.command('give @s ntm:pile_device__loader 1')
        g.command(f'tp @s {p[0]+.5} 101 {p[2]-2.5}')
        g.ctl(f'place {p[0]} {p[1]} {p[2]} --item ntm:pile_device__loader')
    # Without controls or air this melts about half a minute after it is fully loaded.
    for _ in range(5):load(g,LOADERS_B[0],'ra226be')
    for _ in range(5):load(g,LOADERS_B[1],'nu')
    # A forced Nether level ticks after the Overworld, so it is the level in place at
    # the end of every server tick (the host cannot save Overworld block entities then,
    # which is why every machine read happened above).
    g.command('execute in minecraft:the_nether run forceload add 0 0')
    time.sleep(3)
    g.command('execute in minecraft:the_nether run fill -3 100 -3 3 104 3 minecraft:air')
    g.command('execute in minecraft:the_nether run fill -3 99 -3 3 99 3 minecraft:netherrack')
    g.command('execute in minecraft:the_nether run tp @s 0.5 100 0.5')
    time.sleep(3)
    here=g.ctl('block 0 99 0')['block']
    g.check('player_in_nether',here=='minecraft:netherrack',here)
    g.check('overworld_pile_still_loaded_when_leaving',overworld_is(g,CORE_B,'ntm:pile_block__core'),'core present')
    deadline=time.monotonic()+150
    while time.monotonic()<deadline and overworld_is(g,CORE_B,'ntm:pile_block__core'):time.sleep(2)
    g.check('overworld_pile_melts_while_player_in_nether',not overworld_is(g,CORE_B,'ntm:pile_block__core'),'core gone')
    g.check('player_level_restored_after_hook',g.ctl('block 0 99 0')['block']=='minecraft:netherrack',g.ctl('block 0 99 0'))
    log=(g.folder/'client-reopen.log').read_text(errors='replace')
    g.check('nether_meltdown_logged',log.count('pile meltdown')>=2,log.count('pile meltdown'))
    g.command('execute in minecraft:overworld run tp @s 35.5 101 22.5')


if __name__=='__main__':
    mode=sys.argv[2] if len(sys.argv)>2 else 'setup'
    g=Game(sys.argv[1],'pile'+('' if mode=='setup' else '-'+mode))
    g.ctl('wait-ready 120')
    if mode=='setup':setup(g)
    elif mode in ('capture-state','check-state'):persistence(g,mode=='capture-state')
    elif mode=='destroy':destroy(g)
    else:raise SystemExit('unknown mode: '+mode)
    g.evidence['ok']=True;g.save()

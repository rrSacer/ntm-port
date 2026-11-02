#!/usr/bin/env python3
"""MK2 pile acceptance on a disposable native world, with real tool/rod/fluid use.
No machine state is injected. capture-state/check-state check geometry, inventory and continuing numeric state
across a clean client restart (including heat, flux, rod depletion and devices).
"""
import json
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


def load(g,p,kind):
    g.command('clear @s')
    g.command('give @s ntm:pile_rod__'+kind+' 1')
    g.ctl('select ntm:pile_rod__'+kind)
    interact(g,p,'north',(p[0]+.5,101,p[2]-2.5))
    pending=machine(g,p)['data']['Loader']
    g.check('rod_in_loader_'+kind+'_'+str(len(g.evidence['steps'])),pending['slots'][0]['count']==1,pending)
    interact(g,p,'north')
    time.sleep(1.6)


def setup(g):
    g.close()
    g.command('gamemode creative @s')
    g.command('forceload add 30 0 50 24')
    time.sleep(3)
    g.command('fill 32 99 3 48 108 18 minecraft:air')
    g.command('fill 32 99 3 48 99 18 minecraft:stone')
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
        g.command('setblock '+' '.join(map(str,p))+' ntm:pile_device__control[facing=north]')
        g.command(f'setblock {p[0]} {p[1]} {p[2]-1} minecraft:redstone_block')
    # A full original compressed-air package in a barrel, sent through AIR ducts.
    g.command(f'setblock {BARREL[0]} {BARREL[1]} {BARREL[2]} ntm:barrel_steel[facing=north]')
    for x in (35,36):g.command(f'setblock {x} 101 10 ntm:fluid_duct_neo')
    type_duct(g,'air',[(35,101,10),(36,101,10)],(34,100,7))
    g.command('clear @s');g.command('tp @s 34.5 100 7.5')
    fill_barrel(g,BARREL,1,'ntm:fluid_pack_full__air')
    time.sleep(4)
    c=pile(g)
    g.check('compressed_air_reaches_channel',next(x for x in c['channels'] if x['kind']=='Ventilation')['air']>0,c['channels'])
    g.check('controls_extracted',all(x['control']>.99 for x in c['channels'] if x['kind']=='Control'),c['channels'])
    for _ in range(5):load(g,LOADERS[0],'ra226be')
    for _ in range(5):load(g,LOADERS[1],'pu239')
    c=pile(g)
    g.check('real_rods_loaded',all(len([r for r in fuel(c,i)['rods'] if r])==5 for i in (0,1)),c['channels'])
    g.check('reactor_heats',fuel(c,1)['heat']>20,c['channels'])
    # Close the controls and measure the loss of cross-segment input.
    before=fuel(c,1)['incoming']
    for p in CONTROLS:g.command(f'setblock {p[0]} {p[1]} {p[2]-1} minecraft:air')
    time.sleep(5)
    closed=pile(g)
    after=fuel(closed,1)['incoming']
    g.check('control_reduces_reaction',after < before*.8,{'open':before,'closed':after})
    for p in CONTROLS:g.command(f'setblock {p[0]} {p[1]} {p[2]-1} minecraft:redstone_block')
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
    time.sleep(45)
    c=pile(g);a=fuel(c,1)['heat'];time.sleep(4);b=fuel(pile(g),1)['heat']
    g.command('setblock 36 101 10 ntm:fluid_duct_neo')
    type_duct(g,'air',[(36,101,10)],(34,100,7))
    time.sleep(4)
    cooled=pile(g);d=fuel(cooled,1)['heat']
    g.check('vent_cools_heated_channel',d < b and b>a,{'no_air_before':a,'no_air_after':b,'air_after':d})
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

if __name__=='__main__':
    mode=sys.argv[2] if len(sys.argv)>2 else 'setup'
    g=Game(sys.argv[1],'pile'+('' if mode=='setup' else '-'+mode))
    g.ctl('wait-ready 120')
    if mode=='setup':setup(g)
    elif mode in ('capture-state','check-state'):persistence(g,mode=='capture-state')
    else:raise SystemExit('unknown mode: '+mode)
    g.evidence['ok']=True;g.save()

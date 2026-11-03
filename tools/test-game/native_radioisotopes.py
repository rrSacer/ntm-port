#!/usr/bin/env python3
"""RTG/radgen acceptance: UI and hopper inputs, real HE battery, natural decay.

No machine state, pellet life or processing queue is injected. All acceleration
uses /tick sprint. Run via run-native-reactors.py radioisotopes.
"""
import json
import re
import sys
import time
from native_game import Game
from native_solar_stirling import place
from native_zirnox import machine, pos_text, block, sprint, add
from native_combustion import battery_power


def data(g,p):
    value=machine(g,p)['data']
    return value.get('Rtg',value.get('Radgen'))


def pellet_life(g,p,slot):
    text=' '.join(g.command(f'data get block {pos_text(p)} Items[{{Slot:{slot}b}}].components."minecraft:custom_data".PELLET_DEPLETION').get('chat',[]))
    found=re.search(r'(-?\d+)[lL]?\s*$',text)
    if not found:raise AssertionError(text)
    return int(found[1])


def open_machine(g,p):
    # Approach the RTG from its free south face; its east wire blocks that
    # ray. Radgen is opened from the side opposite its loading hopper.
    g.close();m=machine(g,p)
    if m['block'].endswith('radgen'):
        if m['facing'] in ('north','south'):
            target=add(p,(-1,0,0));side='west';stand=(p[0]-3.5,p[1],p[2]+.5)
        else:
            target=add(p,(0,0,1));side='south';stand=(p[0]+.5,p[1],p[2]+3.5)
    else:
        target=p;side='south';stand=(p[0]+.5,p[1],p[2]+3.5)
    g.command('tp @s '+' '.join(map(str,stand)))
    g.ctl('interact '+pos_text(target)+' '+side);time.sleep(.7)
    g.check('menu_'+str(len(g.evidence['steps'])),g.ctl('screen').get('screen')=='container',g.ctl('screen'))


def setup(g):
    g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.ground(180,101,8,width=45,height=9)
    rtg=tuple(place(g,'machine_rtg_grey',185,101,8))
    radgen=tuple(place(g,'machine_radgen',210,101,8))
    g.check('radgen_volume',block(g,add(radgen,(0,2,0))).startswith('ntm:dummy_'),radgen)
    rb=add(rtg,(3,0,0))
    for dx in (1,2):g.command('setblock '+pos_text(add(rtg,(dx,0,0)))+' ntm:red_cable')
    facing=machine(g,radgen)['facing'];direction={'north':(0,0,-1),'south':(0,0,1),'west':(-1,0,0),'east':(1,0,0)}[facing]
    back=lambda n: tuple(radgen[i]-direction[i]*n for i in range(3))
    gb=back(5);g.command('setblock '+pos_text(back(4))+' ntm:red_cable')
    for p in (rb,gb):g.command('setblock '+pos_text(p)+' ntm:machine_battery[facing=north]')
    open_machine(g,rtg);g.check('rtg_slots',len(g.slots())==15,g.slots())
    g.put('ntm:pellet_rtg_lead',1,0);g.put('ntm:pellet_rtg_radium',1,1)
    g.snapshot('rtg-loaded');g.close()
    # The 15 slots are visible to every side, but Java denies BOTH automated
    # insertion and extraction. Manual Slot accepts any item.
    rh=add(rtg,(0,1,0));g.command('setblock '+pos_text(rh)+' minecraft:hopper[facing=down]')
    g.command('item replace block '+pos_text(rh)+' container.0 with ntm:pellet_rtg_gold 1')
    time.sleep(1)
    open_machine(g,rtg)
    g.check('rtg_hopper_insertion_refused',not any(s.get('item')=='ntm:pellet_rtg_gold' for s in g.slots()),g.slots())
    text=' '.join(g.command('data get block '+pos_text(rh)+' Items').get('chat',[]))
    g.check('rtg_hopper_keeps_pellet','pellet_rtg_gold' in text,text)
    g.put('ntm:pellet_rtg_gold',1,2)
    g.put('minecraft:stone',1,14)
    g.check('rtg_manual_accepts_nonfuel',g.slots()[14].get('item')=='minecraft:stone',g.slots())
    # An extracting hopper must never remove fresh, expired or arbitrary RTG items.
    r_out=add(rtg,(0,-1,0));g.command('setblock '+pos_text(r_out)+' minecraft:hopper[facing=east]')
    g.close()
    open_machine(g,radgen);g.check('radgen_slots',len(g.slots())==24,g.slots())
    g.put('ntm:nuclear_waste_short_tiny__uranium235',1,0)
    g.put('ntm:scrap_nuclear',1,1)
    g.put('minecraft:stone',1,11)
    g.check('radgen_manual_accepts_nonfuel',g.slots()[11].get('item')=='minecraft:stone',g.slots())
    target=next(s['slot'] for s in g.ctl('screen')['slots'] if s['slot']<36 and not s.get('item'))
    g.ctl('click-slot 311');time.sleep(.3);g.ctl('click-slot '+str(target));time.sleep(.3)
    g.check('radgen_nonfuel_removed_manually',not g.slots()[11].get('item'),g.slots())
    # Manual Slot insertion is unrestricted, so all twelve queues can be
    # exercised before the engine exposes conditional hopper insertion.
    for slot in range(12):g.put('ntm:gem_rad',2,slot)
    g.check('twelve_manual_queues',all(q['processing'] for q in data(g,radgen)['queues']),data(g,radgen))
    g.snapshot('radgen-twelve-manual-queues');g.close()
    for p in (rb,gb):
        g.command('setblock '+pos_text(p)+' minecraft:air')
        g.command('setblock '+pos_text(p)+' ntm:machine_battery[facing=north]')
    before=[battery_power(g,p) for p in (rb,gb)];time.sleep(.5);after=[battery_power(g,p) for p in (rb,gb)]
    g.check('both_charge_real_batteries',all(b>a for a,b in zip(before,after)),[before,after])
    # Redstone is deliberately ignored by both source generators.
    before=pellet_life(g,rtg,1)
    g.command('setblock '+pos_text(add(rtg,(-1,0,0)))+' minecraft:redstone_block')
    time.sleep(.5);after=pellet_life(g,rtg,1)
    g.check('rtg_runs_under_redstone',after<before,[before,after])
    c={'rtg':rtg,'radgen':radgen,'rtg_battery':rb,'radgen_battery':gb,'rtg_output_hopper':r_out}
    (g.folder/'radioisotopes-config.json').write_text(json.dumps(c))


def decay(g):
    c=json.loads((g.folder/'radioisotopes-config.json').read_text());r=c['rtg'];rad=c['radgen']
    sprint(g,6500)
    open_machine(g,rad);slots=g.slots()
    g.check('tiny_waste_residue',slots[12].get('item')=='ntm:nuclear_waste_short_depleted_tiny__uranium235',slots)
    g.check('scrap_leaves_no_residue',not slots[13].get('item'),slots)
    g.close()
    # 21,600-life lead is the shortest source pellet. Sum regular ticks too,
    # then advance enough normally to cross zero and the replacement tick.
    remaining=pellet_life(g,r,0);sprint(g,max(1,remaining+2))
    open_machine(g,r);slots=g.slots()
    g.check('natural_lead_depletion',slots[0].get('item')=='ntm:pellet_rtg_depleted__bismuth',slots)
    g.check('other_pellets_survive',slots[1].get('item')=='ntm:pellet_rtg_radium' and any(s.get('item')=='ntm:pellet_rtg_gold' for s in slots),slots)
    g.snapshot('rtg-depleted');g.close()
    text=' '.join(g.command('data get block '+pos_text(c['rtg_output_hopper'])+' Items').get('chat',[]))
    g.check('rtg_never_automatically_extracts','[]' in text,text)
    # Finish the 36,000-tick gems without resetting queues or changing life.
    a=data(g,rad);remaining=max(q['max_progress']-q['progress'] for q in a['queues'] if q['processing'])
    sprint(g,remaining+2);open_machine(g,rad);slots=g.slots()
    g.check('natural_gem_diamonds',all(s.get('item')=='minecraft:diamond' for s in slots[14:24]),slots)
    g.check('earlier_waste_preserved',slots[12].get('item')=='ntm:nuclear_waste_short_depleted_tiny__uranium235',slots)
    g.snapshot('radgen-residues');g.close()
    # Core path: port-proxy resolution is a separate wave-0 acceptance.
    # Inputs and in-flight fuel must remain in their queues/slots.
    out=add(rad,(0,-1,0));g.command('setblock '+pos_text(out)+' minecraft:hopper[facing=east]');time.sleep(2)
    text=' '.join(g.command('data get block '+pos_text(out)+' Items').get('chat',[]))
    g.check('radgen_core_hopper_extracts_only_residue','nuclear_waste_short_depleted_tiny' in text and 'gem_rad' not in text,text)
    g.command('setblock '+pos_text(out)+' minecraft:air')


def inventory(g,p):
    """Read actual NBT stacks without altering them (container conservation)."""
    text=' '.join(g.command('data get block '+pos_text(p)+' Items').get('chat',[]))
    entries=[]
    for compound in re.findall(r'\{[^{}]*\}',text):
        item=re.search(r'id:\s*["\']([^"\']+)["\']',compound)
        count=re.search(r'count:\s*(\d+)',compound)
        if item and count:entries.append((item[1],int(count[1])))
    return entries


def automation(g):
    """Java balancing at the side port; extraction at both port and core."""
    g.close();g.ground(245,101,8,width=18,height=9)
    core=tuple(place(g,'machine_radgen',250,101,8))
    hopper=add(core,(2,0,0));nonport=add(core,(2,1,0))
    g.command('setblock '+pos_text(hopper)+' minecraft:hopper[facing=west]')
    g.command('item replace block '+pos_text(hopper)+' container.0 with ntm:gem_rad 24')
    g.command('setblock '+pos_text(nonport)+' minecraft:hopper[facing=west]')
    g.command('item replace block '+pos_text(nonport)+' container.0 with minecraft:stone 1')
    deadline=time.monotonic()+40
    while time.monotonic()<deadline:
        a=data(g,core)
        if all(q['processing'] for q in a['queues']) and not inventory(g,hopper):break
        time.sleep(.5)
    g.check('twelve_hopper_queues',all(q['processing'] for q in a['queues']),a)
    open_machine(g,core);slots=g.slots()
    inputs=[s.get('count',0) if s.get('item') else 0 for s in slots[:12]]
    g.check('hopper_balanced_twelve_inputs',inputs==[1]*12,slots)
    count=sum(inputs)+sum(q['processing']['count'] for q in data(g,core)['queues'] if q['processing'])
    g.check('hopper_conserves_twenty_four_gems',count==24,count)
    g.check('nonport_cannot_insert',inventory(g,nonport)==[('minecraft:stone',1)],inventory(g,nonport))
    g.snapshot('radgen-twelve-hopper-queues');g.close()
    # Repeat the RTG refusal in this phase, against both live and depleted
    # pellets. These positions were built by setup without bypassing the UI.
    c=json.loads((g.folder/'radioisotopes-config.json').read_text())
    top=add(c['rtg'],(0,1,0))
    g.check('rtg_hopper_still_refuses_insert',inventory(g,top)==[('ntm:pellet_rtg_gold',1)],inventory(g,top))
    g.check('rtg_hopper_still_refuses_extract',inventory(g,c['rtg_output_hopper'])==[],inventory(g,c['rtg_output_hopper']))
    # The shortest remaining queue completion bounds a natural sprint; after
    # the latest one finishes there are exactly 12 diamonds +12 in-flight gems.
    a=data(g,core);sprint(g,max(q['max_progress']-q['progress'] for q in a['queues'])+2)
    open_machine(g,core);slots=g.slots();a=data(g,core)
    diamonds=sum(s.get('count',0) for s in slots[12:] if s.get('item')=='minecraft:diamond')
    remaining=sum(s.get('count',0) for s in slots[:12] if s.get('item')=='ntm:gem_rad')+sum(q['processing']['count'] for q in a['queues'] if q['processing'])
    g.check('natural_twelve_gem_residues',diamonds==12,slots)
    g.check('completion_conserves_twenty_four',diamonds+remaining==24,{'diamonds':diamonds,'gems':remaining})
    g.close()
    # A hopper below the side port first; preserve its contents in place and
    # switch it off with redstone before checking the under-core hopper.
    port_out=add(core,(1,-1,0));core_out=add(core,(0,-1,0))
    g.command('setblock '+pos_text(port_out)+' minecraft:hopper[facing=east]')
    time.sleep(1.5)
    g.command('setblock '+pos_text(add(core,(2,-1,0)))+' minecraft:redstone_block')
    port_items=inventory(g,port_out)
    g.check('side_port_extracts_residue',any(i=='minecraft:diamond' and n>0 for i,n in port_items),port_items)
    g.check('side_port_never_extracts_gems',all(i=='minecraft:diamond' for i,n in port_items),port_items)
    g.command('setblock '+pos_text(core_out)+' minecraft:hopper[facing=west]')
    time.sleep(3)
    core_items=inventory(g,core_out)
    g.check('core_extracts_residue',any(i=='minecraft:diamond' and n>0 for i,n in core_items),core_items)
    g.check('core_never_extracts_gems',all(i=='minecraft:diamond' for i,n in core_items),core_items)
    open_machine(g,core);slots=g.slots();a=data(g,core)
    outputs=sum(s.get('count',0) for s in slots[12:] if s.get('item')=='minecraft:diamond')
    outputs+=sum(n for i,n in inventory(g,port_out)+core_items if i=='minecraft:diamond')
    remaining=sum(s.get('count',0) for s in slots[:12] if s.get('item')=='ntm:gem_rad')+sum(q['processing']['count'] for q in a['queues'] if q['processing'])
    g.check('extraction_conserves_twenty_four',outputs==12 and outputs+remaining==24,{'diamonds':outputs,'gems':remaining,'port':port_items,'core':core_items})
    g.snapshot('radgen-port-and-core-extraction');g.close()


def persistence(g,capture):
    c=json.loads((g.folder/'radioisotopes-config.json').read_text());path=g.folder/'radioisotopes-saved.json'
    current={'radium_life':pellet_life(g,c['rtg'],1),'rtg':data(g,c['rtg']),'radgen':data(g,c['radgen'])}
    if capture:
        # Fresh batteries on the same wires make strict resumed HE growth
        # observable even after the earlier batteries reached their cap.
        for k in ('rtg_battery','radgen_battery'):
            p=c[k];g.command('setblock '+pos_text(p)+' minecraft:air');g.command('setblock '+pos_text(p)+' ntm:machine_battery[facing=north]')
        path.write_text(json.dumps(current,indent=2));g.ctl('save')
    else:
        old=json.loads(path.read_text());g.close();sprint(g,100)
        new_life=pellet_life(g,c['rtg'],1);now=data(g,c['radgen'])
        g.check('rtg_reopen_strict_life',new_life<old['radium_life'],[old['radium_life'],new_life])
        continuing=[(a,b) for a,b in zip(old['radgen']['queues'],now['queues']) if a['processing']]
        g.check('radgen_reopen_strict_progress',bool(continuing) and all(b['processing']==a['processing'] and b['progress']>a['progress'] for a,b in continuing),[old['radgen'],now])
        # Reset sinks after reopening, then require a new positive transfer.
        for k in ('rtg_battery','radgen_battery'):
            p=c[k];g.command('setblock '+pos_text(p)+' minecraft:air');g.command('setblock '+pos_text(p)+' ntm:machine_battery[facing=north]')
            before=battery_power(g,p);time.sleep(1);after=battery_power(g,p)
            g.check(k+'_strict_reopen_generation',after>before,[before,after])
        open_machine(g,c['rtg']);g.check('depleted_pellet_survives_reopen',g.slots()[0].get('item')=='ntm:pellet_rtg_depleted__bismuth',g.slots())
        g.snapshot('radioisotopes-reopened');g.close()


if __name__=='__main__':
    g=Game(sys.argv[1],'radioisotopes-'+sys.argv[2]);mode=sys.argv[2]
    g.evidence['phase']=mode;g.ctl('wait-ready 120')
    {'setup':setup,'decay':decay,'automation':automation,'capture-state':lambda g:persistence(g,True),'check-state':lambda g:persistence(g,False)}[mode](g)
    g.evidence['ok']=True;g.save()

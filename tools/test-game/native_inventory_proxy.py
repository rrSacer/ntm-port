#!/usr/bin/env python3
"""Engine inventory fixture; run through run-native-reactors.py inventory-proxy.

The disposable hmod changes only the ZIRNOX shape to three radgen-style ports.
Slot contents are explicit fixture inputs: this tests transport, not fuel decay.
"""
import re
import sys
import time
from collections import Counter
from native_game import Game
from native_solar_stirling import place
from native_zirnox import add, block, machine, pos_text, sprint


def contents(g, at, size):
    result = Counter()
    for slot in range(size):
        base = f'data get block {pos_text(at)} Items[{{Slot:{slot}b}}]'
        text = ' '.join(g.command(base + '.id').get('chat', []))
        found = re.search(r'[\"\\\x27]?(ntm:[\w./]+)[\"\\\x27]?\s*$', text)
        if not found:
            continue
        count = ' '.join(g.command(base + '.count').get('chat', []))
        number = re.search(r'(-?\d+)[bBsSlL]?\s*$', count)
        if not number:
            raise AssertionError('missing count: ' + count)
        result[found.group(1)] += int(number.group(1))
    return result


def open_core(g, port):
    g.close()
    g.command(f'tp @s {port[0]-1.5} {port[1]} {port[2]+.5}')
    g.ctl(f'interact {pos_text(port)} west')
    time.sleep(.7)
    g.check('menu_open_'+str(len(g.evidence['steps'])), g.ctl('screen').get('screen') == 'container', g.ctl('screen'))


def setup(g):
    g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.ground(40,101,8,width=20,height=8)
    core=tuple(place(g,'machine_zirnox',45,101,8))
    facing=machine(g,core)['facing']
    front={'north':(0,-1),'south':(0,1),'east':(1,0),'west':(-1,0)}[facing]
    forward=(-front[0],-front[1]);right=(-forward[1],forward[0])
    def cell(x,y,z):
        return add(core,(right[0]*x+forward[0]*z,y,right[1]*x+forward[1]*z))
    port=cell(1,0,0);nonport=cell(1,0,-2)
    g.check('fixture_port_and_nonport_are_real_dummies',
            all(block(g,p).startswith('ntm:dummy_zirnox') for p in (port,nonport)),
            {'core':core,'port':port,'nonport':nonport,'facing':facing})
    active='ntm:rod_zirnox__uranium_fuel';spent='ntm:rod_zirnox_tritium'
    open_core(g,port)
    g.put('ntm:rod_zirnox__lithium',1,0)
    g.put('ntm:rod_zirnox__natural_uranium_fuel',1,7)
    g.put('ntm:fluid_barrel_full__carbondioxide',1,24)
    g.put('ntm:fluid_barrel_full__water',2,25)
    g.button([144,35,14,14],[203,256]);time.sleep(.4)
    g.close()
    sprint(g,20050)
    open_core(g,port);g.button([144,35,14,14],[203,256]);time.sleep(.4);g.close()
    before=contents(g,core,24)
    g.check('fixture_converted_one_rod_naturally',
            before==Counter({spent:1,'ntm:rod_zirnox__natural_uranium_fuel':1}),dict(before))
    incoming=add(port,(0,1,0));outgoing=add(port,(0,-1,0));blocked=add(nonport,(0,1,0))
    for p in (incoming,outgoing,blocked):
        g.command('setblock '+pos_text(p)+' minecraft:hopper[facing=down]')
    g.command('item replace block '+pos_text(incoming)+' container.0 with '+active+' 1')
    g.command('item replace block '+pos_text(incoming)+' container.1 with '+active+' 1')
    g.command('item replace block '+pos_text(blocked)+' container.0 with '+active+' 1')
    time.sleep(3)
    actual=contents(g,core,24);received=contents(g,outgoing,5)
    source=contents(g,incoming,5);refused=contents(g,blocked,5)
    g.check('proxy_insertion_reaches_core',
            actual==Counter({active:2,'ntm:rod_zirnox__natural_uranium_fuel':1}),dict(actual))
    g.check('proxy_extraction_obeys_conditional_policy',received==Counter({spent:1}),dict(received))
    g.check('nonport_has_no_inventory_access',refused==Counter({active:1}),dict(refused))
    expected=Counter({active:3,'ntm:rod_zirnox__natural_uranium_fuel':1,spent:1})
    g.check('proxy_exact_item_conservation',actual+received+source+refused==expected,
            {'core':dict(actual),'output':dict(received),'input':dict(source),'blocked':dict(refused)})
    g.snapshot('inventory-proxy')


if __name__=='__main__':
    g=Game(sys.argv[1],'inventory-proxy');g.ctl('wait-ready 120')
    setup(g);g.evidence['ok']=True;g.save();print(g.output)

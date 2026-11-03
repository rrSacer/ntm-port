#!/usr/bin/env python3
"""Catalog regression: all K3 identities and the 70 radioisotope identities.

Uses the same client/package as the runtime scenarios, in a disposable world.
Block identity and item delivery prove content loading, not machine behavior.
"""
import json
from pathlib import Path
import sys
import time
from native_game import Game


def setup(g):
    root=Path(__file__).resolve().parents[2]
    source=json.loads((root/'tools/content-gen/catalog-k3.json').read_text())
    variables=set(source['block']+source['item'])
    rows=json.loads((root/'content/ids.json').read_text())['entries']
    selected=[r for r in rows if r['source_variable'] in variables or r['id'] in ('ntm:machine_rtg_grey','ntm:machine_radgen') or r['source_variable'].startswith(('pellet_rtg','nuclear_waste_short','nuclear_waste_long'))]
    blocks=[r for r in selected if r['type']=='block']
    g.close();g.command('gamemode creative @s');g.command('clear @s')
    g.ground(10,101,8,width=40,height=7)
    g.command('forceload add 0 0 50 40')
    g.command('fill 0 100 0 50 100 40 minecraft:stone')
    for index,row in enumerate(blocks):
        x,z=2+(index%14)*3,2+(index//14)*3
        g.command(f'setblock {x} 101 {z} '+row['id'])
        actual=g.ctl(f'block {x} 101 {z}')
        g.check('catalog_block_'+row['id'],actual['block']==row['id'],actual)
    items=[r['id'] for r in selected]
    for offset in range(0,len(items),30):
        g.command('clear @s');batch=items[offset:offset+30]
        for item in batch:g.command('give @s '+item+' 1')
        deadline=time.monotonic()+15
        while True:
            counts=g.ctl('inventory')['counts']
            if all(counts.get(item,0)==1 for item in batch) or time.monotonic()>deadline:break
            time.sleep(.25)
        for item in batch:g.check('catalog_item_'+item,counts.get(item,0)==1,counts.get(item,0))
    g.evidence['catalog']={'blocks':len(blocks),'items':len(items),'selection':'all K3 + all radioisotope identities'}
    g.snapshot('catalog-k3-radioisotopes');g.close()


if __name__=='__main__':
    g=Game(sys.argv[1],'catalog');g.ctl('wait-ready 120');setup(g)
    g.evidence['ok']=True;g.save()

#!/usr/bin/env python3
"""Exercise actual crafting grid and output clicks in the isolated test client."""
import json,socket,time
from pathlib import Path
root=Path(__file__).resolve().parents[2]
scratch=Path((root/'recipe-gen-evidence/run-path.txt').read_text().strip())
s=socket.socket(socket.AF_UNIX);s.settimeout(30);s.connect(str(scratch/'control.sock'));f=s.makefile()
evidence=[]
def cmd(line):
    s.sendall((line+'\n').encode()); r=json.loads(f.readline());evidence.append({'command':line,'reply':r});time.sleep(.12)
    if 'error' in r: raise RuntimeError(r)
    return r
pos=cmd('state')['player']['block_pos']
table=f'{pos[0]+1} {pos[1]} {pos[2]}'
cmd('close');cmd('command setblock '+table+' minecraft:crafting_table');time.sleep(.5)
def craft(ingredients, expected, count):
    cmd('close');cmd('command clear @s');time.sleep(.4)
    for item,n,slots in ingredients: cmd('give '+item+' '+str(n))
    cmd('interact '+table);time.sleep(.4)
    menu=cmd('menu')
    for item,n,slots in ingredients:
        slot=next(s['slot'] for s in menu['slots'] if s['stack']['item']==item and s['slot']<36)
        cmd('click-slot '+str(slot))
        for cell in slots: cmd('click-slot '+str(100+cell)+' right')
        cmd('click-slot '+str(slot))
    result=cmd('menu')
    # Table crafting result is slot 200, separate from station recipes (650).
    out=next(x['stack'] for x in result['slots'] if x['slot']==200)
    assert (out['item'],out['count'])==(expected,count),out
    cmd('click-slot 200 shift');time.sleep(.4)
    inv=cmd('inventory')
    return {'expected':expected,'count':count,'result':out,'inventory_after':inv}
try:
    results=[]
    results.append(craft([('ntm:ingot_uranium',9,list(range(9)))],'ntm:block_uranium',1))
    results.append(craft([('ntm:ingot_uranium',1,[0])],'ntm:nugget_uranium',9))
    results.append(craft([('minecraft:copper_ingot',1,[0]),('minecraft:redstone',1,[1])],'ntm:ingot_red_copper',2))
    (root/'recipe-gen-evidence/crafts.json').write_text(json.dumps(results,indent=2)+'\n')
finally:
    (root/'recipe-gen-evidence/probe.json').write_text(json.dumps(evidence,indent=2)+'\n')

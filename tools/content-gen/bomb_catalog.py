#!/usr/bin/env python3
"""Content contract requested by the M2 bomb logic agent. No explosion logic here."""
import re
from pathlib import Path
from generate import registration,write_json
HERE=Path(__file__).resolve().parent;SRC=HERE.parents[2]/'ntm-src'
# ROOT is .../Workspace/ntm-port; source is its sibling.
SRC=HERE.parents[1].parent/'ntm-src'
rows=[]
for var in ['tnt','dynamite','semtex','c4','nuke_boy','nuke_gadget','nuke_man','nuke_mike','nuke_tsar','nuke_n2','nuke_fstbmb','sellafield_slaked','waste_trinitite','waste_log','waste_planks','waste_leaves','fallout']:
    text=(SRC/'src/main/java/com/hbm/blocks/ModBlocks.java').read_text();name,tex=registration(text,var,'block')
    bomb=var not in ('sellafield_slaked','waste_trinitite','waste_log','waste_planks','waste_leaves','fallout')
    row={'id':name,'source_variable':var,'type':'block','category':'bomb' if bomb else 'fallout','template':'minecraft:redstone_lamp' if bomb else 'minecraft:snow' if var=='fallout' else 'minecraft:oak_log' if var=='waste_log' else 'minecraft:oak_leaves' if var=='waste_leaves' else 'minecraft:stone'}
    if bomb:row['logic']={'kind':'bomb','powered':'lit'}
    if var.startswith('nuke_'):
        row['template']='minecraft:beehive'
        row['logic']={'kind':'bomb','powered':None,'facing':'facing','level':{'prop':'honey_level','max':5}}
    if var in ('tnt','dynamite','semtex','c4'):
        row['source_texture']='src/main/resources/assets/hbm/textures/blocks/'+tex+'_side.png'
        row['surface_textures']={k:'src/main/resources/assets/hbm/textures/blocks/'+tex+'_'+v+'.png' for k,v in [('side','side'),('top','top'),('bottom','bottom')]}
    elif var=='waste_log':
        row['source_texture']='src/main/resources/assets/hbm/textures/blocks/waste_log_side.png'
        row['surface_textures']={k:'src/main/resources/assets/hbm/textures/blocks/'+v+'.png' for k,v in [('side','waste_log_side'),('top','waste_log_top'),('bottom','waste_log_top')]}
    else:row['source_texture']='src/main/resources/assets/hbm/textures/blocks/'+tex+'.png'
    if bomb and var.startswith('nuke_'):row['icon_cube']=True;row['model_gap']='NTM TESR/OBJ approximated by opaque icon on steel cube; explosion/assembly belongs to Rust logic'
    if not bomb:row['behavior_gap']='Fallout surface only; radiation, spread, decay and original random loot require Rust logic'
    rows.append(row)
for var in ['boy_shielding','boy_target','boy_bullet','boy_propellant','boy_igniter','gadget_wireing','gadget_core','early_explosive_lenses','man_igniter','man_core','explosive_lenses','mike_core','mike_deut','mike_cooling_unit','tsar_core','n2_charge','egg_balefire','battery_spark','detonator']:
    text=(SRC/'src/main/java/com/hbm/items/ModItems.java').read_text();name,tex=registration(text,var,'item')
    stmt=re.search(r'^\s*'+var+r'\s*=\s*new\s+.*?;',text,re.M|re.S)[0];stack=re.search(r'\.setMaxStackSize\((\d+)\)',stmt)
    row={'id':name,'source_variable':var,'type':'item','category':'bomb_component','source_texture':'src/main/resources/assets/hbm/textures/items/'+tex+'.png','max_stack_size':int(stack[1]) if stack else 64}
    if var=='detonator':row.update(template='minecraft:stick',max_stack_size=1,logic={'kind':'detonator'})
    if var=='mike_deut':row['behavior_gap']='Container-item return requires logic; no fake container recipe'
    rows.append(row)
for row in rows:
    for tex in [row['source_texture'],*row.get('surface_textures',{}).values()]:
        assert (SRC/tex).is_file(),tex
write_json(HERE/'bombs.json',{'schema':1,'entries':rows})
print(len(rows),'bomb/fallout content contracts')

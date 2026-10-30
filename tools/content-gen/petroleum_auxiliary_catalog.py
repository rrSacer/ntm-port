#!/usr/bin/env python3
"""Oil-chain auxiliary consumables and metadata-preserving plant identities."""
import re
from generate import lang,registration
from bulk_recipes import clean,split

BASE='src/main/java/com/hbm/'


def build(source):
    items=(source/(BASE+'items/ModItems.java')).read_text()
    translations={loc:lang(source/f'src/main/resources/assets/hbm/lang/{old}.lang')
                  for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    rows=[]
    for var in ['bio_wafer','pill_herbal']:
        name,texture=registration(items,var,'item')
        rows.append({'id':name,'source_variable':var,'type':'item','category':'consumable',
            'template':'minecraft:iron_ingot','max_stack_size':64,
            'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}.png',
            'names':{loc:tr['item.'+name+'.name'] for loc,tr in translations.items()},
            'behavior_gap':'Recipe component only; source nutrition/eating and ItemPill medical effects/side effects require consumable logic.'})
    path=BASE+'blocks/generic/BlockNTMFlower.java';text=clean((source/path).read_text())
    body=text.split('enum EnumFlowerType {',1)[1].split(';',1)[0]
    for meta,member in enumerate(split(body)):
        m=re.fullmatch(r'(\w+)\((true|false)\)',member.strip())
        if not m:raise ValueError('Plant enum drift: '+member)
        suffix=m[1].lower()
        rows.append({'id':'plant_flower__'+suffix,'source_variable':'plant_flower','type':'block',
            'category':'plant','template':'minecraft:poppy','metadata':meta,'subtype_kind':'palette',
            'source_enum':'EnumFlowerType.'+m[1],'needs_oil':m[2]=='true','cross_model':True,
            'block_properties':{'hardness':0,'explosion_resistance':0,'requires_tool':False,'sounds':'minecraft:grass'},
            'source_texture':f'src/main/resources/assets/hbm/textures/blocks/plant_flower.{suffix}.png',
            'names':{loc:tr['tile.plant_flower.'+suffix+'.name'] for loc,tr in translations.items()},
            **({'drop':'ntm:plant_flower__cd0'} if suffix=='cd1' else {}),
            'behavior_gap':'Vanilla flower placement/soil only; source growth, oil dependence, CD1 to CD0 regrowth and plant interactions require server logic. Surface oil worldgen vegetation remains outside this approximation.'})
    return rows

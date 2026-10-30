#!/usr/bin/env python3
"""Source identities missing from several machine recipe snapshots."""
import re
from bulk_recipes import clean, split
from generate import lang, registration

BASE = 'src/main/java/com/hbm/'


def build(source):
    items = (source / (BASE + 'items/ModItems.java')).read_text()
    tr = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
          for loc, old in [('en_us','en_US'),('pt_br','pt_BR')]}
    rows = []
    text = clean((source / (BASE + 'items/special/ItemPlasticScrap.java')).read_text())
    members = [s.strip() for s in split(text.split('enum ScrapType {',1)[1].split('}',1)[0])]
    if len(members) != 23: raise ValueError('Plastic scrap enum drift')
    name, texture = registration(items, 'scrap_plastic', 'item')
    for meta, member in enumerate(members):
        rows.append({'id':name+'__'+member.lower(),'type':'item','category':'scrap','source_variable':'scrap_plastic',
                     'metadata':meta,'subtype_kind':'enum','source_enum':'ScrapType.'+member,
                     'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                     'names':{loc:loc_tr['item.scrap_plastic.name'] for loc,loc_tr in tr.items()},
                     'behavior_gap':'Original hidden intermediate; enum retained, no item logic'})
    text = clean((source / (BASE + 'items/machine/ItemChemicalDye.java')).read_text())
    body = text.split('enum EnumChemDye {',1)[1].split(';',1)[0]
    colors = re.findall(r'(\w+)\((\d+),\s*"[^"\n]+"\)',body)
    if len(colors) != 16: raise ValueError('Chemical dye enum drift')
    name, texture = registration(items,'chemical_dye','item')
    for meta,(member,color) in enumerate(colors):
        suffix = member.lower()
        rows.append({'id':name+'__'+suffix,'type':'item','category':'component','source_variable':'chemical_dye',
                     'metadata':meta,'subtype_kind':'enum','source_enum':'EnumChemDye.'+member,
                     'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                     'overlay_textures':[{'source':'src/main/resources/assets/hbm/textures/items/chemical_dye_overlay.png','tint':{'multiply':f'{int(color):06X}'}}],
                     'names':{loc:loc_tr[f'item.chemical_dye.{suffix}.name'] for loc,loc_tr in tr.items()}})
    return rows

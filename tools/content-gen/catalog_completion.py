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
    return rows

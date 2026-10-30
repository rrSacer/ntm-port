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
    name, texture = registration(items, 'fluid_identifier_multi', 'item')
    from m3_catalog import fluids
    none = fluids(source)[0]
    rows.append({'id': name, 'type': 'item', 'category': 'component',
                 'source_variable': 'fluid_identifier_multi', 'max_stack_size': 1,
                 'source_texture': f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                 'overlay_textures': [{'source': 'src/main/resources/assets/hbm/textures/items/fluid_identifier_overlay.png',
                                       'tint': {'multiply': none['color']}}],
                 'names': {loc: translations['item.fluid_identifier_multi.name'] for loc, translations in tr.items()},
                 'fluid_identifier': {'custom_data_key': 'ntm_fluid', 'value': 'fluids.json string id', 'default': none['id'],
                                      'source_primary': 'fluid1', 'source_secondary': 'fluid2'},
                 'behavior_gap': 'Native logic reads custom_data.ntm_fluid; selection GUI, secondary type swap, ALT capture and dynamic overlay tint require native item hooks. Static overlay uses NONE color.'})
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
    blocks = (source / (BASE + 'blocks/ModBlocks.java')).read_text()
    for var in ['powder_cement','ingot_semtex','ingot_schraranium','meteorite_sword_treated','meteorite_sword_etched']:
        name,texture = registration(items,var,'item')
        sword = var.startswith('meteorite_sword_')
        rows.append({'id':name,'type':'item','category':'component' if sword else 'ingot' if var.startswith('ingot_') else 'powder',
                     'source_variable':var,'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                     'names':{loc:loc_tr[f'item.{name}.name'] for loc,loc_tr in tr.items()},
                     **({'max_stack_size':1} if sword else {}),
                     'behavior_gap':'Processing identity only; special sword abilities and indestructibility remain in equipment backlog' if sword else
                                    'Original edible/special item use is not reproduced; processing identity and hazard data only'})
    for var in ['block_graphite','rebar']:
        name,texture = registration(blocks,var,'block')
        texture = texture or name  # BlockBase.setBlockName sets its own texture.
        rows.append({'id':name,'type':'block','category':'decorative','source_variable':var,
                     'template':'minecraft:iron_bars' if var=='rebar' else 'minecraft:iron_block' if var=='block_graphite' else 'minecraft:stone',
                     'source_texture':f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                     'names':{loc:loc_tr[f'tile.{name}.name'] for loc,loc_tr in tr.items()},
                     'behavior_gap':'Original block texture with cube JSON fallback; graphite fire/heat and rebar custom geometry are not reproduced'})
    return rows

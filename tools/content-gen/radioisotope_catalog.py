"""Original RTG pellets and metadata-split radgen wastes (energy batch 5)."""
import re
from generate import lang, registration
from bulk_recipes import clean

BASE='src/main/java/com/hbm/'


def waste_classes(source, family):
    text=clean((source/(BASE+f'items/special/ItemWaste{family.title()}.java')).read_text())
    values=re.findall(r'(\w+)\("([^"]+)",\s*(\d+),\s*(\d+)\)',text.split('enum WasteClass {',1)[1])
    if len(values)!={'short':8,'long':5}[family]:raise ValueError('Waste enum drift: '+family)
    return values


def build(source):
    items=(source/(BASE+'items/ModItems.java')).read_text()
    translations={loc:lang(source/f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    rows=[]
    for match in re.finditer(r'^\s*(pellet_rtg\w*)\s*=\s*new ItemRTGPellet\(',items,re.M):
        var=match[1];name,texture=registration(items,var,'item')
        rows.append({'id':name,'source_variable':var,'type':'item','category':'pellet','max_stack_size':1,
                     'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                     'names':{loc:tr[f'item.{name}.name'] for loc,tr in translations.items()},
                     'behavior_gap':'RTG heat, decay/config and per-stack lifespan are exported in radioisotopes.json; native machine logic owns them. Source flavor chat is not implemented.'})
    if len(rows)!=10:raise ValueError('RTG pellet count drift')
    text=clean((source/(BASE+'items/machine/ItemRTGPelletDepleted.java')).read_text())
    members=[m.strip() for m in text.split('enum DepletedRTGMaterial {',1)[1].split(';',1)[0].split(',')]
    if len(members)!=6:raise ValueError('Depleted RTG enum drift')
    for meta,member in enumerate(members):
        suffix=member.lower();name='pellet_rtg_depleted'
        rows.append({'id':name+'__'+suffix,'source_variable':name,'type':'item','category':'pellet','metadata':meta,
                     'subtype_kind':'enum','source_enum':'DepletedRTGMaterial.'+member,
                     'crafting_remainder':'ntm:plate_iron',
                     'source_texture':f'src/main/resources/assets/hbm/textures/items/{name}.{suffix}.png',
                     'names':{loc:tr[f'item.{name}.{suffix}.name'] for loc,tr in translations.items()},
                     'behavior_gap':'Original crafting container item returns plate_iron; native crafting must preserve that return.'})
    for family in ['short','long']:
        for var in [f'nuclear_waste_{family}'+suffix for suffix in ['','_tiny','_depleted','_depleted_tiny']]:
            name,texture=registration(items,var,'item')
            for meta,(member,label,_,_) in enumerate(waste_classes(source,family)):
                pt=label
                for en,translated in [('Uranium','Urânio'),('Plutonium','Plutônio'),('Neptunium','Neptúnio'),('Americium','Amerício'),('Thorium','Tório'),('Schrabidium','Schrabídio')]:pt=pt.replace(en,translated)
                rows.append({'id':name+'__'+member.lower(),'source_variable':var,'type':'item','category':'component',
                             'metadata':meta,'subtype_kind':'enum','source_enum':f'ItemWaste{family.title()}.WasteClass.'+member,
                             'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                             'names':{loc:tr[f'item.{name}.name']+' ('+(pt if loc=='pt_br' else label)+')' for loc,tr in translations.items()},
                             'behavior_gap':'Source isotope tooltip is included in the subtype display name; item radiation and processing are separate runtime behavior.'})
    return rows

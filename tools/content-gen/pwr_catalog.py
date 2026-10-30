"""PWR identities missing from the bulk import: fuel rods, column blocks, controller, internal block, printer.

The source reuses `pwr_fuel` for a block (ModBlocks) and an item (ModItems). The
block keeps `ntm:pwr_fuel`; the item subtypes are `ntm:pwr_fuel__<enum>`, the
same suffix scheme the already-imported `pwr_fuel_hot__*`/`pwr_fuel_depleted__*` use.
"""
import re
from bulk_recipes import clean
from generate import lang, registration

BASE='src/main/java/com/hbm/'
ASSETS='src/main/resources/assets/hbm/'
PILLARS=('pwr_fuel','pwr_control','pwr_channel')
MACHINES=('pwr_controller','pwr_block')
INTERNAL_ROLES=('internal','port')


def fuels(source):
    text=clean((source/(BASE+'items/machine/ItemPWRFuel.java')).read_text()).split('enum EnumPWRFuel {',1)[1].split(';',1)[0]
    result=[]
    for meta,m in enumerate(re.finditer(r'(\w+)\(\s*([\d_.]+)D,\s*new (Function\w+)\(([^)]*)\)(?:\.withDiv\(([\d_.]+)\))?(?:,\s*([\d_]+))?\)',text)):
        level=m[4].replace('_','');level=eval(level) if re.fullmatch(r'[\d.* _]+',level) else level
        result.append({'member':m[1],'metadata':meta,'heat_emission':float(m[2].replace('_','')),'function':m[3],'level':float(level),
                       'divisor':float(m[5].replace('_','')) if m[5] else 1.0,'declared_yield':int(m[6].replace('_','')) if m[6] else None})
    if [f['member'] for f in result]!=['MEU','HEU233','HEU235','MEN','HEN237','MOX','MEP','HEP239','HEP241','MEA','HEA242','HES326','HES327','BFB_AM_MIX','BFB_PU241']:
        raise ValueError('PWR fuel enum drift')
    return result


def build(source):
    blocks=(source/(BASE+'blocks/ModBlocks.java')).read_text();items=(source/(BASE+'items/ModItems.java')).read_text()
    translations={loc:lang(source/f'{ASSETS}lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    def names(key):return {loc:tr.get(key) or translations['en_us'][key] for loc,tr in translations.items()}
    root=ASSETS+'textures/'
    rows=[]
    for var in PILLARS:
        name,_=registration(blocks,var,'block')
        side,top=root+f'blocks/{name}_side.png',root+f'blocks/{name}_top.png'
        rows.append({'id':name,'source_variable':var,'type':'block','category':'decorative','source_texture':side,
                     'surface_textures':{'side':side,'top':top,'bottom':top},'names':names('tile.'+name+'.name'),
                     'behavior_gap':'Source BlockPillarPWR: axis orientation, multiblock membership and the conversion by the controller into pwr_block are native logic; plain cube here. Shares its variable name with the item pwr_fuel; the item subtypes are pwr_fuel__<enum>.'})
    name,texture=registration(blocks,'pwr_controller','block')
    rows.append({'id':name,'source_variable':'pwr_controller','type':'block','category':'machine','template':'minecraft:beehive',
                 'source_texture':root+f'blocks/{texture}.png','textures':{face:'blocks/'+texture for face in ('side','top','front')},
                 'names':names('tile.'+name+'.name'),
                 'behavior_gap':'Source controller: structure validation, flux/heat/coolant ticking, GUI and the conversion of casing into pwr_block are native/visual work; casing cube fallback only.'})
    name,texture=registration(blocks,'pwr_block','block')
    en={'internal':'PWR','port':'PWR (Port)'};pt={'internal':'PWR','port':'PWR (Porta)'}
    for meta,role in enumerate(INTERNAL_ROLES):
        rows.append({'id':'pwr_block__'+role,'source_variable':'pwr_block','type':'block','category':'machine','template':'minecraft:beehive',
                     'metadata':meta,'subtype_kind':'enum','creative_hidden':True,'loot_none':True,'source_texture':root+f'blocks/{texture}.png',
                     'textures':{face:'blocks/'+texture for face in ('side','top','front')},
                     'names':{'en_us':translations['en_us']['tile.pwr_block.name']+(' (Port)' if meta else ''),'pt_br':translations['pt_br'].get('tile.pwr_block.name') or translations['en_us']['tile.pwr_block.name']+(' (Porta)' if meta else '')},
                     'behavior_gap':'Internal block the controller swaps in for assembled parts (metadata 0 body, 1 port variant); remembers the original block and the core position. Hidden, no loot, connected textures are visual work.'})
    for f in fuels(source):
        suffix=f['member'].lower();key=f'item.pwr_fuel.{suffix}.name'
        rows.append({'id':'pwr_fuel__'+suffix,'source_variable':'pwr_fuel','type':'item','category':'component','metadata':f['metadata'],
                     'subtype_kind':'enum','source_enum':'EnumPWRFuel.'+f['member'],
                     'source_texture':f'{ASSETS}textures/items/pwr_fuel.{suffix}.png','names':names(key),
                     'behavior_gap':'Fuel table in machine-recipes/reactors_pwr.json; hot/depleted conversion and per-stack flux state belong to the native reactor.'})
    name,texture=registration(items,'pwr_printer','item')
    rows.append({'id':name,'source_variable':'pwr_printer','type':'item','category':'component',
                 'source_texture':f'{ASSETS}textures/items/{texture}.png','names':names('item.'+name+'.name'),
                 'behavior_gap':'Source ItemPWRPrinter opens a slice-printer GUI over a selected cuboid; native item/GUI logic.'})
    return rows

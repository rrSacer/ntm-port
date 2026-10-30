#!/usr/bin/env python3
"""Source block identities for the petroleum content batches."""
import copy
from generate import lang, registration

BASE = 'src/main/java/com/hbm/'
MACHINES = ('machine_liquefactor', 'machine_solidifier', 'machine_refinery', 'machine_fraction_tower', 'machine_mixer', 'machine_boiler', 'machine_industrial_boiler', 'machine_catalytic_cracker', 'machine_catalytic_reformer', 'machine_hydrotreater', 'machine_vacuum_distill', 'machine_coker', 'machine_well', 'machine_pumpjack', 'machine_fracking_tower', 'machine_transformer')


def build(source):
    blocks = (source / (BASE + 'blocks/ModBlocks.java')).read_text()
    translations = {loc:lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    result = []
    for var in MACHINES:
        name,texture = registration(blocks,var,'block')
        result.append({'id':name,'source_variable':var,'type':'block','category':'machine',
                       'template':'minecraft:beehive',
                       'names':{loc:tr['tile.'+name+'.name'] for loc,tr in translations.items()},
                       'source_texture':f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                       'textures':{face:'blocks/'+texture for face in ('side','top','front')},
                       'opaque_machine_textures':True,
                       'model_gap':'Source casing cube fallback; visual-gen owns original model, screen and multiblock'})
    for var,texture in [('steel_scaffold','scaffold_steel'),('steel_beam','steel_beam'),('steel_grate','grate_top'),('barricade','barricade')]:
        name,_=registration(blocks,var,'block')
        barricade=var=='barricade'
        result.append({'id':name,'source_variable':var,'type':'block','category':'decorative',
                       'template':'minecraft:stone' if barricade else 'minecraft:iron_block',
                       'source_texture':f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                       'names':{loc:tr['tile.'+name+'.name'] for loc,tr in translations.items()},
                       'block_properties':{'hardness':1 if barricade else 2 if var=='steel_grate' else 5,
                                           'explosion_resistance':2.5 if barricade else 5 if var=='steel_grate' else 15,
                                           'requires_tool':not barricade},
                       **({'loot_none':True,'creative_hidden':True} if barricade else
                          {'model_gap':'Structural cube fallback; source scaffold/beam/grate shapes, placement and scaffold colors not represented'})})
    scaffold=next(r for r in result if r['source_variable']=='steel_scaffold')
    scaffold.update(template='minecraft:quartz_pillar',metadata=0,subtype_kind='palette',
                    surface_textures={face:scaffold['source_texture'] for face in ('side','top','bottom')},
                    model_gap='Vanilla axis x/y/z placement; source fourth vertical-yaw orientation and custom scaffold collision/interaction remain gaps')
    for meta,color,en,pt in [(1,'red','Red','Vermelho'),(2,'white','White','Branco'),(3,'yellow','Yellow','Amarelo')]:
        row=copy.deepcopy(scaffold);row['id']='steel_scaffold__'+color;row['metadata']=meta
        row['source_texture']=f'src/main/resources/assets/hbm/textures/blocks/scaffold_{color}.png'
        row['surface_textures']={face:row['source_texture'] for face in ('side','top','bottom')}
        row['names']={loc:title+' ('+(en if loc=='en_us' else pt)+')' for loc,title in scaffold['names'].items()}
        result.append(row)
    grate=next(r for r in result if r['source_variable']=='steel_grate')
    grate['template']='minecraft:stone_slab'
    grate['model_gap']='Vanilla bottom/top/double slab states; source ten click-selected 2px heights and fine collision remain gaps. visual-gen owns source geometry.'
    for var in ['ore_oil','ore_oil_empty','ore_bedrock_oil','ore_oil_sand','oil_pipe']:
        name,texture=registration(blocks,var,'block')
        sand=var=='ore_oil_sand';bedrock=var=='ore_bedrock_oil';pipe=var=='oil_pipe'
        result.append({'id':name,'source_variable':var,'type':'block','category':'oil_deposit' if not pipe else 'oil_pipe',
                       'template':'minecraft:sand' if sand else 'minecraft:bedrock' if bedrock else 'minecraft:stone',
                       'source_texture':f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                       'names':{loc:tr['tile.'+name+'.name'] for loc,tr in translations.items()},
                       'block_properties':{'hardness':-1 if bedrock else 0.5 if sand else 5,
                                           'explosion_resistance':1000000 if bedrock else 1 if sand else 10,
                                           'requires_tool':not sand,
                                           **({'sounds':'minecraft:sand'} if sand else {})},
                       **({'creative_hidden':True,'loot_none':True} if pipe else {}),
                       'behavior_gap':'Oil extraction/depletion belongs to server logic; no independent tile entity. Sand keeps vanilla falling behavior.'})
    return result

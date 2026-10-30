#!/usr/bin/env python3
"""Source block identities for the petroleum content batches."""
from generate import lang, registration

BASE = 'src/main/java/com/hbm/'
MACHINES = ('machine_liquefactor', 'machine_solidifier')


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
    return result

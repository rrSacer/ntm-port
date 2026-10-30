#!/usr/bin/env python3
"""Stage-three/four machines and the four original electrode materials."""
import re
from bulk_recipes import clean
from generate import lang, registration

MACHINES = ('machine_pyrooven', 'machine_electrolyser', 'machine_arc_furnace')
BASE = 'src/main/java/com/hbm/'


def build(source):
    blocks = (source / (BASE + 'blocks/ModBlocks.java')).read_text()
    items = (source / (BASE + 'items/ModItems.java')).read_text()
    translations = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    result = []
    for var in MACHINES:
        name, texture = registration(blocks, var, 'block')
        # Java registers MachineArcFurnaceLarge under machine_arc_furnace.
        ident = name + '_large' if var == 'machine_arc_furnace' else name
        result.append({'id': ident, 'source_variable': var, 'type': 'block', 'category': 'machine',
                       'template': 'minecraft:beehive',
                       'names': {loc: tr['tile.' + name + '.name'] for loc, tr in translations.items()},
                       'source_texture': f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                       'textures': {face: 'blocks/' + texture for face in ('side', 'top', 'front')},
                       'opaque_machine_textures': True,
                       'model_gap': 'Steel casing cube fallback; visual-gen owns OBJ, screen and multiblock'})
    electrode_source = BASE + 'items/machine/ItemArcElectrode.java'
    text = clean((source / electrode_source).read_text())
    body = text.split('enum EnumElectrodeType {', 1)[1].split(';', 1)[0]
    members = re.findall(r'(\w+)\(\s*(\d+)\)', body)
    if len(members) != 4: raise ValueError('Electrode enum drift')
    for var in ('arc_electrode', 'arc_electrode_burnt'):
        name, texture = registration(items, var, 'item')
        for meta, (material, durability) in enumerate(members):
            suffix = material.lower()
            result.append({'id': name + '__' + suffix, 'source_variable': var, 'type': 'item', 'category': 'component',
                           'metadata': meta, 'subtype_kind': 'enum', 'source_enum': 'EnumElectrodeType.' + material,
                           'source_texture': f'src/main/resources/assets/hbm/textures/items/{texture}.{suffix}.png',
                           'names': {loc: tr[f'item.{name}.{suffix}.name'] for loc, tr in translations.items()},
                           **({'max_stack_size': 1} if var == 'arc_electrode' else {}),
                           'electrode': {'max_uses': int(durability), 'spent': var.endswith('_burnt'),
                                         'burnt_item': 'ntm:arc_electrode_burnt__' + suffix,
                                         'source': electrode_source},
                           'behavior_gap': 'Wear uses custom_data durability per processed batch; server logic owns consumption'})
    return result

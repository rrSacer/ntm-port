#!/usr/bin/env python3
"""Stage-one bedrock deposit, drill machines, raw ore and exact drill subtypes."""
import re
from bulk_recipes import clean
from generate import lang, registration

DRILL_SOURCE = 'src/main/java/com/hbm/items/machine/ItemDrillbit.java'
MACHINES = ('machine_excavator', 'machine_ore_slopper')


def drillbits(source):
    text = clean((source / DRILL_SOURCE).read_text())
    body = text.split('enum EnumDrillType {', 1)[1].split(';', 1)[0]
    result = []
    for match in re.finditer(r'(\w+)\s*\(([0-9.]+)D,\s*(\d+),\s*(\d+),\s*(true|false),\s*(true|false)\)', body):
        name, speed, tier, fortune, vein, silk = match.groups()
        result.append({'item': 'ntm:drillbit__' + name.lower(), 'name': name, 'ordinal': len(result),
                       'speed': float(speed), 'tier': int(tier), 'fortune': int(fortune),
                       'vein': vein == 'true', 'silk': silk == 'true'})
    if len(result) != 10: raise ValueError('Drill enum drift')
    return result


def build(source):
    blocks = (source / 'src/main/java/com/hbm/blocks/ModBlocks.java').read_text()
    translations = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    result = []
    for var in MACHINES:
        name, texture = registration(blocks, var, 'block')
        result.append({'id': name, 'source_variable': var, 'type': 'block', 'category': 'machine',
                       'template': 'minecraft:beehive', 'names': {loc: tr['tile.' + name + '.name'] for loc, tr in translations.items()},
                       'source_texture': f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                       'textures': {face: 'blocks/' + texture for face in ('side', 'top', 'front')},
                       'opaque_machine_textures': True,
                       'model_gap': 'Original steel casing cube fallback; visual-gen owns multiblock/OBJ/screen'})
    result.append({'id': 'ore_bedrock', 'source_variable': 'ore_bedrock', 'type': 'block', 'category': 'bedrock_deposit',
                   'template': 'minecraft:bedrock', 'texture_reference': 'minecraft:block/bedrock',
                   'block_properties': {'hardness': -1, 'explosion_resistance': 3600000}, 'loot_none': True,
                   'names': {loc: tr['tile.ore_bedrock.name'] for loc, tr in translations.items()},
                   'behavior_gap': 'No block entity: density/type/tier/acid/yield derived server-side from chunk; dynamic ore overlay omitted. Optional command/creative item is retained.'})
    result.append({'id': 'bedrock_ore_base', 'source_variable': 'bedrock_ore_base', 'type': 'item', 'category': 'processed_ore',
                   'source_texture': 'src/main/resources/assets/hbm/textures/items/bedrock_ore_new.png',
                   'names': {loc: tr['item.bedrock_ore_base.name'] for loc, tr in translations.items()},
                   'behavior_gap': 'Densities stored by server in custom_data; asset has no independent logic'})
    for bit in drillbits(source):
        name = bit['item'].split(':')[1]
        key = 'item.drillbit_' + bit['name'].lower() + '.name'
        result.append({'id': name, 'source_variable': 'drillbit', 'type': 'item', 'category': 'component',
                       'metadata': bit['ordinal'], 'subtype_kind': 'enum', 'source_enum': 'EnumDrillType.' + bit['name'],
                       'source_texture': 'src/main/resources/assets/hbm/textures/items/drillbit_' + bit['name'].lower() + '.png',
                       'names': {loc: tr[key] for loc, tr in translations.items()}, 'max_stack_size': 1,
                       'drill': {k: bit[k] for k in ('speed', 'tier', 'fortune', 'vein', 'silk')}})
    return result

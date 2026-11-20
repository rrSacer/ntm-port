#!/usr/bin/env python3
"""Source registrations for foundry blocks and the 26 static ItemMold subtypes."""
import re
from bulk_recipes import calls, clean, split
from generate import lang, registration

MOLD = 'src/main/java/com/hbm/items/machine/ItemMold.java'
BLOCKS = ('machine_crucible', 'heater_firebox', 'foundry_channel', 'foundry_tank',
          'foundry_outlet', 'foundry_slagtap', 'foundry_mold', 'foundry_basin')


def molds(source):
    text = clean((source / MOLD).read_text())
    result = []
    for pos, expr, _ in calls(text, r'registerMold'):
        match = re.fullmatch(r'new (MoldShape|MoldMulti|MoldBlock)\((.*)\)', expr, re.S)
        if not match: continue  # custom shape loops and method declaration
        args = split(match[2])
        result.append({'id': int(args[0]), 'size': {'S': 0, 'L': 1}[args[1]],
                       'name': args[2].strip('"'), 'class': match[1], 'args': args[3:],
                       'source': MOLD + ':' + str(text[:pos].count('\n') + 1)})
    if len(result) != 26: raise ValueError('ItemMold default registration count drift')
    return result


def build(source):
    blocks = (source / 'src/main/java/com/hbm/blocks/ModBlocks.java').read_text()
    items = (source / 'src/main/java/com/hbm/items/ModItems.java').read_text()
    translations = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    result = []
    for var in BLOCKS:
        name, texture = registration(blocks, var, 'block')
        result.append({'id': name, 'source_variable': var, 'type': 'block', 'category': 'machine',
                       'template': 'minecraft:beehive',
                       'names': {loc: tr['tile.' + name + '.name'] for loc, tr in translations.items()},
                       'source_texture': 'src/main/resources/assets/hbm/textures/blocks/' + texture + '.png',
                       'textures': {face: 'blocks/' + texture for face in ('side', 'top', 'front')},
                       'opaque_machine_textures': True,
                       'model_gap': 'Source block texture on oriented cube fallback; visual pipeline owns TESR/multiblock',
                       'source_orientation': var in ('machine_crucible', 'heater_firebox', 'foundry_outlet')})
    # What the slag tap pours (`BlockDynamicSlag`): a block with no item and no facing, whose height is its fill in
    # eighths (`layer`); its colour is the material's in the Java and neutral here (declared gap).
    name, texture = registration(blocks, 'slag', 'block')
    result.append({'id': name, 'source_variable': 'slag', 'type': 'block', 'category': 'machine',
                   'template': 'minecraft:iron_block', 'layer_block': True, 'loot_none': True,
                   'names': {'en_us': 'Slag (DEBUG)', 'pt_br': translations['pt_br']['tile.slag.name']},
                   'source_texture': 'src/main/resources/assets/hbm/textures/blocks/' + texture + '.png',
                   'block_properties': {'requires_tool': False, 'hardness': 5.0, 'explosion_resistance': 10.0, 'sounds': 'minecraft:iron_block'},
                   'model_gap': 'Neutral colour (the Java tints it by the material); no flow of its own'})
    # The eternal coal (`FuelHandler`: single * 16; its own container item, so a firebox never uses it up): obtained from the
    # forgotten bricks, hidden from the creative tabs in the Java too.
    name, texture = registration(items, 'coal_eternal', 'item')
    result.append({'id': name, 'source_variable': 'coal_eternal', 'type': 'item', 'category': 'component',
                   'source_texture': 'src/main/resources/assets/hbm/textures/items/' + texture + '.png', 'max_stack_size': 1})
    name, _ = registration(items, 'mold', 'item')
    for mold in molds(source):
        shape = mold['args'][0].removeprefix('MaterialShapes.') if mold['class'] != 'MoldMulti' else mold['name']
        # Shape titles use the first MaterialShapes prefix, not the item suffix.
        prefixes = {'NUGGET':'nugget', 'BILLET':'billet', 'INGOT':'ingot', 'PLATE':'plate', 'WIRE':'wireFine',
                    'CASTPLATE':'plateTriple', 'DENSEWIRE':'wireDense', 'SHELL':'shell', 'PIPE':'ntmpipe',
                    'BLOCK':'block', 'LIGHTBARREL':'barrelLight', 'HEAVYBARREL':'barrelHeavy',
                    'LIGHTRECEIVER':'receiverLight', 'HEAVYRECEIVER':'receiverHeavy', 'MECHANISM':'gunMechanism', 'STOCK':'stock', 'GRIP':'grip'}
        key = prefixes.get(shape, shape)
        amount = int(mold['args'][1]) if mold['class'] != 'MoldMulti' and len(mold['args']) > 1 else 1
        names = {loc: tr['item.mold.name'] + ' (' + tr.get('shape.' + key, key) + (f' ×{amount}' if amount > 1 else '') + ')'
                 for loc, tr in translations.items()}
        result.append({'id': name + '__' + mold['name'], 'source_variable': 'mold', 'source_class': 'ItemMold',
                       'type': 'item', 'category': 'component', 'metadata': mold['id'], 'subtype_kind': 'mold',
                       'source_texture': 'src/main/resources/assets/hbm/textures/items/mold_' + mold['name'] + '.png',
                       'names': names, 'source_location': mold['source'], 'mold_size': mold['size'],
                       'source_recipe_machine': [{'machine':'anvil', 'source':'AnvilRecipes.java mold engraving; vanilla crafting only creates mold_base'}]})
    return result

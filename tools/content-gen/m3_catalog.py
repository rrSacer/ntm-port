#!/usr/bin/env python3
"""M3 block contracts and exact, metadata-split FluidContainerRegistry items."""
import re
import json
from pathlib import Path
from generate import HERE, ROOT, lang, registration, write_json
from bulk_recipes import clean, split

SOURCE = ROOT.parent / 'ntm-src'


def fluids(source):
    path = 'src/main/java/com/hbm/inventory/fluid/Fluids.java'
    text = clean((source / path).read_text())
    result = []
    for match in re.finditer(r'\b([A-Z][A-Z0-9_]*)\s*=\s*new FluidType\((.*?);', text, re.S):
        body = match[2]
        head = re.match(r'(?:(\d+),\s*)?"([^"]+)",\s*0x([0-9a-fA-F]{6})', body)
        if not head:
            raise ValueError('Unsupported fluid declaration: ' + match[1])
        number = len(result)
        if head[1] is not None and int(head[1]) != number:
            raise ValueError('Fluid ordinal drift: ' + match[1])
        containers = {}
        for descriptor, arguments in re.findall(r'new CD_(Canister|Gastank)\(([^)]*)\)', body):
            colors = re.findall(r'0x([0-9a-fA-F]{6})', arguments)
            if len(colors) != (1 if descriptor == 'Canister' else 2):
                raise ValueError('Unsupported container palette: ' + body)
            containers[descriptor] = colors
        temperature = re.search(r'\.setTemp\((-?[\d_]+)\)', body)
        result.append({'id': head[2].lower(), 'string_id': head[2].lower(), 'source_id': number, 'source_variable': match[1],
                       'metadata': number, 'color': head[3],
                       'temp': int(temperature[1].replace('_', '')) if temperature else 20,
                       'no_container': bool(re.search(r'\bNOCON\b', body)),
                       'lead_container': bool(re.search(r'\bLEADCON\b', body)),
                       'descriptors': containers,
                       'source': path + ':' + str(text[:match.start()].count('\n') + 1)})
    if not result or result[0]['id'] != 'none':
        raise ValueError('Missing NONE fluid at metadata zero')
    return result


def build(source=SOURCE, output=ROOT / 'content'):
    blocks = (source / 'src/main/java/com/hbm/blocks/ModBlocks.java').read_text()
    items = (source / 'src/main/java/com/hbm/items/ModItems.java').read_text()
    translations = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    rows = []
    names = {
        'machine_assembly_machine': ('Assembly Machine', 'Montadora'),
        'machine_chemical_plant': ('Chemical Plant', 'Planta Química'),
        'machine_purex': ('PUREX', 'PUREX'),
        'machine_crystallizer': ('Ore Acidizer', 'Acidificador de Minério'),
        'machine_centrifuge': ('Centrifuge', 'Centrífuga'),
        'machine_gascent': ('Gas Centrifuge', 'Centrífuga de Gás'),
        'barrel_steel': ('Steel Barrel', 'Barril de Aço'),
        'machine_fluidtank': ('Fluid Tank', 'Tanque de Fluidos'),
        'machine_shredder': ('Shredder', 'Triturador'),
        'machine_blast_furnace': ('Blast Furnace', 'Alto-Forno'),
    }
    for var, (en, pt) in names.items():
        name, texture = registration(blocks, var, 'block')
        front = 'blocks/' + var if var not in ('machine_assembly_machine', 'machine_chemical_plant', 'machine_purex') else 'models/machines/' + var.removeprefix('machine_')
        surfaces = {'side': 'blocks/' + str(texture), 'top': 'blocks/' + str(texture), 'front': front}
        if var == 'machine_shredder':
            surfaces = {face: 'blocks/machine_shredder_' + face + '_alt' for face in ('side', 'top', 'bottom', 'front')}
            front = surfaces['front']
        if var == 'machine_blast_furnace':
            front = 'blocks/brick_fire'
            surfaces = {face: front for face in ('side', 'top', 'front')}
        rows.append({'id': name, 'source_variable': var, 'type': 'block', 'category': 'machine',
                     'template': 'minecraft:beehive', 'names': {'en_us': en, 'pt_br': pt},
                     'source_texture': 'src/main/resources/assets/hbm/textures/' + front + '.png',
                     'textures': surfaces,
                     'opaque_machine_textures': True,
                     'model_gap': 'NTM TESR/multiblock represented by one oriented cube using original texture; Rust owns processing/storage',
                     **({'fluid_capacity_mb': 8000} if var == 'machine_crystallizer' else {}),
                     **({'fluid_capacity_mb': 16000 if var == 'barrel_steel' else 256000} if var in ('barrel_steel', 'machine_fluidtank') else {})})
    name, texture = registration(blocks, 'fluid_duct_neo', 'block')
    rows.append({'id': name, 'source_variable': 'fluid_duct_neo', 'type': 'block', 'category': 'duct',
                 'icon_cube': True,
                 'template': 'minecraft:stone', 'source_texture': 'src/main/resources/assets/hbm/textures/blocks/' + texture + '.png',
                 'names': {'en_us': 'Fluid Duct', 'pt_br': 'Duto de Fluidos'},
                 'model_gap': 'Full cube; connections/fluid type stored by Rust logic'})
    # These three ordinary MaterialShapes were missing from the M2 shape table
    # and are essential assembly ingredients. Reuse the same material identity.
    base_rows = json.loads((HERE / 'bulk.json').read_text())['entries']
    materials = {row['source_material']: row for row in base_rows if 'source_material' in row}
    mats = clean((source / 'src/main/java/com/hbm/inventory/material/Mats.java').read_text())
    dictionary = (source / 'src/main/java/com/hbm/inventory/OreDictManager.java').read_text()
    for line in mats.splitlines():
        material = re.search(r'\b(MAT_\w+)\s*=\s*make\w*\([^,]+,\s*(\w+)', line)
        autogen = re.search(r'\.setAutogen\(([^)]*)\)', line)
        if not material or not autogen: continue
        forms = split(autogen[1]); base = materials.get(material[1])
        if base is None: continue
        colors = re.findall(r'0x([0-9A-Fa-f]{6})', line)
        for var, shape, key, en, pt in [('bolt', 'BOLT', 'boltntm', '{} Bolt', 'Parafuso de {}'), ('shell', 'SHELL', 'shellntm', '{} Shell', 'Carcaça de {}'), ('pipe', 'PIPE', 'pipentm', '{} Pipe', 'Tubo de {}')]:
            if shape not in forms: continue
            name, texture = registration(items, var, 'item')
            tint = {'light': colors[0], 'dark': colors[1]} if len(colors) > 1 and colors[0] != colors[1] else {'multiply': colors[0]}
            frame = re.search(r'\b' + material[2] + r'\s*=\s*new DictFrame\("([^"]+)"', dictionary)
            key = (frame[1] if frame else material[2]).lower()
            localized = {loc: translations[loc].get('hbmmat.' + key, base['material'].replace('_', ' ').title()) for loc in translations}
            rows.append({'id': name + '__' + base['material'], 'source_variable': var, 'type': 'item', 'category': 'component',
                         'subtype_kind': 'material', 'source_material': material[1], 'material': base['material'],
                         'metadata': base['metadata'], 'source_texture': 'src/main/resources/assets/hbm/textures/items/' + texture + '.png',
                         'tint': tint, 'names': {'en_us': en.format(localized['en_us']), 'pt_br': pt.format(localized['pt_br'])},
                         'source_recipe_machine': [{'machine': 'foundry', 'source': 'Mats.java MaterialShapes autogen; no invented crafting bridge'}]})
    registry = fluids(source)
    families = {
        'fluid_tank_full': ('fluid_tank', 'Tank of {}', 'Tanque de {}', 1000, 'fluid_tank_empty'),
        'fluid_tank_lead_full': ('fluid_tank_lead', 'Lead Tank of {}', 'Tanque de Chumbo de {}', 1000, 'fluid_tank_lead_empty'),
        'fluid_barrel_full': ('fluid_barrel', 'Barrel of {}', 'Barril de {}', 16000, 'fluid_barrel_empty'),
        'canister_full': ('canister_empty', 'Canister of {}', 'Galão de {}', 1000, 'canister_empty'),
        'gas_full': ('gas_empty', 'Gas Tank of {}', 'Cilindro de {}', 1000, 'gas_empty'),
        'fluid_pack_full': ('fluid_pack', 'Fluid Package of {}', 'Pacote de {}', 32000, 'fluid_pack_empty'),
    }
    for fluid in registry[1:]:
        variants = []
        if not fluid['no_container']:
            variants.extend(('fluid_tank_lead_full', 'fluid_pack_full'))
            if not fluid['lead_container']:
                variants.extend(('fluid_tank_full', 'fluid_barrel_full'))
        if 'Canister' in fluid['descriptors']: variants.append('canister_full')
        if 'Gastank' in fluid['descriptors']: variants.append('gas_full')
        for var in variants:
            name, _ = registration(items, var, 'item')
            base, en, pt, amount, empty = families[var]
            palette = fluid['descriptors'].get('Canister' if var == 'canister_full' else 'Gastank')
            layers = [('gas_bottle', palette[0]), ('gas_label', palette[1])] if var == 'gas_full' else [('canister_overlay', palette[0])] if var == 'canister_full' else [(base + '_overlay', fluid['color'])]
            localized = {loc: translations[loc].get('hbmfluid.' + fluid['id'], translations['en_us'].get('hbmfluid.' + fluid['id'], fluid['id'].replace('_', ' ').title())) for loc in translations}
            rows.append({'id': name + '__' + fluid['id'], 'source_variable': var, 'type': 'item',
                         'category': 'fluid_container', 'subtype_kind': 'fluid', 'metadata': fluid['metadata'],
                         'source_fluid': fluid['source_variable'],
                         'source_texture': f'src/main/resources/assets/hbm/textures/items/{base}.png',
                         'overlay_textures': [{'source': f'src/main/resources/assets/hbm/textures/items/{tex}.png', 'tint': {'multiply': color}} for tex, color in layers],
                         'names': {'en_us': en.format(localized['en_us']), 'pt_br': pt.format(localized['pt_br'])},
                         'fluid': {'id': fluid['id'], 'amount_mb': amount, 'empty': 'ntm:' + empty},
                         'behavior_gap': 'Filled item only; server logic must consume/return container, never a temporary crafting bridge',
                         'source_container': 'src/main/java/com/hbm/inventory/FluidContainerRegistry.java:69-91'})
    name, texture = registration(items, 'battery_creative', 'item')
    rows.append({'id': name, 'source_variable': 'battery_creative', 'type': 'item', 'category': 'component',
                 'max_stack_size': 1,
                 'source_texture': 'src/main/resources/assets/hbm/textures/items/' + texture + '.png',
                 'names': {loc: tr['item.battery_creative.name'] for loc, tr in translations.items()},
                 'behavior_gap': 'Creative-only infinite battery; server logic owns charging, no crafting recipe in NTM'})
    import bedrock_catalog
    rows.extend(bedrock_catalog.build(source))
    import bedrock_processing_catalog
    rows.extend(bedrock_processing_catalog.build(source))
    import catalog_completion
    rows.extend(catalog_completion.build(source))
    import petroleum_catalog
    rows.extend(petroleum_catalog.build(source))
    import reactor_catalog
    rows.extend(reactor_catalog.build(source))
    rows.extend(reactor_catalog.pile_tools(source))
    import rbmk_catalog
    rows.extend(rbmk_catalog.build(source))
    import pwr_catalog
    rows.extend(pwr_catalog.build(source))
    import watz_catalog
    rows.extend(watz_catalog.build(source))
    import catalog_k3
    rows.extend(catalog_k3.build(source))
    import energy_catalog
    rows.extend(energy_catalog.build(source))
    import petroleum_auxiliary_catalog
    rows.extend(petroleum_auxiliary_catalog.build(source))
    import foundry_catalog
    rows.extend(foundry_catalog.build(source))
    import anvil_catalog
    rows.extend(anvil_catalog.build(source))
    for row in rows:
        for texture in [*([row['source_texture']] if row.get('source_texture') else []), *[x['source'] for x in row.get('overlay_textures', [])]]:
            if not (source / texture).is_file(): raise ValueError('Missing texture: ' + texture)
    for var in ('blades_steel', 'blades_titanium', 'blades_desh'):
        name, texture = registration(items, var, 'item')
        durability = int(re.search(r'\b' + var + r'\s*=\s*new ItemBlades\((\d+)\)', items)[1])
        rows.append({'id': name, 'source_variable': var, 'type': 'item', 'category': 'component',
                     'source_texture': 'src/main/resources/assets/hbm/textures/items/' + texture + '.png',
                     'max_stack_size': 1, 'durability': {'max_damage': durability, 'wear': durability > 0},
                     **({'max_damage': durability} if durability else {})})
    m1_categories = {row['source_variable']: row['category'] for row in json.loads((HERE / 'm1.json').read_text())['entries']}
    stamp_declarations = list(re.finditer(r'\b(\w+)\s*=\s*new ItemStamp\((\d+),\s*StampType\.(\w+)\)', clean(items)))
    if len(stamp_declarations) != 32:
        raise ValueError(f'Expected 32 ItemStamp declarations, found {len(stamp_declarations)}')
    for match in stamp_declarations:
        var, durability = match[1], int(match[2])
        name, texture = registration(items, var, 'item')
        rows.append({'id': name, 'source_variable': var, 'type': 'item',
                     'category': m1_categories.get(var, 'component'),
                     'source_texture': 'src/main/resources/assets/hbm/textures/items/' + texture + '.png',
                     'max_stack_size': 1, 'durability': {'max_damage': durability, 'wear': durability > 0},
                     **({'max_damage': durability} if durability else {})})
    write_json(HERE / 'm3.json', {'schema': 1, 'entries': rows})
    write_json(output / 'fluids.json', {'schema': 1, 'namespace': 'ntm', 'fluids': registry})
    backlog = output / 'machines_backlog.json'
    if backlog.is_file():
        data = json.loads(backlog.read_text())
        for entry in data['entries']:
            k3_ids=['ntm:'+r['id'] for r in rows if r['type']=='block' and r.get('source_class_file') and r['source_variable']==entry['source_variable']]
            if k3_ids:
                entry.update(content_id=k3_ids[0],content_ids=k3_ids,status='K3 content declared; native registration/behavior and faithful model deferred')
                continue
            if entry['source_variable'] in set(names) | set(foundry_catalog.BLOCKS) | set(bedrock_catalog.MACHINES) | set(bedrock_processing_catalog.MACHINES) | set(petroleum_catalog.MACHINES) | set(energy_catalog.MACHINES) | set(reactor_catalog.MACHINES) | set(rbmk_catalog.MACHINES) | set(pwr_catalog.MACHINES) | set(watz_catalog.MACHINES):
                entry['content_id'] = 'ntm:' + entry['id'] + ('_large' if entry['source_variable'] == 'machine_arc_furnace' else '')
                if entry['source_variable'] in set(reactor_catalog.MACHINES) | set(pwr_catalog.MACHINES):
                    entry['content_ids']=['ntm:'+r['id'] for r in rows if r['type']=='block' and r['source_variable']==entry['source_variable']]
                    entry['content_id']=entry['content_ids'][0]
                entry['status'] = ('Reactor R4 (Watz) content declared; native structure/fuel logic and visuals deferred'
                                   if entry['source_variable'] in watz_catalog.MACHINES else
                                   'Reactor R3 (PWR) content declared; native structure/fuel logic and visuals deferred'
                                   if entry['source_variable'] in pwr_catalog.MACHINES else
                                   'Reactor R2 (RBMK) content declared; native column/console/fuel logic and visuals deferred'
                                   if entry['source_variable'] in rbmk_catalog.MACHINES else
                                   'Reactor R1 content declared; native structure/fuel logic and visuals deferred'
                                   if entry['source_variable'] in reactor_catalog.MACHINES else
                                   'Energy content declared; heat/processing and visuals tracked separately'
                                   if entry['source_variable'] in energy_catalog.MACHINES else
                                   'Crystallizer content declared; stage-two processing/upgrades tracked separately'
                                   if entry['source_variable'] == 'machine_crystallizer' else
                                   'Petroleum content declared; processing and visuals tracked separately'
                                   if entry['source_variable'] in petroleum_catalog.MACHINES else
                                   'Bedrock stage-three/four content declared; processing and visuals tracked separately'
                                   if entry['source_variable'] in bedrock_processing_catalog.MACHINES else
                                   'Bedrock stage-one content contract declared; extraction/slopping and visuals tracked by their agents'
                                   if entry['source_variable'] in bedrock_catalog.MACHINES else
                                   'Foundry content contract declared; heat/processing/flow/casting tracked by the machine and visual agents'
                                   if entry['source_variable'] in foundry_catalog.BLOCKS else
                                   'PUREX content contract declared; processing/multiblock logic and visuals tracked separately'
                                   if entry['source_variable'] == 'machine_purex' else
                                   'Blast furnace content contract declared; processing/multiblock logic tracked separately'
                                   if entry['source_variable'] == 'machine_blast_furnace' else
                                   'Shredder content contract declared; processing/storage logic tracked separately by the machine agent'
                                   if entry['source_variable'] == 'machine_shredder' else
                                   'M3 content contract declared; processing/storage logic tracked separately by the M3 machine agent')
        write_json(backlog, data)
    print(f'M3: {len(registry)} fluid definitions; {len(rows)} content contracts')
    return registry


if __name__ == '__main__': build()

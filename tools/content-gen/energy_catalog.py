"""Source identities for the approved nonreactor energy content batches."""
from generate import lang, registration

BASE = 'src/main/java/com/hbm/'
MACHINES = ('heater_oven', 'heater_oilburner', 'heater_electric', 'heater_heatex',
            'machine_steam_engine', 'machine_industrial_turbine', 'machine_chungus',
            'machine_condenser', 'machine_condenser_powered')


def build(source):
    text = (source / (BASE + 'blocks/ModBlocks.java')).read_text()
    translations = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    rows = []
    for variable in MACHINES:
        name, texture = registration(text, variable, 'block')
        rows.append({'id': name, 'source_variable': variable, 'type': 'block', 'category': 'machine',
                     'template': 'minecraft:beehive',
                     'names': {loc: tr['tile.' + name + '.name'] for loc, tr in translations.items()},
                     'source_texture': f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                     'textures': {face: 'blocks/' + texture for face in ('side', 'top', 'front')},
                     'opaque_machine_textures': True,
                     'model_gap': 'Source casing cube fallback; visual-gen owns source model, screen and multiblock'})
    items = (source / (BASE + 'items/ModItems.java')).read_text()
    name, texture = registration(items, 'gear_large', 'item')
    for metadata, material in [(0, 'iron'), (1, 'steel')]:
        key = 'item.' + name + ('_steel' if metadata else '') + '.name'
        rows.append({'id': name + '__' + material, 'source_variable': 'gear_large',
                     'type': 'item', 'category': 'component', 'metadata': metadata,
                     'subtype_kind': 'palette', 'template': 'minecraft:iron_ingot', 'max_stack_size': 64,
                     'source_texture': f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                     'names': {loc: tr[key] for loc, tr in translations.items()},
                     'behavior_gap': 'Source ItemGear has metadata/localization only; Stirling repair interactions belong to machine logic.'})
    return rows

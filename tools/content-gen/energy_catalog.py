"""Source identities for the approved nonreactor energy content batches."""
import re
from bulk_recipes import clean
from generate import lang, registration

BASE = 'src/main/java/com/hbm/'
MACHINES = ('heater_oven', 'heater_oilburner', 'heater_electric', 'heater_heatex',
            'machine_steam_engine', 'machine_industrial_turbine', 'machine_chungus',
            'machine_condenser', 'machine_condenser_powered',
            'machine_diesel', 'machine_combustion_engine', 'machine_turbofan', 'machine_turbinegas',
            'machine_solar_boiler', 'solar_mirror', 'machine_stirling', 'machine_stirling_steel', 'machine_stirling_creative')


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
    # ItemPistons: one icon, name and efficiency row per EnumPistonType member.
    members = re.search(r'enum EnumPistonType \{(.*?);\s*public double', clean((source / (BASE + 'items/machine/ItemPistons.java')).read_text()), re.S)
    members = re.findall(r'(\w+)\s*\(', members[1])
    if members != ['STEEL', 'DURA', 'DESH', 'STARMETAL']: raise ValueError('Piston enum drift: ' + repr(members))
    for metadata, member in enumerate(members):
        suffix = member.lower()
        rows.append({'id': f'piston_set__{suffix}', 'source_variable': 'piston_set', 'type': 'item', 'category': 'component',
                     'metadata': metadata, 'subtype_kind': 'enum', 'source_enum': 'EnumPistonType.' + member,
                     'template': 'minecraft:iron_ingot', 'max_stack_size': 1,
                     'source_texture': f'src/main/resources/assets/hbm/textures/items/piston_set_{suffix}.png',
                     'names': {loc: tr[f'item.piston_set_{suffix}.name'] for loc, tr in translations.items()},
                     'behavior_gap': 'Efficiency matrix lives in machine-recipes/combustion.json; the combustion engine slot logic belongs to machine logic.'})
    # ItemStarmetal is a four-icon item, but only metadata 0 has recipes or lang in the source.
    name, texture = registration(items, 'ingot_starmetal', 'item')
    rows.append({'id': name, 'source_variable': 'ingot_starmetal', 'type': 'item', 'category': 'ingot',
                 'source_texture': f'src/main/resources/assets/hbm/textures/items/{texture}.png',
                 'names': {loc: tr['item.ingot_starmetal.name'] for loc, tr in translations.items()},
                 'behavior_gap': 'Astra/ursa/orion icon variants (metadata 1-3) are hidden cosmetic subtypes with no recipe or localization; only the base ingot is declared.'})
    return rows

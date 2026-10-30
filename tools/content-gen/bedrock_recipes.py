#!/usr/bin/env python3
"""Exact static bedrock grades/types, centrifuge expansion and fragment packing."""
import json
import math
import re
from pathlib import Path
from fractions import Fraction
from anvil_recipes import snapshot
from bedrock_catalog import drillbits
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import MachineResolver, integer

BASE = 'src/main/java/com/hbm/'
ORE = BASE + 'items/special/ItemBedrockOreNew.java'
CENT = BASE + 'inventory/recipes/CentrifugeRecipes.java'
WORLD = BASE + 'world/feature/BedrockOre.java'
CONFIG = BASE + 'config/WorldConfig.java'
ASSEMBLY = BASE + 'inventory/recipes/AssemblyMachineRecipes.java'
FIELDS = ['primary1', 'primary2'] + [group + str(i) for group in ('byproductAcid', 'byproductSolvent', 'byproductRad') for i in range(1, 4)]


def enum_values(text, name):
    body = text.split('enum ' + name + ' {', 1)[1].split(';', 1)[0]
    return [(m[1], split(m[2])) for part in split(body) if (m := re.fullmatch(r'(\w+)\((.*)\)', part, re.S))]


def tables(source, rows):
    text = clean((source / ORE).read_text())
    fragments = {r['source_material']: r['id'].split('__')[1] for r in rows if r['source_variable'] == 'bedrock_ore_fragment'}
    types = []; missing = []
    for name, args in enum_values(text, 'BedrockOreType'):
        row = {'name': name, 'ordinal': len(types), 'suffix': json.loads(args[2])}
        for field, expr in zip(FIELDS, args[3:], strict=True):
            match = re.fullmatch(r'o\((MAT_\w+),\s*(\d+)\)', expr)
            if not match: raise Unsupported('bedrock output: ' + expr)
            material = fragments.get(match[1])
            if material is None:
                missing.append({'type': name, 'field': field, 'material': match[1]})
                material = match[1][4:].lower()
            row[field] = [material, int(match[2])]
        for group in ('byproductAcid', 'byproductSolvent', 'byproductRad'):
            row[group] = [row[group + str(i)] for i in range(1, 4)]
        types.append(row)
    grades = [{'name': name, 'ordinal': i, 'prefix': json.loads(args[1]),
               'items': {t['suffix']: 'ntm:bedrock_ore__' + name.lower() + '_' + t['suffix'] for t in types}}
              for i, (name, args) in enumerate(enum_values(text, 'BedrockOreGrade'))]
    known = {r['id'] for r in rows}
    if len(types) != 6 or len(grades) != 26: raise ValueError('Bedrock enum count drift')
    if any(ident not in known for g in grades for ident in g['items'].values()): raise ValueError('Missing grade identity')
    return types, grades, missing


def extract_count(amount, multiplier):
    return min(math.ceil(amount * Fraction(multiplier)), 64)


def centrifuge(source, rows, types):
    text = clean((source / CENT).read_text()); result = []; skipped = []; known = {r['id'] for r in rows}
    resolver = MachineResolver(source, rows)
    def stack(expr, ore_type):
        make = re.fullmatch(r'ItemBedrockOreNew.make\(BedrockOreGrade\.(\w+),\s*type(?:,\s*(\d+))?\)', expr)
        if make: return ['ntm:bedrock_ore__' + make[1].lower() + '_' + ore_type['suffix'], int(make[2] or 1)]
        extract = re.fullmatch(r'ItemBedrockOreNew.extract\(type\.(\w+),\s*([0-9.]+)\)', expr)
        if extract:
            material, amount = ore_type[extract[1]]
            return ['ntm:bedrock_ore_fragment__' + material, extract_count(amount, extract[2])]
        return list(resolver.item(expr))
    for pos, args, _ in calls(text, r'recipes\.put'):
        if 'ItemBedrockOreNew.make' not in args: continue
        arguments = split(args); origin = CENT + ':' + str(text[:pos].count('\n') + 1)
        match = re.fullmatch(r'new ComparableStack\((.*)\)', arguments[0], re.S)
        array = re.fullmatch(r'new ItemStack\[\]\s*\{(.*)\}', arguments[1], re.S)
        if not match or not array: raise Unsupported('bedrock centrifuge structure drift')
        for ore_type in types:
            try:
                input_stack = stack(match[1], ore_type); outputs = [stack(x, ore_type) for x in split(array[1])]
                absent = [x[0] for x in [input_stack, *outputs] if x[0].startswith('ntm:') and x[0] not in known]
                if absent: raise Unsupported('missing catalog identities: ' + ', '.join(absent))
                result.append({'name': 'bedrock.centrifuge.' + input_stack[0].split('__')[1], 'source': origin,
                               'inputItems': [{'ids': [input_stack[0]], 'count': input_stack[1]}], 'outputItems': outputs})
            except Unsupported as error:
                skipped.append({'machine': 'centrifuge', 'type': ore_type['suffix'], 'source': origin, 'reason': str(error), 'expression': args})
    if len({r['inputItems'][0]['ids'][0] for r in result}) != len(result): raise ValueError('Duplicate bedrock centrifuge input')
    return result, skipped


def fragment_recipes(source, rows):
    resolver, materials, _, _ = snapshot(source, rows)
    byconstant = {m['constant']: m for m in materials}; recipes = []; skipped = []
    for row in rows:
        if row['source_variable'] != 'bedrock_ore_fragment': continue
        material = byconstant[row['source_material']]; name = material['names'][0]
        choice = next(((prefix, resolver.ordered_ore[prefix + name]) for prefix in ('dust', 'gem', 'crystal', 'ingot', 'billet', 'ore') if resolver.ordered_ore.get(prefix + name)), None)
        if choice is None:
            skipped.append({'machine': 'crafting', 'item': row['id'], 'reason': 'No material OreDict output in port; original fallback ModItems.nothing is not invented'})
            continue
        prefix, alternatives = choice; output = alternatives[0]
        # MineralRecipes.addBilletFragment uses six; all current fragments find an earlier form.
        pattern = ['###', '###'] if prefix == 'billet' else ['###', '###', '###']
        recipes.append({'name': 'bedrock_fragment__' + row['id'].split('__')[1], 'source': BASE + 'inventory/OreDictManager.java:656-662',
                        'input': row['id'], 'count': sum(len(x) for x in pattern), 'output': output, 'ore_key': prefix + name,
                        'recipe': {'type': 'minecraft:crafting_shaped', 'pattern': pattern, 'key': {'#': row['id']}, 'result': {'id': output, 'count': 1}}})
    return recipes, skipped


def assembly(source, rows):
    resolver = MachineResolver(source, rows); text = clean((source / ASSEMBLY).read_text()); result = []; skipped = []
    for pos, args, _ in calls(text, r'this\.register'):
        if not re.search(r'outputItems\(new ItemStack\(Mod(?:Items\.drillbit|Blocks\.machine_(?:excavator|ore_slopper))\b', args): continue
        name = re.search(r'new GenericRecipe\("([^"]+)"\)', args)[1]
        origin = ASSEMBLY + ':' + str(text[:pos].count('\n') + 1)
        try:
            methods = {method[1:]: split(value) for _, value, method in calls(args, r'\.\w+')}
            inputs = [resolver.input(x) for x in methods['inputItems']]
            result.append({'name': name, 'source': origin, 'duration': integer(methods['setup'][0]), 'power': integer(methods['setup'][1]),
                           'inputItems': [{'ids': choices, 'count': representative[1]} for representative, choices in inputs],
                           'outputItems': [list(resolver.item(x)) for x in methods['outputItems']]})
        except Unsupported as error: skipped.append({'machine': 'assembly', 'name': name, 'source': origin, 'reason': str(error), 'expression': args})
    return result, skipped


def export(source, output, rows):
    types, grades, missing = tables(source, rows)
    cent, skipped = centrifuge(source, rows, types)
    packing, gaps = fragment_recipes(source, rows); skipped.extend(gaps)
    ass, gaps = assembly(source, rows); skipped.extend(gaps)
    world = clean((source / WORLD).read_text()); config = clean((source / CONFIG).read_text()); resolver = MachineResolver(source, rows)
    nether = []
    for pos, args, _ in calls(world, r'registerBedrockOre'):
        parts = split(args)
        if len(parts) != 3 or not parts[1].startswith('new BedrockOreDefinition('): continue
        inner = split(next(calls(parts[1], r'new BedrockOreDefinition'))[1]); ident, count = resolver.item(inner[0])
        weight = int(re.search(r'\b' + parts[2].split('.')[-1] + r'\s*=\s*(\d+)\s*;', config)[1])
        nether.append({'item': ident, 'count': count, 'weight': weight, 'tier': integer(inner[1]), 'source': WORLD + ':' + str(world[:pos].count('\n') + 1)})
    data = {'schema': 1, 'types': types, 'grades': grades, 'nether': nether, 'drillbits': drillbits(source),
            'centrifuge': cent, 'assembly': ass, 'fragment_packing': [{k:v for k,v in r.items() if k != 'recipe'} for r in packing],
            'skipped': skipped, 'missing_fragments': missing,
            'coverage': {'types': len(types), 'grades': len(grades), 'grade_items': sum(len(g['items']) for g in grades), 'centrifuge': len(cent), 'assembly': len(ass), 'fragment_packing': len(packing), 'skipped': len(skipped)},
            'notes': ['Only static content: deposits and raw density custom_data are owned by server logic.', 'Legacy ore_bedrock__* items are not used by this chain.', 'Centrifuge output slots remain separate, including duplicates; extraction is min(ceil(amount*k),64).']}
    target = output / 'machine-recipes/bedrock.json'; target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(data, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(json.dumps({'bedrock': data['coverage']}))
    return data


def generate_crafting(source, rows, emit):
    if not any(r['id'] == 'ntm:bedrock_ore_base' for r in rows): return
    for recipe in fragment_recipes(source, rows)[0]: emit('data/ntm/recipe/' + recipe['name'] + '.json', recipe['recipe'])


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

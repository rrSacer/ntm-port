#!/usr/bin/env python3
"""Ordered CrystallizerRecipes defaults with finite loops and explicit fluid keys."""
import json
import re
from pathlib import Path
from anvil_recipes import snapshot, substitute
from blast_furnace_recipes import block_end
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import integer

BASE = 'src/main/java/com/hbm/'
SOURCE = BASE + 'inventory/recipes/CrystallizerRecipes.java'
TILE = BASE + 'tileentity/machine/TileEntityMachineCrystallizer.java'
ENUM_FILES = {'ScrapType': 'items/special/ItemPlasticScrap.java', 'EnumBedrockOre': 'items/special/ItemBedrockOre.java',
              'BedrockOreType': 'items/special/ItemBedrockOreNew.java', 'BedrockOreGrade': 'items/special/ItemBedrockOreNew.java'}


def enums(source, name):
    text = clean((source / (BASE + ENUM_FILES[name])).read_text())
    body = text.split('enum ' + name + ' {', 1)[1]
    body = re.split(r'[;}]', body, maxsplit=1)[0]
    result = []
    for value in split(body):
        match = re.fullmatch(r'(\w+)(?:\((.*)\))?', value, re.S)
        if not match: raise Unsupported('unrecognized enum member: ' + value)
        result.append((match[1], split(match[2]) if match[2] else []))
    return result


def expansions(source, text, start, end, resolver, shapes):
    source_calls = [(pos, args) for pos, args, _ in calls(text, r'\bregisterRecipe') if start <= pos < end]
    loops = []
    for pos, header, _ in calls(text, r'\bfor'):
        if not start <= pos < end: continue
        body = text.find('(', pos) + len(header) + 2
        while text[body].isspace(): body += 1
        stop = block_end(text, body) if text[body] == '{' else text.index(';', body) + 1
        loops.append((pos, body, stop, header.strip()))
    def domain(header):
        if header == 'String ore : OreDictManager.TH232.all(MaterialShapes.ONLY_ORE)':
            return [({'ore': json.dumps(prefix + name)}, 'thorium_' + str(i)) for i, (name, prefix) in enumerate((n, p) for n in resolver.frames['TH232'] for p in shapes['ONLY_ORE']['prefixes'])]
        if header == 'int i = 0; i < ScrapType.values().length; i++':
            return [({'i': i}, 'plastic_' + name.lower()) for i, (name, _) in enumerate(enums(source, 'ScrapType'))]
        if header == 'EnumBedrockOre ore : EnumBedrockOre.values()':
            return [({'i': i}, 'legacy_' + name.lower()) for i, (name, _) in enumerate(enums(source, 'EnumBedrockOre'))]
        if header == 'BedrockOreType type : BedrockOreType.values()':
            return [({'_type': (i, json.loads(args[2]))}, 'bedrock_' + json.loads(args[2])) for i, (_, args) in enumerate(enums(source, 'BedrockOreType'))]
        if header == 'FluidStack dye : dyes':
            body = re.search(r'FluidStack\[\] dyes\s*=\s*new FluidStack\[\]\s*\{(.*?)\};', text, re.S)[1]
            return [({'dye': expr}, 'dye_' + re.search(r'Fluids\.(\w+)', expr)[1].lower()) for expr in split(body)]
        raise Unsupported('unrecognized source loop: ' + header)
    events = [(pos, 'call', (i, args)) for i, (pos, args) in enumerate(source_calls) if not any(lo <= pos < hi for lo, _, hi, _ in loops)]
    events += [(loop[0], 'loop', loop) for loop in loops]
    for pos, kind, event in sorted(events):
        if kind == 'call': yield pos, event[0], event[1], {}, None
        else:
            _, lo, hi, header = event
            for env, label in domain(header):
                for i, (call_pos, args) in enumerate(source_calls):
                    if lo <= call_pos < hi: yield call_pos, i, args, env, label


def export(source, output, rows):
    resolver, _, shapes, _ = snapshot(source, rows)
    resolver.ore['sand'].update(['minecraft:sand', 'minecraft:red_sand'])
    text = clean((source / SOURCE).read_text()); tile = clean((source / TILE).read_text())
    opening = text.index('{', text.index('public void registerDefaults()')); end = block_end(text, opening)
    default_fluid = re.search(r'registerRecipe\(input, recipe, (new FluidStack\(Fluids\.\w+,\s*[\d_]+\))\);', text)[1]
    constants = dict(re.findall(r'(?:final\s+)?int\s+(\w+)\s*=\s*(\d+)\s*;', text[opening:end]))
    fluids = dict(re.findall(r'FluidStack\s+(\w+)\s*=\s*(new FluidStack\([^;]+\));', text[opening:end]))
    grades = {name: i for i, (name, _) in enumerate(enums(source, 'BedrockOreGrade'))}
    demand = int(re.search(r'final int demand\s*=\s*(\d+)', tile)[1])
    duration_default = int(re.search(r'short duration\s*=\s*(\d+)', tile)[1])
    tank = re.search(r'tank = new FluidTank\(Fluids\.(\w+),\s*(\d+)\)', tile)
    controls = []
    for pos, condition, _ in calls(text, r'\bif'):
        if not opening <= pos < end: continue
        var = re.fullmatch(r'(\w+) != null && !\1.isEmpty\(\)', condition.strip())
        if not var: raise Unsupported('unrecognized registerDefaults guard: ' + condition)
        body = text.find('(', pos) + len(condition) + 2
        while text[body].isspace(): body += 1
        key = re.search(r'List<ItemStack> ' + var[1] + r'\s*=\s*OreDictionary.getOres\((.*?)\);', text, re.S)[1]
        controls.append((body, block_end(text, body), var[1], key))
    names = {r['id']: r['names'] for r in rows}
    vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/lang')
    langs = {loc: json.loads((vanilla / (loc + '.json')).read_text()) if (vanilla / (loc + '.json')).is_file() else {} for loc in ('en_us', 'pt_br')}
    recipes = []; skipped = []; total = 0
    for pos, index, original, env, expansion in expansions(source, text, opening, end, resolver, shapes):
        total += 1; origin = SOURCE + ':' + str(text[:pos].count('\n') + 1)
        name = 'crystallizer.' + str(index) + ('.' + expansion if expansion else '')
        missing = []
        try:
            for lo, hi, var, key in controls:
                if lo <= pos < hi:
                    try: resolver.input('new OreDictStack(' + key + ')')
                    except Unsupported: raise Unsupported('OreDict guard empty in port: ' + key) from None
            for kind, var in dict.fromkeys(re.findall(r'Mod(Items|Blocks)\.(\w+)', original)):
                if not any(r['type'] == ('item' if kind == 'Items' else 'block') for r in resolver.byvar.get(var, [])):
                    missing.append('Mod' + kind + '.' + var)
            if missing: raise Unsupported('missing catalog identities: ' + ', '.join(missing))
            expression = substitute(original, {k: v for k, v in env.items() if not k.startswith('_')})
            expression = substitute(expression, constants)
            if '_type' in env:
                ordinal, suffix = env['_type']
                # ComparableStack(ItemStack) wraps the made stack; keep its identity.
                expression = re.sub(r'new ComparableStack\((ItemBedrockOreNew.make\(BedrockOreGrade\.\w+,\s*type\))\)', r'\1', expression)
                expression = re.sub(r'ItemBedrockOreNew.make\(BedrockOreGrade\.(\w+),\s*type\)', lambda m: f'new ItemStack(ModItems.bedrock_ore, 1, {(grades[m[1]] << 4) | ordinal})', expression)
            parts = split(expression)
            constructor = next(calls(parts[1], r'new CrystallizerRecipe'))
            args = split(constructor[1])
            if len(args) != 2: raise Unsupported('CrystallizerRecipe constructor arity')
            modifiers = {method[1:]: value for _, value, method in calls(parts[1], r'\.\w+') if method in ('.prod', '.setReq', '.setDuration')}
            trailing = parts[1][parts[1].find('(', constructor[0]) + len(constructor[1]) + 2:]
            if re.sub(r'\.(?:prod|setReq|setDuration)\([^)]*\)', '', trailing).strip(): raise Unsupported('unrecognized recipe modifier: ' + trailing)
            representative, ids = resolver.input(parts[0])
            duration = integer(modifiers.get('setDuration', args[1])); count = integer(modifiers.get('setReq', '1'))
            productivity = float(modifiers.get('prod', '0').rstrip('FfDd'))
            if count < 1 or duration < 1 or not 0 <= productivity <= 1: raise Unsupported('invalid recipe quantities')
            output_expr = args[0]
            scraps = re.fullmatch(r'ItemScraps.create\(new MaterialStack\(Mats\.(MAT_\w+), MaterialShapes.INGOT.q\(1\)\)\)', output_expr)
            if scraps:
                if shapes['INGOT']['amount'] != 72: raise Unsupported('non-default scrap quantity requires custom_data')
                row = resolver.bymat.get(('scraps', scraps[1]))
                if row is None: raise Unsupported('missing scrap material ' + scraps[1])
                output_stack = [row['id'], 1]
            else: output_stack = list(resolver.item(output_expr))
            fluid_expr = parts[2] if len(parts) == 3 else default_fluid if len(parts) == 2 else None
            if fluid_expr is None: raise Unsupported('unexpected fluid argument count')
            fluid_expr = substitute(fluids.get(fluid_expr, fluid_expr), constants)
            fluid = resolver.fluid(fluid_expr)
            row = {'name': name, 'source_index': index, 'source': origin, 'expansion': expansion,
                   'inputItems': [{'ids': ids, 'count': count}], 'inputFluids': [fluid], 'outputItems': [output_stack],
                   'outputFluids': [], 'duration': duration, 'duration_source': 'CrystallizerRecipe constructor' if 'setDuration' not in modifiers else 'setDuration',
                   'power': demand, 'productivity': productivity}
            for locale, label in [('pt_br', 'label'), ('en_us', 'label_en')]:
                ident, amount = output_stack
                title = names[ident][locale] if ident in names else langs[locale].get('item.' + ident.replace(':', '.'), langs[locale].get('block.' + ident.replace(':', '.'), ident.split(':')[1].replace('_', ' ').title()))
                row[label] = title + (f' ×{amount}' if amount > 1 else '')
            if scraps: row['source_scrap_amount'] = 72
            recipes.append(row)
        except (Unsupported, StopIteration, ValueError) as error:
            skipped.append({'name': name, 'source_index': index, 'source': origin, 'expansion': expansion, 'reason': str(error), 'missing_items': missing, 'expression': original})
    data = {'schema': 1, 'machine': 'crystallizer', 'profile': {'external_mods': [], 'ore_dictionary': 'conservative NTM + legacy vanilla snapshot'},
            'machine_config': {'power': demand, 'tank_capacity_mb': int(tank[2]), 'energy_capacity_he': int(re.search(r'long maxPower\s*=\s*(\d+)', tile)[1]), 'default_fluid': tank[1].lower(), 'default_duration': duration_default, 'default_recipe_fluid': resolver.fluid(default_fluid), 'source': TILE},
            'coverage': {'source_calls': len(list(calls(text[opening:end], r'\bregisterRecipe'))), 'expanded': total, 'emitted': len(recipes), 'skipped': len(skipped), 'bedrock': sum(bool(r['expansion'] and r['expansion'].startswith('bedrock_')) for r in recipes)},
            'recipes': recipes, 'skipped': skipped,
            'notes': ['Source iteration-major registerDefaults order retained; source_index identifies the declaration before expansion.',
                      'One item input and exactly one explicit input fluid per recipe; omitted Java fluid defaults to peroxide 500 mB.',
                      'setReq overrides item count; duration is the source constructor argument unless explicitly overridden.',
                      'Productivity is documentary: NTM EFFECT upgrade gives min(level*productivity,0.99) chance to keep the item input; fluid is always consumed; without upgrade chance is zero.',
                      'Water washing is 1 BASE to 1 BASE_WASHED; the subsequent centrifuge doubles PRIMARY.',
                      'NBT copper scraps output uses the existing 72-quanta default identity; no material quantity is invented.']}
    target = output / 'machine-recipes/crystallizer.json'; target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(data, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(json.dumps({'crystallizer': data['coverage']}))
    return data


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

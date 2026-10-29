#!/usr/bin/env python3
"""Ordered PUREXRecipes defaults, autoSwitchGroup contracts, ChanceOutput chances and skipped reactor fuels."""
import json
import re
from pathlib import Path
from anvil_recipes import snapshot
from blast_furnace_recipes import block_end
from bulk_recipes import Unsupported, calls, clean, split
from generate import lang
from m3_catalog import fluids
from machine_recipes import PROFILE, integer, ranges, string

RECIPE_SOURCE = 'src/main/java/com/hbm/inventory/recipes/PUREXRecipes.java'
SHORT_SOURCE = 'PUREXRecipes.java'


def method_body(text, name):
    match = re.search(r'public void ' + name + r'\(\)\s*\{', text)
    if match is None:
        raise ValueError('Missing ' + name + '() in ' + RECIPE_SOURCE)
    start = match.end()
    return start, block_end(text, start - 1) - 1


def local_constants(text, start, end):
    body = text[start:end]
    longs = {
        match[1]: integer(match[2])
        for match in re.finditer(r'\blong\s+(\w+)\s*=\s*([0-9_Ll +*-]+)\s*;', body)
    }
    strings = {
        match[1]: string(match[2])
        for match in re.finditer(r'\bString\s+(\w+)\s*=\s*("[^"]*")\s*;', body)
    }
    return longs, strings


def resolve_int(expr, longs):
    token = expr.strip()
    if token in longs:
        return longs[token]
    return integer(token)


def resolve_string(expr, strings):
    token = expr.strip()
    if token in strings:
        return strings[token]
    return string(token)


def parse_output_item(expr, resolver):
    expr = expr.strip()
    if expr.startswith('new ChanceOutput('):
        _, args, _ = next(calls(expr, r'new ChanceOutput'))
        parts = split(args)
        if len(parts) not in (1, 2):
            raise Unsupported('unexpected ChanceOutput arity: ' + expr)
        ident, count = resolver.item(parts[0])
        chance = float(parts[1].rstrip('Ff')) if len(parts) == 2 else 1.0
        if not 0.0 < chance <= 1.0:
            raise Unsupported('invalid output chance: ' + expr)
        return [ident, count, chance]
    ident, count = resolver.item(expr)
    return [ident, count, 1.0]


def classify_family(name, group):
    if group == 'autoswitch.pile':
        return 'pile'
    if group == 'autoswitch.zirnox':
        return 'zirnox'
    if group == 'autoswitch.plate':
        return 'plate'
    if group == 'autoswitch.pwr':
        return 'pwr'
    if group == 'autoswitch.watz':
        return 'watz'
    if name == 'purex.thoriumsalt':
        return 'molten_salt'
    if name == 'purex.icf':
        return 'icf'
    if name.startswith('purex.vit'):
        return 'vitrification'
    if group == 'autoswitch.schrab' or name == 'purex.schraranium':
        return 'schrabidium'
    return 'synthesis'


def export(source, output, rows):
    resolver, _, _, _ = snapshot(source, rows)
    resolver.fluids = {f['source_variable']: f for f in fluids(source)}
    runtime_fluids_path = Path(__file__).resolve().parents[2] / 'crates/ntm-core/data/fluids.json'
    runtime_fluid_ids = (
        {f['string_id'] for f in json.loads(runtime_fluids_path.read_text())['fluids']}
        if runtime_fluids_path.is_file()
        else {f['id'] for f in resolver.fluids.values()}
    )

    names = {row['id']: row['names'] for row in rows}
    translations = {
        loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
        for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]
    }
    vanilla_names = {
        loc: json.loads((resolver.vanilla.parents[1] / 'assets/minecraft/lang' / (loc + '.json')).read_text())
        for loc in ('en_us', 'pt_br')
    }

    def item_title(ident, locale):
        if ident in names:
            return names[ident][locale]
        path = ident.split(':', 1)[1]
        return vanilla_names[locale].get(
            'item.minecraft.' + path,
            vanilla_names[locale].get('block.minecraft.' + path, path.replace('_', ' ').title()),
        )

    def fluid_title(ident, locale):
        return translations[locale].get(
            'hbmfluid.' + ident,
            translations['en_us'].get('hbmfluid.' + ident, ident.replace('_', ' ').title()),
        )

    def recipe_label(row, wrapper, first_icon, explicit_fluid_icon, locale):
        # GenericRecipe.getLocalizedName(): icon display name (setIconToFirstIngredient,
        # setIcon(fluid_icon, fluid), or first output item / fluid), wrapped by nameWrapper if present.
        if first_icon and row['inputItems']:
            base = item_title(row['inputItems'][0]['ids'][0], locale)
        elif explicit_fluid_icon is not None:
            base = fluid_title(explicit_fluid_icon, locale)
            if not wrapper and row['outputFluids'] and row['outputFluids'][0][0] == explicit_fluid_icon:
                base = f"{base} · {row['outputFluids'][0][1]} mB"
        elif row['outputItems']:
            ident, count, _ = row['outputItems'][0]
            base = item_title(ident, locale) + (f' ×{count}' if count > 1 else '')
        elif row['outputFluids']:
            ident, amount = row['outputFluids'][0]
            base = f'{fluid_title(ident, locale)} · {amount} mB'
        else:
            base = row['name']
        if wrapper:
            template = translations[locale].get(wrapper)
            if template and '%s' in template:
                return template.replace('%s', base)
        return base

    text = clean((source / RECIPE_SOURCE).read_text())
    start, end = method_body(text, 'registerDefaults')
    longs, strings = local_constants(text, start, end)
    controls = ranges(text)
    source_calls = [(pos, args) for pos, args, _ in calls(text, r'\bthis\.register') if start <= pos < end]

    allowed_methods = {
        'setup',
        'setupNamed',
        'setNameWrapper',
        'setGroup',
        'setIcon',
        'setIconToFirstIngredient',
        'inputItems',
        'inputItemsEx',
        'inputFluids',
        'inputFluidsEx',
        'outputItems',
        'outputFluids',
        'setPools',
        'setPools528',
    }

    recipes = []
    skipped = []
    order = []
    families = {}

    for index, (pos, args) in enumerate(source_calls):
        line = text[:pos].count('\n') + 1
        origin = f'{SHORT_SOURCE}:{line}'
        name_match = re.search(r'new PUREXRecipe\("([^"]+)"\)', args)
        name = name_match[1] if name_match else f'purex.line_{line}'
        order.append(name)

        methods = {method[1:]: split(values) for _, values, method in calls(args, r'\.\w+')}
        raw_group = None
        if 'setGroup' in methods and methods['setGroup']:
            try:
                raw_group = resolve_string(methods['setGroup'][0], strings)
            except Unsupported:
                raw_group = None
        family = classify_family(name, raw_group)
        fam_stats = families.setdefault(family, {'emitted': 0, 'skipped': 0})

        try:
            if not name_match:
                raise Unsupported('dynamic PUREXRecipe name')
            for ctrl_start, ctrl_end, control, condition in controls:
                if ctrl_start < pos < ctrl_end:
                    if control == 'if' and condition == 'naquadriaNuggets.size() != 0':
                        if not resolver.ordered_ore.get('nuggetNaquadria'):
                            raise Unsupported(
                                'conditional OreDict guard if(naquadriaNuggets.size() != 0): nuggetNaquadria absent from catalog'
                            )
                    else:
                        raise Unsupported(f'dynamic/control-dependent {control}({condition})')

            for match in re.finditer(r'\)\s*\.([A-Za-z0-9_]+)\(', args):
                if match[1] not in allowed_methods:
                    raise Unsupported('unsupported recipe method ' + match[1])

            missing_refs = []
            for mod_cls, var in dict.fromkeys(re.findall(r'Mod(Items|Blocks)\.(\w+)', args)):
                if var == 'fluid_icon':
                    continue
                if not resolver.byvar.get(var):
                    missing_refs.append(f'Mod{mod_cls}.{var}')
            if missing_refs:
                raise Unsupported('missing catalog identities: ' + ', '.join(missing_refs))

            setup = methods.get('setup', methods.get('setupNamed'))
            if setup is None or len(setup) != 2:
                raise Unsupported('missing literal setup(duration, power)')
            duration = resolve_int(setup[0], longs)
            power = resolve_int(setup[1], longs)

            inputs = [resolver.input(expr) for expr in methods.get('inputItems', [])]
            if len(inputs) > 3:
                raise Unsupported(f'PUREX inputItemLimit(3) exceeded: {len(inputs)}')
            input_items = [{'ids': options, 'count': rep[1]} for rep, options in inputs]

            # Preserve exact output order and duplicate entries (independent chance rolls in NTM).
            output_items = [parse_output_item(expr, resolver) for expr in methods.get('outputItems', [])]
            if len(output_items) > 6:
                raise Unsupported(f'PUREX outputItemLimit(6) exceeded: {len(output_items)}')

            input_fluids = [resolver.fluid(expr) for expr in methods.get('inputFluids', [])]
            if len(input_fluids) > 3:
                raise Unsupported(f'PUREX inputFluidLimit(3) exceeded: {len(input_fluids)}')

            output_fluids = [resolver.fluid(expr) for expr in methods.get('outputFluids', [])]
            if len(output_fluids) > 1:
                raise Unsupported(f'PUREX outputFluidLimit(1) exceeded: {len(output_fluids)}')

            for fid, _ in input_fluids + output_fluids:
                if fid not in runtime_fluid_ids:
                    raise Unsupported(f'fluid {fid} absent from crates/ntm-core/data/fluids.json')

            if not output_items and not output_fluids:
                raise Unsupported('recipe has no resolved output')

            pools = [resolve_string(expr, strings) for expr in methods.get('setPools', [])]
            wrapper = resolve_string(methods['setNameWrapper'][0], strings) if 'setNameWrapper' in methods else None
            first_icon = 'setIconToFirstIngredient' in methods
            explicit_fluid_icon = None
            if 'setIcon' in methods and len(methods['setIcon']) == 2 and methods['setIcon'][0].strip() == 'ModItems.fluid_icon':
                fl_match = re.fullmatch(r'Fluids\.(\w+)\.getID\(\)', methods['setIcon'][1].strip())
                if fl_match and fl_match[1] in resolver.fluids:
                    explicit_fluid_icon = resolver.fluids[fl_match[1]]['id']

            row = {
                'name': name,
                'source_index': index,
                'source': origin,
                'duration': duration,
                'power': power,
                'inputItems': input_items,
                'outputItems': output_items,
                'inputFluids': input_fluids,
                'outputFluids': output_fluids,
                'pools': pools,
                'label': recipe_label(
                    {'name': name, 'inputItems': input_items, 'outputItems': output_items, 'outputFluids': output_fluids},
                    wrapper,
                    first_icon,
                    explicit_fluid_icon,
                    'pt_br',
                ),
                'label_en': recipe_label(
                    {'name': name, 'inputItems': input_items, 'outputItems': output_items, 'outputFluids': output_fluids},
                    wrapper,
                    first_icon,
                    explicit_fluid_icon,
                    'en_us',
                ),
                'autoSwitchGroup': raw_group,
            }
            recipes.append(row)
            fam_stats['emitted'] += 1
        except (Unsupported, SyntaxError, ValueError, KeyError) as error:
            missing = [
                f'Mod{mod_cls}.{var}'
                for mod_cls, var in dict.fromkeys(re.findall(r'Mod(Items|Blocks)\.(\w+)', args))
                if var != 'fluid_icon' and not resolver.byvar.get(var)
            ]
            skipped.append({
                'name': name,
                'source_index': index,
                'source': origin,
                'family': family,
                'autoSwitchGroup': raw_group,
                'reason': str(error),
                **({'missing_items': missing} if missing else {}),
                'expression': args,
            })
            fam_stats['skipped'] += 1

    coverage = {
        'emitted': len(recipes),
        'scanned_source_calls': len(source_calls),
        'skipped_source_calls': len(skipped),
        'by_family': families,
        'note': (
            'Preserves exact PUREXRecipes.registerDefaults() source_index order (0..58), conservative 1.7.10 OreDict '
            'snapshot, independent duplicate output entries, and conditional nuggetNaquadria OreDict guard.'
        ),
    }
    body = {
        'schema': 1,
        'machine': 'purex',
        'profile': {**PROFILE, 'modern_vanilla_ore_variants': False},
        'units': {'duration': 'ticks', 'power': 'HE/t', 'fluids': 'mB'},
        'limits': {'inputItems': 3, 'inputFluids': 3, 'outputItems': 6, 'outputFluids': 1},
        'ingredient_policy': 'conservative Forge 1.7.10 OreDict snapshot; first ntm id, then lexical vanilla',
        'coverage': coverage,
        'source_recipe_order': order,
        'recipes': recipes,
        'skipped': skipped,
    }
    target = output / 'machine-recipes/purex.json'
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(body, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'purex': {k: v for k, v in coverage.items() if k not in ('by_family', 'note')}}, ensure_ascii=False))
    return body


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

#!/usr/bin/env python3
"""Ordered anvil recipes, finite source loops and explicit unsupported identities."""
import json
import re
from collections import defaultdict
from pathlib import Path
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import integer, split_ternary
from shredder_recipes import ShredderResolver
from foundry_recipes import material_table, ore_snapshot, shape_table
from blast_furnace_recipes import block_end

SOURCE = 'src/main/java/com/hbm/inventory/recipes/anvil/AnvilRecipes.java'


class AnvilResolver(ShredderResolver):
    def item(self, expr, unqualified=False):
        expr = expr.strip()
        if expr.startswith('exp ?'):
            parts = split_ternary(expr)
            if parts is None: raise Unsupported('unsupported expensive-mode ternary')
            return self.item(parts[2], unqualified)
        # ComparableStack(ItemStack) wraps the existing stack, not its item id.
        match = re.fullmatch(r'new ComparableStack\(((?:DictFrame\.)?fromOne\(.*\))\)', expr, re.S)
        if match: return self.item(match[1], unqualified)
        match = re.fullmatch(r'new (ItemStack|ComparableStack)\((.*)\)', expr, re.S)
        if match:
            args = split(match[2])
            if len(args) > 2:
                # Finite numeric-loop offsets; enum/material metadata stay exact.
                if re.fullmatch(r'[0-9 +*()-]+', args[2]): args[2] = str(integer(args[2]))
            expr = 'new ' + match[1] + '(' + ','.join(args) + ')'
        return super().item(expr, unqualified)


def snapshot(source, rows):
    resolver = AnvilResolver(source, rows)
    shapes = shape_table(source)
    materials = material_table(source, resolver)
    ores, origins = ore_snapshot(source, resolver, materials, shapes)
    ordered = {key: list(ids) for key, ids in ores.items()}

    def add(alias, ids, origin):
        target = ordered.setdefault(alias, [])
        for ident in ids:
            if ident not in target:
                target.append(ident)
        if origin not in origins[alias]:
            origins[alias].append(origin)

    for group in re.finditer(r'\b(\w+)\s*=\s*new DictGroup\("([^"]+)",([^;]+)\);', resolver.odtext):
        origin = 'OreDictManager.java:DictGroup ' + group[1]
        for member in split(group[3]):
            for name in resolver.frames.get(member, []):
                for key, ids in list(ordered.items()):
                    if key.endswith(name):
                        add(key[:-len(name)] + group[2], ids, origin)
            if member in resolver.constants:
                add('any' + group[2], ordered.get(resolver.constants[member], []), origin)
    # Reuse the conservative legacy snapshot instead of the base resolver's
    # modern convenience aliases (notably vanilla copper and iron nuggets).
    resolver.ore = defaultdict(set, {key: set(ids) for key, ids in ordered.items()})
    resolver.ordered_ore = ordered
    return resolver, materials, shapes, origins


def method_body(text, name):
    match = re.search(r'public static void ' + name + r'\(\)\s*\{', text)
    if match is None: raise Unsupported('missing method ' + name)
    start = match.end()
    return start, block_end(text, start - 1) - 1


def substitute(expr, env):
    for key, value in env.items():
        expr = re.sub(r'(?<!\w)' + re.escape(key) + r'(?!\w)', lambda _: str(value), expr)
    return expr


def expanded_calls(text, method, collection, materials, ores, shapes):
    """Visit loops before their contained calls, preserving iteration-major order."""
    start, end = method_body(text, method)
    recipes = [(pos, args) for pos, args, _ in calls(text, collection + r'\.add') if start <= pos < end]
    loops = []
    for pos, header, _ in calls(text, r'\bfor'):
        if not start <= pos < end: continue
        body = text.find('(', pos) + len(header) + 2
        while text[body].isspace(): body += 1
        stop = block_end(text, body) if text[body] == '{' else text.index(';', body) + 1
        if text[body:].startswith('if('):
            _, condition, _ = next(calls(text[body:], r'if'))
            consequent = text.find('(', body) + len(condition) + 2
            while text[consequent].isspace(): consequent += 1
            if text[consequent] == '{': stop = block_end(text, consequent)
        loops.append((pos, body, stop, header.strip()))

    def domain(header):
        if header == 'Block anvil : anvils':
            return [{'anvil': 'ModBlocks.anvil_iron'}, {'anvil': 'ModBlocks.anvil_lead'}]
        numeric = re.fullmatch(r'int i = (\d+); i < (\d+); i(?:\+\+| \+= (\d+))', header)
        if numeric:
            return [{'i': i} for i in range(int(numeric[1]), int(numeric[2]), int(numeric[3] or 1))]
        if header == 'NTMMaterial mat : Mats.orderedList':
            return [{'mat.id': mat['id'], 'mat.names[0]': json.dumps(mat['names'][0]),
                     '_material': mat} for mat in materials]
        raise Unsupported('unsupported anvil source loop: ' + header)

    def visit(lo, hi, env):
        children = [loop for loop in loops if lo <= loop[0] < hi
                    and not any(lo <= parent[0] < loop[0] < parent[2] <= hi for parent in loops)]
        events = [(pos, 'recipe', args) for pos, args in recipes if lo <= pos < hi
                  and not any(loop[0] <= pos < loop[2] for loop in children)]
        events += [(loop[0], 'loop', loop) for loop in children]
        for pos, kind, value in sorted(events, key=lambda event: event[0]):
            if kind == 'recipe':
                expression = value
                if '_material' in env:
                    mat = env['_material']
                    body = text[lo:hi]
                    form = re.search(r'mat.autogen.contains\(MaterialShapes\.(\w+)\)', body)
                    if not form: raise Unsupported('unverified material-loop condition')
                    if form[1] not in mat['autogen']: continue
                    ingot = shapes['INGOT']['prefixes'][0] + mat['names'][0]
                    if form[1] == 'WIRE' and not ores.get(ingot): continue
                    plate = shapes['PLATE']['prefixes'][0] + mat['names'][0]
                    values = {key: val for key, val in env.items() if key != '_material'}
                    values['key'] = json.dumps(plate if ores.get(plate) else ingot)
                    expression = substitute(expression, values)
                    expression = expression.replace('MaterialShapes.INGOT.name() + ' + json.dumps(mat['names'][0]), json.dumps(ingot))
                    expression = expression.replace('MaterialShapes.PLATE.name() + ' + json.dumps(mat['names'][0]), json.dumps(plate))
                else: expression = substitute(expression, env)
                yield pos, expression, {k: v for k, v in env.items() if k != '_material'}
            else:
                _, body, stop, header = value
                for bindings in domain(header):
                    yield from visit(body + (text[body] == '{'), stop - (text[body] == '{'), {**env, **bindings})
    yield from visit(start, end, {})


def ingredient(resolver, expression):
    representative, ids = resolver.input(expression)
    return {'ids': ids, 'count': representative[1]}


def mold_candidates(prefix, ores, shapes):
    longer = {p for shape in shapes.values() for p in shape['prefixes'] if len(p) > len(prefix)}
    names = defaultdict(list)
    for name, ids in ores.items():
        for ident in ids: names[ident].append(name)
    result = []
    for ident, keys in names.items():
        # Match the Java short-circuit order: a longer prefix rejects before a
        # startsWith success on that ore name, with names in registration order.
        for name in keys:
            if any(name.startswith(p) for p in longer): break
            if name.startswith(prefix):
                result.append(ident)
                break
    return sorted(result, key=lambda ident: (not ident.startswith('ntm:'), ident))


def smithing_recipe(expression, resolver, shapes):
    _, args, cls = next(calls(expression, r'new AnvilSmithing\w+'))
    parts = split(args)
    if cls != 'new AnvilSmithingRecipe' and cls != 'new AnvilSmithingMold':
        raise Unsupported(cls.removeprefix('new ') + ' requires per-item NBT/state')
    if cls == 'new AnvilSmithingRecipe':
        left, right = [ingredient(resolver, expr) for expr in parts[2:]]
        return {'tier': integer(parts[0]), 'left': left, 'right': right,
                'out': list(resolver.item(parts[1])), 'shapeless': '.makeShapeless()' in expression,
                'consume': [left['count'], right['count']], 'kind': 'plain'}
    meta = integer(parts[0])
    if parts[2].startswith('new OreDictStack'):
        ore = split(next(calls(parts[2], 'new OreDictStack'))[1])
        key = ore[0]
        shape = re.fullmatch(r'MaterialShapes\.(\w+)\.name\(\)', key)
        prefix = shapes[shape[1]]['prefixes'][0] if shape else json.loads(key)
        ids = mold_candidates(prefix, resolver.ordered_ore, shapes)
        count = integer(ore[1]) if len(ore) > 1 else 1
        extra = {'prefix': prefix}
    else:
        stacks = [resolver.item('new ItemStack(' + args + ')') for _, args, _ in calls(parts[2], r'new ItemStack')]
        ids = [ident for ident, _ in stacks]
        counts = {count for _, count in stacks}
        if len(counts) != 1: raise Unsupported('mold alternatives have different quantities')
        count = counts.pop()
        extra = {}
    if not ids: raise Unsupported('no catalog items match mold engraving')
    return {'tier': 1, 'left': {'ids': ids, 'count': count, 'exact': True, **extra},
            'right': ingredient(resolver, 'new ComparableStack(ModItems.mold_base)'),
            'out': [resolver.row('mold', meta), 1], 'shapeless': False,
            'consume': [0, 1], 'kind': 'mold'}


def construction_recipe(expression, resolver):
    _, args, _ = next(calls(expression, r'new AnvilConstructionRecipe'))
    inputs, outputs = split(args)
    input_array = re.fullmatch(r'new AStack\[\]\s*\{(.*)\}', inputs, re.S)
    output_array = re.fullmatch(r'new AnvilOutput\[\]\s*\{(.*)\}', outputs, re.S)
    parsed_inputs = [ingredient(resolver, expr) for expr in (split(input_array[1]) if input_array else [inputs])]
    parsed_outputs = []
    for expr in (split(output_array[1]) if output_array else [outputs]):
        _, args, _ = next(calls(expr, r'new AnvilOutput'))
        parts = split(args)
        chance = float(parts[1].rstrip('Ff')) if len(parts) > 1 else 1.0
        if not 0 <= chance <= 1: raise Unsupported('invalid chance')
        parsed_outputs.append([*resolver.item(parts[0]), chance])
    tier = re.search(r'\.setTier\(([^)]+)\)', expression)
    tier_range = re.search(r'\.setTierRange\(([^)]+)\)', expression)
    low, high = integer(tier[1]) if tier else 0, -1
    if tier_range: low, high = map(integer, split(tier_range[1]))
    overlay = 'NONE' if input_array and output_array else 'CONSTRUCTION' if input_array else 'RECYCLING' if output_array else 'SMITHING'
    explicit = re.search(r'\.setOverlay\(OverlayType\.(\w+)\)', expression)
    if explicit: overlay = explicit[1]
    return {'in': parsed_inputs, 'out': parsed_outputs, 'tier_lower': low,
            'tier_upper': high, 'overlay': overlay}


def export(source, output, rows):
    resolver, materials, shapes, origins = snapshot(source, rows)
    text = clean((source / SOURCE).read_text())
    emitted = {'smithing': [], 'construction': []}
    skipped = []; orders = {}; scanned = {}
    methods = {'smithing': ['registerSmithing'],
               'construction': ['registerConstruction', 'registerConstructionRecipes', 'registerConstructionStamps',
                                'registerConstructionAmmo', 'registerConstructionSirens',
                                'registerConstructionUpgrades', 'registerConstructionRecycling']}
    for section, names in methods.items():
        stream = [entry for name in names for entry in expanded_calls(text, name, section + 'Recipes', materials, resolver.ordered_ore, shapes)]
        orders[section] = []
        scanned[section] = len({pos for pos, _, _ in stream})
        for index, (pos, expression, expansion) in enumerate(stream):
            name = 'anvil.' + section + '.' + str(index)
            orders[section].append(name)
            provenance = {'name': name, 'source_index': index,
                          'source': SOURCE + ':' + str(text[:pos].count('\n') + 1),
                          **({'expansion': expansion} if expansion else {})}
            try:
                recipe = smithing_recipe(expression, resolver, shapes) if section == 'smithing' else construction_recipe(expression, resolver)
                emitted[section].append({**provenance, **recipe})
            except (Unsupported, ValueError, KeyError) as error:
                skipped.append({**provenance, 'section': section, 'reason': str(error), 'expression': expression})
    anvils = [{'id': row['id'], 'tier': row['tier']} for row in rows if row.get('source_class') == 'NTMAnvil']
    coverage = {section: {'source_calls': scanned[section], 'expanded': len(orders[section]),
                          'emitted': len(emitted[section]), 'skipped': sum(row['section'] == section for row in skipped)}
                for section in emitted}
    body = {'schema': 1, 'machine': 'anvil', 'anvils': anvils,
            'profile': {'enableLBSM': False, 'enableLBSMUnlockAnvil': False,
                        'enableExpensiveMode': False, 'enable528': False, 'external_mods': []},
            'coverage': coverage, 'source_recipe_order': orders, **emitted, 'skipped': skipped,
            'ore_dictionary': {key: list(ids) for key, ids in resolver.ordered_ore.items()},
            'ore_sources': dict(origins),
            'notes': ['source_index is zero-based global list order after finite loop expansion, including skipped entries.',
                      'Ordered default methods only; source commented recipes and external JSON overrides are excluded.',
                      'LBSM && LBSMUnlockAnvil would set smithing tier and construction lower bound to 1; setTierRange would set both bounds to 1. Both flags are false here.',
                      'Plain smithing accepts at least count; mold engraving requires exact left count, consumes only mold_base, and rejects longer MaterialShapes prefixes in OreDict iteration order.',
                      'Material WIRE/PIPE existence uses the registered catalog OreDict snapshot; absent/external identities are not invented.',
                      'Construction constructor overloads determine default overlay even for one-element arrays. Output chances are independent rolls; duplicate outputs remain separate.',
                      'Beehive state contract omits source anvil gravity/falling damage; source OBJ and narrow collision bounds require visual/runtime work.']}
    target = output / 'machine-recipes/anvil.json'
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(body, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(json.dumps({'anvil': coverage}))
    return body


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

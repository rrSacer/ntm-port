#!/usr/bin/env python3
"""Ordered BlastFurnaceRecipesNT plus the handoff's conservative fuel subset."""
import ast
import json
import re
from pathlib import Path
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import integer
from shredder_recipes import ShredderResolver, dictionary

RECIPE = 'src/main/java/com/hbm/inventory/recipes/BlastFurnaceRecipesNT.java'
FUEL = 'src/main/java/com/hbm/handler/FuelHandler.java'
MODULE = 'src/main/java/com/hbm/module/ModuleBurnTime.java'
TILE = 'src/main/java/com/hbm/tileentity/machine/TileEntityMachineBlastFurnace.java'


def block_end(text, opening):
    depth = 1; end = opening + 1
    while depth and end < len(text):
        if text[end] == '{': depth += 1
        elif text[end] == '}': depth -= 1
        end += 1
    if depth: raise Unsupported('unbalanced source block')
    return end


def fuel_number(expression, single):
    def read(node):
        if isinstance(node, ast.Constant) and type(node.value) is int: return node.value
        if isinstance(node, ast.Name) and node.id == 'single': return single
        if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Mult, ast.Div)):
            left, right = read(node.left), read(node.right)
            if isinstance(node.op, ast.Mult): return left * right
            if right <= 0 or left % right: raise Unsupported('non-exact fuel division')
            return left // right
        raise Unsupported('nonliteral fuel expression: ' + expression)
    return read(ast.parse(expression.strip(), mode='eval').body)


def fuel_table(source, rows, resolver, ores):
    text = clean((source / FUEL).read_text()); items = clean((source / 'src/main/java/com/hbm/items/ModItems.java').read_text())
    single = int(re.search(r'\bint single\s*=\s*(\d+)', text)[1])
    raw = {}; provenance = {}; gaps = []
    for pos, condition, _ in calls(text, r'\bif'):
        match = re.fullmatch(r'fuel.getItem\(\)(?:\.equals\(Mod(Items|Blocks)\.(\w+)\)|\s*==\s*Mod(Items|Blocks)\.(\w+)|\s*==\s*Item.getItemFromBlock\(ModBlocks\.(\w+)\))', condition.strip())
        if not match: continue
        variable = match[2] or match[4] or match[5]
        end = text.find('(', pos) + len(condition) + 2
        tail = text[end:]; direct = re.match(r'\s*return\s+([^;]+);', tail)
        origin = FUEL + ':' + str(text[:pos].count('\n') + 1)
        try:
            if direct:
                for row in resolver.byvar.get(variable, []):
                    raw[row['id']] = fuel_number(direct[1], single); provenance[row['id']] = origin
            elif tail.lstrip().startswith('{'):
                opening = end + len(tail) - len(tail.lstrip())
                body = text[opening:block_end(text, opening)]
                cases = {int(meta): fuel_number(expr, single) for meta, expr in re.findall(r'case\s+(\d+)\s*:\s*return\s+([^;]+);', body)}
                if not cases:
                    gaps.append({'source': origin, 'variable': variable, 'reason': 'unsupported FuelHandler branch'})
                for row in resolver.byvar.get(variable, []):
                    if row.get('metadata') in cases:
                        raw[row['id']] = cases[row['metadata']]; provenance[row['id']] = origin
        except (Unsupported, SyntaxError, ValueError) as error:
            gaps.append({'source': origin, 'variable': variable, 'reason': str(error)})
    # The two vanilla values are also the existing port's burn_ticks defaults.
    for ident, ticks in [('minecraft:coal', 1600), ('minecraft:coal_block', 16000)]:
        raw[ident] = ticks; provenance[ident] = 'vanilla 1.7.10 furnace; port crates/ntm-core/src/machine.rs::burn_ticks'
    keys = {}
    for key, values in ores.items():
        for ident in values: keys.setdefault(ident, []).append(key)
    byid = {row['id']: row for row in rows}
    direct = {row['id'] for row in rows if row['source_variable'].startswith('solid_fuel') or row['source_variable'] == 'rocket_fuel'}
    candidates = set(raw) | direct | {ident for ident, names in keys.items() if any(any(token in name for token in ('Coal', 'Coke', 'Lignite')) for name in names)}
    accepted = {}; details = []; rejected = []
    for ident in sorted(candidates):
        row = byid.get(ident, {}); variable = row.get('source_variable'); names = keys.get(ident, [])
        container = bool(variable and (re.search(r'\b' + variable + r'\.setContainerItem\(', items) or re.search(r'\b' + variable + r'\s*=\s*new\b[^;]*\.setContainerItem\(', items)))
        category = ('balefire' if variable.endswith('_bf') else 'solid') if variable and variable.startswith('solid_fuel') else 'rocket' if variable == 'rocket_fuel' else None
        if category is None:
            for name in names:
                category = next((token.lower() for token in ('Coke', 'Coal', 'Lignite') if token in name), None)
                if category is not None: break
        ticks = raw.get(ident, 0)
        reason = 'hasContainerItem' if container else 'no allowed Coal/Coke/Lignite/solid/rocket category' if category is None else 'zero/unresolved FuelHandler or vanilla burn time' if not ticks else None
        if not reason and row and row.get('source_class') not in ('Item', 'ItemEnumMulti', 'BlockCoke'):
            reason = 'unverified fuel class container behavior'
            gaps.append({'item': ident, 'reason': reason, 'class': row.get('source_class')})
        if reason:
            rejected.append({'item': ident, 'reason': reason, 'base_ticks': ticks, 'ore_keys': names})
            continue
        accepted[ident] = ticks
        details.append({'item': ident, 'ticks': ticks, 'base_ticks': ticks, 'heat_modifier': 1, 'category': category, 'source': provenance[ident], 'ore_keys': names})
    return accepted, details, rejected, gaps


def export(source, output, rows):
    resolver = ShredderResolver(source, rows); ores, _ = dictionary(resolver)
    for key, values in ores.items(): resolver.ore[key].update(values)
    # Forge registers Blocks.sand with wildcard metadata (sand and red sand).
    resolver.ore['sand'].add('minecraft:red_sand')
    text = clean((source / RECIPE).read_text()); recipes = []; skipped = []; order = []
    for index, (pos, expression, _) in enumerate(calls(text, r'this\.register')):
        name = re.search(r'new GenericRecipeNoPower\("([^"]+)"\)', expression)[1]
        origin = RECIPE + ':' + str(text[:pos].count('\n') + 1); order.append(name)
        missing = []
        for variable in dict.fromkeys(re.findall(r'ModItems\.(\w+)', expression)):
            if not resolver.byvar.get(variable): missing.append('ModItems.' + variable)
        try:
            if missing: raise Unsupported('missing catalog identities: ' + ', '.join(missing))
            for mutation in re.finditer(r'\)\s*\.(\w+)\(', expression):
                if mutation[1] not in ('setDuration', 'inputItems', 'outputItems'):
                    raise Unsupported('unsupported recipe method ' + mutation[1])
            methods = {method[1:]: split(args) for _, args, method in calls(expression, r'\.\w+')}
            inputs = [resolver.input(expr) for expr in methods['inputItems']]
            outputs = [list(resolver.item(expr)) for expr in methods['outputItems']]
            recipes.append({'name': name, 'source_index': index, 'source': origin, 'duration': integer(methods['setDuration'][0]),
                            'inputItems': [{'ids': choices, 'count': representative[1]} for representative, choices in inputs], 'outputItems': outputs})
        except Unsupported as error:
            skipped.append({'name': name, 'source_index': index, 'source': origin, 'reason': str(error), 'missing_items': missing, 'expression': expression})
    names = {row['id']: row['names'] for row in rows}
    for recipe in recipes:
        ident, count = recipe['outputItems'][0]
        for locale, field in [('pt_br', 'label'), ('en_us', 'label_en')]:
            recipe[field] = names[ident][locale] + (f' ×{count}' if count > 1 else '')
    fuels, details, rejected, fuel_gaps = fuel_table(source, rows, resolver, ores)
    body = {'schema': 1, 'machine': 'blast_furnace', 'source_recipe_order': order,
            'profile': {'external_mods': [], 'wood_log_uncategorized_heat': 0, 'allowed_fuel_categories': ['Coal', 'Coke', 'Lignite', 'solid_fuel*', 'rocket_fuel'], 'container_items': 'rejected'},
            'coverage': {'source_recipes': len(order), 'emitted': len(recipes), 'skipped': len(skipped), 'fuels': len(fuels)},
            'recipes': recipes, 'skipped': skipped, 'fuels': fuels, 'fuel_details': details, 'fuel_rejected': rejected, 'fuel_gaps': fuel_gaps,
            'notes': ['Current registerDefaults has 15 declarations; no synthetic 16th recipe.',
                      'Duration is source ticks; no electrical power is assigned.',
                      'Restricted fuel profile is the handoff contract. ModuleBurnTime log heat defaults to 1 and getBurnHeat ignores its def argument; logs remain excluded here by explicit scope.',
                      'Source getBurnTime rejects hasContainerItem before ModuleBurnTime. Omitted fuel IDs are rejected/zero, never inferred by their registry path.'],
            'fuel_sources': [FUEL, MODULE, TILE + ':216-219']}
    target = output / 'machine-recipes/blast_furnace.json'; target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(body, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(json.dumps({'blast_furnace': body['coverage']}))
    return body


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

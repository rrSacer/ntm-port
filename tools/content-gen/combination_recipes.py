#!/usr/bin/env python3
"""CombinationRecipes defaults (the combination oven): an item into an item
and a fluid. A key is an exact item or an OreDict name (`getOutput` tries the
item first, then its names)."""
import re
from bulk_recipes import Unsupported, split
from bedrock_processing_common import Context, events, skip, write


def key_of(ctx, expr):
    expr = expr.strip()
    named = re.fullmatch(r'(?:[A-Z][A-Z0-9_]*\.\w+\(\)|KEY_\w+)', expr)
    if named:
        # The OreDict name as the source spells it, and the items it holds now.
        wrapped = expr if expr.startswith('KEY_') else expr
        representative, ids = ctx.resolver.input('new OreDictStack(' + wrapped + ')')
        return {'dict': ctx.resolver.constants.get(expr, expr), 'ids': ids}
    stack = ctx.ingredient(expr)
    return {'ids': stack['ids']}


def export(source, output, rows):
    ctx = Context(source, rows)
    recipes = []; skipped = []; expanded = 0
    for i, origin, expr, ore_type in events(ctx, 'CombinationRecipes', r'\brecipes\.put'):
        name = f'combination.{i}' + ('.' + ore_type['suffix'] if ore_type else '')
        expanded += 1
        try:
            args = split(expr)
            if len(args) != 2: raise Unsupported('unexpected recipes.put arguments')
            pair = re.fullmatch(r'new Pair\((.*)\)', args[1].strip(), re.S)
            if not pair: raise Unsupported('unexpected value ' + args[1])
            out, fluid = split(pair[1])
            row = {'name': name, 'source': origin, 'source_index': i, 'in': key_of(ctx, args[0] if not ore_type else args[0]) if not ('ItemBedrockOreNew.make' in args[0]) else {'ids': [ctx.stack(args[0], ore_type)[0]]},
                   'outputItems': [] if out.strip() == 'null' else [ctx.stack(out, ore_type)],
                   'outputFluids': [] if fluid.strip() == 'null' else [ctx.fluid(fluid)]}
            row['inputItems'] = []; row['inputFluids'] = []; row['outputMaterials'] = []
            ctx.label(row)
            recipes.append(row)
        except (Unsupported, KeyError, ValueError, IndexError) as error:
            skip(skipped, name, origin, expr, error)
    return write(output, 'combination', {
        'machine_config': {'process_time': 20000, 'max_heat': 100000, 'diffusion': 0.25, 'tank_capacity_mb': 24000,
                           'source': 'src/main/java/com/hbm/tileentity/machine/TileEntityFurnaceCombination.java:38-44'},
        'coverage': {'expanded': expanded, 'emitted': len(recipes), 'skipped': len(skipped)},
        'recipes': recipes, 'skipped': skipped})


if __name__ == '__main__':
    import json
    from pathlib import Path
    root = Path(__file__).resolve().parents[2]
    src = Path(__import__('sys').argv[1]) if len(__import__('sys').argv) > 1 else root.parent / 'ntm-src'
    export(src, root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

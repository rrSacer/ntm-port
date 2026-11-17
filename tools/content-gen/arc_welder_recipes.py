#!/usr/bin/env python3
"""ArcWelderRecipes defaults: one to three ingredients (the three input
slots, matched in any order by `getRecipe`), an optional fluid, an output,
ticks and HE a tick."""
import re
from bulk_recipes import Unsupported, calls, split
from machine_recipes import integer
from bedrock_processing_common import Context, body, skip, write


def export(source, output, rows):
    ctx = Context(source, rows)
    path, text, start, end = body(source, 'ArcWelderRecipes')
    recipes = []; skipped = []
    for pos, args, _ in calls(text, r'recipes\.add'):
        if not start < pos < end: continue
        origin = path + ':' + str(text[:pos].count('\n') + 1)
        name = f'arcwelder.{len(recipes) + len(skipped)}'
        try:
            wrapped = re.fullmatch(r'new ArcWelderRecipe\((.*)\)', args.strip(), re.S)
            if not wrapped: raise Unsupported('unsupported expression ' + args[:60])
            parts = split(wrapped[1])
            row = ctx.row(name, origin, len(recipes))
            row['outputItems'] = [ctx.stack(parts[0])]
            row['duration'] = integer(parts[1]); row['power'] = integer(parts[2])
            rest = parts[3:]
            if rest and rest[0].startswith('new FluidStack'):
                row['inputFluids'] = [ctx.fluid(rest[0])]; rest = rest[1:]
            row['inputItems'] = [ctx.ingredient(x) for x in rest]
            if not 1 <= len(row['inputItems']) <= 3: raise Unsupported('ingredient count')
            recipes.append(ctx.label(row))
        except (Unsupported, KeyError, ValueError, IndexError) as error:
            skip(skipped, name, origin, args, error)
    return write(output, 'arc_welder', {
        'machine_config': {'power_buffer_min': 2000, 'tank_capacity_mb': 24000, 'slots': 8, 'inputs': [0, 3], 'output': 3, 'battery': 4, 'fluid_identifier': 5, 'upgrades': [6, 8],
                           'source': 'src/main/java/com/hbm/tileentity/machine/TileEntityMachineArcWelder.java'},
        'coverage': {'emitted': len(recipes), 'skipped': len(skipped)}, 'recipes': recipes, 'skipped': skipped})


if __name__ == '__main__':
    import json, sys
    from pathlib import Path
    root = Path(__file__).resolve().parents[2]
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else root.parent / 'ntm-src'
    export(src, root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

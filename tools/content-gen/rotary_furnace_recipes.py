#!/usr/bin/env python3
"""RotaryFurnaceRecipes defaults: one to three ingredients (slots 0..3,
matched in any order by `getRecipe`), an optional fluid, a molten material
output, duration ticks and steam per tick."""
import re
from bulk_recipes import Unsupported, calls, split
from machine_recipes import integer
from bedrock_processing_common import Context, body, skip, write


def export(source, output, rows):
    ctx = Context(source, rows)
    path, text, start, end = body(source, 'RotaryFurnaceRecipes')
    recipes = []
    skipped = []
    for pos, args, _ in calls(text, r'recipes\.add'):
        if not start < pos < end:
            continue
        origin = path + ':' + str(text[:pos].count('\n') + 1)
        name = f'rotary.{len(recipes) + len(skipped)}'
        try:
            wrapped = re.fullmatch(r'new RotaryFurnaceRecipe\((.*)\)', args.strip(), re.S)
            if not wrapped:
                raise Unsupported('unsupported expression ' + args[:60])
            parts = split(wrapped[1])
            row = ctx.row(name, origin, len(recipes))
            mat_expr = re.sub(r'(?<!Mats\.)\b(MAT_\w+)\b', r'Mats.\1', parts[0])
            mat = ctx.material(mat_expr)
            if mat is None:
                raise Unsupported('non-smeltable output ' + parts[0])
            row['outputMaterials'] = [mat]
            row['duration'] = integer(parts[1])
            row['steam'] = integer(parts[2])
            rest = parts[3:]
            if rest and rest[0].startswith('new FluidStack'):
                row['inputFluids'] = [ctx.fluid(rest[0])]
                rest = rest[1:]
            row['inputItems'] = [ctx.ingredient(x) for x in rest]
            if not 0 <= len(row['inputItems']) <= 3:
                raise Unsupported('ingredient count')
            recipes.append(ctx.label(row))
        except (Unsupported, KeyError, ValueError, IndexError) as error:
            skip(skipped, name, origin, args, error)
    return write(output, 'rotary_furnace', {
        'machine_config': {
            'max_output_quanta': ctx.shapes['BLOCK']['amount'] * 16,
            'pour_quanta': ctx.shapes['INGOT']['amount'],
            'tank_in_mb': 16000,
            'steam_tank_mb': 12000,
            'spent_steam_tank_mb': 120,
            'slots': 5,
            'inputs': [0, 3],
            'fluid_identifier': 3,
            'fuel': 4,
            'source': 'src/main/java/com/hbm/tileentity/machine/TileEntityMachineRotaryFurnace.java',
        },
        'coverage': {'emitted': len(recipes), 'skipped': len(skipped)},
        'recipes': recipes,
        'skipped': skipped,
    })


if __name__ == '__main__':
    import json, sys
    from pathlib import Path
    root = Path(__file__).resolve().parents[2]
    default_src = root.parent / 'ntm-src'
    if not default_src.exists():
        default_src = Path('/mnt/ssd120/sacer/horizon-ntm/ntm-src')
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else default_src
    export(src, root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

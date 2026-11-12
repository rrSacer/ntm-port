#!/usr/bin/env python3
"""SolderingRecipes defaults (the soldering station): toppings (3 slots), pcb
(2) and solder (1), an optional fluid, an output, ticks and HE a tick. The
helper calls `addFirstUpgrade`/`addSecondUpgrade` are expanded as the source
does; `no528` is true (the port's profile), the OpenComputers branch is out."""
import re
from bulk_recipes import Unsupported, calls, split
from machine_recipes import integer
from bedrock_processing_common import Context, body, skip, write

PROFILE_LBSM = False


def lbsm(expr):
    """`lbsm ? a : b` with LBSM off."""
    return re.sub(r'\blbsm\s*\?\s*([^:,()]+?)\s*:\s*([^,()]+?)(?=\s*[,)])', lambda m: m[2], expr)


def stacks(ctx, text):
    arr = re.fullmatch(r'new AStack\[\]\s*\{(.*)\}', text.strip(), re.S)
    if not arr: raise Unsupported('unexpected AStack array: ' + text[:60])
    return [ctx.ingredient(x) for x in split(arr[1])] if arr[1].strip() else []


def recipe_from(ctx, name, origin, i, args):
    wrapped = re.fullmatch(r'new SolderingRecipe\((.*)\)', args.strip(), re.S)
    if not wrapped: raise Unsupported('unsupported expression ' + args[:60])
    parts = split(lbsm(wrapped[1]))
    row = ctx.row(name, origin, i)
    out = parts[0]
    row['outputItems'] = [ctx.stack(out)]
    duration, consumption = integer(parts[1]), integer(parts[2])
    rest = parts[3:]
    fluid = None
    if rest[0].startswith('new FluidStack'):
        fluid = ctx.fluid(rest[0]); rest = rest[1:]
    if len(rest) != 3: raise Unsupported('unexpected soldering arguments')
    row.update(duration=duration, power=consumption, toppings=stacks(ctx, rest[0]), pcb=stacks(ctx, rest[1]), solder=stacks(ctx, rest[2]))
    row['inputFluids'] = [fluid] if fluid else []
    return ctx.label(row)


def export(source, output, rows):
    ctx = Context(source, rows)
    path, text, start, end = body(source, 'SolderingRecipes')
    recipes = []; skipped = []
    helpers = {}
    for m in re.finditer(r'public static void (addFirstUpgrade|addSecondUpgrade)\(Item lower, Item higher\)\s*\{', text):
        helpers[m[1]] = text[m.end():]
    for pos, args, _ in calls(text, r'recipes\.add'):
        if not start < pos < end: continue
        origin = path + ':' + str(text[:pos].count('\n') + 1)
        # `Loader.isModLoaded("OpenComputers")` is false; its recipes sit after that line.
        if text.rfind('isModLoaded("OpenComputers")', 0, pos) > text.rfind('}\n\t}', 0, pos): continue
        try:
            recipes.append(recipe_from(ctx, f'soldering.{len(recipes)}', origin, len(recipes), args))
        except (Unsupported, KeyError, ValueError, IndexError) as error:
            skip(skipped, f'soldering.{len(recipes)}', origin, args, error)
    # The upgrade chains: addFirstUpgrade / addSecondUpgrade bodies, each call expanded.
    for pos, args, method in calls(text, r'\b(?:addFirstUpgrade|addSecondUpgrade)'):
        if not start < pos < end: continue
        origin = path + ':' + str(text[:pos].count('\n') + 1)
        lower, higher = [a.strip().replace('ModItems.', 'ntm:') for a in split(args)]
        first = method.strip().endswith('addFirstUpgrade')
        item = lambda a: a
        try:
            fluid = None if first else ctx.fluid('new FluidStack(Fluids.SOLVENT, 500)')
            chip, cap = (8, 4) if first else (16, 16)
            plastic = ('ANY_PLASTIC', 4) if first else ('RUBBER', 4)
            ingot = ctx.ingredient(f'new OreDictStack({plastic[0]}.ingot(), 4)')
            row = ctx.row(f'soldering.{len(recipes)}', origin, len(recipes))
            row['outputItems'] = [[higher, 1]]
            row.update(duration=300 if first else 400, power=10_000 if first else 25_000,
                       toppings=[{'ids': [ctx.stack('new ComparableStack(ModItems.circuit, 1, EnumCircuitType.CHIP)')[0]], 'count': chip},
                                 {'ids': [ctx.stack('new ComparableStack(ModItems.circuit, 1, EnumCircuitType.CAPACITOR)')[0]], 'count': cap}],
                       pcb=[{'ids': [lower], 'count': 1}, ingot], solder=[])
            row['inputFluids'] = [fluid] if fluid else []
            recipes.append(ctx.label(row))
        except (Unsupported, KeyError, ValueError, IndexError) as error:
            skip(skipped, f'soldering.upgrade.{origin}', origin, args, error)
    return write(output, 'soldering', {
        'machine_config': {'power_buffer_min': 2000, 'tank_capacity_mb': 8000, 'slots': 11, 'toppings': [0, 3], 'pcb': [3, 5], 'solder': [5, 6], 'output': 6, 'battery': 7, 'fluid_identifier': 8, 'upgrades': [9, 11],
                           'source': 'src/main/java/com/hbm/tileentity/machine/TileEntityMachineSolderingStation.java'},
        'coverage': {'emitted': len(recipes), 'skipped': len(skipped)}, 'recipes': recipes, 'skipped': skipped})


if __name__ == '__main__':
    import json, sys
    from pathlib import Path
    root = Path(__file__).resolve().parents[2]
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else root.parent / 'ntm-src'
    export(src, root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

#!/usr/bin/env python3
"""Electrolyser fluid and metal defaults, with exact molten quanta."""
import re
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import integer
from bedrock_processing_common import Context, BASE, body, config, events, skip, write


def export(source, output, rows):
    ctx = Context(source, rows); sections = {}
    for section, filename in [('fluid', 'ElectrolyserFluidRecipes'), ('metal', 'ElectrolyserMetalRecipes')]:
        recipes = []; skipped = []; total = 0
        _, text, start, end = body(source, filename)
        products = {}
        for match in re.finditer(r'(products\w+)\.add\(new Pair\(([^;]+)\)\);', text[start:end]):
            products.setdefault(match[1], []).append(split(match[2]))
        for index, origin, expr, ore_type in events(ctx, filename, r'recipes\.put'):
            total += 1; name = f'electrolyser.{section}.{index}' + ('.' + ore_type['suffix'] if ore_type else '')
            try:
                row = ctx.row(name, origin, index); args = split(expr)
                if section == 'fluid':
                    constructor = split(next(calls(args[1], r'new ElectrolysisRecipe'))[1])
                    fluid = args[0].removeprefix('Fluids.')
                    row['inputFluids'] = [ctx.fluid(f'new FluidStack(Fluids.{fluid}, {integer(constructor[0])})')]
                    slots = [ctx.fluid(x) for x in constructor[1:3]]
                    row['outputFluidSlots'] = slots
                    row['outputFluids'] = [x for x in slots if x[0] != 'none' and x[1] > 0]
                    tail = constructor[3:]; row['duration'] = 20
                    if tail and re.fullmatch(r'[\d_]+', tail[0]): row['duration'] = integer(tail.pop(0))
                    row['outputItems'] = [ctx.stack(x) for x in tail]
                else:
                    row['inputItems'] = [ctx.ingredient(args[0], ore_type)]
                    row['inputFluids'] = [['nitric_acid', 100]]
                    helper = re.fullmatch(r'makeBedrockOreProduct\((\w+)\)', args[1])
                    if helper:
                        row['duration'] = 20
                        for value, multiplier in products[helper[1]]:
                            if value.startswith('type.'):
                                field = value.split('.')[1]
                                material = ctx.material(f'ItemBedrockOreNew.toFluid(type.{field}, {multiplier})', ore_type)
                                if material and len(row['outputMaterials']) < 2:
                                    row['outputMaterials'].append(material)
                                else:
                                    fragment, amount = ore_type[field]
                                    row['outputItems'].append(['ntm:bedrock_ore_fragment__' + fragment, min(amount * integer(multiplier), 64)])
                            else: row['outputItems'].append(ctx.stack(value, ore_type))
                        if not row['outputMaterials']: row['outputMaterials'] = [['slag', ctx.shapes['INGOT']['amount'] * 2]]
                    else:
                        constructor = split(next(calls(args[1], r'new ElectrolysisMetalRecipe'))[1])
                        slots = [ctx.material(x) if x != 'null' else None for x in constructor[:2]]
                        row['outputMaterialSlots'] = slots
                        row['outputMaterials'] = [x for x in slots if x]
                        tail = constructor[2:]; row['duration'] = 600
                        if tail and re.fullmatch(r'[\d_]+', tail[0]): row['duration'] = integer(tail.pop(0))
                        row['outputItems'] = [ctx.stack(x) for x in tail]
                row['power'] = 10000
                recipes.append(ctx.label(row))
            except (Unsupported, KeyError, ValueError, StopIteration) as error:
                skip(skipped, name, origin, expr, error)
        sections[section] = {'selection': 'input tank fluid type' if section == 'fluid' else 'input item (exact identity before OreDict), requires nitric acid 100 mB',
                             'recipes': recipes, 'skipped': skipped, 'coverage': {'expanded': total, 'emitted': len(recipes), 'skipped': len(skipped)}}
    machine = config(source, 'tileentity/machine/TileEntityElectrolyser.java', {
        'power_metal': r'int usageOreBase = ([\d_]+)', 'power_fluid': r'int usageFluidBase = ([\d_]+)',
        'energy_capacity_he': r'long maxPower = ([\d_]+)', 'tank_capacity_mb': r'tanks\[0\] = new FluidTank\(Fluids.WATER, ([\d_]+)',
        'nitric_acid_per_metal_mb': r'this.tanks\[3\].getFill\(\) - ([\d_]+)'} )
    tile_path = BASE + 'tileentity/machine/TileEntityElectrolyser.java'
    tile = clean((source / tile_path).read_text())
    capacity = re.search(r'maxMaterial = MaterialShapes.(\w+).q\((\d+)\)', tile)
    machine['molten_capacity_per_output_quanta'] = ctx.shapes[capacity[1]]['amount'] * int(capacity[2])
    machine['sources']['molten_capacity_per_output_quanta'] = tile_path + ':' + str(tile[:capacity.start()].count('\n') + 1)
    return write(output, 'electrolyser', {**sections, 'machine_config': machine,
        'coverage': {k: v['coverage'] for k, v in sections.items()},
        'notes': ['outputMaterials uses foundry material names and quanta (72 per ingot), not fluid mB.',
                  'Two independent fluid and metal processes can run concurrently. Base power is per active process, without upgrades.',
                  'NONE output slots are retained in outputFluidSlots but omitted from outputFluids.',
                  'Bedrock helper emits at most two molten materials, otherwise fragments; no molten product gives two ingots of slag.']})

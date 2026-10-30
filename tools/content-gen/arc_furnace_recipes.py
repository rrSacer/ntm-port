#!/usr/bin/env python3
"""Separate solid/liquid arc recipes, source precedence and finite autogen."""
import json
import re
import tempfile
import bulk_recipes
from bulk_recipes import Unsupported, calls, clean, split
from foundry_recipes import distribution
from bedrock_processing_common import Context, BASE, body, config, events, skip, write


def export(source, output, rows):
    ctx = Context(source, rows); sections = {k: {'recipes': [], 'skipped': []} for k in ('solid', 'fluid')}
    occupied = {k: set() for k in sections}; collisions = []; empty = []
    def register(row, expr, modes, origin):
        # Java rejects the ENTIRE candidate if either claimed mode overlaps.
        ids = set(row['inputItems'][0]['ids'])
        conflicts = {mode: sorted(ids & occupied[mode]) for mode in modes if ids & occupied[mode]}
        if conflicts:
            collisions.append({'name': row['name'], 'source': origin, 'conflicts': conflicts,
                               'reason': 'register rejects whole recipe when any claimed input/mode is occupied'})
            return
        for mode in modes:
            out = dict(row)
            out['outputMaterials' if mode == 'solid' else 'outputItems'] = []
            sections[mode]['recipes'].append(ctx.label(out)); occupied[mode].update(ids)

    explicit = 0
    for index, origin, expr, ore_type in events(ctx, 'ArcFurnaceRecipes', r'\bregister'):
        # register(dict, recipe) in autogen is handled below with real domains.
        if expr in ('dict, recipe', 'comp, recipe'): continue
        explicit += 1; name = f'arc.{index}' + ('.' + ore_type['suffix'] if ore_type else '')
        try:
            args = split(expr); row = ctx.row(name, origin, index)
            row['inputItems'] = [ctx.ingredient(args[0], ore_type)]
            for _, value, method in calls(args[1], r'\.(?:solid|fluid|fluidNull)'):
                if method == '.solid': row['outputItems'] = [ctx.stack(value, ore_type)]
                else:
                    values = [ctx.material(x.replace('i3', '3'), ore_type) for x in split(value)]
                    if method == '.fluid' and any(x is None for x in values): raise Unsupported('null output in fluid()')
                    row['outputMaterials'] = [x for x in values if x]
            modes = [mode for mode, key in [('solid','outputItems'), ('fluid','outputMaterials')] if row[key]]
            row.update(duration=400, power=1000)
            if not modes: empty.append({'name':name,'source':origin,'reason':'fluidNull has no smeltable output; Java registers a recipe with no usable mode'})
            else: register(row, expr, modes, origin)
        except (Unsupported, KeyError, ValueError, StopIteration) as error:
            modes = [mode for mode, pattern in [('solid', r'\.solid\('), ('fluid', r'\.fluid(?:Null)?\(')] if re.search(pattern, expr)]
            for mode in modes: skip(sections[mode]['skipped'], name, origin, expr, error)

    path, text, _, _ = body(source, 'ArcFurnaceRecipes')
    def generated(name, ids, materials, origin):
        if not ids or not materials: return
        row = ctx.row(name, origin, None); row['inputItems'] = [{'ids': ids, 'count': 1}]
        row['outputMaterials'] = materials; row.update(duration=400, power=1000)
        register(row, name, ['fluid'], origin)
    auto_source = path + ':' + str(text[:text.index('for(NTMMaterial material')].count('\n') + 1)
    for material in ctx.materials:
        target = ctx.byname[material['smelts_into']]
        if target['behavior'] != 'SMELTABLE': continue
        for key, shape in ctx.shapes.items():
            if shape['no_autogen']: continue
            ore = (shape['prefixes'][0] if shape['prefixes'] else 'unknown') + material['names'][0]
            ids = ctx.resolver.ordered_ore.get(ore, [])
            if ids:
                amount = shape['amount'] * material['conv'][1] // material['conv'][0]
                generated('arc.auto.' + material['name'] + '.' + key.lower(), ids, [[target['name'], amount]], auto_source)
    exact, ores, rules, gaps = distribution(source, ctx.resolver, ctx.shapes)
    sections['fluid']['skipped'].extend(gaps)
    custom_source = path + ':' + str(text[:text.index('for(Entry<String, List<MaterialStack>>')].count('\n') + 1)
    # The Java HashMaps have unspecified ordering. Sort keys for a stable
    # snapshot; report collisions rather than pretend to reproduce hash order.
    for ore, products in sorted(ores.items()):
        molten = [x for x in products if ctx.byname[x[0]]['behavior'] == 'SMELTABLE']
        generated('arc.custom.ore.' + ore, ctx.resolver.ordered_ore.get(ore, []), molten, custom_source)
    for ident, products in sorted(exact.items()):
        molten = [x for x in products if ctx.byname[x[0]]['behavior'] == 'SMELTABLE']
        generated('arc.custom.item.' + ident.replace(':', '.'), [ident], molten, custom_source)
    # getOutput has a dynamic custom_data-aware scraps fast path. Publish its
    # default catalog amounts as data, and keep the runtime requirement explicit.
    scrap_defaults = {}
    for row in rows:
        if row['source_variable'] == 'scraps':
            mat = ctx.byconstant[row['source_material']]
            if mat['behavior'] == 'SMELTABLE': scrap_defaults[row['id']] = [[mat['name'], 72]]

    # Reuse resolved NTM vanilla-furnace registrations, not blasting bridges.
    eligible = set()
    for key, ids in ctx.resolver.ordered_ore.items():
        if key.startswith(('ingot','ore','plate','block')): eligible.update(ids)
    for _, args, _ in calls(ctx.resolver.odtext, r'arcSmeltable\.add'):
        try: eligible.add(ctx.stack(args)[0])
        except Unsupported: pass
    smelting = []
    resolved = {}
    with tempfile.TemporaryDirectory(prefix='ntm-arc-smelting-') as directory:
        from pathlib import Path
        bulk_recipes.generate(source, Path(directory), rows, lambda path, value: resolved.__setitem__(path, value))
    for recipe_path, recipe in sorted(resolved.items()):
        if recipe.get('type') != 'minecraft:smelting': continue
        ingredient = recipe.get('ingredient'); result = recipe['result']
        if isinstance(ingredient, str) and not ingredient.startswith('#') and (ingredient in eligible or result['id'] in eligible):
            smelting.append((ingredient, [result['id'], result.get('count',1)], recipe_path.split('/')[-1]))
    furnace_source = path + ':' + str(text[:text.index('registerFurnaceSmeltables();')].count('\n') + 1)
    for ident, result, file in smelting:
        row = ctx.row('arc.furnace.' + file.removesuffix('.json'), furnace_source, None)
        row['inputItems'] = [{'ids':[ident], 'count':1}]; row['outputItems']=[result]
        row.update(duration=400, power=1000, furnace_snapshot_recipe='ntm:' + file.removesuffix('.json'))
        register(row, file, ['solid'], furnace_source)
    skip(sections['solid']['skipped'], 'arc.furnace.runtime', furnace_source, 'registerFurnaceSmeltables()',
         'Only resolved port smelting registrations are available; legacy vanilla and external runtime FurnaceRecipes outside that snapshot are not enumerated')
    conf = config(source, 'tileentity/machine/TileEntityMachineArcFurnaceLarge.java', {
        'energy_capacity_he':r'long maxPower = ([\d_]+)', 'power':r'int consumption = \(int\) \(([\d_]+) \* Math.pow',
        'duration':r'int duration = ([\d_]+) /'})
    conf['molten_capacity_quanta'] = ctx.shapes['BLOCK']['amount'] * 128
    conf['sources']['molten_capacity_quanta'] = BASE + 'tileentity/machine/TileEntityMachineArcFurnaceLarge.java:84'
    for section in sections.values(): section['selection'] = 'input identity/OreDict; first registered matching recipe in selected solid or liquid machine mode'
    return write(output, 'arc_furnace', {**sections, 'machine_config':conf, 'scrap_default_materials':scrap_defaults,
        'empty_source_recipes':empty, 'collisions':collisions,
        'coverage':{'explicit_expanded':explicit, **{k:{'emitted':len(v['recipes']),'skipped':len(v['skipped'])} for k,v in sections.items()}, 'collisions_kept_first':len(collisions)},
        'notes':['Molten material names/quanta match foundry.json; no molten product becomes a filled fluid container.',
                 'Source register rejects a whole candidate if any input collides in any claimed mode. Material/shape auto precedes custom smeltables, which precede furnace recipes.',
                 'Custom-smeltable source HashMaps have no declared iteration order; this snapshot sorts keys and reports collisions. External mods/config/IMC excluded.',
                 'Dynamic scraps use stored MaterialStack quantity, not the 72-quanta default table, and require server logic.',
                 'Base 400 ticks and 1000 HE/t apply to a BATCH of inputs; up to three input slots and molten capacity bound the batch. Electrode wear is once per batch.']})

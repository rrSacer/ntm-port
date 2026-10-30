#!/usr/bin/env python3
"""All pyrolysis defaults, including source-derived solid-fuel amounts."""
import re
import ast
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import integer
from bedrock_processing_common import Context, body, config, events, skip, write


def fuel_heat(source):
    text = clean((source / 'src/main/java/com/hbm/inventory/fluid/Fluids.java').read_text())
    heat = {}
    for match in re.finditer(r'\b([A-Z][A-Z0-9_]*)\s*=\s*new FluidType\((.*?);', text, re.S):
        trait = re.search(r'new FT_Flammable\(([^)]+)\)', match[2])
        if trait: heat[match[1]] = integer(trait[1])
    values = {}
    def number(expr):
        expr = re.sub(r'(\w+)\.getTrait\(FT_Flammable.class\)\.getHeatEnergy\(\)', lambda m: str(heat[m[1]]), expr)
        expr = re.sub(r'(?<=\d)[DLF]\b', '', expr).replace('_', '')
        def read(node):
            if isinstance(node, ast.Constant) and type(node.value) in (int,float): return node.value
            if isinstance(node, ast.Name) and node.id in values: return values[node.id]
            if isinstance(node, ast.BinOp):
                a, b = read(node.left), read(node.right)
                if isinstance(node.op, ast.Mult): return a * b
                if isinstance(node.op, ast.Div): return a / b
                if isinstance(node.op, ast.Add): return a + b
                if isinstance(node.op, ast.Sub): return a - b
            raise Unsupported('unsupported calculated fuel arithmetic: ' + expr)
        return read(ast.parse(expr.strip(), mode='eval').body)
    # Declarations and calculated trait updates execute in their source order.
    events = [(m.start(), 'var', (m[1],m[2])) for m in re.finditer(r'(?:long|double|int)\s+(\w+)\s*=\s*([^;]+);', text)]
    events += [(p,'fuel',split(args)) for p,args,_ in calls(text,r'\bregisterCalculatedFuel') if split(args)[0] in heat or re.fullmatch(r'[A-Z][A-Z0-9_]*', split(args)[0])]
    for _, kind, args in sorted(events):
        if kind == 'var':
            try: values[args[0]] = number(args[1])
            except (Unsupported, KeyError, SyntaxError): continue  # unrelated fluid code
        else:
            amount = int(number(args[1]))
            for limit, quantum in [(10000000,100000),(1000000,10000),(100000,1000),(10000,100),(1000,10)]:
                if amount > limit:
                    amount -= amount % quantum
                    break
            heat[args[0]] = amount
    return heat


def export(source, output, rows):
    ctx = Context(source, rows)
    _, text, _, _ = body(source, 'PyroOvenRecipes')
    heat = fuel_heat(source)
    default = re.search(r'registerSFAuto\(fluid,\s*([\d_]+L),\s*ModItems\.(\w+)\)', text)
    default_heat, default_fuel = integer(default[1]), default[2]
    bonus = float(re.search(r'double bonus = ([0-9.]+)D', text)[1])
    recipes = []; skipped = []; expanded = 0
    for i, origin, expr, ore_type in events(ctx, 'PyroOvenRecipes', r'\b(?:registerSFAuto|recipes\.add)'):
        name = f'pyrooven.{i}' + ('.' + ore_type['suffix'] if ore_type else '')
        expanded += 1
        try:
            row = ctx.row(name, origin, i)
            if not expr.startswith('new PyroOvenRecipe'):
                args = split(expr); var = args[0].removeprefix('Fluids.')
                tu = integer(args[1]) if len(args) == 3 else default_heat
                item = args[2] if len(args) == 3 else 'ModItems.' + default_fuel
                amount = int(tu * 1000 * bonus / heat[var])
                if amount > 10000: amount -= amount % 1000
                elif amount > 1000: amount -= amount % 100
                elif amount > 100: amount -= amount % 10
                row['inputFluids'] = [ctx.fluid(f'new FluidStack(Fluids.{var}, {max(amount, 1)})')]
                row['outputItems'] = [ctx.stack('new ItemStack(' + item + ')')]
                row['duration'] = 60
                row['solid_fuel_auto'] = {'heat_per_bucket': heat[var], 'tu_per_fuel': tu, 'bonus': bonus}
            else:
                row['duration'] = integer(next(calls(expr, r'new PyroOvenRecipe'))[1])
                overwritten = []
                for _, args, method in calls(expr, r'\.(?:in|out)'):
                    direction = 'input' if method == '.in' else 'output'
                    isfluid = args.startswith('new FluidStack')
                    field = direction + ('Fluids' if isfluid else 'Items')
                    if row[field]: overwritten.append({field: row[field]})
                    row[field] = [ctx.fluid(args) if isfluid else ctx.ingredient(args, ore_type) if direction == 'input' else ctx.stack(args, ore_type)]
                if overwritten: row['overwritten_source_fields'] = overwritten
            row['power'] = 10000
            recipes.append(ctx.label(row))
        except (Unsupported, KeyError, ValueError, StopIteration) as error:
            skip(skipped, name, origin, expr, error)
    return write(output, 'pyrooven', {'selection': 'input item and fluid type; cached last match, otherwise first matching source recipe; item-less recipe requires empty input slot',
        'machine_config': config(source, 'tileentity/machine/oil/TileEntityMachinePyroOven.java', {
            'power': r'int consumption = ([\d_]+)', 'energy_capacity_he': r'long maxPower = ([\d_]+)',
            'tank_capacity_mb': r'tanks\[0\] = new FluidTank\(Fluids.NONE, ([\d_]+)'}),
        'coverage': {'expanded': expanded, 'emitted': len(recipes), 'skipped': len(skipped),
                     'solid_fuel_auto': sum('solid_fuel_auto' in r for r in recipes),
                     'bedrock': sum('.' in r['name'].split('.', 1)[1] for r in recipes)},
        'recipes': recipes, 'skipped': skipped,
        'notes': ['Base configuration without upgrades. Source in/out setters overwrite earlier values of the same kind; overwritten_source_fields documents this, including soot hydrogen overwritten by carbon dioxide.']})

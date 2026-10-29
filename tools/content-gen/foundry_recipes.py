#!/usr/bin/env python3
"""Deterministic Mats/MatDistribution/CrucibleRecipes snapshot for the foundry."""
import ast
import json
import re
from pathlib import Path
from bulk_recipes import Unsupported, calls, clean, split
from shredder_recipes import ShredderResolver, dictionary
from blast_furnace_recipes import block_end, fuel_number, FUEL
from foundry_catalog import molds

BASE = 'src/main/java/com/hbm/'
MATS = BASE + 'inventory/material/Mats.java'
SHAPES = BASE + 'inventory/material/MaterialShapes.java'
DISTRIBUTION = BASE + 'inventory/material/MatDistribution.java'
ALLOYS = BASE + 'inventory/recipes/CrucibleRecipes.java'


def number(expr, values):
    """Small fail-closed arithmetic reader, never evaluate Java/Python code."""
    def read(node):
        if isinstance(node, ast.Constant) and type(node.value) is int: return node.value
        if isinstance(node, ast.Name) and node.id in values: return values[node.id]
        if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Mult)):
            a, b = read(node.left), read(node.right)
            return a + b if isinstance(node.op, ast.Add) else a * b
        raise Unsupported('unsupported number: ' + expr)
    expr = re.sub(r'\b0+(\d+)\b', lambda m: str(int(m[0])), expr.replace('.quantity', ''))
    return read(ast.parse(expr.strip(), mode='eval').body)


def shape_table(source):
    text = clean((source / SHAPES).read_text()); result = {}; values = {}
    for m in re.finditer(r'public static final MaterialShapes (\w+)\s*=\s*new MaterialShapes\(([^;]+);', text):
        # The first constructor closing paren occurs before optional .noAutogen().
        args = split(m[2].split(')', 1)[0]); quantity = number(args[0], values)
        values[m[1]] = quantity
        result[m[1]] = {'amount': quantity, 'prefixes': [json.loads(a) for a in args[1:]],
                        'no_autogen': '.noAutogen()' in m[0]}
    if values.get('INGOT') != 72: raise ValueError('Quanta unit drift')
    return result


def material_table(source, resolver):
    text = clean((source / MATS).read_text()); result = []
    for m in re.finditer(r'public static final NTMMaterial (MAT_\w+)\s*=\s*(make\w*)\(([^;]+);', text):
        expr = m[3]
        args = split(next(calls('make(' + expr, r'make'))[1])
        names = [json.loads(args[1][3:-1])] if args[1].startswith('df(') else resolver.frames[args[1]]
        colors = [int(a, 16) for a in args[2:]]
        if not colors: colors = [0xFF4A00, 0x802000, 0xFF4A00]
        if len(colors) == 1: colors *= 3
        if len(colors) != 3: raise ValueError('Unsupported material color: ' + m[0])
        conversion = re.search(r'\.setConversion\((MAT_\w+),\s*(\d+),\s*(\d+)\)', expr)
        autogen = re.search(r'\.setAutogen\(([^)]*)\)', expr)
        behavior = {'make':'NOT_SMELTABLE', 'makeSmeltable':'SMELTABLE', 'makeAdditive':'ADDITIVE', 'makeNonSmeltable':'NOT_SMELTABLE'}[m[2]]
        result.append({'id': number(args[0], {'_VS':0, '_AS':30, '_ES':20000}), 'name':m[1][4:].lower(),
                       'constant':m[1], 'names':names, 'behavior':behavior,
                       'smelts_into': (conversion[1] if conversion else m[1])[4:].lower(),
                       'conv':[int(conversion[2]), int(conversion[3])] if conversion else [1,1],
                       'colors':[colors[0], colors[2], colors[1]],
                       'traits':['METAL'] if '.m()' in expr else ['NONMETAL'] if '.n()' in expr else [],
                       'autogen':split(autogen[1]) if autogen else [],
                       'source':MATS + ':' + str(text[:m.start()].count('\n') + 1)})
    if len(result) != 109 or len({m['id'] for m in result}) != 109: raise ValueError('Mats count/identity drift')
    return result


def ore_snapshot(source, resolver, materials, shapes):
    ores, sources = dictionary(resolver)
    def add(key, ids, origin):
        target = ores.setdefault(key, [])
        for ident in ids:
            if ident not in target: target.append(ident)
        if origin not in sources[key]: sources[key].append(origin)
    # The shredder snapshot intentionally needs fewer prefixes. Extend it from
    # DictFrame method declarations, including billet and the tiny alias.
    methods = {}
    for m in re.finditer(r'public DictFrame (\w+)\(Object\.\.\.\s+\w+\)\s*\{([^}]+)\}', resolver.odtext):
        methods[m[1]] = re.findall(r'makeObject\((\w+),', m[2])
    for chain in re.finditer(r'\b([A-Z][A-Z0-9_]*)\s*\.\s*[a-z]\w*\([^;]*;', resolver.odtext, re.S):
        if chain[1] not in resolver.frames: continue
        for _, args, method in calls(chain[0], r'\.\w+'):
            for shape in methods.get(method[1:], []):
                for expr in split(args):
                    try: ids = resolver.values(expr)
                    except Unsupported: continue
                    for name in resolver.frames[chain[1]]:
                        add(shapes[shape]['prefixes'][0] + name, ids, 'OreDictManager.java:' + str(resolver.odtext[:chain.start()].count('\n') + 1))
    byconstant = {m['constant']:m for m in materials}
    forms = {'ingot_raw':'INGOT', 'nugget_raw':'NUGGET', 'wire_fine':'WIRE', 'wire_dense':'DENSEWIRE',
             'plate_cast':'CASTPLATE', 'plate_welded':'WELDEDPLATE', 'bedrock_ore_fragment':'FRAGMENT', 'bolt':'BOLT',
             'shell':'SHELL', 'pipe':'PIPE', 'part_barrel_light':'LIGHTBARREL', 'part_barrel_heavy':'HEAVYBARREL',
             'part_receiver_light':'LIGHTRECEIVER', 'part_receiver_heavy':'HEAVYRECEIVER', 'part_mechanism':'MECHANISM',
             'part_stock':'STOCK', 'part_grip':'GRIP'}
    for row in resolver.rows:
        shape = forms.get(row['source_variable']); mat = byconstant.get(row.get('source_material'))
        if shape and mat:
            for name in mat['names']: add(shapes[shape]['prefixes'][0] + name, [row['id']], 'OreDictManager.java:543-565')
    # Forge 1.7.10 initializes these keys too; dictionary() focuses on shredding.
    for key, var in [('stone','stone'), ('cobblestone','cobblestone'), ('nuggetGold','gold_nugget')]:
        add(key, [resolver.vanilla_item('Items.' + var)], 'Forge 1.7.10 OreDictionary.initVanillaEntries')
    return ores, sources


def quantity(expr, shapes):
    match = re.fullmatch(r'(?:MaterialShapes\.)?(\w+)\.q\((.*)\)', expr)
    if not match: raise Unsupported('unsupported shape quantity: ' + expr)
    args = [number(a, {}) for a in split(match[2])]
    return shapes[match[1]]['amount'] * args[0] // (args[1] if len(args) > 1 else 1)


def distribution(source, resolver, shapes):
    text = clean((source / DISTRIBUTION).read_text()).split('public static void registerEntry', 1)[0]
    exact = {}; ores = {}; rules = []; skipped = []
    for pos, args, method in calls(text, r'\b(?:registerEntry|registerOre)'):
        parts = split(args); origin = DISTRIBUTION + ':' + str(text[:pos].count('\n') + 1)
        try:
            mats = [[parts[i].removeprefix('Mats.')[4:].lower(), quantity(parts[i+1], shapes)] for i in range(1, len(parts), 2)]
            if method == 'registerEntry':
                key = resolver.item(parts[0])[0]; exact[key] = mats; keys = [key]
            else:
                expr = parts[0]
                if expr.startswith('"'): keys = [json.loads(expr)]
                elif expr == 'ore':
                    # The one finite loop is TH232.all(ONLY_ORE).
                    keys = ['ore' + n for n in resolver.frames['TH232']]
                else:
                    match = re.fullmatch(r'OreDictManager\.(\w+)\.ore\(\)', expr)
                    if not match: raise Unsupported('unsupported material OreDict expression: ' + expr)
                    keys = ['ore' + n for n in resolver.frames[match[1]]]
                for key in keys: ores[key] = mats
            rules.append({'kind':method, 'keys':keys, 'materials':mats, 'source':origin})
        except (Unsupported, KeyError, IndexError) as error:
            skipped.append({'source':origin, 'expression':args, 'reason':str(error)})
    return exact, ores, rules, skipped


def item_table(resolver, materials, shapes, ores, exact, special):
    byname = {name:mat for mat in materials for name in mat['names']}; byinternal = {mat['name']:mat for mat in materials}
    items = {}; matches = {}; collisions = []
    prefixes = {prefix:shape for shape in shapes.values() for prefix in shape['prefixes']}
    for key, ids in ores.items():
        mats = special.get(key)
        if mats is None:
            candidates = []
            for prefix, shape in prefixes.items():
                if not key.startswith(prefix): continue
                mat = byname.get(key[len(prefix):])
                if mat and byinternal[mat['smelts_into']]['behavior'] in ('SMELTABLE','ADDITIVE'):
                    candidates.append([[mat['name'], shape['amount']]])
            if not candidates: continue
            if any(x != candidates[0] for x in candidates): raise ValueError('Ambiguous prefix: ' + key)
            mats = candidates[0]
        for ident in ids:
            if ident not in items:
                items[ident] = [list(x) for x in mats]; matches[ident] = {'ore_key':key}
            elif items[ident] != mats:
                collisions.append({'item':ident, 'kept':matches[ident]['ore_key'], 'ignored':key,
                                   'kept_materials':items[ident], 'ignored_materials':mats})
    for ident, mats in exact.items():
        items.setdefault(ident, []).extend([list(x) for x in mats]); matches.setdefault(ident, {})['exact'] = True
    for row in resolver.byvar.get('scraps', []):
        mat = next(m for m in materials if m['constant'] == row['source_material'])
        items.setdefault(row['id'], []).append([mat['name'], 72]); matches.setdefault(row['id'], {})['scrap_default'] = True
    return dict(sorted(items.items())), dict(sorted(matches.items())), collisions


def alloys(source, resolver):
    text = clean((source / ALLOYS).read_text()).split('public String getFileName', 1)[0]; active = []; disabled = []
    for index, (pos, expr, _) in enumerate(calls(text, r'this\.register')):
        name = re.search(r'new CrucibleRecipe\("([^"]+)"\)', expr)[1]
        methods = {method[1:]:split(args) for _, args, method in calls(expr, r'\.\w+')}
        def stacks(expressions):
            out = []
            for expression in expressions:
                match = re.fullmatch(r'new MaterialStack\(Mats\.(MAT_\w+),\s*(.*)\)', expression)
                if not match: raise Unsupported('unsupported alloy material: ' + expression)
                out.append([match[1][4:].lower(), number(match[2], {'n':8, 'i':72})])
            return out
        gt6 = name in ('crucible.steelWrought','crucible.steelPig','crucible.steelMeteoric')
        row = {'name':name, 'source_index':index, 'frequency':number(methods['setup'][0], {}),
               'icon':resolver.item(methods['setup'][1])[0], 'in':stacks(methods['inputs']), 'out':stacks(methods['outputs']),
               'requires_mod':'gregtech' if gt6 else None, 'enabled_by_default':not gt6,
               'source':ALLOYS + ':' + str(text[:pos].count('\n') + 1)}
        (disabled if gt6 else active).append(row)
    return active, disabled


def mold_table(source, resolver, materials, shapes, ores):
    result = []; gaps = []
    for definition in molds(source):
        row = {k:v for k,v in definition.items() if k not in ('args','class')}
        row['item'] = 'ntm:mold__' + row['name']; args = definition['args']; outputs = {}
        if definition['class'] == 'MoldMulti':
            row.update(shape=None, amount=1, cost=quantity(args[0], shapes))
            for constant, expr in zip(args[1::2], args[2::2]):
                try: outputs[constant.removeprefix('Mats.MAT_').lower()] = list(resolver.item(expr))
                except Unsupported as error: gaps.append({'mold':row['name'], 'expression':expr, 'reason':str(error)})
        else:
            shape = args[0].removeprefix('MaterialShapes.'); amount = int(args[1]) if len(args) > 1 else 1
            row.update(shape=shape.lower(), amount=amount, cost=shapes[shape]['amount'] * amount)
            for mat in materials:
                if mat['behavior'] != 'SMELTABLE': continue
                if definition['class'] == 'MoldBlock' and mat['name'] in ('stone','obsidian'):
                    outputs[mat['name']] = ['minecraft:' + mat['name'], 1]; continue
                for name in mat['names']:
                    ids = ores.get(shapes[shape]['prefixes'][0] + name, [])
                    if ids:
                        ident = next((x for x in ids if x.startswith('ntm:') and 'fragment' not in x), ids[0])
                        outputs[mat['name']] = [ident, amount]; break
        row['outputs'] = outputs; result.append(row)
    return result, gaps


def firebox_fuels(source, resolver, ores):
    text = clean((source / FUEL).read_text()); single = int(re.search(r'\bint single\s*=\s*(\d+)', text)[1])
    raw = {}; origins = {}; gaps = []
    for pos, condition, _ in calls(text, r'\bif'):
        match = re.fullmatch(r'fuel.getItem\(\)(?:\.equals\(Mod(Items|Blocks)\.(\w+)\)|\s*==\s*Mod(Items|Blocks)\.(\w+)|\s*==\s*Item.getItemFromBlock\(ModBlocks\.(\w+)\))', condition.strip())
        if not match: continue
        var = match[2] or match[4] or match[5]; end = text.find('(', pos) + len(condition) + 2; tail = text[end:]
        direct = re.match(r'\s*return\s+([^;]+);', tail); origin = FUEL + ':' + str(text[:pos].count('\n') + 1)
        try:
            cases = None
            if not direct:
                opening = end + len(tail) - len(tail.lstrip())
                if text[opening] != '{': raise Unsupported('unrecognized fuel branch')
                cases = {int(meta):fuel_number(expr, single) for meta,expr in re.findall(r'case\s+(\d+)\s*:\s*return\s+([^;]+);', text[opening:block_end(text, opening)])}
            if not resolver.byvar.get(var): gaps.append({'variable':var, 'source':origin, 'reason':'FuelHandler item missing from catalog'})
            for row in resolver.byvar.get(var, []):
                ticks = fuel_number(direct[1], single) if direct else cases.get(row.get('metadata'), 0)
                if ticks: raw[row['id']] = ticks; origins[row['id']] = origin
        except Unsupported as error: gaps.append({'variable':var, 'source':origin, 'reason':str(error)})
    # Explicit legacy furnace identities; no new woods/material categories inferred
    # from modern registry substrings. Values are the 1.7.10 furnace defaults.
    vanilla = {'coal':1600, 'charcoal':1600, 'coal_block':16000, 'lava_bucket':20000, 'blaze_rod':2400,
               'stick':100, 'oak_fence':300, 'oak_fence_gate':300, 'chest':300, 'trapped_chest':300,
               'crafting_table':300, 'bookshelf':300, 'jukebox':300, 'note_block':300,
               'oak_trapdoor':300, 'oak_pressure_plate':300, 'daylight_detector':300}
    for wood in ('oak','spruce','birch','jungle','acacia','dark_oak'):
        for suffix,ticks in [('log',300), ('planks',300), ('stairs',300), ('slab',150), ('sapling',100)]: vanilla[wood+'_'+suffix] = ticks
    for tool in ('sword','pickaxe','axe','shovel','hoe'): vanilla['wooden_'+tool] = 200
    for name,ticks in vanilla.items():
        ident = resolver.vanilla_item('Items.' + name); raw[ident] = ticks; origins[ident] = 'vanilla 1.7.10 TileEntityFurnace.getItemBurnTime legacy profile'
    itemtext = clean((source / (BASE + 'items/ModItems.java')).read_text())
    containers = {}
    for var, expr in re.findall(r'\b(\w+)\.setContainerItem\(([^)]+)\)', itemtext):
        try: containers[var] = resolver.item(expr)[0]
        except Unsupported: pass
    for var,expr in re.findall(r'^\s*(\w+)\s*=\s*new[^;]*?\.setContainerItem\(([^)]+)\)', itemtext, re.M):
        try: containers[var] = resolver.item(expr)[0]
        except Unsupported: pass
    byid = {row['id']:row for row in resolver.rows}; keys = {}
    for key,ids in ores.items():
        for ident in ids: keys.setdefault(ident, []).append(key)
    result = {}; details = []
    for ident,ticks in sorted(raw.items()):
        var = byid.get(ident, {}).get('source_variable', '')
        category = 'balefire' if var.startswith('solid_fuel') and var.endswith('_bf') else 'solid' if var.startswith('solid_fuel') else 'rocket' if var == 'rocket_fuel' else None
        if not category:
            for key in keys.get(ident, []):
                category = next((x.lower() for x in ('Coke','Coal','Lignite') if x in key), None)
                if category: break
                if key.startswith('log'): category = 'log'; break
                if 'Wood' in key: category = 'wood'; break
        time,heat = {'coal':(1.25,2), 'coke':(1.25,2), 'lignite':(1.25,2), 'solid':(1.5,3), 'rocket':(1.5,5), 'balefire':(0.5,15)}.get(category,(1,1))
        result[ident] = {'ticks':int(ticks*time), 'heat':100*heat,
                         'container':'minecraft:bucket' if ident == 'minecraft:lava_bucket' else containers.get(var)}
        details.append({'item':ident, 'base_ticks':ticks, 'time_modifier':time, 'heat_modifier':heat,
                        'category':category or 'uncategorized', 'ore_keys':keys.get(ident, []), 'source':origins[ident]})
    gaps.append({'reason':'Other-mod fuels/config overrides, modern-only vanilla fuels and unverified generic Material.wood NTM block subclasses are outside the explicit legacy fuel snapshot'})
    return result, details, gaps


def export(source, output, rows):
    resolver = ShredderResolver(source, rows); shapes = shape_table(source); materials = material_table(source, resolver)
    ores, ore_sources = ore_snapshot(source, resolver, materials, shapes)
    exact, special, rules, skipped = distribution(source, resolver, shapes)
    items, item_sources, collisions = item_table(resolver, materials, shapes, ores, exact, special)
    active, disabled = alloys(source, resolver)
    mold_data, mold_gaps = mold_table(source, resolver, materials, shapes, ores)
    fuels, fuel_details, fuel_gaps = firebox_fuels(source, resolver, ores)
    coverage = {'materials':len(materials), 'items':len(items), 'scraps':len(resolver.byvar['scraps']),
                'alloys':len(active), 'disabled_alloys':len(disabled), 'molds':len(mold_data),
                'distribution_rules':len(rules), 'distribution_skipped':len(skipped), 'firebox_fuels':len(fuels)}
    body = {'schema':1, 'machine':'foundry', 'profile':{'external_mods':[], 'gt6':False, 'custom_materials_shapes_recipes':False},
            'color_fields':['solidColorLight','moltenColor','solidColorDark'],
            'quanta':{name.lower():row['amount'] for name,row in shapes.items()}, 'shapes':shapes,
            'materials':materials, 'items':items, 'alloys':active, 'disabled_alloys':disabled, 'molds':mold_data,
            'firebox_fuels':fuels, 'fuel_details':fuel_details, 'fuel_gaps':fuel_gaps,
            'coverage':coverage, 'distribution_rules':rules, 'skipped':skipped, 'mold_gaps':mold_gaps,
            'item_sources':item_sources, 'ore_collisions':collisions,
            'ore_dictionary':ores, 'ore_sources':dict(ore_sources),
            'precedence':['Ore keys in snapshot registration order: exact materialOreEntries entry before prefix for that key; stop on first matching key',
                          'Append exact materialEntries (last registration wins per item), even after OreDict match',
                          'Append scrap material with default 72 quanta; all items are singular before stack count multiplication',
                          'Return unconverted materials; apply amount * convOut / convIn with integer truncation at runtime'],
            'gaps':[{'reason':'Scrap NBT amount/liquid flags cannot pass through current slots: all registered scraps use default 72'},
                    {'reason':'Mold engraving uses NTM anvil smithing/construction, not vanilla crafting; stage 3 runtime casting deferred'},
                    {'reason':'Ore key order across external mod registrations is excluded; snapshot uses existing shredder profile plus exact foundry prefixes'}]}
    target = output / 'machine-recipes/foundry.json'; target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(body, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(json.dumps({'foundry':coverage})); return body


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

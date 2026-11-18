#!/usr/bin/env python3
"""Export the literal default NTM machine DSL; unresolved calls stay in skipped.

This is a source translator, not a Java evaluator. No external mod, configuration
override, dynamic loop or probability distribution is silently approximated.
"""
import ast
import json
import re
from pathlib import Path
from bulk_recipes import Resolver, Unsupported, calls, clean, split
from m3_catalog import fluids
from generate import lang


PROFILE = {'enable528': False, 'enableExpensiveMode': False, 'enableLBSM': False,
           'enableLBSMSimpleCentrifuge': False, 'external_mods': []}


def integer(expr):
    try: tree = ast.parse(expr.strip().replace('_', '').removesuffix('L'), mode='eval')
    except SyntaxError: raise Unsupported('nonliteral integer: ' + expr) from None
    def read(node):
        if isinstance(node, ast.Constant) and type(node.value) is int: return node.value
        if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Sub, ast.Mult)):
            left, right = read(node.left), read(node.right)
            return left + right if isinstance(node.op, ast.Add) else left - right if isinstance(node.op, ast.Sub) else left * right
        raise Unsupported('nonliteral integer: ' + expr)
    value = read(tree.body)
    if value < 0: raise Unsupported('negative quantity: ' + expr)
    return value


def ranges(text):
    """Braced control regions, excluding braces inside strings."""
    result = []
    for pos, args, control in calls(text, r'\b(?:if|for|while)'):
        start = text.find('(', pos) + len(args) + 2
        while start < len(text) and text[start].isspace(): start += 1
        if start == len(text) or text[start] != '{': continue
        depth = 1; quote = False; escape = False; end = start + 1
        while end < len(text) and depth:
            char = text[end]
            if quote:
                if escape: escape = False
                elif char == '\\': escape = True
                elif char == '"': quote = False
            elif char == '"': quote = True
            elif char == '{': depth += 1
            elif char == '}': depth -= 1
            end += 1
        result.append((start, end, control, args.strip()))
    return result


class MachineResolver(Resolver):
    def __init__(self, source, rows):
        super().__init__(source, rows)
        self.fluids = {f['source_variable']: f for f in fluids(source)}
        containers = (source / 'src/main/java/com/hbm/inventory/FluidContainerRegistry.java').read_text()
        self.registered_fluid_containers = set(re.findall(r'new ItemStack\(ModItems\.(\w+),\s*1,\s*(?:id|i)\)', containers))
        # Translate DictFrame method names into MaterialShapes registration keys.
        self.shapes = {'plateCast': 'plateTriple', 'plateWelded': 'plateSextuple',
                       'heavyComp': 'plateSextuple', 'pipe': 'ntmpipe',
                       'lightBarrel': 'barrelLight', 'heavyBarrel': 'barrelHeavy',
                       'lightReceiver': 'receiverLight', 'heavyReceiver': 'receiverHeavy',
                       'mechanism': 'gunMechanism', 'fragment': 'bedrockorefragment'}
        for old, new in self.shapes.items():
            for key, values in list(self.ore.items()):
                if key.startswith(old): self.ore[new + key[len(old):]].update(values)
        mats = clean((source / 'src/main/java/com/hbm/inventory/material/Mats.java').read_text())
        for row in rows:
            if row['source_variable'] not in ('bolt', 'shell', 'pipe'): continue
            match = re.search(r'\b' + row['source_material'] + r'\s*=\s*make\w*\([^,]+,\s*(\w+)', mats)
            if match:
                for name in self.frames.get(match[1], []):
                    self.ore[{'bolt': 'bolt', 'shell': 'shell', 'pipe': 'ntmpipe'}[row['source_variable']] + name].add(row['id'])
        # A leading registerOres() brace hides the first chain from the older
        # vanilla extractor. Parse exact frame chains independently here.
        for match in re.finditer(r'\b([A-Z][A-Z0-9_]*)\s*\.\s*[a-z]\w*\([^;]*;', self.odtext, re.S):
            if match[1] not in self.frames: continue
            for _, args, method in calls(match[0], r'\.\w+'):
                shape = self.shapes.get(method[1:], method[1:])
                for expr in split(args):
                    all_enum = re.fullmatch(r'fromAll\((\w+),\s*(\w+)\.class\)', expr)
                    try:
                        if all_enum:
                            values = [r['id'] for r in self.byvar[all_enum[1]] if r.get('source_enum', '').startswith(all_enum[2] + '.')]
                        else: values = [self.item(expr, True)[0]]
                    except Unsupported: continue
                    for name in self.frames[match[1]]: self.ore[shape + name].update(values)
        for group in re.finditer(r'\b(\w+)\s*=\s*new DictGroup\("([^"]+)",([^;]+)\);', self.odtext):
            self.frames[group[1]] = [group[2]]
            members = split(group[3])
            for member in members:
                for name in self.frames.get(member, []):
                    for key, values in list(self.ore.items()):
                        if key.endswith(name): self.ore[key[:-len(name)] + group[2]].update(values)
                if member in self.constants: self.ore['any' + group[2]].update(self.ore.get(self.constants[member], []))
        for _, args, _ in calls(self.odtext, r'OreDictionary\.registerOre'):
            parts = split(args)
            if len(parts) != 2: continue
            key = self.constants.get(parts[0])
            if not key: continue
            try: self.ore[key].add(self.item(parts[1], True)[0])
            except Unsupported: pass
        # Groups with literal KEY_* members are registered after these ores in
        # Java. Rebuild those aliases now, including the four ANY_TAR variants.
        for group in re.finditer(r'\b(\w+)\s*=\s*new DictGroup\("([^"]+)",([^;]+)\);', self.odtext):
            for member in split(group[3]):
                if member in self.constants:
                    self.ore['any' + group[2]].update(self.ore.get(self.constants[member], []))
        for key, value in {'oreCoal': 'coal_ore', 'oreRedstone': 'redstone_ore', 'oreLapis': 'lapis_ore', 'oreNetherQuartz': 'nether_quartz_ore'}.items():
            self.ore[key].add('minecraft:' + value)

    def ingredient(self, expr, emit):
        for old, new in self.shapes.items():
            expr = expr.replace('.' + old + '()', '.' + new + '()')
        return super().ingredient(expr, emit)

    def metadata(self, expr, var):
        expr = expr.strip().removeprefix('ItemEnums.').removeprefix('BlockEnums.')
        match = re.fullmatch(r'Fluids\.(\w+)\.getID\(\)', expr)
        if match and match[1] in self.fluids: return self.fluids[match[1]]['metadata']
        if re.fullmatch(r'\w+\.\w+', expr) and (var, expr) in self.byenum:
            return self.byenum[var, expr]['metadata']
        return super().metadata(expr, var)

    def item(self, expr, unqualified=False):
        expr = expr.strip().replace('ItemEnums.', '')
        # LBSM defaults to false; only select a top-level, literal lbs ternary.
        if expr.startswith('lbs ?'):
            pieces = split_ternary(expr)
            if pieces is None: raise Unsupported('unsupported ternary: ' + expr)
            return self.item(pieces[2], unqualified)
        match = re.fullmatch(r'new (ItemStack|ComparableStack)\((.*)\)', expr, re.S)
        if match:
            parts = split(match[2])
            if len(parts) > 1: parts[1] = str(integer(parts[1]))
            expr = 'new ' + match[1] + '(' + ','.join(parts) + ')'
        return super().item(expr, unqualified)

    def expand_tag(self, ident, local, seen=None):
        if not ident.startswith('#'): return [ident]
        seen = set() if seen is None else seen
        if ident in seen: raise Unsupported('cyclic item tag ' + ident)
        namespace, path = ident[1:].split(':', 1)
        key = f'data/{namespace}/tags/item/{path}.json'
        if key in local: values = local[key]['values']
        elif namespace == 'minecraft':
            file = self.vanilla / 'tags/item' / (path + '.json')
            if not file.is_file(): raise Unsupported('missing vanilla tag ' + ident)
            values = json.loads(file.read_text())['values']
        else: raise Unsupported('unresolved item tag ' + ident)
        out = set()
        for value in values:
            value = value['id'] if isinstance(value, dict) else value
            out.update(self.expand_tag(value, local, seen | {ident}))
        return sorted(out)

    def input(self, expr):
        match = re.fullmatch(r'new OreDictStack\((.*)\)', expr.strip(), re.S)
        if not match:
            ident, count = self.item(expr)
            return [ident, count], [ident]
        args = split(match[1]); count = integer(args[1]) if len(args) > 1 else 1
        container = re.fullmatch(r'Fluids\.(\w+)\.getDict\(([^)]+)\)', args[0])
        if container:
            if container[1] not in self.fluids: raise Unsupported('unknown container fluid: ' + args[0])
            fluid = self.fluids[container[1]]['id']; amount = integer(container[2])
            options = sorted(row['id'] for row in self.rows
                             if row.get('source_variable') in self.registered_fluid_containers
                             and row.get('fluid', {}).get('id') == fluid
                             and row['fluid']['amount_mb'] == amount)
            if not options: raise Unsupported('no registered catalog container: ' + args[0])
            return [options[0], count], options
        local = {}
        ident = self.ingredient(args[0], lambda path, body: local.__setitem__(path, body))
        options = self.expand_tag(ident, local)
        if not options: raise Unsupported('empty ingredient ' + expr)
        # Prefer a port item; preserve all interchangeable choices alongside it.
        options.sort(key=lambda ident: (not ident.startswith('ntm:'), ident))
        return [options[0], count], options

    def fluid(self, expr):
        match = re.fullmatch(r'new FluidStack\(\s*Fluids\.(\w+)\s*,(.*)\)', expr.strip(), re.S)
        if not match or match[1] not in self.fluids: raise Unsupported('unresolved fluid: ' + expr)
        return [self.fluids[match[1]]['id'], integer(match[2])]


def output_item(resolver, expr):
    """An output: an item stack, or `new ChanceOutputMulti(new ChanceOutput(stack, weight), ...)`, one of its stacks
    picked by weight (`WeightedRandom.getRandomItem`), as `{"multi": [[id, count, weight], ...]}`."""
    multi = re.fullmatch(r'new ChanceOutputMulti\((.*)\)', expr.strip(), re.S)
    if not multi:
        return list(resolver.item(expr))
    pool = []
    for part in split(multi[1]):
        one = re.fullmatch(r'new ChanceOutput\((.*)\)', part.strip(), re.S)
        if not one: raise Unsupported('unsupported weighted output ' + part)
        args = split(one[1])
        if len(args) != 2 or not re.fullmatch(r'\d+', args[1].strip()): raise Unsupported('weighted output without an int weight: ' + part)
        ident, count = resolver.item(args[0])
        pool.append([ident, count, int(args[1])])
    return {'multi': pool}


def split_ternary(expr):
    depth = 0; question = None
    for pos, char in enumerate(expr):
        if char in '([{': depth += 1
        elif char in ')]}': depth -= 1
        elif char == '?' and depth == 0: question = pos
        elif char == ':' and depth == 0 and question is not None:
            return expr[:question].strip(), expr[question + 1:pos].strip(), expr[pos + 1:].strip()


def string(expr):
    expr = re.sub(r'(?:GenericRecipes\.)?POOL_PREFIX_(ALT|DISCOVER|SECRET|528)',
                  lambda match: json.dumps({'ALT': 'alt.', 'DISCOVER': 'discover.', 'SECRET': 'secret.', '528': '528.'}[match[1]]), expr)
    parts = re.split(r'\s*\+\s*(?=")', expr.strip())
    try:
        values = [json.loads(part) for part in parts]
        if not all(isinstance(part, str) for part in values): raise ValueError()
        return ''.join(values)
    except (ValueError, TypeError): raise Unsupported('dynamic string: ' + expr)


def export(source, output, rows):
    resolver = MachineResolver(source, rows)
    names = {row['id']: row['names'] for row in rows}
    translations = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    vanilla_names = {loc: json.loads((resolver.vanilla.parents[1] / 'assets/minecraft/lang' / (loc + '.json')).read_text())
                     for loc in ('en_us', 'pt_br')}
    def label(row, locale):
        direction = 'Empacotar' if locale == 'pt_br' else 'Package'
        if row['name'].startswith('ass.unpackage'): direction = 'Desempacotar' if locale == 'pt_br' else 'Unpackage'
        if row['name'].startswith(('ass.packagehbmfluid.', 'ass.unpackagehbmfluid.')):
            fluid = row['name'].split('hbmfluid.', 1)[1]
            return direction + ': ' + translations[locale].get('hbmfluid.' + fluid, fluid.title())
        outputs = row.get('outputs', row.get('outputItems', []))
        if outputs:
            # A weighted pick (`ChanceOutputMulti`) is named by its first item.
            ident, count = outputs[0]['multi'][0][:2] if isinstance(outputs[0], dict) else outputs[0]; title = names.get(ident, {}).get(locale)
            if title is None:
                path = ident.split(':', 1)[1]
                title = vanilla_names[locale].get('item.minecraft.' + path, vanilla_names[locale].get('block.minecraft.' + path, path.replace('_', ' ').title()))
            return title + (f' ×{count}' if count > 1 else '')
        ident, amount = row['outputFluids'][0]
        return translations[locale].get('hbmfluid.' + ident, ident.title()) + f' · {amount} mB'
    counts = {}
    for machine, cls in [('assembly', 'AssemblyMachineRecipes'), ('chemical_plant', 'ChemicalPlantRecipes'), ('centrifuge', 'CentrifugeRecipes'), ('rockmill', 'RockMillRecipes')]:
        path = Path('src/main/java/com/hbm/inventory/recipes') / (cls + '.java')
        text = clean((source / path).read_text())
        if machine == 'rockmill':
            # Forge registers Blocks.sand with wildcard metadata: the rock mill's `sand` takes red sand too.
            resolver.ore['sand'].add('minecraft:red_sand')
        controls = ranges(text); recipes = []; skipped = []
        pattern = r'\brecipes\.put' if machine == 'centrifuge' else r'\bthis\.register'
        start = text.index('public void registerDefaults()')
        next_method = re.search(r'\n\t(?:public|protected|private)\s', text[start + 1:])
        end = start + 1 + next_method.start() if next_method else len(text)
        source_calls = [(pos, args) for pos, args, _ in calls(text, pattern) if start < pos < end]
        defaults = text[start:end]
        expanded_calls = []
        for pos, args in source_calls:
            if machine == 'assembly' and re.search(r'new GenericRecipe\("ass\.(?:un)?package"\s*\+\s*type.getUnlocalizedName\(\)\)', args):
                for fluid in resolver.fluids.values():
                    if fluid['metadata'] == 0 or fluid['no_container']: continue
                    expanded = re.sub(r'"(ass\.(?:un)?package)"\s*\+\s*type.getUnlocalizedName\(\)',
                                      lambda match: json.dumps(match[1] + 'hbmfluid.' + fluid['id']), args)
                    expanded = expanded.replace('type.getID()', str(fluid['metadata']))
                    expanded = re.sub(r'\btype\b', 'Fluids.' + fluid['source_variable'], expanded)
                    expanded_calls.append((pos, expanded, True))
            else: expanded_calls.append((pos, args, False))
        for pos, args, expanded in expanded_calls:
            origin = str(path) + ':' + str(text[:pos].count('\n') + 1)
            if machine == 'rockmill':
                # RockMillRecipes.registerDefaults' local ints (`int duraShort = 100;`) stand for their values.
                for var, value in re.findall(r'\bint\s+(\w+)\s*=\s*(\d+)\s*;', defaults):
                    args = re.sub(r'\b' + var + r'\b', value, args)
            name_match = re.search(r'new GenericRecipe\("([^"]+)"\)', args)
            name = name_match[1] if name_match else 'source_line_' + origin.rsplit(':', 1)[1]
            try:
                for start, end, control, condition in controls:
                    if start < pos < end and not (control == 'if' and condition in ('!GeneralConfig.enable528', 'no528')) and not (expanded and control == 'for' and condition == 'int i = 1; i < order.length; ++i'):
                        raise Unsupported('dynamic/control-dependent ' + control + '(' + condition + ')')
                row = {'name': name, 'source': origin}
                if machine == 'centrifuge':
                    arguments = split(args)
                    if len(arguments) != 2: raise Unsupported('unexpected recipes.put arguments')
                    representative, options = resolver.input(arguments[0])
                    row['input'] = {'ids': options, 'count': representative[1]}
                    array = re.fullmatch(r'new ItemStack\[\]\s*\{(.*)\}', arguments[1], re.S)
                    if not array: raise Unsupported('dynamic centrifuge output array')
                    row['outputs'] = [list(resolver.item(x)) for x in split(array[1]) if x != 'null']
                else:
                    if not name_match: raise Unsupported('dynamic recipe name')
                    methods = {method[1:]: split(values) for _, values, method in calls(args, r'\.\w+')}
                    allowed = {'setup', 'setupNamed', 'inputItems', 'inputFluids', 'outputItems', 'outputFluids', 'setPools', 'setPools528', 'setGroup', 'setIcon', 'setIconToFirstIngredient', 'inputItemsEx', 'inputFluidsEx', 'setNameWrapper'}
                    # Nested stack/frame methods are handled by the resolver;
                    # reject unknown top-level chain mutation by its boundary.
                    for match in re.finditer(r'\)\s*\.([A-Za-z]+)\(', args):
                        if match[1] not in allowed: raise Unsupported('unsupported recipe method ' + match[1])
                    setup = methods.get('setup', methods.get('setupNamed'))
                    if setup is None or len(setup) != 2: raise Unsupported('missing literal setup')
                    row.update(duration=integer(setup[0]), power=integer(setup[1]))
                    inputs = [resolver.input(x) for x in methods.get('inputItems', [])]
                    row['inputItems'] = [{'ids': options, 'count': representative[1]} for representative, options in inputs]
                    row['outputItems'] = [output_item(resolver, x) for x in methods.get('outputItems', [])]
                    # `setGroup(group, INSTANCE)`: an auto switch group, a literal or a local String of registerDefaults.
                    group = methods.get('setGroup') if machine == 'rockmill' else None
                    if group:
                        literal = re.fullmatch(r'\s*"([^"]+)"\s*', group[0])
                        local = re.search(r'String\s+' + re.escape(group[0].strip()) + r'\s*=\s*"([^"]+)"', defaults)
                        if not (literal or local): raise Unsupported('dynamic auto switch group ' + group[0])
                        row['autoSwitchGroup'] = (literal or local)[1]
                    for method in ('inputFluids', 'outputFluids'): row[method] = [resolver.fluid(x) for x in methods.get(method, [])]
                    row['pools'] = [string(x) for x in methods.get('setPools', [])]
                if not row.get('outputs', row.get('outputItems', [])) and not row.get('outputFluids'):
                    raise Unsupported('recipe has no resolved output')
                row['label'] = label(row, 'pt_br'); row['label_en'] = label(row, 'en_us')
                if expanded: row['expansion'] = 'Fluids.getInNiceOrder() excluding NONE/NOCON'
                recipes.append(row)
            except (Unsupported, SyntaxError, ValueError) as error:
                skipped.append({'name': name, 'source': origin, 'reason': str(error), 'expression': args})
        body = {'schema': 1, 'machine': machine, 'profile': PROFILE,
                'units': {'duration': 'ticks', 'power': 'HE/t', 'fluids': 'mB'},
                'ingredient_policy': 'first ntm id, then lexical vanilla; alternatives are authoritative OreDict choices',
                'coverage': {'emitted': len(recipes), 'scanned_source_calls': len(source_calls), 'skipped_source_calls': len(skipped),
                             'expanded_fluid_recipes': sum('expansion' in row for row in recipes),
                             'note': 'Skipped calls are not expanded loop instances; 2 fluid-pack source calls expand explicitly. External IMC/config JSON overrides excluded'},
                'recipes': recipes, 'skipped': skipped}
        target = output / 'machine-recipes' / (machine + '.json')
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(body, ensure_ascii=False, indent=2) + '\n')
        counts[machine] = body['coverage']
    import shredder_recipes
    counts['shredder'] = shredder_recipes.export(source, output, rows)['coverage']
    import blast_furnace_recipes
    counts['blast_furnace'] = blast_furnace_recipes.export(source, output, rows)['coverage']
    import foundry_recipes
    counts['foundry'] = foundry_recipes.export(source, output, rows)['coverage']
    import anvil_recipes
    counts['anvil'] = anvil_recipes.export(source, output, rows)['coverage']
    import press_recipes
    counts['press'] = press_recipes.export(source, output, rows)['coverage']
    import purex_recipes
    counts['purex'] = purex_recipes.export(source, output, rows)['coverage']
    import bedrock_recipes
    counts['bedrock'] = bedrock_recipes.export(source, output, rows)['coverage']
    import crystallizer_recipes
    counts['crystallizer'] = crystallizer_recipes.export(source, output, rows)['coverage']
    import pyrooven_recipes, electrolyser_recipes, arc_furnace_recipes
    for module, name in [(pyrooven_recipes, 'pyrooven'), (electrolyser_recipes, 'electrolyser'), (arc_furnace_recipes, 'arc_furnace')]:
        counts[name] = module.export(source, output, rows)['coverage']
    import petroleum_recipes, petroleum_refining, petroleum_processing
    for module in [petroleum_recipes, petroleum_refining, petroleum_processing]:
        counts.update({name: data['coverage'] for name,data in module.export(source,output,rows).items()})
    import oil_extraction
    counts['oil_extraction'] = oil_extraction.export(source,output,rows)['coverage']
    import petroleum_auxiliary
    counts.update({name: data['coverage'] for name,data in petroleum_auxiliary.export(source,output,rows).items()})
    import heaters
    counts['heaters'] = heaters.export(source,output,rows)['coverage']
    import steam
    counts['steam'] = steam.export(source,output,rows)['coverage']
    import combustion
    counts['combustion'] = combustion.export(source,output,rows)['coverage']
    import solar_stirling
    counts['solar_stirling'] = solar_stirling.export(source,output,rows)['coverage']
    import radioisotopes
    counts['radioisotopes'] = radioisotopes.export(source,output,rows)['coverage']
    import reactors_simple
    counts['reactors_simple'] = reactors_simple.export(source,output,rows)['coverage']
    import reactors_rbmk
    counts['reactors_rbmk'] = reactors_rbmk.export(source,output,rows)['coverage']
    import reactors_pwr
    counts['reactors_pwr'] = reactors_pwr.export(source,output,rows)['coverage']
    import reactors_watz
    counts['reactors_watz'] = reactors_watz.export(source,output,rows)['coverage']
    import energy_storage
    counts['energy_storage'] = energy_storage.export(source,output,rows)['coverage']
    print(json.dumps({'machine_recipes': counts}, ensure_ascii=False))
    return counts


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

"""RBMK T00: executable Java evidence and verbatim executable source, no runtime."""
import hashlib
import json
import re
from pathlib import Path
from bedrock_processing_common import BASE, write
from bulk_recipes import calls, clean, split
from rbmk_catalog import item_variables

ORACLE = Path(__file__).resolve().parents[1] / 'rbmk-oracle'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_oracle(source):
    data = json.loads((ORACLE / 'fixtures/java.json').read_text())
    for relative, expected in data['provenance']['sources_sha256'].items():
        if digest(source / relative) != expected:
            raise ValueError('RBMK Java source drift; regenerate oracle: ' + relative)
    for relative, expected in data['provenance']['harness_sha256'].items():
        if digest(ORACLE / relative) != expected:
            raise ValueError('RBMK harness drift; regenerate oracle: ' + relative)
    return data


def method_body(text, name):
    match = re.search(r'public\s+(?:static\s+)?\w+\s+' + name + r'\([^)]*\)\s*\{', text)
    if not match:
        raise ValueError('method not found: ' + name)
    start = match.end()
    depth = 1
    for end in range(start, len(text)):
        depth += (text[end] == '{') - (text[end] == '}')
        if not depth:
            return text[start:end].strip()
    raise ValueError('unbalanced method: ' + name)


def expressions(body):
    # Enum comments are deliberately excluded. These returns are compiled by the oracle.
    return {m[1]: ' '.join(m[2].split()) for m in
            re.finditer(r'case\s+(\w+)\s*:\s*return\s+([^;]+);', body)}


def condition_ranges(text):
    """Lexical if/else containment; unknown guards remain explicit."""
    def end_block(opening, left, right):
        depth, end = 1, opening + 1
        while depth:
            depth += (text[end] == left) - (text[end] == right)
            end += 1
        return end
    result = []
    # Hide string contents while retaining offsets (recipe patterns contain braces).
    masked = re.sub(r'"(?:\\.|[^"\\])*"', lambda m: ' ' * len(m[0]), text)
    original, text = text, masked
    for match in re.finditer(r'\bif\s*\(', text):
        end = end_block(match.end() - 1, '(', ')')
        condition = ' '.join(original[match.end():end - 1].split())
        opening = end
        while opening < len(text) and text[opening].isspace(): opening += 1
        if text[opening:opening + 1] != '{': continue
        closing = end_block(opening, '{', '}')
        result.append((opening, closing, condition, True))
        other = re.match(r'\s*else\s*\{', text[closing:])
        if other:
            lo = closing + other.end() - 1
            result.append((lo, end_block(lo, '{', '}'), condition, False))
    return result


def recipe_manifest(source):
    result = []
    templates = []
    rod_source = BASE + 'crafting/RodRecipes.java'
    rod_text = clean((source / rod_source).read_text())
    helper_ranges = []
    for match in re.finditer(r'public static void addRBMKRod\(([^)]+)\)\s*\{', rod_text):
        params = [dict(zip(('type', 'name'), p.strip().split())) for p in split(match[1])]
        opening = match.end() - 1
        depth, end = 1, opening + 1
        while depth:
            depth += (rod_text[end] == '{') - (rod_text[end] == '}')
            end += 1
        helper_ranges.append((opening, end, {
            'id': 'com.hbm.crafting.RodRecipes#addRBMKRod(' + ','.join(p['type'] for p in params) + ')',
            'source': rod_source, 'declaration_line': rod_text[:match.start()].count('\n') + 1,
            'parameters': params}))
    if len(helper_ranges) != 2:
        raise ValueError('RBMK helper overload drift')
    item_text = clean((source / (BASE + 'items/ModItems.java')).read_text())
    dict_text = clean((source / (BASE + 'inventory/OreDictManager.java')).read_text())
    files = ('main/CraftingManager.java', 'crafting/RodRecipes.java',
             'inventory/recipes/AssemblyMachineRecipes.java', 'inventory/recipes/anvil/AnvilRecipes.java')
    pattern = r'\b(?:addRecipeAuto|addShapelessAuto|addRBMKRod|this\.register|constructionRecipes\.add)'
    for filename in files:
        text = clean((source / (BASE + filename)).read_text())
        ranges = condition_ranges(text)
        for pos, args, callee in calls(text, pattern):
            if not re.search(r'\b(?:rbmk_|pribris|deco_rbmk)\w*', args): continue
            scope = next((h for lo, hi, h in helper_ranges if filename == 'crafting/RodRecipes.java' and lo < pos < hi), None)
            if scope is not None:
                templates.append({**scope,
                    'registration_call': {'source_line': text[:pos].count('\n') + 1,
                                          'callee': callee, 'arguments_java': ' '.join(args.split())},
                    'evidence': 'parameterized helper body; not a concrete registration'})
                continue
            guards = [{'expression': c, 'branch': b} for lo, hi, c, b in ranges if lo < pos < hi]
            profile = True
            for guard in guards:
                c = guard['expression'].replace(' ', '')
                if c in ('GeneralConfig.enable528', '!GeneralConfig.enable528'):
                    profile &= c.startswith('!') == guard['branch']
                else:
                    profile = None
                    break
            row = {'source': BASE + filename, 'source_line': text[:pos].count('\n') + 1,
                           'callee': callee, 'arguments_java': ' '.join(args.split()), 'guards': guards,
                           'enabled_default_profile': profile, 'profile': 'enable528=false; external mods absent',
                           'kind': 'direct_registration',
                           'evidence': 'concrete lexical call site, not executed crafting'}
            if callee == 'addRBMKRod':
                arguments = split(args)
                first = arguments[0].strip()
                item = re.fullmatch(r'ModItems\.(\w+)', first)
                if item and re.search(r'public\s+static\s+Item\s+' + item[1] + r'\s*;', item_text):
                    first_type = 'Item'
                elif re.fullmatch(r'\w+', first) and re.search(r'public\s+static\s+final\s+DictFrame\s+' + first + r'\s*=', dict_text):
                    first_type = 'DictFrame'
                else:
                    raise ValueError('unresolved RBMK overload argument: ' + first)
                helper = next(h for _, _, h in helper_ranges if h['parameters'][0]['type'] == first_type)
                if len(arguments) != len(helper['parameters']):
                    raise ValueError('RBMK helper arity drift')
                row.update(kind='helper_invocation', template_id=helper['id'],
                           bindings={p['name']: a.strip() for p, a in zip(helper['parameters'], arguments)})
            result.append(row)
    return {'registrations': result, 'templates': templates,
            'counts': {'lexical_call_sites': len(result) + len(templates),
                       'helper_templates': len(templates), 'concrete_registration_sites': len(result),
                       'helper_invocations': sum(r['kind'] == 'helper_invocation' for r in result),
                       'direct_registration_sites': sum(r['kind'] == 'direct_registration' for r in result)}}


def rod_table(source, rod_vars, pellet_vars):
    byvar = {r['variable']: r for r in load_oracle(source)['rods']}
    rows = []
    for var in rod_vars:
        rod = byvar[var]
        row = {k: v for k, v in rod.items() if k not in (
            'fresh_stack', 'used_stack', 'fresh_tooltip_keys', 'used_tooltip_keys',
            'fresh_function_description', 'fresh_durability', 'burn_heat_trace')}
        if row['pellet'] is not None and row['pellet'] not in pellet_vars:
            raise ValueError('unknown pellet ' + row['pellet'])
        if row['pellet']: row['pellet_item'] = 'ntm:' + row['pellet']
        rows.append(row)
    return rows


def class_manifest(source, oracle):
    java = source / BASE
    files = set(java.rglob('*RBMK*.java')) | set((java / 'tileentity/machine/rbmk').glob('*.java'))
    files |= {java / p for p in (
        'render/tileentity/RenderCraneConsole.java', 'handler/neutron/NeutronHandler.java',
        'handler/neutron/NeutronStream.java', 'handler/neutron/NeutronNode.java',
        'handler/neutron/NeutronNodeWorld.java', 'tileentity/network/RTTYSystem.java',
        'items/tool/ItemRTTYDesignator.java', 'tileentity/machine/TileEntityWasteDrum.java',
        'tileentity/machine/TileEntitySILEX.java', 'inventory/recipes/FuelPoolRecipes.java',
        'inventory/recipes/SILEXRecipes.java', 'inventory/recipes/OutgasserRecipes.java')}
    result = []
    for path in sorted(files):
        text = clean(path.read_text())
        relative = str(path.relative_to(java))
        declaration = re.search(r'public\s+(?:(?:abstract|final)\s+)?(?:class|interface)\s+[^\{]+', text)
        result.append({'source': str(path.relative_to(source)), 'sha256': digest(path),
                       'declaration': ' '.join(declaration[0].split()) if declaration else None,
                       'oracle_whole_class': relative in oracle['provenance']['whole_classes'],
                       'runtime': 'absent RBMK; catalog/partial presentation only, see rbmk-plano'})
    return result


def export(source, output, rows):
    oracle = load_oracle(source)
    rod_vars, pellet_vars = item_variables(source)
    rods = rod_table(source, rod_vars, set(pellet_vars))
    known = {r['id'] for r in rows}
    for r in rods:
        for ident in (r['item'], r.get('pellet_item')):
            if ident and ident not in known: raise ValueError('Missing RBMK id ' + ident)
    text = clean((source / (BASE + 'items/machine/ItemRBMKRod.java')).read_text())
    formulas = expressions(method_body(text, 'reactivityFunc'))
    depletions = expressions(method_body(text, 'reactivityModByEnrichment'))
    if (len(formulas), len(depletions)) != (9, 5): raise ValueError('RBMK switch drift')
    dials = [{k: d[k] for k in ('key', 'game_rule', 'default', 'default_java_type', 'cached_effective')}
             for d in oracle['dials']]
    recipes = oracle['recipes']
    crafting = recipe_manifest(source)
    aliases = recipes['silex_aliases']
    return write(output, 'reactors_rbmk', {
        'schema': 2, 'recipes': [], 'skipped': [], 'rods': rods,
        'java_manifest': class_manifest(source, oracle),
        'oracle': {'fixture': 'tools/rbmk-oracle/fixtures/java.json',
                   'fixture_sha256': digest(ORACLE / 'fixtures/java.json'), **oracle['provenance']},
        'burn_functions': list(formulas), 'burn_formulas': formulas,
        'depletion_functions': list(dict.fromkeys(p['function'] for p in oracle['functions']['depletion'])),
        'enrichment_formulas': depletions,
        'formula_inputs': {'flux': 'in * reactivityModByEnrichment(enrichment)',
                           'reactivity': 'rod.reactivity', 'selfRate': 'rod.selfRate',
                           'notation': 'Verbatim Java return expressions; descriptive, not a runtime evaluator'},
        'executable_methods': {name: method_body(text, name) for name in (
            'reactivityFunc', 'reactivityModByEnrichment', 'burn', 'xenonGenFunc', 'xenonBurnFunc',
            'updateHeat', 'provideHeat', 'getEnrichment', 'getDouble', 'setDouble')},
        'rod_defaults': oracle['defaults']['rod'],
        'rod_nbt': {'yield': 'double, remaining potential influx', 'xenon': 'double [0,100] after burn clamp',
                    'core': 'double', 'hull': 'double'},
        'rod_derived': {'enrichment': 'getYield(stack) / rod.yield; no clamp in this getter',
                        'poison_level': 'getPoison(stack) / 100D'},
        'nbt_initialization': {k: v for k, v in oracle['defaults'].items() if k.startswith('nbt_')},
        'pellet_state': {'metadata': 'rectify=abs(meta)%10; grade=meta%5; poison when meta>=5',
                         'ported_as': 'one ID per type; 0..9 custom_data codec deferred to T03/T14',
                         'types': oracle['pellets']},
        'rod_to_pellet': 'RBMKFuelCraftingHandler: cold core/hull<50, enrichment<=0.99; 8 pellets + container casing; meta=4-clamp(ceil(e*5-1),0,4), +5 if poison>=0.5; runtime pending',
        'dials': dials, 'dial_special_cases': oracle['dial_special_cases'],
        'silex_recipes': recipes['silex_registered'], 'silex_tiny_lookup': recipes['silex_tiny_lookup'],
        'silex_aliases': aliases,
        'silex_missing_pellet_states': recipes['silex_missing_pellet_states'],
        'outgasser_recipes': recipes['outgasser_defaults'], 'crafting_manifest': crafting['registrations'],
        'crafting_templates': crafting['templates'], 'crafting_manifest_counts': crafting['counts'],
        'runtime_gaps': ['All 33 RBMK machines remain Inert. Rod simulation, dynamic tooltip/hazard, pellet state, disassembly, SILEX/fuel pool and meltdown are later tickets. T00 only fixes data and supplies executable Java fixtures.',
                         'D8: leverStop blocked pending user licensing decision; no substitute or asset imported.'],
        'coverage': {'rods': len(rods), 'pellet_rods': sum(r['pellet'] is not None for r in rods),
                     'dials': len(dials), 'burn_functions': len(formulas), 'depletion_functions': len(depletions),
                     'silex_registered': len(recipes['silex_registered']),
                     'silex_item_translations': len(aliases['item_translation']),
                     'silex_dict_translations': len(aliases['dict_translation']),
                     'silex_all_ingredients': aliases['all_ingredients_count'],
                     'silex_rbmk_pellet_states': sum(r['input'].get('stack', {}).get('item', '').startswith('rbmk_pellet_') for r in recipes['silex_registered']),
                     'outgasser_defaults': len(recipes['outgasser_defaults'])}})

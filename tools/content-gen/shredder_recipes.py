#!/usr/bin/env python3
"""Translate ShredderRecipes defaults and its ordered OreDict post pass."""
import json
import re
from pathlib import Path
from collections import defaultdict
from bulk import enums
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import MachineResolver

RECIPE_PATH = 'src/main/java/com/hbm/inventory/recipes/ShredderRecipes.java'
FORGE_SOURCE = 'https://github.com/MinecraftForge/MinecraftForge/blob/1.7.10/src/main/java/net/minecraftforge/oredict/OreDictionary.java'


class ShredderResolver(MachineResolver):
    def vanilla_item(self, expr, meta=0):
        var = expr.split('.')[-1]
        if var == 'quartz_ore': return super().vanilla_item('Blocks.nether_quartz_ore', meta)
        variants = {'quartz_block': ['quartz_block', 'chiseled_quartz_block', 'quartz_pillar'],
                    'stone_slab': ['smooth_stone_slab', 'sandstone_slab', 'petrified_oak_slab', 'cobblestone_slab', 'brick_slab', 'stone_brick_slab', 'nether_brick_slab', 'quartz_slab'],
                    'skull': ['skeleton_skull', 'wither_skeleton_skull', 'zombie_head', 'player_head', 'creeper_head']}
        if var in variants:
            try: return super().vanilla_item('Items.' + variants[var][meta])
            except IndexError: raise Unsupported(f'unsupported vanilla metadata {expr}:{meta}')
        return super().vanilla_item(expr, meta)

    def values(self, expr):
        """Expand only explicit source enum/wildcard lists, with direct port IDs."""
        match = re.fullmatch(r'(?:DictFrame\.)?fromAll\((\w+),\s*(\w+)\.class\)', expr.strip())
        if match:
            return [row['id'] for row in sorted(self.byvar[match[1]], key=lambda row: row.get('metadata', 0)) if row.get('source_enum', '').startswith(match[2] + '.')]
        match = re.fullmatch(r'new (?:ItemStack|ComparableStack)\(\s*Mod(?:Items|Blocks)\.(\w+)\s*,\s*1\s*,\s*OreDictionary.WILDCARD_VALUE\s*\)', expr.strip())
        if match: return [row['id'] for row in sorted(self.byvar[match[1]], key=lambda row: row.get('metadata', 0))]
        return [self.item(expr, True)[0]]


def dictionary(resolver):
    """Ordered registrations; method spellings are not themselves ore prefixes."""
    ores = {}; sources = defaultdict(list)
    def add(key, ids, source):
        if key not in ores: ores[key] = []
        for ident in ids:
            if ident not in ores[key]: ores[key].append(ident)
        if source not in sources[key]: sources[key].append(source)
    # Forge 1.7.10 initVanillaEntries, mapped to the existing 26.3 identities.
    # Modern deepslate/copper variants have no legacy registration here.
    vanilla = {
        'oreGold': 'gold_ore', 'oreIron': 'iron_ore', 'oreLapis': 'lapis_ore', 'oreDiamond': 'diamond_ore',
        'oreRedstone': 'redstone_ore', 'oreEmerald': 'emerald_ore', 'oreQuartz': 'nether_quartz_ore', 'oreCoal': 'coal_ore',
        'blockGold': 'gold_block', 'blockIron': 'iron_block', 'blockLapis': 'lapis_block', 'blockDiamond': 'diamond_block',
        'blockRedstone': 'redstone_block', 'blockEmerald': 'emerald_block', 'blockQuartz': 'quartz_block', 'blockCoal': 'coal_block',
        'ingotIron': 'iron_ingot', 'ingotGold': 'gold_ingot', 'ingotBrick': 'brick', 'ingotBrickNether': 'nether_brick',
        'gemDiamond': 'diamond', 'gemEmerald': 'emerald', 'gemQuartz': 'quartz', 'gemLapis': 'lapis_lazuli',
        'dustRedstone': 'redstone', 'dustGlowstone': 'glowstone_dust'}
    for key, var in vanilla.items(): add(key, [resolver.vanilla_item('Items.' + var)], FORGE_SOURCE + ':71-105')
    for key, variants in [('logWood', [x + '_log' for x in ('oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak')]),
                          ('plankWood', [x + '_planks' for x in ('oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak')]),
                          ('treeSapling', [x + '_sapling' for x in ('oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak')])]:
        add(key, [resolver.vanilla_item('Items.' + var) for var in variants], FORGE_SOURCE + ':57-70')
    shape = {'ingot': 'ingot', 'nugget': 'nugget', 'dustSmall': 'dustTiny', 'dust': 'dust', 'plate': 'plate', 'plateCast': 'plateTriple', 'gem': 'gem', 'crystal': 'crystal', 'block': 'block', 'ore': 'ore', 'oreNether': 'oreNether'}
    text = resolver.odtext
    for match in re.finditer(r'\b([A-Z][A-Z0-9_]*)\s*\.\s*[a-z]\w*\([^;]*;', text, re.S):
        if match[1] not in resolver.frames: continue
        for _, args, method in calls(match[0], r'\.\w+'):
            if method[1:] not in shape: continue
            for expression in split(args):
                try: values = resolver.values(expression)
                except Unsupported: continue  # Only registered catalog items belong in this snapshot.
                for material in resolver.frames[match[1]]:
                    add(shape[method[1:]] + material, values, 'OreDictManager.java:' + str(text[:match.start()].count('\n') + 1))
    # Generated MaterialShapes present in the port, including df("...") frames.
    mats = clean((resolver.src / 'src/main/java/com/hbm/inventory/material/Mats.java').read_text())
    forms = {'ingot_raw': 'ingot', 'nugget_raw': 'nugget', 'plate_cast': 'plateTriple', 'plate_welded': 'plateSextuple',
             'wire_fine': 'wireFine', 'wire_dense': 'wireDense', 'bedrock_ore_fragment': 'bedrockorefragment', 'bolt': 'bolt', 'shell': 'shell', 'pipe': 'ntmpipe'}
    for row in resolver.rows:
        if row['source_variable'] not in forms or 'source_material' not in row: continue
        match = re.search(r'\b' + row['source_material'] + r'\s*=\s*make\w*\([^,]+,\s*(df\("[^"]+"\)|\w+)', mats)
        if not match: continue
        names = [match[1][4:-2]] if match[1].startswith('df(') else resolver.frames.get(match[1], [])
        for name in names: add(forms[row['source_variable']] + name, [row['id']], 'OreDictManager.java:543-569')
    # Ore-enriched metadata is registered in the following source enum loop.
    enum = enums(resolver.src / 'src/main/java/com/hbm')['EnumBedrockOre']
    for name, args in enum:
        material = json.loads(split(args)[0])
        row = resolver.byenum.get(('ore_enriched', 'EnumBedrockOre.' + name))
        if row: add('ore' + material, [row['id']], 'OreDictManager.java:572-574')
    for pos, args, _ in calls(text, r'OreDictionary\.registerOre'):
        parts = split(args)
        if len(parts) != 2: continue
        key = resolver.constants.get(parts[0])
        if parts[0].startswith('"') and parts[0].endswith('"'): key = json.loads(parts[0])
        if not key: continue
        try: values = resolver.values(parts[1])
        except Unsupported: continue
        add(key, values, 'OreDictManager.java:' + str(text[:pos].count('\n') + 1))
    return ores, sources


def export(source, output, rows):
    resolver = ShredderResolver(source, rows); ores, ore_sources = dictionary(resolver)
    text = clean((source / RECIPE_PATH).read_text()); start = text.index('public void registerDefaults()'); end = text.index('public static ItemStack getDustByName', start)
    enum_values = enums(source / 'src/main/java/com/hbm'); skipped = []; no_output = []; collisions = []; exact = {}; wildcard = {}
    def put(ident, result, origin, phase, **extra):
        row = {'name': 'shredder.' + ident.replace(':', '.'), 'input': {'ids': [ident], 'count': 1}, 'outputs': [list(result)], 'source': origin, 'phase': phase, **extra}
        table = wildcard if extra.get('wildcard_fallback') else exact
        if ident in table:
            collisions.append({'input': ident, 'kept': table[ident], 'ignored': row, 'reason': 'setRecipe inserts only when containsKey is false'})
        else: table[ident] = row
    # Explicit calls plus finite source loops whose domains we can prove.
    loops = []
    for match in re.finditer(r'for\s*\(([^)]*(?:\([^)]*\)[^)]*)?)\)\s*\{', text[start:end]):
        begin = start + match.end(); depth = 1; stop = begin
        while depth and stop < end:
            if text[stop] == '{': depth += 1
            elif text[stop] == '}': depth -= 1
            stop += 1
        loops.append((begin, stop, match[1]))
    calls_seen = 0
    for pos, args, _ in calls(text, r'ShredderRecipes\.setRecipe'):
        if not start < pos < end: continue
        calls_seen += 1; origin = RECIPE_PATH + ':' + str(text[:pos].count('\n') + 1)
        expressions = [args]
        context = next((condition for begin, stop, condition in loops if begin <= pos < stop), None)
        prefix = text[max(start, text.rfind('\n', start, pos)):pos]
        try:
            if context:
                if context == 'EnumBedrockOre ore : EnumBedrockOre.values()':
                    expressions = [re.sub(r'\bi\b', str(i), args) for i in range(len(enum_values['EnumBedrockOre']))]
                elif context == 'int i = 0; i < 16; i++': expressions = [re.sub(r'\bi\b', str(i), args) for i in range(16)]
                else: raise Unsupported('unsupported source loop: ' + context)
            elif 'for(' in prefix or 'for (' in prefix:
                match = re.search(r'for\(ItemStack (\w+) : (logs|planks|saplings|lapis)\)', prefix)
                if match:
                    key = {'logs': 'logWood', 'planks': 'plankWood', 'saplings': 'treeSapling', 'lapis': 'dustLapis'}[match[2]]
                    result = resolver.item(split(args)[1])
                    for ident in ores.get(key, []): put(ident, result, origin, 'defaults', ore_key=key)
                    continue
                if 'i < 5' in prefix: expressions = [re.sub(r'\bi\b', str(i), args) for i in range(5)]
                else: raise Unsupported('unsupported single-line loop')
            if 'gcMoonBlock' in args or 'arMoonTurf' in args: raise Unsupported('external mod compatibility disabled')
            for expression in expressions:
                parts = split(expression)
                if len(parts) != 2: raise Unsupported('unexpected setRecipe arguments')
                # The one compat ternary has a fixed, no-GT6 default.
                result = resolver.item(parts[1].replace('Compat.isModLoaded(Compat.MOD_GT6) ? 4 : 5', '5'))
                if 'OreDictionary.WILDCARD_VALUE' in parts[0]:
                    values = resolver.values(parts[0])
                    if not values: raise Unsupported('wildcard source item absent from catalog')
                    for ident in values: put(ident, result, origin, 'defaults', wildcard_fallback=True)
                else: put(resolver.item(parts[0])[0], result, origin, 'defaults')
        except Unsupported as error:
            skipped.append({'source': origin, 'phase': 'defaults', 'reason': str(error), 'expression': args})
    # registerPost follows OreDictionary insertion order and cannot overwrite defaults.
    for key, matches in ores.items():
        if 'Any' in key: continue
        result = None
        if key.startswith('dustTiny') and len(key) >= 8: result = ('ntm:dust_tiny', 1)
        elif key.startswith('dust') and len(key) >= 4: result = ('ntm:dust', 1)
        else:
            prefix = next((p for p in ('ingot', 'plate', 'gem', 'crystal', 'ore', 'block') if key.startswith(p) and len(key) > len(p)), None)
            if prefix is None: continue
            material = key[len(prefix):]; candidates = ores.get('dust' + material, [])
            # Default preferredOutputMod=[hbm]: first NTM output, otherwise first registration.
            dust = next((ident for ident in candidates if ident.startswith('ntm:')), candidates[0] if candidates else None)
            if dust is None or dust == 'ntm:scrap':
                # Java getDustByName returns scrap and putIfValid never runs: the source emits no recipe.
                no_output.append({'phase': 'post', 'ore_key': key, 'source': RECIPE_PATH + ':38-95', 'reason': 'getDustByName returns scrap: no dust<Material> OreDict entry, so the source emits no recipe', 'inputs': matches})
                continue
            count = 2 if prefix == 'ore' else 9 if prefix == 'block' and (ores.get('ingot' + material) or ores.get('gem' + material)) else 4 if prefix == 'block' else 1
            result = (dust, count)
        for ident in matches: put(ident, result, RECIPE_PATH + ':38-95', 'post', ore_key=key, ore_sources=ore_sources[key])
    for ident, row in wildcard.items():
        if ident not in exact: exact[ident] = row
    names = {row['id']: row['names'] for row in rows}
    vanilla = {loc: json.loads((resolver.vanilla.parents[1] / 'assets/minecraft/lang' / (loc + '.json')).read_text()) for loc in ('pt_br', 'en_us')}
    def title(ident, loc):
        if ident in names: return names[ident][loc]
        path = ident.split(':')[1]
        return vanilla[loc].get('item.minecraft.' + path, vanilla[loc].get('block.minecraft.' + path, path.replace('_', ' ').title()))
    for ident, row in exact.items():
        result, count = row['outputs'][0]
        for locale, label in [('pt_br', 'label'), ('en_us', 'label_en')]:
            row[label] = title(ident, locale) + ' → ' + title(result, locale) + (f' ×{count}' if count > 1 else '')
    items = (source / 'src/main/java/com/hbm/items/ModItems.java').read_text(); blades = []
    for var in ('blades_steel', 'blades_titanium', 'blades_desh'):
        match = re.search(r'\b' + var + r'\s*=\s*new ItemBlades\((\d+)\)', items)
        if not match: raise ValueError('Missing blade declaration: ' + var)
        blades.append({'id': resolver.row(var), 'durability': int(match[1]), 'unbreakable': match[1] == '0', 'source': 'src/main/java/com/hbm/items/ModItems.java:' + str(items[:match.start()].count('\n') + 1)})
    body = {'schema': 1, 'machine': 'shredder', 'profile': {'external_mods': [], 'preferred_output_mod': ['ntm'], 'modern_vanilla_ore_variants': False},
            'precedence': 'registerDefaults then registerPost; first insertion per source key wins (setRecipe:417-420); exact metadata lookup precedes wildcard fallback (getShredderResult)',
            'fallback_output': ['ntm:scrap', 1], 'blades': blades, 'coverage': {'emitted': len(exact), 'explicit_calls': calls_seen, 'ore_keys': len(ores), 'skipped': len(skipped), 'no_output': len(no_output), 'collisions_kept_first': len(collisions)},
            'recipes': sorted(exact.values(), key=lambda row: row['input']['ids'][0]), 'skipped': skipped, 'no_output': no_output, 'collisions': collisions}
    target = output / 'machine-recipes/shredder.json'; target.parent.mkdir(parents=True, exist_ok=True)
    # This large table is generated for machine ingestion, including full
    # provenance/collision records; compact JSON avoids repeating indentation.
    target.write_text(json.dumps(body, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(json.dumps({'shredder': body['coverage']}))
    return body


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

#!/usr/bin/env python3
"""Ordered PressRecipes defaults, finite Mats WIRE expansion and cataloged ItemStamp table."""
import json
import re
from pathlib import Path
from anvil_recipes import snapshot
from blast_furnace_recipes import block_end
from bulk_recipes import Unsupported, calls, clean, split

RECIPE_SOURCE = 'src/main/java/com/hbm/inventory/recipes/PressRecipes.java'
STAMP_SOURCE = 'src/main/java/com/hbm/items/machine/ItemStamp.java'
ITEMS_SOURCE = 'src/main/java/com/hbm/items/ModItems.java'


def stamp_types(source):
    text = clean((source / STAMP_SOURCE).read_text())
    match = re.search(r'public static enum StampType\s*\{([^}]+)\}', text)
    if match is None:
        raise ValueError('Missing StampType enum in ' + STAMP_SOURCE)
    names = [entry.strip().rstrip(';').strip() for entry in match[1].split(',') if entry.strip()]
    if not names:
        raise ValueError('Empty StampType enum in ' + STAMP_SOURCE)
    return names


def extract_stamps(source, rows, known_types):
    by_var = {}
    for row in rows:
        if row['type'] == 'item':
            by_var.setdefault(row['source_variable'], []).append(row)
    raw = (source / ITEMS_SOURCE).read_text()
    text = clean(raw)
    stamps = []
    skipped = []
    pattern = re.compile(r'\b(\w+)\s*=\s*new\s+(ItemStamp\w*)\(([^)]*)\)')
    for match in pattern.finditer(text):
        var, cls, arg_str = match[1], match[2], match[3].strip()
        origin = ITEMS_SOURCE + ':' + str(text[:match.start()].count('\n') + 1)
        catalog_rows = by_var.get(var, [])
        if cls == 'ItemStamp':
            parts = split(arg_str)
            if len(parts) != 2 or not parts[0].isdigit() or not parts[1].startswith('StampType.'):
                raise ValueError(f'Unexpected ItemStamp constructor at {origin}: {match[0]}')
            durability = int(parts[0])
            type_upper = parts[1].split('.', 1)[1]
            if type_upper not in known_types:
                raise ValueError(f'Unknown StampType {type_upper} at {origin}')
            if len(catalog_rows) != 1:
                skipped.append({
                    'variable': 'ModItems.' + var,
                    'class': cls,
                    'type': type_upper.lower(),
                    'durability': durability,
                    'source': origin,
                    'reason': f'ModItems.{var} absent from catalog',
                })
                continue
            stamps.append({
                'item': catalog_rows[0]['id'],
                'type': type_upper.lower(),
                'durability': durability,
                'unbreakable': durability == 0,
                'source': origin,
            })
        elif cls == 'ItemStampBook':
            printing = [t.lower() for t in known_types if t.startswith('PRINTING')]
            if not catalog_rows:
                skipped.append({
                    'variable': 'ModItems.' + var,
                    'class': cls,
                    'types': printing,
                    'durability': 0,
                    'source': origin,
                    'reason': 'ModItems.stamp_book absent from catalog (setCreativeTab(null) secret item)',
                })
            else:
                for row in sorted(catalog_rows, key=lambda r: r.get('metadata', 0)):
                    meta = row.get('metadata', 0) % len(printing)
                    stamps.append({
                        'item': row['id'],
                        'type': printing[meta],
                        'durability': 0,
                        'unbreakable': True,
                        'source': origin,
                    })
        else:
            raise ValueError(f'Unsupported ItemStamp subclass {cls} at {origin}')
    catalog_stamp_ids = {
        row['id'] for row in rows
        if row.get('category') == 'stamp' or row.get('source_class') in ('ItemStamp', 'ItemStampBook')
    }
    extracted_ids = {entry['item'] for entry in stamps}
    if catalog_stamp_ids != extracted_ids:
        raise ValueError(f'Catalog stamp mismatch: {sorted(catalog_stamp_ids ^ extracted_ids)}')
    return stamps, skipped


def expanded_calls(text, materials, ores, shapes):
    match = re.search(r'public void registerDefaults\(\)\s*\{', text)
    if match is None:
        raise ValueError('Missing registerDefaults() in ' + RECIPE_SOURCE)
    start = match.end()
    end = block_end(text, start - 1) - 1
    direct_calls = [(pos, args) for pos, args, _ in calls(text, r'\bmakeRecipe') if start <= pos < end]
    loops = []
    for pos, header, _ in calls(text, r'\bfor'):
        if not start <= pos < end:
            continue
        body = text.find('(', pos) + len(header) + 2
        while text[body].isspace():
            body += 1
        if text[body] != '{':
            raise Unsupported('unbraced loop in PressRecipes: ' + header)
        loops.append((pos, body, block_end(text, body), header.strip()))

    events = [
        (pos, 'call', args)
        for pos, args in direct_calls
        if not any(loop[0] <= pos < loop[2] for loop in loops)
    ]
    events += [(loop[0], 'loop', loop) for loop in loops]

    stream = []
    for pos, kind, payload in sorted(events, key=lambda item: item[0]):
        if kind == 'call':
            stream.append((pos, payload, None))
            continue
        loop_pos, body_start, body_end, header = payload
        if header != 'NTMMaterial mat : Mats.orderedList':
            raise Unsupported('unsupported PressRecipes loop: ' + header)
        inner = [(cpos, cargs) for cpos, cargs in direct_calls if body_start <= cpos < body_end]
        body_text = text[body_start:body_end]
        cond = (
            'mat.autogen.contains(MaterialShapes.WIRE)' in body_text
            and 'OreDictionary.doesOreNameExist(MaterialShapes.INGOT.make(mat))' in body_text
        )
        if not cond or len(inner) != 1:
            raise Unsupported('unverified PressRecipes material loop body')
        call_pos, call_args = inner[0]
        ingot_prefix = shapes['INGOT']['prefixes'][0]
        for mat in materials:
            if 'WIRE' not in mat['autogen']:
                continue
            ore_key = ingot_prefix + mat['names'][0]
            if not ores.get(ore_key):
                continue
            expr = call_args.replace('MaterialShapes.INGOT.make(mat)', json.dumps(ore_key))
            expr = re.sub(r'\bmat\.id\b', str(mat['id']), expr)
            stream.append((
                call_pos,
                expr,
                {'material': mat['name'], 'material_id': mat['id'], 'ore': ore_key},
            ))
    return len(direct_calls), stream


def parse_recipe(expression, resolver, known_types):
    parts = split(expression)
    if len(parts) != 3:
        raise Unsupported('unexpected makeRecipe arity: ' + expression)
    if not parts[0].startswith('StampType.'):
        raise Unsupported('nonliteral StampType: ' + parts[0])
    stamp_upper = parts[0].split('.', 1)[1]
    if stamp_upper not in known_types:
        raise Unsupported('unknown StampType: ' + stamp_upper)
    missing = [
        'ModItems.' + var
        for var in dict.fromkeys(re.findall(r'ModItems\.(\w+)', expression))
        if not resolver.byvar.get(var)
    ]
    if missing:
        raise Unsupported('missing catalog identities: ' + ', '.join(missing))
    representative, ids = resolver.input(parts[1])
    out_id, out_count = resolver.item(parts[2])
    return {
        'stamp': stamp_upper.lower(),
        'in': {'ids': ids, 'count': representative[1]},
        'out': [out_id, out_count],
    }


def export(source, output, rows):
    resolver, materials, shapes, _ = snapshot(source, rows)
    known_types = stamp_types(source)
    stamps, skipped_stamps = extract_stamps(source, rows, known_types)
    text = clean((source / RECIPE_SOURCE).read_text())
    source_calls, stream = expanded_calls(text, materials, resolver.ordered_ore, shapes)

    names = {row['id']: row['names'] for row in rows}
    vanilla = {
        loc: json.loads((resolver.vanilla.parents[1] / 'assets/minecraft/lang' / (loc + '.json')).read_text())
        for loc in ('pt_br', 'en_us')
    }

    def title(ident, loc):
        if ident in names:
            return names[ident][loc]
        path = ident.split(':', 1)[1]
        return vanilla[loc].get(
            'item.minecraft.' + path,
            vanilla[loc].get('block.minecraft.' + path, path.replace('_', ' ').title()),
        )

    recipes = []
    skipped = []
    order = []
    by_stamp = {t.lower(): {'emitted': 0, 'skipped': 0} for t in known_types}

    for index, (pos, expression, expansion) in enumerate(stream):
        name = f'press.{index}'
        order.append(name)
        origin = RECIPE_SOURCE + ':' + str(text[:pos].count('\n') + 1)
        raw_parts = split(expression)
        stamp_hint = (
            raw_parts[0].split('.', 1)[1].lower()
            if raw_parts and raw_parts[0].startswith('StampType.')
            else 'unknown'
        )
        provenance = {
            'name': name,
            'source_index': index,
            'source': origin,
            **({'expansion': expansion} if expansion else {}),
        }
        try:
            parsed = parse_recipe(expression, resolver, known_types)
            out_id, out_count = parsed['out']
            suffix = f' ×{out_count}' if out_count > 1 else ''
            row = {
                **provenance,
                **parsed,
                'label': title(out_id, 'pt_br') + suffix,
                'label_en': title(out_id, 'en_us') + suffix,
            }
            recipes.append(row)
            by_stamp[parsed['stamp']]['emitted'] += 1
        except (Unsupported, ValueError, KeyError) as error:
            missing = [
                'ModItems.' + var
                for var in dict.fromkeys(re.findall(r'ModItems\.(\w+)', expression))
                if not resolver.byvar.get(var)
            ]
            skipped.append({
                **provenance,
                'stamp': stamp_hint,
                'reason': str(error),
                **({'missing_items': missing} if missing else {}),
                'expression': expression,
            })
            if stamp_hint in by_stamp:
                by_stamp[stamp_hint]['skipped'] += 1

    coverage = {
        'source_calls': source_calls,
        'expanded': len(order),
        'emitted': len(recipes),
        'skipped': len(skipped),
        'stamps': len(stamps),
        'skipped_stamps': len(skipped_stamps),
        'by_stamp': by_stamp,
    }
    body = {
        'schema': 1,
        'machine': 'press',
        'profile': {'external_mods': [], 'modern_vanilla_ore_variants': False},
        'coverage': coverage,
        'source_recipe_order': order,
        'stamps': stamps,
        'skipped_stamps': skipped_stamps,
        'recipes': recipes,
        'skipped': skipped,
        'notes': [
            'source_index preserves PressRecipes.registerDefaults() source order with Mats.orderedList WIRE expansion in iteration order.',
            'OreDict resolution uses the conservative Forge 1.7.10 snapshot (no modern vanilla copper_ingot alias; carbon maps to ingotCarbon -> ntm:ingot_graphite, gold maps to ingotGold -> minecraft:gold_ingot).',
            'Desh stamps have durability 0 (unbreakable in ItemStamp / TileEntityMachinePress).',
            'C357 and C44 stamps exist in ModItems.java (4 items), while PressRecipes.registerDefaults() defines 0 recipes for them in NTM 1.7.10.',
            'PRINTING1..PRINTING8 recipes resolve paper -> ntm:page_of___page1..8; their stamp provider ModItems.stamp_book (ItemStampBook, CreativeTab null) is absent from ids.json and recorded in skipped_stamps.',
        ],
    }
    target = output / 'machine-recipes/press.json'
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(body, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'press': {k: v for k, v in coverage.items() if k != 'by_stamp'}}))
    return body


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    export(root.parent / 'ntm-src', root / 'content', json.loads((root / 'content/ids.json').read_text())['entries'])

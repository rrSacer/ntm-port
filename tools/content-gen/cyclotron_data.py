#!/usr/bin/env python3
"""Recipes of the cyclotron (`machine_cyclotron`), `CyclotronRecipes.registerDefaults` (lines 28-93).

A recipe is a particle (`ComparableStack`), an ingredient (an `OreDictStack` of a literal name or `Frame.dust()`, or a
`ComparableStack`), the output item and the antimatter in mB (a literal or one of the `int xxA = n;` of its group). The
ingredient's ore name is `dust` + the first name of the frame (`DictFrame.dust`); its items are the catalog's `oredict.json`.
What the catalog does not have (an ore name with no item, an item that is not there) is listed in `skipped` with the reason, not
invented. `recipes` is a `HashMap` in Java, so a stack that two recipes of one particle take would be matched in hash order:
`collisions` lists those (there are none).
"""
import argparse
import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT.parent / 'ntm-src'
HBM = 'src/main/java/com/hbm/'
OUTPUT = ROOT / 'crates/ntm-core/data/machines/cyclotron.json'


def frames(source):
    text = (source / HBM / 'inventory/OreDictManager.java').read_text()
    out = {}
    for var, args in re.findall(r'DictFrame\s+(\w+)\s*=\s*new DictFrame\((.*?)\);', text):
        if '?' in args:
            args = args.split(':', 1)[1]
        out[var] = re.search(r'"([^"]+)"', args)[1]
    return out


def build(source=None):
    source = Path(source or os.environ.get('NTM_SRC', DEFAULT_SOURCE))
    entries = json.loads((ROOT / 'content/ids.json').read_text())['entries']
    ids_of = {}
    for e in entries:
        ids_of.setdefault(e.get('source_variable'), []).append(e['id'])
    stack = {e['id']: e.get('max_stack_size', 64) for e in entries}
    names = json.loads((ROOT / 'crates/ntm-core/data/oredict.json').read_text())['names']
    frame = frames(source)
    java = (source / HBM / 'inventory/recipes/CyclotronRecipes.java').read_text().split('\n')
    start = next(i for i, l in enumerate(java) if 'registerDefaults' in l)
    amounts, recipes, skipped = {}, [], []
    for i in range(start, len(java)):
        line = java[i]
        m = re.match(r'\s*int\s+(\w+)\s*=\s*(\d+);', line)
        if m:
            amounts[m[1]] = int(m[2])
            continue
        m = re.match(r'\s*makeRecipe\(new ComparableStack\(ModItems\.(\w+)\), (.*), new ItemStack\(ModItems\.(\w+)\),\s*(\w+)\);', line)
        if not m:
            continue
        particle, ingredient, output, amat = m[1], m[2], m[3], m[4]
        where = f'CyclotronRecipes.java:{i + 1}'
        amat = int(amat) if amat.isdigit() else amounts[amat]
        ore, item = None, None
        o = re.fullmatch(r'new OreDictStack\("(\w+)"\)', ingredient)
        f = re.fullmatch(r'new OreDictStack\((\w+)\.dust\(\)\)', ingredient)
        c = re.fullmatch(r'new ComparableStack\(ModItems\.(\w+)\)', ingredient)
        if o:
            ore = o[1]
        elif f:
            ore = 'dust' + frame[f[1]]
        elif c:
            item = c[1]
        else:
            skipped.append({'source': where, 'reason': 'unknown ingredient ' + ingredient})
            continue
        ingredients = names.get(ore, []) if ore else ids_of.get(item, [])[:1]
        why = None
        if particle not in ids_of:
            why = f'particle {particle} is not in the catalog'
        elif output not in ids_of:
            why = f'output {output} is not in the catalog'
        elif not ingredients:
            why = f'no item behind {ore or item}'
        if why:
            skipped.append({'source': where, 'reason': why})
            continue
        recipes.append({'particle': ids_of[particle][0], 'ore': ore, 'ingredients': ingredients, 'output': ids_of[output][0],
                        'output_stack': stack[ids_of[output][0]], 'amat': amat, 'source': where})
    seen, collisions = {}, []
    for r in recipes:
        for i in r['ingredients']:
            key = (r['particle'], i)
            if key in seen:
                collisions.append({'particle': r['particle'], 'ingredient': i, 'kept': seen[key], 'other': r['source']})
            else:
                seen[key] = r['source']
    return {'schema': 1, 'station': 'cyclotron', 'source': 'CyclotronRecipes.registerDefaults', 'recipes': recipes, 'skipped': skipped,
            'collisions': collisions, 'coverage': {'recipes': len(recipes), 'skipped': len(skipped), 'collisions': len(collisions)}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(build(), indent=2, ensure_ascii=False) + '\n'
    if args.check:
        raise SystemExit(0 if OUTPUT.read_text() == text else 'cyclotron.json is out of date')
    OUTPUT.write_text(text)
    print(json.loads(text)['coverage'])


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Recipes of the exposure chamber (`machine_exposure_chamber`), `ExposureChamberRecipes.registerDefaults` (lines 28-40).

A recipe is a particle capsule (`ComparableStack`, any stack size), an ingredient (an `OreDictStack` of `DictFrame.ingot()`) and
the output stack. The ingredient's ore name is `ingot` + the first name of the frame (`DictFrame.ingot`, line 753); its items are
the catalog's `oredict.json` for that name. The dineutronium recipe has two forms, one for `GeneralConfig.enableExpensiveMode`
(off by default); the port takes the normal one and lists the other as skipped. The empty capsule a particle leaves is its
`setContainerItem` in `ModItems`.
"""
import argparse
import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT.parent / 'ntm-src'
HBM = 'src/main/java/com/hbm/'
OUTPUT = ROOT / 'crates/ntm-core/data/machines/exposure.json'


def frames(source):
    """`DictFrame` variable -> the first of its names (the one `ingot()` uses)."""
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
    stack = {e['id']: e.get('max_stack_size', 64) for e in entries}
    ids_of = {}
    for e in entries:
        ids_of.setdefault(e.get('source_variable'), []).append(e['id'])
    names = json.loads((ROOT / 'crates/ntm-core/data/oredict.json').read_text())['names']
    frame = frames(source)
    mod_items = (source / HBM / 'items/ModItems.java').read_text()
    java = (source / HBM / 'inventory/recipes/ExposureChamberRecipes.java').read_text().split('\n')
    start = next(i for i, l in enumerate(java) if 'registerDefaults' in l)
    recipes, skipped, branch = [], [], 'plain'
    for i in range(start, len(java)):
        line = java[i]
        if 'enableExpensiveMode' in line and 'if(' in line:
            branch = 'expensive'
        elif re.match(r'\s*\}\s*else\s*\{', line):
            branch = 'plain'
        m = re.search(r'new ExposureChamberRecipe\(new ComparableStack\(ModItems\.(\w+)\), (.*), new ItemStack\(ModItems\.(\w+)\)\)\);', line)
        if not m:
            continue
        particle, ingredient, output = m[1], m[2], m[3]
        where = f'ExposureChamberRecipes.java:{i + 1}'
        if branch == 'expensive':
            skipped.append({'source': where, 'reason': 'enableExpensiveMode is off by default', 'particle': particle})
            continue
        o = re.fullmatch(r'new OreDictStack\((\w+)\.ingot\(\)\)', ingredient)
        if not o:
            skipped.append({'source': where, 'reason': 'not an ingot OreDictStack: ' + ingredient})
            continue
        ore = 'ingot' + frame[o[1]]
        container = re.search(r'\b' + particle + r'\s*=\s*new Item\(\).*?setContainerItem\(ModItems\.(\w+)\)', mod_items)
        recipes.append({
            'particle': ids_of[particle][0], 'ore': ore, 'ingredients': names.get(ore, []),
            'output': [ids_of[output][0], 1], 'output_stack': stack[ids_of[output][0]],
            'container': ids_of[container[1]][0] if container else None,
            'container_stack': stack[ids_of[container[1]][0]] if container else None, 'source': where})
    return {'schema': 1, 'station': 'exposure_chamber', 'source': 'ExposureChamberRecipes.registerDefaults', 'recipes': recipes,
            'skipped': skipped, 'coverage': {'recipes': len(recipes), 'skipped': len(skipped)}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(build(), indent=2, ensure_ascii=False) + '\n'
    if args.check:
        raise SystemExit(0 if OUTPUT.read_text() == text else 'exposure.json is out of date')
    OUTPUT.write_text(text)
    print(json.loads(text)['coverage'])


if __name__ == '__main__':
    main()

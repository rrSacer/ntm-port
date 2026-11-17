#!/usr/bin/env python3
"""Recipes of the SILEX (`machine_silex`), `SILEXRecipes` as the Java oracle registered them (tools/rbmk-oracle, exported
in content/machine-recipes/reactors_rbmk.json: `silex_recipes`, `silex_aliases`).

A recipe is keyed by a stack (`ComparableStack`: item and metadata) or an ore name, and gives the fluid it loads
(`fluidProduced`), the fluid a run takes (`fluidConsumed`), the weakest laser (`laserStrength`) and the weighted
outputs. `getOutput` (lines 680-716) translates the stack first (`itemTranslation`), then each of its ore names
(`dictTranslation`), then a tiny waste to its whole one (`tinyWasteTranslation`, lines 674-679: the recipe of the whole,
a ninth of its fluid, `(fluidProduced / 900) * 100`). The port names each metadata subtype as its own id
(`content/ids.json`), the 1.7.10 vanilla items as today's (`bulk_recipes.vanilla_item`), and `fluid_icon` of a fluid as
that fluid's name. What the catalog does not have is listed in `skipped` with the reason, not invented: chiefly the
RBMK pellets' metadata 1..9 (their depletion and xenon state), which the port's one id per pellet does not carry yet.
"""
import argparse
import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT.parent / 'ntm-src'
OUTPUT = ROOT / 'crates/ntm-core/data/machines/silex.json'
LASERS = ['NULL', 'IR', 'VISIBLE', 'UV', 'GAMMA', 'DRX']
# 1.7.10 vanilla items by (name, metadata) as today's ids: `Items.dye` 4 is lapis.
VANILLA = {('gold_nugget', 0): 'minecraft:gold_nugget', ('flint', 0): 'minecraft:flint', ('gravel', 0): 'minecraft:gravel',
           ('dye', 4): 'minecraft:lapis_lazuli'}


class Missing(Exception):
    pass


def catalog():
    by = {}
    for e in json.loads((ROOT / 'content/ids.json').read_text())['entries']:
        sv = e.get('source_variable')
        if sv is None:
            continue
        by.setdefault(sv, {}).setdefault(e.get('metadata', 0) or 0, e['id'])
    return by


def fluids():
    return {f['id']: f['string_id'] for f in json.loads((ROOT / 'crates/ntm-core/data/fluids.json').read_text())['fluids']}


def resolve(stack, by, fluid_names):
    """A Java stack as the port's key: an item id, or `fluid:<name>` for a `fluid_icon`."""
    item, meta = stack['item'], stack.get('meta', 0) or 0
    if item == 'fluid_icon':
        name = fluid_names.get(meta)
        if not name:
            raise Missing(f'fluid {meta} is not in the port')
        return 'fluid:' + name
    if item.startswith('minecraft_1_7_10:'):
        name = VANILLA.get((item.split(':', 1)[1], meta))
        if not name:
            raise Missing(f'vanilla {item}:{meta} has no mapping')
        return name
    ids = by.get(item, {})
    if meta not in ids:
        raise Missing(f'{item}:{meta} is not in the catalog' if ids else f'{item} is not in the catalog')
    return ids[meta]


def tiny_waste(source):
    text = (source / 'src/main/java/com/hbm/inventory/recipes/SILEXRecipes.java').read_text()
    return re.findall(r'tinyWasteTranslation\.put\(ModItems\.(\w+), ModItems\.(\w+)\);', text)


def build(source=None):
    source = Path(source or os.environ.get('NTM_SRC', DEFAULT_SOURCE))
    data = json.loads((ROOT / 'content/machine-recipes/reactors_rbmk.json').read_text())
    by, fluid_names = catalog(), fluids()
    recipes, skipped = [], []
    for index, row in enumerate(data['silex_recipes']):
        key = row['input']
        r = row['recipe']
        try:
            if 'ore' in key:
                inp = {'ore': key['ore']}
            else:
                inp = {'item': resolve(key['stack'], by, fluid_names)}
            outputs = []
            for o in r['outputs']:
                outputs.append({'item': resolve(o['stack'], by, fluid_names), 'count': o['stack']['count'], 'weight': o['weight']})
        except Missing as why:
            skipped.append({'index': index, 'input': key, 'reason': str(why)})
            continue
        if r['laser_strength'] not in LASERS or LASERS.index(r['laser_strength']) != r['laser_index']:
            raise ValueError(f'laser drift at {index}: {r["laser_strength"]}')
        recipes.append({'input': inp, 'produced': r['fluid_produced'], 'consumed': r['fluid_consumed'],
                        'laser': r['laser_index'], 'outputs': outputs})
    items, dicts = [], []
    for t in data['silex_aliases']['item_translation']:
        try:
            items.append([resolve(t['from']['stack'], by, fluid_names), resolve(t['to']['stack'], by, fluid_names)])
        except Missing as why:
            skipped.append({'translation': t, 'reason': str(why)})
    for t in data['silex_aliases']['dict_translation']:
        dicts.append([t['from'], t['to']])
    tiny = []
    # `new ItemStack(whole, size, comp.meta)`: each metadata of the tiny one to the same of the whole one.
    for small, whole in tiny_waste(source):
        for meta in sorted(by.get(small, {0: None})):
            try:
                tiny.append([resolve({'item': small, 'meta': meta}, by, fluid_names), resolve({'item': whole, 'meta': meta}, by, fluid_names)])
            except Missing as why:
                skipped.append({'tiny': [small, whole, meta], 'reason': str(why)})
    keys = [json.dumps(r['input'], sort_keys=True) for r in recipes]
    collisions = sorted({k for k in keys if keys.count(k) > 1})
    return {'schema': 1, 'source': 'SILEXRecipes.java via tools/rbmk-oracle (content/machine-recipes/reactors_rbmk.json)',
            'recipes': recipes, 'item_translation': items, 'dict_translation': dicts, 'tiny_waste': tiny,
            'collisions': collisions, 'skipped': skipped}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=None)
    args = parser.parse_args()
    out = build(args.source)
    OUTPUT.write_text(json.dumps(out, indent=1, ensure_ascii=False) + '\n')
    print(json.dumps({'silex': {'recipes': len(out['recipes']), 'skipped': len(out['skipped']), 'collisions': len(out['collisions']),
                                'translations': len(out['item_translation']), 'tiny': len(out['tiny_waste'])}}))


if __name__ == '__main__':
    main()

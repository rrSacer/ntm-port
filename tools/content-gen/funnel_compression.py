#!/usr/bin/env python3
"""Compression table of the funnel (`machine_funnel`).

`TileEntityMachineFunnel.getFrom9/getFrom4` ask `CraftingManager.getRecipeList()` for the first recipe that matches
a 3x3 grid full of one item (`from9`) or the top-left 2x2 of the grid full of it (`from4`) (lines 120-142).
The port has no crafting manager of 1.7.10, so the table is built from two sources, in the order of that list:

1. the vanilla 1.7.10 recipes that fill a grid with one item (registered first by `CraftingManager`'s constructor:
   `RecipesIngots`, `RecipesCrafting`, `RecipesDyes`...). The 1.7.10 jar is obfuscated and not on this machine as
   source, so the list is curated here, each row mapped to its 26.3 identity and checked against `registries.json`;
2. the NTM recipes of the catalog (`content/data/ntm/recipe`: shaped 3x3/2x2 with one key, shapeless with 9/4 equal
   ingredients), tags expanded to their members.

The first recipe for an input wins, as in the Java list; the later ones are kept in `collisions`. What cannot be
resolved (a result or an input the catalog does not have) goes to `skipped` with its reason, never invented.
"""
import argparse
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RESOURCES = ROOT.parents[1] / 'Games/horizon/.local/horizon-client/26.3'
OUTPUT = ROOT / 'crates/ntm-core/data/machines/funnel.json'

PLANKS = ['oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak']

# (input, output id, output count). Vanilla 1.7.10 recipes of a full 3x3 grid of one item.
VANILLA_9 = [
    ('iron_ingot', 'iron_block', 1), ('gold_ingot', 'gold_block', 1), ('diamond', 'diamond_block', 1),
    ('emerald', 'emerald_block', 1), ('lapis_lazuli', 'lapis_block', 1), ('redstone', 'redstone_block', 1),
    ('coal', 'coal_block', 1), ('gold_nugget', 'gold_ingot', 1), ('wheat', 'hay_block', 1), ('melon_slice', 'melon', 1)]
# Vanilla 1.7.10 recipes of a full 2x2 grid of one item.
VANILLA_4 = [
    ('glowstone_dust', 'glowstone', 1), ('quartz', 'quartz_block', 1), ('clay_ball', 'clay', 1), ('brick', 'bricks', 1),
    ('nether_brick', 'nether_bricks', 1), ('snowball', 'snow_block', 1), ('sand', 'sandstone', 1),
    ('stone', 'stone_bricks', 4)] + [(p + '_planks', 'crafting_table', 1) for p in PLANKS]


class Catalog:
    def __init__(self, resources):
        entries = json.loads((ROOT / 'content/ids.json').read_text())['entries']
        self.ntm = {e['id'] for e in entries}
        self.stacks = {e['id']: e['max_stack_size'] for e in entries if e.get('max_stack_size')}
        registries = json.loads((Path(resources) / 'registries.json').read_text())
        self.vanilla = set(registries['minecraft:item']['entries'])

    def stack(self, ident):
        """The result's stack limit (`getMaxStackSize`): the catalog's, else 64 (every vanilla result here stacks to 64)."""
        return self.stacks.get(ident, 64)

    def has(self, ident):
        if ident.startswith('ntm:'):
            return ident in self.ntm
        return ident in self.vanilla


def expand(ingredient, tag_dir, seen=()):
    """Item ids behind a recipe ingredient (an id, a `#tag`, or a list of them)."""
    if isinstance(ingredient, list):
        return [i for x in ingredient for i in expand(x, tag_dir, seen)]
    if isinstance(ingredient, dict):
        ingredient = ingredient.get('item') or ingredient.get('tag')
    if not ingredient.startswith('#'):
        return [ingredient]
    ns, _, path = ingredient[1:].partition(':')
    if ns != 'ntm' or ingredient in seen:
        return []
    file = tag_dir / (path + '.json')
    if not file.exists():
        return []
    values = json.loads(file.read_text())['values']
    return [i for v in values for i in expand(v, tag_dir, seen + (ingredient,))]


def grid_of_one(recipe):
    """(size, ingredient) when the recipe fills a 3x3 or 2x2 grid with one ingredient, else None."""
    if recipe['type'] == 'minecraft:crafting_shaped':
        pattern, key = recipe['pattern'], recipe['key']
        cells = ''.join(pattern)
        used = set(cells)
        if len(used) != 1 or ' ' in used or len(pattern) != len(pattern[0]) or len(pattern) not in (2, 3):
            return None
        return len(pattern) ** 2, key[used.pop()]
    if recipe['type'] == 'minecraft:crafting_shapeless':
        ing = recipe['ingredients']
        if len(ing) in (4, 9) and len({json.dumps(i, sort_keys=True) for i in ing}) == 1:
            return len(ing), ing[0]
    return None


def build(resources=None):
    catalog = Catalog(resources or os.environ.get('HORIZON_RESOURCES', DEFAULT_RESOURCES))
    tag_dir = ROOT / 'content/data/ntm/tags/item'
    tables = {9: {}, 4: {}}
    collisions, skipped = [], []

    def add(size, item, out, count, source):
        if item in tables[size]:
            collisions.append({'size': size, 'input': item, 'kept': tables[size][item]['source'], 'ignored': source,
                               'ignored_output': [out, count]})
            return
        tables[size][item] = {'output': [out, count], 'stack': catalog.stack(out), 'source': source}

    for size, rows in ((9, VANILLA_9), (4, VANILLA_4)):
        for item, out, count in rows:
            item, out = 'minecraft:' + item, 'minecraft:' + out
            if catalog.has(item) and catalog.has(out):
                add(size, item, out, count, 'vanilla 1.7.10')
            else:
                skipped.append({'size': size, 'input': item, 'reason': 'not in the 26.3 registry'})
    for file in sorted((ROOT / 'content/data/ntm/recipe').glob('*.json')):
        recipe = json.loads(file.read_text())
        found = grid_of_one(recipe)
        if not found:
            continue
        size, ingredient = found
        out, count = recipe['result']['id'], recipe['result'].get('count', 1)
        source = 'catalog ' + file.name
        if not catalog.has(out):
            skipped.append({'size': size, 'source': source, 'reason': f'result {out} is not in the catalog'})
            continue
        inputs = expand(ingredient, tag_dir)
        if not inputs:
            skipped.append({'size': size, 'source': source, 'reason': 'no item behind the ingredient'})
        for item in inputs:
            if not catalog.has(item):
                skipped.append({'size': size, 'source': source, 'input': item, 'reason': 'input not in the catalog'})
            else:
                add(size, item, out, count, source)
    return {
        'schema': 1, 'station': 'funnel',
        'source': 'TileEntityMachineFunnel.getFrom9/getFrom4 over CraftingManager (vanilla 1.7.10 curated list, then the catalog)',
        'from9': dict(sorted(tables[9].items())), 'from4': dict(sorted(tables[4].items())),
        'collisions': collisions, 'skipped': skipped,
        'coverage': {'from9': len(tables[9]), 'from4': len(tables[4]), 'collisions': len(collisions), 'skipped': len(skipped)}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true', help='fail if the embedded table is not what the sources give')
    args = parser.parse_args()
    text = json.dumps(build(), indent=2, ensure_ascii=False) + '\n'
    if args.check:
        raise SystemExit(0 if OUTPUT.read_text() == text else 'funnel.json is out of date')
    OUTPUT.write_text(text)
    print(json.loads(text)['coverage'])


if __name__ == '__main__':
    main()

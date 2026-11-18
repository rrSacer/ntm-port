#!/usr/bin/env python3
"""Tables of the radiolysis chamber (`machine_radiolysis`), `TileEntityMachineRadiolysis`.

- `cracking`: `RadiolysisRecipes.getRadiolysis(type)` (100 mB in, a pair of fluid stacks out). `registerRadiolysis` puts
  water -> 80 mB hydrogen peroxide + 20 mB hydrogen (line 50), then every cracking recipe of `CrackingRecipes.registerDefaults`
  with its outputs as they are, so they are read from the cracking extraction (`content/machine-recipes/cracking.json`): the first two
  fluid outputs (the steam is the cracker's own, not part of the pair), a missing second one being `NONE, 0` (kerosene).
- `foods`: the items `sterilize` eats from the sterilization slot (`instanceof ItemFood`, but not the pancake): NTM's own, found by
  scanning the classes that extend `ItemFood`/`ItemSoup` and the items `ModItems` makes from them (an item the catalog does not
  have is skipped), and the vanilla 1.7.10 ones, curated here and mapped to the 26.3 identities (that jar is obfuscated and not
  on this machine as source).
- `depleted`: what `canExtractItem` takes out of the pellet slots (`instanceof ItemRTGPelletDepleted`).
"""
import argparse
import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RESOURCES = ROOT.parents[1] / 'Games/horizon/.local/horizon-client/26.3'
DEFAULT_SOURCE = ROOT.parent / 'ntm-src'
OUTPUT = ROOT / 'crates/ntm-core/data/machines/radiolysis.json'
HBM = 'src/main/java/com/hbm/'

# Vanilla 1.7.10 `ItemFood`s (`ItemSoup` is one): `Items.apple` ... `Items.pumpkin_pie`.
VANILLA_FOODS = ['apple', 'mushroom_stew', 'bread', 'porkchop', 'cooked_porkchop', 'golden_apple', 'cod', 'cooked_cod', 'cookie',
                 'melon_slice', 'beef', 'cooked_beef', 'chicken', 'cooked_chicken', 'rotten_flesh', 'spider_eye', 'carrot', 'potato',
                 'baked_potato', 'poisonous_potato', 'golden_carrot', 'pumpkin_pie']
# `Items.fish` and `Items.cooked_fished` (`ItemFishFood`) of 1.7.10 carry cod, salmon, clownfish (tropical_fish) and pufferfish raw, cod and
# salmon cooked; the enchanted golden apple is `Items.golden_apple` with damage 1.
VANILLA_FOODS += ['salmon', 'cooked_salmon', 'tropical_fish', 'pufferfish', 'enchanted_golden_apple']
NOT_EATEN = {'ntm:pancake'}


def java_foods(source):
    classes = {'ItemFood', 'ItemSoup'}
    files = {p.stem: p.read_text() for p in (source / HBM / 'items').rglob('*.java')}
    changed = True
    while changed:
        changed = False
        for name, text in files.items():
            m = re.search(r'class\s+' + name + r'\s+extends\s+(\w+)', text)
            if m and m[1] in classes and name not in classes:
                classes.add(name)
                changed = True
    mod_items = (source / HBM / 'items/ModItems.java').read_text()
    return sorted(classes), re.findall(r'^\s*(\w+)\s*=\s*new\s+(\w+)\(', mod_items, re.M), classes


def build(resources=None, source=None):
    resources = Path(resources or os.environ.get('HORIZON_RESOURCES', DEFAULT_RESOURCES))
    source = Path(source or os.environ.get('NTM_SRC', DEFAULT_SOURCE))
    entries = json.loads((ROOT / 'content/ids.json').read_text())['entries']
    fluids = {f['string_id'] for f in json.loads((ROOT / 'crates/ntm-core/data/fluids.json').read_text())['fluids']}
    vanilla = set(json.loads((resources / 'registries.json').read_text())['minecraft:item']['entries'])
    cracking, skipped = {}, []
    radiolysis = (source / HBM / 'inventory/recipes/RadiolysisRecipes.java').read_text().split('\n')
    line = next(i for i, l in enumerate(radiolysis, 1) if 'radiolysis.put(Fluids.WATER' in l)
    cracking['water'] = {'out': [['peroxide', 80], ['hydrogen', 20]], 'source': f'RadiolysisRecipes.java:{line}'}
    for r in json.loads((ROOT / 'content/machine-recipes/cracking.json').read_text())['recipes']:
        (name, mb), = [r['inputFluids'][0]]
        assert mb == 100, r['name']
        outs = [o for o in r['outputFluids'] if o[0] != 'spentsteam']
        assert 1 <= len(outs) <= 2, r['name']
        outs += [['none', 0]] * (2 - len(outs))
        cracking[name] = {'out': outs, 'source': r['source']}
    for name, row in cracking.items():
        for f, _ in [[name, 0]] + row['out']:
            assert f in fluids, (name, f)
    classes, made, food_classes = java_foods(source)
    ids_of = {}
    for e in entries:
        ids_of.setdefault(e.get('source_variable'), []).append(e['id'])
    foods = []
    for var, cls in made:
        if cls in food_classes:
            ids = ids_of.get(var)
            if not ids:
                skipped.append({'item': var, 'class': cls, 'reason': 'not in the catalog'})
            foods += ids or []
    foods = sorted(set(foods) - NOT_EATEN)
    vanilla_foods = []
    for f in VANILLA_FOODS:
        if 'minecraft:' + f in vanilla:
            vanilla_foods.append('minecraft:' + f)
        else:
            skipped.append({'item': f, 'reason': 'not in the 26.3 registry'})
    depleted = sorted(e['id'] for e in entries if e['id'].startswith('ntm:pellet_rtg_depleted__'))
    return {'schema': 1, 'station': 'radiolysis', 'source': 'TileEntityMachineRadiolysis, RadiolysisRecipes, CrackingRecipes',
            'cracking': dict(sorted(cracking.items())), 'foods': {'ntm': foods, 'vanilla': sorted(vanilla_foods), 'not_eaten': sorted(NOT_EATEN)},
            'depleted': depleted, 'food_classes': classes, 'skipped': skipped,
            'coverage': {'cracking': len(cracking), 'ntm_foods': len(foods), 'vanilla_foods': len(vanilla_foods), 'depleted': len(depleted), 'skipped': len(skipped)}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(build(), indent=2, ensure_ascii=False) + '\n'
    if args.check:
        raise SystemExit(0 if OUTPUT.read_text() == text else 'radiolysis.json is out of date')
    OUTPUT.write_text(text)
    print(json.loads(text)['coverage'])


if __name__ == '__main__':
    main()

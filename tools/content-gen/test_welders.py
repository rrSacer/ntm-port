#!/usr/bin/env python3
"""The arc welder's skipped recipes (the 18 with a missile product): a guard for the day the items exist.

`arc_welder_recipes.py` skips a recipe whose product is not in the catalog (`ArcWelderRecipes.java:93-113`, the
missile family of the roadmap's wave 5). This fails when one of them already is, so the extractor is rerun
(expected then: 42 recipes emitted, none skipped for a missing product)."""
import json
import re
import unittest
from pathlib import Path

root = Path(__file__).resolve().parents[2]


class SkippedArcWelderRecipes(unittest.TestCase):
    def test_every_skipped_recipe_still_has_a_missing_product(self):
        table = json.loads((root / 'content/machine-recipes/arc_welder.json').read_text())
        ids = {e['id'] for e in json.loads((root / 'content/ids.json').read_text())['entries']}
        ready = []
        for skipped in table['skipped']:
            product = re.search(r'new ArcWelderRecipe\(new ItemStack\(ModItems\.(\w+)', skipped['expression'])
            if product and f'ntm:{product[1]}' in ids:
                ready.append((skipped['name'], product[1]))
        self.assertEqual(ready, [], 'products now in the catalog: rerun tools/content-gen/arc_welder_recipes.py')

    def test_the_count_of_the_skipped_is_what_the_source_says(self):
        table = json.loads((root / 'content/machine-recipes/arc_welder.json').read_text())
        self.assertEqual((table['coverage']['emitted'], table['coverage']['skipped']), (24, 18))
        self.assertEqual({s['reason'].split(' ')[0] for s in table['skipped']}, {'unregistered/ambiguous'})


if __name__ == '__main__':
    unittest.main()

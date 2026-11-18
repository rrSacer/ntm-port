"""crates/ntm-core/data/machines/silex.json against the Java oracle's SILEX tables (content/machine-recipes/reactors_rbmk.json).

Rebuilt on its own (silex_data.build) and compared with the checked-in file; then every skip must be an RBMK pellet's
metadata 1..9, and the tiny waste lookups the oracle computed (`getOutput` of a tiny waste) must come out of the whole
one's recipe with a ninth of its fluid.
"""
import json
import unittest
from pathlib import Path

import silex_data

ROOT = Path(__file__).resolve().parents[2]


class SilexData(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.built = silex_data.build()
        cls.oracle = json.loads((ROOT / 'content/machine-recipes/reactors_rbmk.json').read_text())

    def test_the_checked_in_table_is_the_built_one(self):
        self.assertEqual(json.loads(silex_data.OUTPUT.read_text()), self.built)

    def test_all_but_the_pellet_states_are_kept(self):
        self.assertEqual(len(self.built['recipes']) + len(self.built['skipped']), len(self.oracle['silex_recipes']))
        for s in self.built['skipped']:
            stack = s['input']['stack']
            self.assertTrue(stack['item'].startswith('rbmk_pellet_') and 1 <= stack['meta'] <= 9, s)
        self.assertEqual(self.built['collisions'], [])
        self.assertEqual(len(self.built['recipes']), 68)

    def test_the_vanilla_and_fluid_items_are_mapped(self):
        outputs = {o['item'] for r in self.built['recipes'] for o in r['outputs']}
        inputs = {r['input'].get('item') for r in self.built['recipes']}
        for item in ('minecraft:gold_nugget', 'minecraft:flint'):
            self.assertIn(item, outputs)
        self.assertIn('minecraft:gravel', inputs)
        self.assertIn(['ntm:powder_lapis', 'minecraft:lapis_lazuli'], self.built['item_translation'])
        self.assertIn(['fluid:uf6', 'ntm:ingot_uranium'], self.built['item_translation'])
        self.assertEqual(sorted(i for i in inputs if i and i.startswith('fluid:')),
                         ['fluid:death', 'fluid:fullerene', 'fluid:redmud', 'fluid:vitriol'])

    def test_the_tiny_waste_is_the_whole_ones_recipe_with_a_ninth_of_its_fluid(self):
        whole = {r['input'].get('item'): r for r in self.built['recipes']}
        tiny = dict(self.built['tiny_waste'])
        by = silex_data.catalog()
        checked = 0
        for row in self.oracle['silex_tiny_lookup']:
            small = row['input']
            key = by.get(small['item'], {}).get(small['meta'])
            if key not in tiny or row['recipe'] is None:
                continue
            base = whole.get(tiny[key])
            if base is None:
                continue
            self.assertEqual((base['produced'] // 900) * 100, row['recipe']['fluid_produced'], key)
            self.assertEqual(base['consumed'], row['recipe']['fluid_consumed'], key)
            self.assertEqual(base['laser'], row['recipe']['laser_index'], key)
            checked += 1
        self.assertGreater(checked, 0)


if __name__ == '__main__':
    unittest.main()

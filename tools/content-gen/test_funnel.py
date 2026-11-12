#!/usr/bin/env python3
"""The funnel's compression table: sources, order and what the embedded file must be."""
import json
import os
import unittest
from pathlib import Path
import funnel_compression as f

ROOT = Path(__file__).resolve().parents[2]
RESOURCES = os.environ.get('HORIZON_RESOURCES', f.DEFAULT_RESOURCES)


class FunnelTable(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.table = f.build(RESOURCES)

    def test_embedded_file_is_what_the_sources_give(self):
        self.assertEqual(json.loads((ROOT / 'crates/ntm-core/data/machines/funnel.json').read_text()), self.table)

    def test_vanilla_1710_rows_come_first_and_win(self):
        self.assertEqual(self.table['from9']['minecraft:iron_ingot']['output'], ['minecraft:iron_block', 1])
        self.assertEqual(self.table['from9']['minecraft:iron_ingot']['source'], 'vanilla 1.7.10')
        self.assertEqual(self.table['from4']['minecraft:stone']['output'], ['minecraft:stone_bricks', 4])
        self.assertEqual(self.table['from9']['minecraft:coal']['output'][0], 'minecraft:coal_block')
        self.assertNotIn('minecraft:charcoal', self.table['from9'])
        for modern in ('minecraft:copper_ingot', 'minecraft:iron_nugget', 'minecraft:slime_ball'):
            self.assertNotIn(modern, self.table['from9'], 'not a recipe of 1.7.10')

    def test_catalog_recipes_expand_tags_and_keep_counts(self):
        self.assertEqual(self.table['from9']['ntm:ingot_uranium']['output'], ['ntm:block_uranium', 1])
        self.assertEqual(self.table['from4']['ntm:concrete_smooth']['output'], ['ntm:concrete', 4])

    def test_a_collision_keeps_the_first_and_is_listed(self):
        for c in self.table['collisions']:
            kept = self.table['from9' if c['size'] == 9 else 'from4'][c['input']]
            self.assertNotEqual(kept['source'], c['ignored'])

    def test_every_result_exists_and_has_a_stack_limit(self):
        catalog = f.Catalog(RESOURCES)
        for size in ('from9', 'from4'):
            for item, e in self.table[size].items():
                self.assertTrue(catalog.has(item) and catalog.has(e['output'][0]), item)
                self.assertGreaterEqual(e['stack'], 1)
                self.assertGreaterEqual(e['output'][1], 1)


if __name__ == '__main__':
    unittest.main()

#!/usr/bin/env python3
"""The radiolysis chamber's tables: sources, and that the embedded file is what they give."""
import json
import os
import unittest
from pathlib import Path
import radiolysis_data as r

ROOT = Path(__file__).resolve().parents[2]


class RadiolysisTables(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.t = r.build()

    def test_embedded_file_is_what_the_sources_give(self):
        self.assertEqual(json.loads(r.OUTPUT.read_text()), self.t)

    def test_water_and_every_cracking_recipe(self):
        c = self.t['cracking']
        self.assertEqual(c['water']['out'], [['peroxide', 80], ['hydrogen', 20]])
        self.assertEqual(c['water']['source'], 'RadiolysisRecipes.java:50')
        self.assertEqual(c['oil']['out'], [['crackoil', 80], ['petroleum', 20]])
        self.assertEqual(c['kerosene']['out'], [['petroleum', 60], ['none', 0]], 'a missing second output is NONE, 0')
        self.assertEqual(len(c), 13)
        self.assertNotIn('spentsteam', json.dumps(c), 'the steam is the cracker\'s own')

    def test_foods_skip_what_the_catalog_lacks_and_the_pancake_is_not_eaten(self):
        f = self.t['foods']
        self.assertIn('ntm:ingot_smore', f['ntm'])
        self.assertNotIn('ntm:pancake', f['ntm'])
        self.assertEqual(f['not_eaten'], ['ntm:pancake'])
        self.assertIn('minecraft:bread', f['vanilla'])
        self.assertNotIn('minecraft:dried_kelp', f['vanilla'], 'a food of a later version')
        self.assertIn('crayon', [s['item'] for s in self.t['skipped']])

    def test_depleted_pellets(self):
        self.assertEqual(len(self.t['depleted']), 6)
        self.assertTrue(all(i.startswith('ntm:pellet_rtg_depleted__') for i in self.t['depleted']))


if __name__ == '__main__':
    unittest.main()

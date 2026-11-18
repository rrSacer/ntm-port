#!/usr/bin/env python3
"""The exposure chamber's recipes: sources, order and that the embedded file is what they give."""
import json
import unittest
import exposure_data as e


class ExposureTables(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.t = e.build()

    def test_embedded_file_is_what_the_sources_give(self):
        self.assertEqual(json.loads(e.OUTPUT.read_text()), self.t)

    def test_the_four_default_recipes_in_source_order(self):
        rows = [(r['particle'], r['ore'], r['output'][0], r['source']) for r in self.t['recipes']]
        self.assertEqual(rows, [
            ('ntm:particle_higgs', 'ingotUranium', 'ntm:ingot_schraranium', 'ExposureChamberRecipes.java:29'),
            ('ntm:particle_higgs', 'ingotUranium238', 'ntm:ingot_schrabidium', 'ExposureChamberRecipes.java:30'),
            ('ntm:particle_dark', 'ingotPlutonium', 'ntm:ingot_euphemium', 'ExposureChamberRecipes.java:31'),
            ('ntm:particle_sparkticle', 'ingotSchrabidate', 'ntm:ingot_dineutronium', 'ExposureChamberRecipes.java:36')])

    def test_the_ingredients_come_from_the_ore_names_and_the_capsule_from_the_container_item(self):
        self.assertEqual([r['ingredients'] for r in self.t['recipes']], [['ntm:ingot_uranium'], ['ntm:ingot_u238'], ['ntm:ingot_plutonium'], ['ntm:ingot_schrabidate']])
        self.assertTrue(all(r['container'] == 'ntm:particle_empty' for r in self.t['recipes']))

    def test_the_expensive_mode_recipe_is_listed_as_skipped(self):
        self.assertEqual([(s['source'], s['particle']) for s in self.t['skipped']], [('ExposureChamberRecipes.java:34', 'particle_sparkticle')])
        self.assertIn('enableExpensiveMode', self.t['skipped'][0]['reason'])

    def test_the_frames_use_their_first_name_and_the_false_branch_of_the_gt6_test(self):
        frame = e.frames(e.DEFAULT_SOURCE if e.DEFAULT_SOURCE.exists() else __import__('pathlib').Path('/mnt/ssd120/sacer/horizon-ntm/ntm-src'))
        self.assertEqual((frame['U'], frame['U238'], frame['PU'], frame['SBD']), ('Uranium', 'Uranium238', 'Plutonium', 'Schrabidate'))


if __name__ == '__main__':
    unittest.main()

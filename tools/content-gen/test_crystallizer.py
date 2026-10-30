#!/usr/bin/env python3
"""Source ordering, fluid keys, item requirements and omissions for crystallizer."""
import json
import tempfile
import unittest
from pathlib import Path
from crystallizer_recipes import export

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class CrystallizerContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {row['id']: row for row in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/crystallizer.json').read_text())
        cls.recipes = cls.data['recipes']

    def find(self, ident, fluid):
        matches = [r for r in self.recipes if ident in r['inputItems'][0]['ids'] and r['inputFluids'][0][0] == fluid]
        self.assertEqual(len(matches), 1, (ident, fluid))
        return matches[0]

    def test_all_source_calls_and_iteration_major_order(self):
        self.assertEqual({k:self.data['coverage'][k] for k in ('source_calls','expanded','bedrock')}, {'source_calls':118,'expanded':409,'bedrock':222})
        self.assertEqual(len(self.recipes) + len(self.data['skipped']), 409)
        all_rows = self.recipes + self.data['skipped']
        self.assertEqual({r['source_index'] for r in all_rows}, set(range(118)))
        self.assertEqual(len({r['name'] for r in all_rows}), 409)
        bedrock = [r for r in self.recipes if r['expansion'] and r['expansion'].startswith('bedrock_')]
        self.assertEqual([r['expansion'] for r in bedrock], [f'bedrock_{suffix}' for suffix in ['light', 'heavy', 'rare', 'actinide', 'nonmetal', 'crystal'] for _ in range(37)])
        self.assertEqual([r['source_index'] for r in bedrock[:37]], sorted(r['source_index'] for r in bedrock[:37]))

    def test_one_explicit_fluid_and_all_registered_ids(self):
        fluids = {r['string_id'] for r in json.loads((ROOT / 'content/fluids.json').read_text())['fluids']}
        vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/items')
        for r in self.recipes:
            self.assertEqual(len(r['inputFluids']), 1)
            self.assertEqual(len(r['inputItems']), 1)
            self.assertIn(r['inputFluids'][0][0], fluids)
            self.assertGreater(r['inputFluids'][0][1], 0)
            self.assertGreater(r['duration'], 0)
            self.assertEqual(r['power'], 1000)
            self.assertGreater(r['inputItems'][0]['count'], 0)
            self.assertTrue(r['label']); self.assertTrue(r['label_en'])
            for ident in r['inputItems'][0]['ids'] + [s[0] for s in r['outputItems']]:
                if ident.startswith('ntm:'): self.assertIn(ident, self.rows)
                else: self.assertTrue((vanilla / (ident.split(':')[1] + '.json')).is_file(), ident)

    def test_default_peroxide_durations_and_productivity(self):
        ore = self.find('minecraft:iron_ore', 'peroxide')
        self.assertEqual(ore['inputFluids'], [['peroxide', 500]])
        self.assertEqual(ore['duration'], 600)
        self.assertEqual(ore['productivity'], .05)
        self.assertEqual(ore['outputItems'], [['ntm:crystal_iron', 1]])
        leather = self.find('minecraft:rotten_flesh', 'peroxide')
        self.assertEqual((leather['duration'], leather['productivity']), (100, .25))
        bone = self.find('minecraft:bone', 'sulfuric_acid')
        self.assertEqual(bone['duration'], 20)
        self.assertEqual(bone['inputFluids'], [['sulfuric_acid', 1000]])
        self.assertEqual(bone['outputItems'], [['minecraft:slime_ball', 16]])

    def test_bedrock_washing_requirements_and_fluid_branches(self):
        for suffix in ['light', 'heavy', 'rare', 'actinide', 'nonmetal', 'crystal']:
            washed = self.find('ntm:bedrock_ore__base_' + suffix, 'water')
            self.assertEqual(washed['duration'], 100)
            self.assertEqual(washed['inputItems'][0]['count'], 1)
            self.assertEqual(washed['outputItems'], [['ntm:bedrock_ore__base_washed_' + suffix, 1]])
            for fluid, grade in [('sulfuric_acid', 'sulfuric'), ('solvent', 'solvent'), ('radiosolvent', 'rad'), ('hydrogen', 'first'), ('chlorine', 'second')]:
                row = self.find('ntm:bedrock_ore__primary_' + suffix, fluid)
                self.assertEqual(row['inputFluids'], [[fluid, 250]])
                self.assertEqual(row['outputItems'], [['ntm:bedrock_ore__primary_' + grade + '_' + suffix, 1]])
                self.assertEqual(row['duration'], 200)
            sub = self.find('ntm:bedrock_ore__sulfuric_byproduct_' + suffix, 'water')
            self.assertEqual(sub['inputItems'][0]['count'], 4)
            crumbs = self.find('ntm:bedrock_ore__crumbs_' + suffix, 'slop')
            self.assertEqual(crumbs['inputItems'][0]['count'], 64)
            self.assertEqual(crumbs['inputFluids'], [['slop', 1000]])
            self.assertEqual(crumbs['outputItems'], [['ntm:bedrock_ore__base_' + suffix, 1]])

    def test_setreq_and_default_scraps_quantity(self):
        scrap = self.find('ntm:scrap_oil', 'radiosolvent')
        self.assertEqual((scrap['inputItems'][0]['count'], scrap['productivity']), (16, .3))
        copper = next(r for r in self.recipes if r.get('source_scrap_amount') == 72)
        self.assertEqual(copper['outputItems'], [['ntm:scraps__copper', 1]])
        self.assertEqual(copper['inputFluids'], [['sulfuric_acid', 250]])
        sand = self.find('minecraft:sand', 'peroxide')
        self.assertEqual(set(sand['inputItems'][0]['ids']), {'minecraft:sand', 'minecraft:red_sand'})

    def test_missing_content_and_guarded_compat_are_explicit(self):
        self.assertTrue(all(row['reason'] and row['source'] for row in self.data['skipped']))
        self.assertTrue(any('pill_red' in row['reason'] for row in self.data['skipped']))
        self.assertTrue(any('sand_mix' in row['reason'] for row in self.data['skipped']))
        self.assertTrue(any('guard empty' in row['reason'] and 'Certus' in row['reason'] for row in self.data['skipped']))
        self.assertFalse(any('bedrock_' in (row['expansion'] or '') for row in self.data['skipped']))

    def test_machine_and_assembly(self):
        row = self.rows['ntm:machine_crystallizer']
        self.assertIn(row['id'], self.catalog['machines'])
        self.assertEqual(row['template'], 'minecraft:beehive')
        self.assertEqual(row['logic']['level'], {'prop': 'honey_level', 'max': 5})
        self.assertEqual(row['fluid_capacity_mb'], 8000)
        self.assertEqual(row['names'], {'en_us': 'Ore Acidizer', 'pt_br': 'Acidificador de Minério'})
        assembly = json.loads((ROOT / 'content/machine-recipes/assembly.json').read_text())
        recipe = next(r for r in assembly['recipes'] if r['name'] == 'ass.acidizer')
        self.assertEqual(recipe['outputItems'], [['ntm:machine_crystallizer', 1]])
        self.assertEqual((recipe['duration'], recipe['power']), (200, 100))
        self.assertEqual(self.data['machine_config']['default_recipe_fluid'], ['peroxide', 500])
        self.assertEqual(self.data['machine_config']['energy_capacity_he'], 1000000)

    def test_regeneration_is_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(export(SOURCE, Path(directory), list(self.rows.values())), self.data)


if __name__ == '__main__': unittest.main()

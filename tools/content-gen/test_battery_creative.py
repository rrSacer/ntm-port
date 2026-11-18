#!/usr/bin/env python3
"""The creative battery is a singleton content item; charging belongs to logic."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class CreativeBattery(unittest.TestCase):
    def test_identity_icon_and_stack(self):
        row = next(r for r in json.loads((ROOT / 'content/ids.json').read_text())['entries'] if r['id'] == 'ntm:battery_creative')
        self.assertEqual(row['max_stack_size'], 1)
        self.assertTrue(row['source_texture'].endswith('/battery_creative_new.png'))
        for loc in ['en_us', 'pt_br']: self.assertTrue(row['names'][loc])
        declaration = json.loads((ROOT / 'content/data/ntm/horizon/item/battery_creative.json').read_text())
        self.assertEqual(declaration, {'template': 'minecraft:iron_ingot', 'max_stack_size': 1})

    def test_creative_only_without_synthetic_recipe(self):
        tabs = (ROOT / 'content/data/ntm/creative_tab').glob('*.json')
        self.assertEqual(sum(json.loads(p.read_text())['items'].count('ntm:battery_creative') for p in tabs), 1)
        for file in (ROOT / 'content/data/ntm/recipe').glob('*.json'):
            self.assertNotEqual(json.loads(file.read_text())['result']['id'], 'ntm:battery_creative')
        for file in (ROOT / 'content/machine-recipes').glob('*.json'):
            for recipe in json.loads(file.read_text()).get('recipes', []):
                # A `ChanceOutputMulti` is {'multi': [[id, n, weight], ...]}: each of its outputs counts.
                outputs = [o for output in recipe.get('outputItems', []) for o in (output['multi'] if isinstance(output, dict) else [output])]
                self.assertFalse(any(output[0] == 'ntm:battery_creative' for output in outputs))


if __name__ == '__main__': unittest.main()

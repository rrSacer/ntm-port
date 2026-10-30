"""The surviving source identifier uses item data, not invented metadata IDs."""
import json
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]


class FluidIdentifier(unittest.TestCase):
    def test_identity_and_overlay(self):
        rows=json.loads((ROOT/'content/ids.json').read_text())['entries']
        identifiers=[r for r in rows if r['id'].startswith('ntm:fluid_identifier')]
        self.assertEqual(len(identifiers),1)
        row=identifiers[0]
        self.assertEqual(row['id'],'ntm:fluid_identifier_multi')
        self.assertEqual(row['max_stack_size'],1)
        self.assertNotIn('metadata',row)
        self.assertEqual(row['fluid_identifier']['custom_data_key'],'ntm_fluid')
        none=json.loads((ROOT/'content/fluids.json').read_text())['fluids'][0]
        self.assertEqual(row['overlay_textures'][0]['tint']['multiply'],none['color'])
        tag=json.loads((ROOT/'content/data/ntm/tags/item/fluid_identifiers.json').read_text())
        self.assertEqual(tag['values'],[row['id']])

    def test_original_vertical_recipe_and_generic_dyes(self):
        recipes=[json.loads(p.read_text()) for p in (ROOT/'content/data/ntm/recipe').glob('*.json')]
        recipes=[r for r in recipes if r.get('result',{}).get('id')=='ntm:fluid_identifier_multi']
        self.assertEqual(len(recipes),1)
        recipe=recipes[0]
        self.assertEqual(recipe['pattern'],['D','C','P'])
        self.assertEqual(recipe['result']['count'],1)
        self.assertEqual(recipe['key']['C'],'ntm:circuit__vacuum_tube')
        self.assertIn('plate',recipe['key']['P'])
        self.assertEqual(recipe['key']['D'],'#ntm:bulk/dye')
        dyes=json.loads((ROOT/'content/data/ntm/tags/item/bulk/dye.json').read_text())['values']
        self.assertIn('minecraft:red_dye',dyes)
        self.assertIn('ntm:powder_coal',dyes)


if __name__=='__main__':unittest.main()

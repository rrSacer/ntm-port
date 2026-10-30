#!/usr/bin/env python3
"""Missing identities close real recipes without inventing equipment behavior."""
import json
import unittest
from pathlib import Path
from anvil_recipes import snapshot
from crystallizer_recipes import enums

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class CompletedCatalog(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((ROOT/'content/ids.json').read_text())
        cls.rows={r['id']:r for r in cls.catalog['entries']}
        cls.crystal=json.loads((ROOT/'content/machine-recipes/crystallizer.json').read_text())

    def test_all_plastics_preserve_ordinal_and_original_single_icon(self):
        values=enums(SOURCE,'ScrapType')
        for i,(name,_) in enumerate(values):
            r=self.rows['ntm:scrap_plastic__'+name.lower()]
            self.assertEqual((r['metadata'],r['source_enum']),(i,'ScrapType.'+name))
            self.assertTrue(r['source_texture'].endswith('/scrap_plastic.png'))
        self.assertEqual(len([r for r in self.rows.values() if r['source_variable']=='scrap_plastic']),23)

    def test_dyes_are_sixteen_items_not_eighteen_recipe_instances(self):
        dyes=[r for r in self.rows.values() if r['source_variable']=='chemical_dye']
        self.assertEqual(len(dyes),16)
        self.assertEqual({r['metadata'] for r in dyes},set(range(16)))
        self.assertEqual(self.rows['ntm:chemical_dye__black']['overlay_textures'][0]['tint'],{'multiply':'1E1B1B'})
        self.assertEqual(self.rows['ntm:chemical_dye__white']['metadata'],15)
        self.assertEqual(sum((r.get('expansion') or '').startswith('dye_') for r in self.crystal['recipes']),18)

    def test_srn_is_schraranium_not_strontium(self):
        resolver,_,_,_=snapshot(SOURCE,list(self.rows.values()))
        self.assertEqual(resolver.input('new OreDictStack(SRN.ingot())')[1],['ntm:ingot_schraranium'])
        self.assertNotIn('ntm:powder_strontium',resolver.ore['ingotSchraranium'])

    def test_all_requested_holes_and_required_partners_resolve(self):
        expected={'ntm:powder_cement','ntm:ingot_semtex','ntm:ingot_schraranium','ntm:block_graphite',
                  'ntm:rebar','ntm:concrete_rebar','ntm:meteorite_sword_treated','ntm:meteorite_sword_etched'}
        self.assertTrue(expected<=set(self.rows))
        self.assertEqual(self.crystal['coverage']['emitted'],404)
        self.assertEqual(self.crystal['coverage']['skipped'],5)
        self.assertFalse(any(any(i.split(':')[1] in r.get('reason','') for i in expected) for r in self.crystal['skipped']))
        for name in ['meteorite_sword_treated','meteorite_sword_etched']:
            r=self.rows['ntm:'+name]
            self.assertEqual(r['max_stack_size'],1)
            self.assertNotIn('template',r)  # inventory/processing identity, no vanilla sword behavior
            self.assertIn('equipment backlog',r['behavior_gap'])


if __name__=='__main__':unittest.main()

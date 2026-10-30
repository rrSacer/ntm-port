#!/usr/bin/env python3
"""Legacy scaffold metadata becomes four persistent ids and vanilla axes."""
import json
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]


class Structures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows={r['id']:r for r in json.loads((ROOT/'content/ids.json').read_text())['entries']}

    def test_four_palette_identities_and_vanilla_axis_templates(self):
        for meta,suffix in enumerate(['','__red','__white','__yellow']):
            row=self.rows['ntm:steel_scaffold'+suffix]
            self.assertEqual(row['metadata'],meta);self.assertEqual(row['source_variable'],'steel_scaffold')
            self.assertEqual(row['template'],'minecraft:quartz_pillar')
            decl=json.loads((ROOT/f'content/data/ntm/horizon/block/steel_scaffold{suffix}.json').read_text())
            self.assertEqual(decl['template'],'minecraft:quartz_pillar')
            states=json.loads((ROOT/f'content/assets/ntm/blockstates/steel_scaffold{suffix}.json').read_text())
            self.assertEqual(set(states['variants']),{'axis=x','axis=y','axis=z'})

    def test_original_dye_recipes_resolve_distinct_colors(self):
        recipes=[json.loads(p.read_text()) for p in (ROOT/'content/data/ntm/recipe').glob('*.json')]
        for suffix,dye in [('', 'gray'),('__red','red'),('__white','white'),('__yellow','yellow')]:
            found=[r for r in recipes if r['result']['id']=='ntm:steel_scaffold'+suffix and r.get('pattern')==['SSS','SDS','SSS']]
            self.assertTrue(found,suffix)
            self.assertTrue(any(r['result']['count']==8 and r['key']['D']=='#ntm:bulk/dye'+dye for r in found),suffix)

    def test_grate_keeps_vanilla_slab_states(self):
        self.assertEqual(self.rows['ntm:steel_grate']['template'],'minecraft:stone_slab')
        definition=json.loads((ROOT/'content/data/ntm/horizon/block/steel_grate.json').read_text())
        self.assertEqual(definition['template'],'minecraft:stone_slab')
        states=json.loads((ROOT/'content/assets/ntm/blockstates/steel_grate.json').read_text())
        self.assertEqual(set(states['variants']),{'type=bottom','type=top','type=double'})

    def test_new_colors_have_hazards(self):
        data=json.loads((ROOT/'crates/ntm-core/data/hazards.json').read_text())
        rates={r['id']:r['radiation_millirad_s'] for r in data['entries']}
        for suffix in ['', '__red','__white','__yellow']:self.assertEqual(rates['ntm:steel_scaffold'+suffix],0)


if __name__=='__main__':unittest.main()

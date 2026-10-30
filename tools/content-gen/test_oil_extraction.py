#!/usr/bin/env python3
"""Oil drills preserve probabilistic extraction and distinct bedrock rates."""
import json
import tempfile
import unittest
from pathlib import Path
from oil_extraction import export

ROOT=Path(__file__).resolve().parents[2];SOURCE=ROOT.parent/'ntm-src'


class OilExtraction(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((ROOT/'content/ids.json').read_text())
        cls.rows={r['id']:r for r in cls.catalog['entries']}
        cls.data=json.loads((ROOT/'content/machine-recipes/oil_extraction.json').read_text())

    def test_machine_templates_and_level_contract(self):
        for name in ['machine_well','machine_pumpjack','machine_fracking_tower','machine_transformer']:
            row=self.rows['ntm:'+name]
            self.assertIn(row['id'],self.catalog['machines'])
            self.assertEqual(row['template'],'minecraft:beehive')
            self.assertEqual(row['logic']['facing'],'facing')
            self.assertEqual(row['logic']['level'],{'prop':'honey_level','max':5})

    def test_unupgraded_default_rates_and_probabilities(self):
        for name,energy,power,delay,oil,lo,hi,chance in [
            ('machine_well',100000,100,50,500,100,500,.05),
            ('machine_pumpjack',250000,200,25,750,50,250,.025),
            ('machine_fracking_tower',5000000,5000,20,1000,100,500,.02)]:
            cfg=self.data['machine_config']['ntm:'+name]
            self.assertEqual([cfg[k] for k in ['energy_capacity_he','power_he_per_tick','delay_ticks','oil_per_deposit_mb','gas_min_mb','gas_max_mb','depletion_chance']],
                             [energy,power,delay,oil,lo,hi,chance])
            self.assertTrue(all('.java:' in p for p in cfg['sources'].values()))
            self.assertEqual(cfg['gas_distribution'],'uniform inclusive integer range')
            self.assertTrue(all(t['capacity_mb']==64000 for t in cfg['tanks']))

    def test_fracking_bedrock_and_depth_adaptation(self):
        cfg=self.data['machine_config']['ntm:machine_fracking_tower']
        self.assertEqual([cfg[k] for k in ['fracksol_required_mb','bedrock_oil_mb','bedrock_gas_min_mb','bedrock_gas_max_mb']], [10,100,10,50])
        self.assertEqual((cfg['legacy_drill_depth_y'],cfg['port_drill_depth_y']),(0,-64))
        self.assertIn('ntm:ore_bedrock_oil',cfg['deposit_blocks'])
        self.assertIn('without depletion',self.data['rules']['fracking'])
        for name in ['machine_well','machine_pumpjack']:
            self.assertEqual(self.data['machine_config']['ntm:'+name]['port_drill_depth_y'],5)

    def test_algorithmic_operations_are_not_flat_recipes(self):
        self.assertEqual(self.data['recipes'],[]);self.assertEqual(self.data['skipped'],[])
        self.assertEqual(self.data['coverage'],{'machines':3,'flat_recipes':0,'skipped':0})
        self.assertIn('every eligible server tick',self.data['rules']['energy'])
        self.assertIn('shuffled six directions',self.data['rules']['deposit_search'])

    def test_deposit_properties_and_no_drop_pipe(self):
        bedrock=json.loads((ROOT/'content/data/ntm/horizon/block/ore_bedrock_oil.json').read_text())
        self.assertEqual(bedrock['properties']['hardness'],-1)
        self.assertGreaterEqual(bedrock['properties']['explosion_resistance'],3600000/5)
        self.assertEqual(self.rows['ntm:ore_oil_sand']['template'],'minecraft:sand')
        self.assertTrue(self.rows['ntm:oil_pipe']['creative_hidden'])
        loot=json.loads((ROOT/'content/data/ntm/loot_table/blocks/oil_pipe.json').read_text())
        self.assertEqual(loot['pools'],[])

    def test_constructions_and_arc_furnace_are_available(self):
        assembly=json.loads((ROOT/'content/machine-recipes/assembly.json').read_text())['recipes']
        outputs={o[0] for r in assembly for o in r['outputItems']}
        self.assertTrue({'ntm:'+n for n in ['machine_well','machine_pumpjack','machine_fracking_tower','machine_arc_furnace_large']}<=outputs)
        recipes=[json.loads(p.read_text()) for p in (ROOT/'content/data/ntm/recipe').glob('*.json')]
        self.assertTrue(any(r['result']['id']=='ntm:machine_transformer' for r in recipes))

    def test_fresh_directory_determinism(self):
        with tempfile.TemporaryDirectory() as path:
            self.assertEqual(export(SOURCE,Path(path),list(self.rows.values())),self.data)


if __name__=='__main__':unittest.main()

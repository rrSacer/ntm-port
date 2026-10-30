#!/usr/bin/env python3
"""Auxiliary heating is an instantaneous heat offer; flare fuel is algorithmic."""
import json
import tempfile
import unittest
from pathlib import Path
from petroleum_auxiliary import export
from petroleum_auxiliary_catalog import build

ROOT=Path(__file__).resolve().parents[2];SOURCE=ROOT.parent/'ntm-src'


class PetroleumE(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((ROOT/'content/ids.json').read_text());cls.rows={r['id']:r for r in cls.catalog['entries']}
        cls.tables={name:json.loads((ROOT/f'content/machine-recipes/{name}.json').read_text()) for name in ['hephaestus','flare']}

    def test_both_machines_and_their_source_constructions(self):
        assembly=json.loads((ROOT/'content/machine-recipes/assembly.json').read_text())['recipes']
        for block,recipe in [('machine_hephaestus','ass.hephaestus'),('machine_flare','ass.flarestack')]:
            row=self.rows['ntm:'+block];self.assertIn(row['id'],self.catalog['machines'])
            self.assertEqual(row['template'],'minecraft:beehive');self.assertEqual(row['logic']['level'],{'prop':'honey_level','max':5})
            r=next(r for r in assembly if r['name']==recipe);self.assertEqual(r['outputItems'],[['ntm:'+block,1]])

    def test_six_plant_metadata_names_and_drop(self):
        members=['foxglove','tobacco','nightshade','weed','cd0','cd1']
        for meta,name in enumerate(members):
            ident='ntm:plant_flower__'+name;row=self.rows[ident]
            self.assertEqual(row['metadata'],meta);self.assertEqual(row['template'],'minecraft:poppy')
            self.assertEqual(row['needs_oil'],name.startswith('cd'))
            model=json.loads((ROOT/f'content/assets/ntm/models/block/plant_flower__{name}.json').read_text())
            self.assertEqual(model['parent'],'minecraft:block/cross')
            self.assertEqual(model['textures']['cross'],'ntm:block/plant_flower.'+name)
        loot=json.loads((ROOT/'content/data/ntm/loot_table/blocks/plant_flower__cd1.json').read_text())
        self.assertEqual(loot['pools'][0]['entries'][0]['name'],'ntm:plant_flower__cd0')

    def test_consumables_have_no_invented_medical_behavior(self):
        for name in ['bio_wafer','pill_herbal']:
            row=self.rows['ntm:'+name];self.assertEqual(row['template'],'minecraft:iron_ingot')
            self.assertIn('medical effects',row['behavior_gap'])

    def test_hephaestus_first_heatexchanger_steps_only(self):
        d=self.tables['hephaestus'];self.assertEqual(d['coverage'],{'heating_steps':18,'emitted':9,'skipped':0})
        fluids={r['inputFluids'][0][0] for r in d['recipes']}
        self.assertEqual(fluids,{'water','oil','oil_ds','crackoil','crackoil_ds','coolant','perfluoromethyl','mug','blood'})
        water=next(r for r in d['recipes'] if r['inputFluids'][0][0]=='water')
        self.assertEqual(water['inputFluids'],[['water',1]]);self.assertEqual(water['outputFluids'],[['steam',100]])
        self.assertEqual(water['heat_required_tu'],200)
        for r in d['recipes']:self.assertNotIn('duration',r);self.assertEqual(r['power'],0)
        self.assertIn('without dividing by efficiency',d['machine_config']['efficiency'])

    def test_geothermal_heat_is_not_a_consumed_buffer(self):
        cfg=self.tables['hephaestus']['machine_config']
        self.assertEqual((cfg['scan_layers'],cfg['scan_xz_radius'],cfg['legacy_min_y']),(10,7,0))
        self.assertEqual((cfg['input_capacity_mb'],cfg['output_capacity_mb']),(24000,24000))
        self.assertIn('not a stored or consumed buffer',cfg['heat_supply'])
        self.assertEqual([s['heat_per_block'] for s in cfg['heat_sources']],[5,150,300])
        self.assertTrue(all('.java:' in source for source in cfg['sources'].values()))

    def test_flare_uses_final_flammable_not_combustible(self):
        d=self.tables['flare'];self.assertEqual(d['recipes'],[]);self.assertEqual(d['skipped'],[])
        self.assertEqual(d['coverage'],{'fluids':156,'flammable':60,'ventable':38,'skipped':0})
        traits={r['fluid']:r for r in d['fuel_traits']}
        self.assertEqual(set(traits),{f['string_id'] for f in json.loads((ROOT/'content/fluids.json').read_text())['fluids']} - {'none'})
        gas=traits['gas'];self.assertTrue(gas['gaseous_or_art']);self.assertEqual(gas['burn_penalty'],5)
        self.assertEqual(traits['hydrogen']['burn_penalty'],5)
        self.assertEqual(traits['oil']['burn_penalty'],10)
        self.assertFalse(traits['water']['flammable']);self.assertFalse(traits['water']['ventable'])
        cfg=d['machine_config'];self.assertEqual((cfg['vent_mb_per_tick'],cfg['burn_mb_per_tick']),(50,10))
        self.assertEqual((cfg['energy_capacity_he'],cfg['tank_capacity_mb']),(100000,64000))
        self.assertIn('consume even when energy buffer is full',cfg['burn_rule'])
        self.assertIn('base//=burn_penalty',cfg['energy_formula'])

    def test_regenerated_tables_match_fresh_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(export(SOURCE,Path(directory),list(self.rows.values())),self.tables)
        source=build(SOURCE);self.assertEqual(len(source),8)


if __name__=='__main__':unittest.main()

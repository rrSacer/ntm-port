#!/usr/bin/env python3
"""Source match priority, legacy identities and solidifier overwrite behavior."""
import json
import tempfile
import unittest
from pathlib import Path
from petroleum_recipes import export

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class PetroleumA(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((ROOT/'content/ids.json').read_text())
        cls.rows={r['id']:r for r in cls.catalog['entries']}
        cls.tables={name:json.loads((ROOT/f'content/machine-recipes/{name}.json').read_text())
                    for name in ['liquefaction','solidification']}

    def test_both_machine_contracts_and_original_assembly(self):
        for name in ['machine_liquefactor','machine_solidifier']:
            row=self.rows['ntm:'+name]
            self.assertIn(row['id'],self.catalog['machines'])
            self.assertEqual(row['template'],'minecraft:beehive')
            self.assertEqual(row['logic']['level'],{'prop':'honey_level','max':5})
        assembly=json.loads((ROOT/'content/machine-recipes/assembly.json').read_text())['recipes']
        for name in ['liquefactor','solidifier']:
            r=next(r for r in assembly if r['name']=='ass.'+name)
            self.assertEqual(r['outputItems'],[['ntm:machine_'+name,1]])
            self.assertEqual((r['duration'],r['power']),(200,100))
            if name=='liquefactor':
                tar=next(i for i in r['inputItems'] if 'ntm:oil_tar__crude' in i['ids'])
                self.assertEqual(tar['count'],4)
                self.assertEqual(set(tar['ids']),{'ntm:oil_tar__crude','ntm:oil_tar__crack','ntm:oil_tar__coal','ntm:oil_tar__wood'})

    def test_base_parameters_have_source_locations(self):
        for table in self.tables.values():
            cfg=table['machine_config']
            self.assertEqual({k:cfg[k] for k in ['power','duration','energy_capacity_he','tank_capacity_mb']},
                             {'power':250,'duration':60,'energy_capacity_he':100000,'tank_capacity_mb':24000})
            self.assertEqual(set(cfg['sources']),{'power','duration','energy_capacity_he','tank_capacity_mb'})
            self.assertTrue(all(': ' not in p and '.java:' in p for p in cfg['sources'].values()))

    def find_item(self, ident):
        return next(r for r in self.tables['liquefaction']['recipes'] if ident in r['inputItems'][0]['ids'])

    def test_liquefaction_coal_lead_and_legacy_block_identities(self):
        self.assertEqual(self.find_item('minecraft:coal')['outputFluids'],[['coaloil',250]])
        self.assertEqual(self.find_item('ntm:ingot_lead')['outputFluids'],[['lead',100]])
        self.assertEqual(self.find_item('ntm:block_lead')['outputFluids'],[['lead',900]])
        for ident,fluid,amount in [('snowball','water',125),('snow_block','water',500),('melon_slice','ethanol',100),
                                   ('short_grass','seedslurry',100),('fern','seedslurry',100),('sunflower','sunfloweroil',100)]:
            self.assertEqual(self.find_item('minecraft:'+ident)['outputFluids'],[[fluid,amount]])
        identities={x for r in self.tables['liquefaction']['recipes'] for x in r['inputItems'][0]['ids']}
        self.assertNotIn('minecraft:melon',identities)
        self.assertNotIn('minecraft:snow',identities)

    def test_wildcard_fish_and_source_priority_not_array_order(self):
        row=self.find_item('minecraft:salmon')
        self.assertEqual(row['inputItems'],[{'ids':['minecraft:cod','minecraft:salmon','minecraft:tropical_fish','minecraft:pufferfish'],'count':1}])
        self.assertEqual(row['match_kind'],'wildcard')
        self.assertEqual(row['outputFluids'],[['fishoil',100]])
        self.assertEqual(self.tables['liquefaction']['selection_order'],['exact','wildcard','ore','food_fallback'])
        self.assertEqual(self.find_item('minecraft:coal')['match_kind'],'ore')
        self.assertEqual(self.find_item('minecraft:snowball')['match_kind'],'exact')

    def test_solidification_uses_last_balefire_and_own_penalty(self):
        data=self.tables['solidification']
        bf=[r for r in data['recipes'] if r['inputFluids'][0][0]=='balefire']
        self.assertEqual(len(bf),1)
        self.assertEqual(bf[0]['inputFluids'],[['balefire',110]])
        self.assertEqual(bf[0]['outputItems'],[['ntm:solid_fuel_bf',1]])
        self.assertEqual(bf[0]['solid_fuel_auto']['penalty'],1.25)
        self.assertEqual(len(data['superseded']),1)
        old=data['superseded'][0]
        self.assertEqual(old['recipe']['inputFluids'],[['balefire',250]])
        self.assertEqual(old['replaced_by'],bf[0]['name'])
        kero=next(r for r in data['recipes'] if r['inputFluids'][0][0]=='kerosene')
        self.assertEqual(kero['inputFluids'],[['kerosene',700]])

    def test_coverage_missing_later_batches_and_dynamic_food_gap(self):
        liq=self.tables['liquefaction'];solid=self.tables['solidification']
        self.assertEqual(liq['coverage'],{'source_calls':34,'emitted':31,'skipped':3,'runtime_gaps':1})
        self.assertEqual(solid['coverage'],{'source_calls':48,'emitted':45,'skipped':2,'superseded':1})
        self.assertTrue(any('ore_oil_sand' in r['reason'] for r in liq['skipped']))
        self.assertEqual(sum('plant_flower' in r['expression'] for r in liq['skipped']),2)
        self.assertTrue(any('bio_wafer' in r['reason'] for r in solid['skipped']))
        self.assertEqual(liq['runtime_gaps'][0]['amount_formula'],'int(food_value * saturation_modifier * 20)')
        self.assertEqual(liq['runtime_gaps'][0]['output_fluid'],'salient')
        for table in [liq,solid]:
            indices=[r['source_index'] for r in table['recipes']+table['skipped']]
            indices += [r['recipe']['source_index'] for r in table.get('superseded',[])]
            self.assertEqual(sorted(indices),list(range(table['coverage']['source_calls'])))

    def test_registered_id_and_fluid_graph(self):
        fluids={f['string_id'] for f in json.loads((ROOT/'content/fluids.json').read_text())['fluids']}
        for table in self.tables.values():
            for r in table['recipes']:
                self.assertTrue(r['source'].startswith('src/main/java/'));self.assertTrue(r['label']);self.assertTrue(r['label_en'])
                for ident in [i for a in r['inputItems'] for i in a['ids']]+[o[0] for o in r['outputItems']]:
                    if ident.startswith('ntm:'):self.assertIn(ident,self.rows)
                for fluid,amount in r['inputFluids']+r['outputFluids']:self.assertIn(fluid,fluids);self.assertGreater(amount,0)

    def test_fresh_directory_determinism(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(export(SOURCE,Path(directory),list(self.rows.values())),self.tables)


if __name__=='__main__':unittest.main()

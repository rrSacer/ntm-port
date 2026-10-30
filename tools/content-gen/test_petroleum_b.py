#!/usr/bin/env python3
"""Counter-based refinery solids, complete fraction tables and mixer choices."""
import json
import tempfile
import unittest
from pathlib import Path
from petroleum_refining import export

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class PetroleumB(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((ROOT/'content/ids.json').read_text())
        cls.rows={r['id']:r for r in cls.catalog['entries']}
        cls.tables={n:json.loads((ROOT/f'content/machine-recipes/{n}.json').read_text()) for n in ['refinery','fraction','mixer']}

    def test_machine_contracts(self):
        for name in ['machine_refinery','machine_fraction_tower','machine_mixer']:
            r=self.rows['ntm:'+name]
            self.assertIn(r['id'],self.catalog['machines']);self.assertEqual(r['template'],'minecraft:beehive')
            self.assertEqual(r['logic']['facing'],'facing')
            self.assertEqual(r['logic']['level'],{'prop':'honey_level','max':5})

    def test_barricade_has_no_loot_or_creative_entry(self):
        row=self.rows['ntm:barricade'];self.assertTrue(row['creative_hidden'])
        definition=json.loads((ROOT/'content/data/ntm/horizon/block/barricade.json').read_text())
        self.assertEqual(definition['properties']['hardness'],1)
        self.assertEqual(definition['properties']['explosion_resistance'],2.5)
        self.assertFalse(definition['properties']['requires_tool'])
        self.assertEqual(json.loads((ROOT/'content/data/ntm/loot_table/blocks/barricade.json').read_text())['pools'],[])
        for p in (ROOT/'content/data/ntm/creative_tab').glob('*.json'):
            self.assertNotIn('ntm:barricade',json.loads(p.read_text())['items'])
        for p in (ROOT/'content/data/ntm/recipe').glob('*.json'):
            self.assertNotEqual(json.loads(p.read_text())['result']['id'],'ntm:barricade')

    def test_refinery_counter_is_not_a_probability_or_each_cycle_output(self):
        table=self.tables['refinery'];self.assertEqual(table['coverage'],{'source_calls':4,'emitted':4,'skipped':0})
        self.assertIn('One sulfur counter per machine',table['machine_config']['counter'])
        self.assertIn('preserved on recipe/fluid changes',table['machine_config']['counter'])
        first=table['recipes'][0]
        self.assertEqual(first['inputFluids'],[['hotoil',100]])
        self.assertEqual(first['outputFluids'],[['heavyoil',50],['naphtha',25],['lightoil',15],['petroleum',10]])
        self.assertEqual(first['outputItems'],[])
        self.assertEqual(first['periodicOutputItems'][0]['item'],['ntm:sulfur',1])
        for r in table['recipes']:
            self.assertEqual((r['duration'],r['power']),(1,5))
            self.assertEqual(r['periodicOutputItems'][0]['every'],10)
            self.assertEqual(r['periodicOutputItems'][0]['counter'],'sulfur')
            self.assertEqual(sum(f[1] for f in r['outputFluids']),100)
            self.assertNotIn('chance',r['periodicOutputItems'][0])

    def test_all_nineteen_fraction_defaults_and_coal_route(self):
        data=self.tables['fraction'];self.assertEqual(data['coverage'],{'source_calls':19,'emitted':19,'skipped':0})
        self.assertEqual([r['source_index'] for r in data['recipes']],list(range(19)))
        coal=next(r for r in data['recipes'] if r['inputFluids'][0][0]=='coaloil')
        self.assertEqual(coal['outputFluids'],[['coalgas',30],['oil',70]])
        for r in data['recipes']:
            self.assertEqual(r['inputFluids'][0][1],100);self.assertEqual((r['duration'],r['power']),(10,0))
        cfg=data['machine_config'];self.assertEqual(cfg['tank_capacity_mb'],4000);self.assertIn('No power',cfg['energy'])

    def test_mixer_preserves_fifty_source_alternatives(self):
        data=self.tables['mixer'];c=data['coverage']
        self.assertEqual(c['source_calls'],40);self.assertEqual(c['expanded'],50)
        self.assertEqual((c['emitted'],c['skipped']),(50,0))
        self.assertEqual(len({(r['source_index'],r['alternative_index']) for r in data['recipes']+data['skipped']}),50)
        self.assertTrue(all(r['power']==50 for r in data['recipes']))
        self.assertEqual(data['skipped'],[])
        solvent=[r for r in data['recipes'] if r['outputFluids'][0][0]=='solvent']
        self.assertEqual(len(solvent),4)
        self.assertEqual([r['alternative_index'] for r in solvent],[0,1,2,3])
        self.assertEqual({r['inputFluids'][0][0] for r in solvent},{'naphtha','naphtha_crack','naphtha_ds','naphtha_coker'})
        for r in solvent:self.assertEqual(r['inputFluids'][1],['aromatics',500]);self.assertEqual(r['outputFluids'],[['solvent',1000]])
        radio=next(r for r in data['recipes'] if r['outputFluids'][0][0]=='radiosolvent')
        self.assertEqual(radio['inputFluids'],[['reformgas',750],['chlorine',250]])

    def test_mixer_legacy_fish_sunflower_and_tar(self):
        recipes=self.tables['mixer']['recipes']
        fish=next(r for r in recipes if r['outputFluids'][0][0]=='fishoil')
        self.assertEqual(fish['inputItems'][0]['ids'],['minecraft:'+x for x in ['cod','salmon','tropical_fish','pufferfish']])
        flower=next(r for r in recipes if r['outputFluids'][0][0]=='sunfloweroil')
        self.assertEqual(flower['inputItems'],[{'ids':['minecraft:sunflower'],'count':1}])
        tar=next(r for r in recipes if r['outputFluids'][0][0]=='bitumen')
        self.assertEqual(len(tar['inputItems'][0]['ids']),4)

    def test_construction_recipes_and_structures_exist(self):
        assembly=json.loads((ROOT/'content/machine-recipes/assembly.json').read_text())['recipes']
        self.assertEqual(next(r for r in assembly if r['name']=='ass.refinery')['outputItems'],[['ntm:machine_refinery',1]])
        results={json.loads(p.read_text())['result']['id'] for p in (ROOT/'content/data/ntm/recipe').glob('*.json')}
        for name in ['machine_mixer','machine_fraction_tower','steel_scaffold','steel_beam','steel_grate']:
            self.assertIn('ntm:'+name,results)

    def test_graph_and_sources(self):
        fluids={f['string_id'] for f in json.loads((ROOT/'content/fluids.json').read_text())['fluids']}
        for data in self.tables.values():
            self.assertTrue(data['machine_config']['sources'])
            for r in data['recipes']:
                self.assertTrue(r['label']);self.assertTrue(r['label_en']);self.assertIn('.java:',r['source'])
                items=[i for a in r['inputItems'] for i in a['ids']]+[o[0] for o in r['outputItems']]+[o['item'][0] for o in r.get('periodicOutputItems',[])]
                for ident in items:
                    if ident.startswith('ntm:'):self.assertIn(ident,self.rows)
                for fluid,amount in r['inputFluids']+r['outputFluids']:self.assertIn(fluid,fluids);self.assertGreater(amount,0)

    def test_fresh_directory_determinism(self):
        with tempfile.TemporaryDirectory() as path:self.assertEqual(export(SOURCE,Path(path),list(self.rows.values())),self.tables)


if __name__=='__main__':unittest.main()

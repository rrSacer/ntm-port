#!/usr/bin/env python3
"""Preserve oil polling cycles, atomic energy, catalyst and thermal rules."""
import json
import tempfile
import unittest
from pathlib import Path
from petroleum_processing import export
from pyrooven_recipes import fuel_heat

ROOT=Path(__file__).resolve().parents[2];SOURCE=ROOT.parent/'ntm-src'
NAMES=['cracking','reforming','hydrotreating','vacuum_refinery','coker','boiler']
MACHINES=['machine_boiler','machine_industrial_boiler','machine_catalytic_cracker','machine_catalytic_reformer','machine_hydrotreater','machine_vacuum_distill','machine_coker']


class PetroleumC(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((ROOT/'content/ids.json').read_text());cls.rows={r['id']:r for r in cls.catalog['entries']}
        cls.tables={n:json.loads((ROOT/f'content/machine-recipes/{n}.json').read_text()) for n in NAMES}

    def test_machine_contracts(self):
        for name in MACHINES:
            row=self.rows['ntm:'+name];self.assertEqual(row['template'],'minecraft:beehive')
            self.assertIn(row['id'],self.catalog['machines']);self.assertEqual(row['logic']['level'],{'prop':'honey_level','max':5})

    def test_all_defaults_and_order(self):
        for name,count in [('cracking',12),('reforming',9),('hydrotreating',6),('vacuum_refinery',2),('coker',33)]:
            data=self.tables[name]
            self.assertEqual(data['coverage'],{'source_calls':count,'emitted':count,'skipped':0})
            self.assertEqual([r['source_index'] for r in data['recipes']],list(range(count)))

    def test_cracker_is_two_operations_every_five_ticks(self):
        cfg=self.tables['cracking']['machine_config']
        self.assertEqual((cfg['interval_ticks'],cfg['max_operations_per_interval'],cfg['energy_per_operation_he']),(5,2,0))
        for r in self.tables['cracking']['recipes']:
            self.assertEqual(r['inputFluids'][0][1],100);self.assertEqual(r['inputFluids'][1],['steam',200])
            self.assertEqual(r['outputFluids'][-1],['spentsteam',2]);self.assertEqual(r['max_operations_per_interval'],2)
            self.assertTrue(all(f!='none' and amount>0 for f,amount in r['outputFluids']))
        kero=next(r for r in self.tables['cracking']['recipes'] if r['inputFluids'][0][0]=='kerosene')
        self.assertEqual(kero['outputFluids'],[['petroleum',60],['spentsteam',2]])

    def test_atomic_energy_is_not_continuous_or_averaged(self):
        for name,amount,interval in [('reforming',20000,1),('hydrotreating',20000,2),('vacuum_refinery',10000,1)]:
            for r in self.tables[name]['recipes']:
                self.assertEqual(r['energy_per_operation_he'],amount);self.assertEqual(r['interval_ticks'],interval)
                self.assertEqual(r['duration'],interval);self.assertEqual(r['energy_draw'],'instant_on_success');self.assertEqual(r['power'],0)

    def test_catalysts_are_not_consumed(self):
        for name in ['reforming','hydrotreating']:
            for r in self.tables[name]['recipes']:
                self.assertEqual(r['inputItems'],[])
                self.assertEqual(r['catalysts'][0]['item'],'ntm:catalytic_converter')
                self.assertEqual(r['catalysts'][0]['count'],1);self.assertFalse(r['catalysts'][0]['consumed'])
                self.assertEqual(r['catalysts'][0]['slot'],10)

    def test_hydrogen_pressure_and_vacuum(self):
        for r in self.tables['hydrotreating']['recipes']:
            self.assertEqual(r['inputFluidPressures'],[0,1]);self.assertEqual(r['inputFluids'][1][0],'hydrogen')
        first=self.tables['hydrotreating']['recipes'][0]
        self.assertEqual(first['inputFluids'],[['oil',100],['hydrogen',5]])
        self.assertEqual(first['outputFluids'],[['oil_ds',90],['sourgas',15]])
        for r in self.tables['vacuum_refinery']['recipes']:self.assertEqual(r['inputFluidPressures'],[2])

    def test_coker_thermal_progress_and_explicit_outputs(self):
        data=self.tables['coker'];cfg=data['machine_config']
        self.assertEqual((cfg['heat_required_tu'],cfg['heat_capacity_tu'],cfg['diffusion']),(20000,100000,0.25))
        self.assertIn('floor(heat/100)',cfg['progress']);self.assertIn('at most one operation',cfg['progress'])
        for r in data['recipes']:
            self.assertEqual(r['heat_required_tu'],20000);self.assertEqual(r['power'],0);self.assertNotIn('duration',r)
        explicit=next(r for r in data['recipes'] if r['inputFluids'][0][0]=='bitumen')
        self.assertEqual(explicit['inputFluids'],[['bitumen',16000]])
        self.assertEqual(explicit['outputFluids'],[['oil_coker',1600]])
        self.assertEqual(explicit['outputItems'],[['ntm:coke__petroleum',1]])
        wood=next(r for r in data['recipes'] if r['inputFluids'][0][0]=='woodoil')
        self.assertEqual(wood['outputItems'],[['minecraft:charcoal',1]])

    def test_coker_auto_uses_maximum_final_heat_and_combustion(self):
        flame=fuel_heat(SOURCE);heat=fuel_heat(SOURCE,include_combustion=True)
        self.assertTrue(any(heat[n]>flame.get(n,0) for n in heat))
        for r in self.tables['coker']['recipes']:
            if 'source_auto' not in r:continue
            auto=r['source_auto'];var=r['inputFluids'][0][0].upper()
            self.assertEqual(auto['heat_per_bucket'],heat[var]);self.assertEqual(auto['penalty'],1)
            amount=int(auto['tu_per_output']*1000/heat[var])
            if amount>10000:amount-=amount%1000
            elif amount>1000:amount-=amount%100
            elif amount>100:amount-=amount%10
            self.assertEqual(r['inputFluids'][0][1],amount)
            self.assertEqual(r['outputFluids'][0][1],max(10,amount//10))

    def test_boiler_only_first_eligible_step_is_active(self):
        data=self.tables['boiler'];self.assertEqual(len(data['recipes']),6);self.assertEqual(data['skipped'],[])
        water=[s for s in data['heating_steps'] if s['input'][0]=='water']
        self.assertEqual(len(water),4);self.assertEqual([s['active_boiler'] for s in water],[True,False,False,False])
        r=next(r for r in data['recipes'] if r['inputFluids'][0][0]=='water')
        self.assertEqual(r['inputFluids'],[['water',1]]);self.assertEqual(r['outputFluids'],[['steam',100]])
        self.assertEqual(r['heat_required_tu'],200);self.assertNotIn('duration',r)
        oils=[r for r in data['recipes'] if r['inputFluids'][0][0] in ['oil','oil_ds','crackoil','crackoil_ds']]
        self.assertEqual(len(oils),4)
        for r in oils:self.assertEqual(r['heat_required_tu'],10);self.assertEqual(r['inputFluids'][0][1],1);self.assertEqual(r['outputFluids'][0][1],1)
        for name,heat,volume in [('machine_boiler',3200000,16000),('machine_industrial_boiler',12800000,64000)]:
            cfg=data['machine_config']['ntm:'+name];self.assertEqual(cfg['heat_capacity_tu'],heat);self.assertEqual(cfg['input_capacity_mb'],volume);self.assertEqual(cfg['diffusion'],0.1)

    def test_registered_graph_and_source_locations(self):
        fluids={f['string_id'] for f in json.loads((ROOT/'content/fluids.json').read_text())['fluids']}
        for table in self.tables.values():
            for r in table['recipes']:
                self.assertIn('.java:',r['source']);self.assertTrue(r['label']);self.assertTrue(r['label_en'])
                for ident in [i for a in r['inputItems'] for i in a['ids']]+[o[0] for o in r['outputItems']]+[c['item'] for c in r.get('catalysts',[])]:
                    if ident.startswith('ntm:'):self.assertIn(ident,self.rows)
                for f,n in r['inputFluids']+r['outputFluids']:self.assertIn(f,fluids);self.assertGreater(n,0)

    def test_all_seven_construction_recipes_are_available(self):
        assembly=json.loads((ROOT/'content/machine-recipes/assembly.json').read_text())['recipes']
        outputs={o[0] for r in assembly for o in r['outputItems']}
        anvil=json.loads((ROOT/'content/machine-recipes/anvil.json').read_text())['construction']
        outputs.update(o[0] for r in anvil for o in r['out'])
        self.assertTrue({'ntm:'+n for n in MACHINES}<=outputs)

    def test_fresh_directory_determinism(self):
        with tempfile.TemporaryDirectory() as path:self.assertEqual(export(SOURCE,Path(path),list(self.rows.values())),self.tables)


if __name__=='__main__':unittest.main()

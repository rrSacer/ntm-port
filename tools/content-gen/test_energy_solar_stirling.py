"""Solar ratios, targeting boundaries and real Stirling source semantics."""
import json
import tempfile
import unittest
from pathlib import Path
import solar_stirling

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'
MACHINES = ['machine_solar_boiler','solar_mirror','machine_stirling','machine_stirling_steel','machine_stirling_creative']


class SolarStirling(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((ROOT/'content/ids.json').read_text())
        cls.rows={r['id']:r for r in cls.catalog['entries']}
        cls.data=json.loads((ROOT/'content/machine-recipes/solar_stirling.json').read_text())
        cls.cfg=cls.data['machine_config']

    def test_five_machines_have_supported_states_names_loot(self):
        for name in MACHINES:
            ident='ntm:'+name; row=self.rows[ident]
            self.assertIn(ident,self.catalog['machines'])
            self.assertEqual(row['template'],'minecraft:beehive')
            self.assertEqual(set(row['names']),{'en_us','pt_br'})
            self.assertEqual(row['logic']['level'],{'prop':'honey_level','max':5})
            loot=json.loads((ROOT/f'content/data/ntm/loot_table/blocks/{name}.json').read_text())
            self.assertIn(ident,json.dumps(loot))

    def test_original_builds_resolve_and_creative_has_no_recipe(self):
        recipes=[json.loads(p.read_text()) for p in (ROOT/'content/data/ntm/recipe').glob('*.json')]
        for name,count in [('machine_solar_boiler',1),('solar_mirror',3)]:
            found=[r for r in recipes if r.get('result',{}).get('id')=='ntm:'+name]
            self.assertEqual(len(found),1); self.assertEqual(found[0]['result'].get('count',1),count)
        builds=json.loads((ROOT/'content/machine-recipes/anvil.json').read_text())['construction']
        for name,gear in [('machine_stirling','iron'),('machine_stirling_steel','steel')]:
            found=[r for r in builds if r['out']==[['ntm:'+name,1,1.0]]]
            self.assertEqual(len(found),1)
            self.assertEqual(found[0]['tier_lower'],2)
            self.assertIn({'ids':['ntm:gear_large__'+gear],'count':1},found[0]['in'])
        self.assertFalse([r for r in recipes if r.get('result',{}).get('id')=='ntm:machine_stirling_creative'])
        self.assertFalse([r for r in builds if any(i[0]=='ntm:machine_stirling_creative' for i in r['out'])])

    def test_solar_boiler_keeps_ratio_limits_and_discarded_remainder(self):
        boiler=self.cfg['ntm:machine_solar_boiler']
        self.assertEqual(boiler['tank_capacities_mb'],{'water':100,'steam':10000})
        self.assertEqual((boiler['heat_per_water_mb'],boiler['steam_per_water_mb']),(50,100))
        self.assertIn('BEFORE capacity limits',boiler['operation'])
        self.assertIn('Reset heat=0',boiler['operation'])

    def test_mirror_target_is_boiler_core_plus_one_and_tool_checks_boundaries(self):
        mirror=self.cfg['ntm:solar_mirror']
        self.assertEqual(mirror['sky_light_subtraction'],11)
        self.assertEqual(mirror['target_boiler_offset'],[0,-1,0])
        self.assertEqual(mirror['targeting']['max_distance_blocks'],100)
        self.assertIn('<=',mirror['targeting']['angle_condition'])
        self.assertIn('isOn can be true without',mirror['operation'])

    def test_stirling_creative_needs_heat_and_base_uses_literal_heat_limit(self):
        for name,limit,eff,creative in [('machine_stirling',300,.5,False),('machine_stirling_steel',1500,.5,False),('machine_stirling_creative',1500,1.,True)]:
            cfg=self.cfg['ntm:'+name]
            self.assertEqual((cfg['max_heat_tu_per_tick'],cfg['diffusion'],cfg['efficiency']),(limit,.1,eff))
            self.assertEqual(cfg['overspeed']['enabled'],not creative)
            self.assertFalse(cfg['generates_without_heat'])
            self.assertEqual(cfg['overspeed']['limit_ticks'],300)
            self.assertIn('provide previous powerBuffer first',cfg['tick_order'])
            self.assertIn('hardcoded 300/1500',cfg['source_config_gap'])
        self.assertIsNone(self.cfg['ntm:machine_stirling_creative']['repair_item'])

    def test_snapshot_regenerates_without_fictitious_crafter_recipes(self):
        self.assertEqual(self.data['recipes'],[]); self.assertEqual(self.data['skipped'],[])
        self.assertEqual(self.data['coverage'],{'machines':5,'skipped':0})
        for cfg in self.cfg.values():
            self.assertNotIn('duration',cfg); self.assertNotIn('power',cfg)
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(solar_stirling.export(SOURCE,Path(directory),list(self.rows.values())),self.data)


if __name__=='__main__':unittest.main()

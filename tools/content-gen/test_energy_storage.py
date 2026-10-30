"""Battery capacity arithmetic and the cable/endpoint split agreed for L3."""
import json
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]


class EnergyStorage(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows={r['id']:r for r in json.loads((ROOT/'content/ids.json').read_text())['entries']}
        cls.data=json.loads((ROOT/'content/machine-recipes/energy_storage.json').read_text())

    def test_twelve_pack_metadatas_keep_the_original_rates(self):
        packs={p['member']:p for p in self.data['battery_packs']}
        self.assertEqual(len(packs),12)
        self.assertEqual({p['metadata'] for p in packs.values()},set(range(12)))
        for p in packs.values():
            row=self.rows[p['id']]
            self.assertEqual(row['max_stack_size'],1)
            self.assertEqual(row['source_enum'],'EnumBatteryPack.'+p['member'])
            self.assertTrue(row['source_texture'].endswith('/battery_creative_new.png'))
        self.assertEqual(packs['BATTERY_REDSTONE']['capacity_he'],1800000)
        self.assertEqual(packs['BATTERY_QUANTUM']['capacity_he'],72000000000)
        self.assertEqual(packs['BATTERY_QUANTUM']['duration_ticks'],72000)
        self.assertEqual(packs['CAPACITOR_SPARK']['capacity_he'],6000000000)
        for p in packs.values():
            self.assertEqual(p['charge_he_per_tick'],p['discharge_he_per_tick']*(1 if p['capacitor'] else 10))

    def test_simple_cables_and_stateful_endpoints_are_distinct(self):
        net=self.data['network']
        self.assertEqual(len(net['cables']),4)
        for ident in net['cables']:
            self.assertEqual(self.rows[ident]['category'],'cable')
            self.assertEqual(self.rows[ident]['template'],'minecraft:stone')
        stateful=['ntm:machine_battery_socket','ntm:machine_battery_redd',*net['wire_endpoints'],*net['controls']]
        self.assertEqual(len(stateful),13)
        for ident in stateful:
            self.assertEqual(self.rows[ident]['template'],'minecraft:beehive')
            self.assertEqual(self.rows[ident]['logic']['facing'],'facing')
        self.assertNotIn('ntm:cable_diode',self.rows)

    def test_wire_limits_are_endpoint_minimum_and_compatible_groups(self):
        endpoints=self.data['network']['wire_endpoints']
        for name,limit,group in [('red_connector',10,'SINGLE'),('red_connector_super',100,'SINGLE'),
                                 ('red_pylon',25,'SINGLE'),('red_pylon_medium_steel',45,'TRIPLE'),('red_pylon_large',100,'QUAD')]:
            endpoint=endpoints['ntm:'+name]
            self.assertEqual((endpoint['max_wire_length_blocks'],endpoint['connection_type']),(limit,group))
        self.assertIn('<= min(',self.data['network']['link_validation'])
        self.assertIn('does not measure energy',self.data['network']['detector']['operation'])

    def test_redd_is_unbounded_storage_with_a_bounded_network_window(self):
        cfg=self.data['machine_config']['ntm:machine_battery_redd']
        self.assertIn('Unbounded BigInteger',cfg['storage'])
        self.assertEqual(cfg['network_window_he'],92233720368547758)
        self.assertEqual(cfg['provision_window_he'],46116860184273879)
        self.assertNotIn('power_max_he',cfg)
        net=self.data['network']['distribution']
        self.assertEqual(net['priorities'],['LOWEST','LOW','NORMAL','HIGH','HIGHEST'])
        self.assertEqual(net['rounding_iterations'],100)
        self.assertIn('cumulative energyUsed',net['source_quirk'])
        for p in (ROOT/'content/data/ntm/recipe').glob('*.json'):
            self.assertNotEqual(json.loads(p.read_text()).get('result',{}).get('id'),'ntm:machine_battery_redd')

    def test_all_packs_have_source_progression_and_quantum_returns_coolant(self):
        products=set()
        for name in ['assembly','chemical_plant']:
            recipes=json.loads((ROOT/f'content/machine-recipes/{name}.json').read_text())['recipes']
            for recipe in recipes:
                products.update(item for item,_ in recipe['outputItems'])
                if recipe['name']=='chem.batteryquantum':
                    self.assertEqual(recipe['inputFluids'],[['perfluoromethyl_cold',8000]])
                    self.assertEqual(recipe['outputFluids'],[['perfluoromethyl',8000]])
        for path in (ROOT/'content/data/ntm/recipe').glob('*.json'):
            products.add(json.loads(path.read_text()).get('result',{}).get('id'))
        self.assertTrue({p['id'] for p in self.data['battery_packs']}<=products)


if __name__=='__main__':unittest.main()

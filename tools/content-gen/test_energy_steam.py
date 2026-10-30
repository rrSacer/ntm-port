"""The steam cycle preserves source throughput, energy and construction costs."""
import json
import tempfile
import unittest
from pathlib import Path
from machine_recipes import MachineResolver
from bulk_recipes import Unsupported
import steam

ROOT = Path(__file__).resolve().parents[2]; SOURCE = ROOT.parent / 'ntm-src'
MACHINES = ['machine_steam_engine', 'machine_industrial_turbine', 'machine_chungus', 'machine_condenser', 'machine_condenser_powered']


class EnergySteam(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text()); cls.rows = {r['id']: r for r in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/steam.json').read_text()); cls.cfg = cls.data['machine_config']
        cls.resolver = MachineResolver(SOURCE, list(cls.rows.values()))

    def test_five_supported_machine_states_and_no_deprecated_turbine(self):
        for name in MACHINES:
            row = self.rows['ntm:' + name]
            self.assertIn(row['id'], self.catalog['machines'])
            self.assertEqual(row['template'], 'minecraft:beehive')
            self.assertEqual(row['logic']['level'], {'prop': 'honey_level', 'max': 5})
        self.assertNotIn('ntm:machine_large_turbine', self.rows)

    def test_gears_keep_metadata_texture_and_original_crafting(self):
        recipes = [json.loads(p.read_text()) for p in (ROOT / 'content/data/ntm/recipe').glob('*.json')]
        for meta, material in enumerate(['iron', 'steel']):
            row = self.rows['ntm:gear_large__' + material]
            self.assertEqual(row['metadata'], meta); self.assertEqual(row['source_variable'], 'gear_large')
            self.assertTrue(row['source_texture'].endswith('/gear_large.png'))
            matches = [r for r in recipes if r.get('result', {}).get('id') == row['id']]
            self.assertEqual(len(matches), 1)
            self.assertEqual(matches[0]['type'], 'minecraft:crafting_shaped')
        self.assertNotIn('ntm:gear_large__wood', self.rows)

    def test_original_construction_stations_and_lubricant_alternatives(self):
        anvil = json.loads((ROOT / 'content/machine-recipes/anvil.json').read_text())['construction']
        row = next(r for r in anvil if any(o[0] == 'ntm:machine_steam_engine' for o in r['out']))
        self.assertEqual(row['tier_lower'], 2)
        self.assertIn('ntm:gear_large__iron', {i for a in row['in'] for i in a['ids']})
        assembly = json.loads((ROOT / 'content/machine-recipes/assembly.json').read_text())['recipes']
        for name in ['ass.iturbine', 'ass.leviturbine', 'ass.hpcondenser']:
            self.assertEqual(len([r for r in assembly if r['name'] == name]), 1)
        condenser = next(r for r in assembly if r['name'] == 'ass.hpcondenser')
        fluid_input = next(i for i in condenser['inputItems'] if 'ntm:canister_full__lubricant' in i['ids'])
        self.assertEqual(fluid_input['count'], 4)
        self.assertEqual(set(fluid_input['ids']), {'ntm:canister_full__lubricant', 'ntm:fluid_tank_full__lubricant', 'ntm:fluid_tank_lead_full__lubricant'})
        self.assertNotIn('ntm:fluid_barrel_full__lubricant', fluid_input['ids'])
        with self.assertRaises(Unsupported): self.resolver.input('new OreDictStack(Fluids.LUBRICANT.getDict(999), 4)')

    def test_steam_ratios_and_generator_units(self):
        ops = {r['name']: r for r in self.data['turbine_operations']}
        self.assertEqual(len(ops), 4)
        self.assertEqual(ops['turbine.steam']['inputFluids'], [['steam', 100]])
        self.assertEqual(ops['turbine.steam']['outputFluids'], [['spentsteam', 1]])
        self.assertEqual(ops['turbine.steam']['energy_he_per_operation_at_machine_efficiency_1'], 200)
        self.assertEqual(ops['turbine.ultrahotsteam']['energy_he_per_operation_at_machine_efficiency_1'], 120)
        small = self.cfg['ntm:machine_steam_engine']
        self.assertEqual((small['input_capacity_mb'], small['output_capacity_mb'], small['efficiency']), (2000, 20, .85))
        self.assertIn('steam only', small['input_policy'])

    def test_industrial_flywheel_and_resizing_are_distinct_from_chungus(self):
        industrial = self.cfg['ntm:machine_industrial_turbine']; chungus = self.cfg['ntm:machine_chungus']
        self.assertEqual((industrial['input_capacity_mb'], industrial['output_capacity_mb']), (750000, 3000000))
        self.assertEqual(industrial['consumption_fraction'], .2); self.assertEqual(industrial['flywheel_scale_he'], 50000000)
        self.assertTrue(industrial['resizes_on_lever']); self.assertIn('No explicit spin cap', industrial['flywheel_tick'])
        self.assertEqual(chungus['consumption_fraction'], 1); self.assertFalse(chungus['resizes_on_lever'])
        self.assertEqual(chungus['input_capacity_mb'], 1000000000)
        self.assertNotIn('flywheel_scale_he', chungus)
        self.assertIn('int32', chungus['java_integer_semantics'])

    def test_condenser_preserves_the_powered_all_or_nothing_tolerance(self):
        simple = self.cfg['ntm:machine_condenser']; powered = self.cfg['ntm:machine_condenser_powered']
        self.assertEqual(simple['input_capacity_mb'], 100)
        self.assertEqual(simple['energy_consumption_he_per_mb'], 0)
        self.assertEqual((powered['input_capacity_mb'], powered['energy_capacity_he'], powered['energy_consumption_he_per_mb']), (1000000, 10000000, 10))
        self.assertIn('*0.95', powered['operations']); self.assertIn('not limited by affordable', powered['operations'])
        self.assertEqual(self.data['condensation'], {'inputFluids': [['spentsteam', 1]], 'outputFluids': [['water', 1]]})

    def test_no_fictitious_generic_duration_or_consumption(self):
        self.assertEqual(self.data['recipes'], []); self.assertEqual(self.data['skipped'], [])
        for op in self.data['turbine_operations']:
            self.assertNotIn('power', op); self.assertNotIn('duration', op)
            self.assertRegex(op['source'], r'Fluids.java:\d+$')

    def test_independent_regeneration(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(steam.export(SOURCE, Path(directory), list(self.rows.values())), self.data)


if __name__ == '__main__': unittest.main()

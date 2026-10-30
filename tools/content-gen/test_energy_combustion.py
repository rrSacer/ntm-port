"""Fuel generators keep the grade tables, the piston matrix and the original builds."""
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path
import combustion

ROOT = Path(__file__).resolve().parents[2]; SOURCE = ROOT.parent / 'ntm-src'
MACHINES = ['machine_diesel', 'machine_combustion_engine', 'machine_turbofan', 'machine_turbinegas']
PISTONS = ['steel', 'dura', 'desh', 'starmetal']
BUILDS = {'ass.dieselgen': 'machine_diesel', 'ass.combustiongen': 'machine_combustion_engine',
          'ass.turbofan': 'machine_turbofan', 'ass.gasturbine': 'machine_turbinegas',
          **{'ass.pistonset' + k: 'piston_set__' + v for k, v in zip(['steel', 'dura', 'desh', 'star'], PISTONS)}}


class EnergyCombustion(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text()); cls.rows = {r['id']: r for r in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/combustion.json').read_text()); cls.cfg = cls.data['machine_config']
        cls.traits = json.loads((ROOT / 'content/fluid_traits.json').read_text())
        cls.assembly = json.loads((ROOT / 'content/machine-recipes/assembly.json').read_text())

    def test_four_machines_are_supported_states(self):
        for name in MACHINES:
            row = self.rows['ntm:' + name]
            self.assertIn(row['id'], self.catalog['machines'])
            self.assertEqual(row['template'], 'minecraft:beehive')
            self.assertEqual(row['logic']['level'], {'prop': 'honey_level', 'max': 5})

    def test_pistons_keep_enum_metadata_texture_and_stack(self):
        for meta, suffix in enumerate(PISTONS):
            row = self.rows['ntm:piston_set__' + suffix]
            self.assertEqual((row['metadata'], row['source_enum'], row['max_stack_size']), (meta, 'EnumPistonType.' + suffix.upper(), 1))
            self.assertTrue(row['source_texture'].endswith(f'/piston_set_{suffix}.png'))
            self.assertEqual(set(row['names']), {'en_us', 'pt_br'})

    def test_starmetal_ingot_is_the_original_item_with_only_source_recipes(self):
        self.assertIn('ntm:ingot_starmetal', self.rows)
        self.assertEqual([i for i in self.rows if i.startswith('ntm:ingot_starmetal')], ['ntm:ingot_starmetal'])
        producers = []
        for path in (ROOT / 'content/data/ntm/recipe').glob('*.json'):
            recipe = json.loads(path.read_text())
            if recipe.get('result', {}).get('id') == 'ntm:ingot_starmetal': producers.append(recipe['type'].split(':')[-1])
        # Crystal smelting/blasting appear twice, as for every other crystal: the bulk crafting-manager and the crystal rule.
        self.assertEqual(sorted(producers), ['blasting', 'blasting', 'crafting_shaped', 'smelting', 'smelting'])
        star = next(r for r in self.assembly['recipes'] if r['name'] == 'ass.pistonsetstar')
        self.assertIn({'ids': ['ntm:ingot_starmetal'], 'count': 24}, star['inputItems'])

    def test_every_original_assembly_build_is_emitted_not_skipped(self):
        emitted = {r['name']: r for r in self.assembly['recipes']}
        for name, output in BUILDS.items():
            self.assertEqual(emitted[name]['outputItems'], [['ntm:' + output, 1]])
        self.assertFalse([r for r in self.assembly.get('skipped', []) if r.get('name') in BUILDS])

    def test_fuel_grade_table_is_separate_from_flammable_units(self):
        fuels = self.traits['combustible']
        self.assertEqual(self.traits['fuel_grades'], ['LOW', 'MEDIUM', 'HIGH', 'AERO', 'GAS'])
        self.assertEqual(Counter(f['grade'] for f in fuels), {'HIGH': 16, 'LOW': 6, 'MEDIUM': 12, 'AERO': 2, 'GAS': 7})
        self.assertEqual(self.traits['coverage']['combustible'], 43); self.assertEqual(self.traits['deferred_traits'], [])
        by = {f['fluid']: f for f in fuels}
        flammable = {f['fluid']: f for f in self.traits['flammable']}
        self.assertEqual(by['diesel']['he_per_bucket'], 1370000)
        self.assertNotEqual(by['diesel']['he_per_bucket'], flammable['diesel']['heat_tu_per_bucket'])
        self.assertEqual((by['kerosene']['grade'], by['gas']['grade'], by['heatingoil']['grade']), ('AERO', 'GAS', 'LOW'))
        for fuel in fuels:
            self.assertEqual(fuel['grade_ordinal'], self.traits['fuel_grades'].index(fuel['grade']))
            self.assertRegex(fuel['source'], r'Fluids.java:\d+$')

    def test_diesel_generator_excludes_low_and_divides_before_multiplying(self):
        diesel = self.cfg['ntm:machine_diesel']
        self.assertEqual((diesel['power_capacity_he'], diesel['fuel_capacity_mb'], diesel['burn_mb_per_tick']), (50000, 16000, 1))
        self.assertEqual(diesel['efficiency_by_grade'], {'MEDIUM': .5, 'HIGH': .75, 'AERO': .1})
        self.assertEqual(diesel['zero_efficiency_grades'], ['LOW', 'GAS'])
        rates = {r['fluid']: r['he_per_mb'] for r in diesel['derived_he_per_mb']}
        self.assertEqual(rates['diesel'], int(1370000 // 1000 * .75)); self.assertEqual(rates['kerosene'], int(3850000 // 1000 * .1))
        self.assertNotIn('heatingoil', rates); self.assertNotIn('gas', rates)

    def test_piston_matrix_matches_the_source_rows(self):
        engine = self.cfg['ntm:machine_combustion_engine']; matrix = engine['piston_efficiency']
        self.assertEqual(matrix['STEEL'], {'LOW': 1.0, 'MEDIUM': .75, 'HIGH': .25, 'AERO': 0.0, 'GAS': 0.0})
        self.assertEqual(matrix['STARMETAL'], {'LOW': .5, 'MEDIUM': .75, 'HIGH': 1.0, 'AERO': .9, 'GAS': .5})
        self.assertEqual(matrix['DURA']['LOW'], .5); self.assertEqual(matrix['DESH']['LOW'], 0.0)
        self.assertEqual(engine['piston_metadata'], {'STEEL': 0, 'DURA': 1, 'DESH': 2, 'STARMETAL': 3})
        self.assertEqual((engine['power_capacity_he'], engine['tank_capacity_mb'], engine['setting_range_gui']), (2500000, 24000, [0, 30]))
        self.assertIn('tenths', engine['operation']); self.assertIn('compound assignment', engine['operation'])

    def test_turbofan_and_gas_turbine_keep_their_grades_and_tables(self):
        fan = self.cfg['ntm:machine_turbofan']; gas = self.cfg['ntm:machine_turbinegas']
        self.assertEqual((fan['accepted_grade'], fan['power_capacity_he'], fan['fuel_capacity_mb'], fan['upgrade']['max_level']), ('AERO', 1000000, 24000, 3))
        self.assertEqual(fan['upgrade']['override_level'], 100); self.assertIn('int32', fan['java_integer_semantics'])
        self.assertEqual(gas['accepted_grade'], 'GAS')
        self.assertEqual(gas['max_consumption_mb_per_tick'], {'gas': 50.0, 'syngas': 10.0, 'oxyhydrogen': 100.0, 'reformgas': 5.0})
        self.assertEqual(gas['tank_capacities_mb'], {'gas': 100000, 'lubricant': 16000, 'water': 16000, 'hotsteam': 160000})
        self.assertEqual(gas['default_consumption_mb_per_tick'], 5)

    def test_no_fictitious_generic_duration_or_power(self):
        self.assertEqual(self.data['recipes'], []); self.assertEqual(self.data['skipped'], [])
        self.assertEqual(self.data['coverage'], {'machines': 4, 'pistons': 4, 'combustible_fluids': 43, 'skipped': 0})
        for config in self.cfg.values():
            self.assertNotIn('duration', config); self.assertRegex(config['source'], r'TileEntity\w+\.java$')
        self.assertNotIn('ntm:machine_industrial_generator', self.rows)

    def test_independent_regeneration(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(combustion.export(SOURCE, Path(directory), list(self.rows.values())), self.data)
            self.assertEqual(json.loads((Path(directory) / 'fluid_traits.json').read_text()), self.traits)


if __name__ == '__main__': unittest.main()

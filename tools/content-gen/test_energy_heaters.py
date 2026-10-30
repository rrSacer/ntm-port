"""Heat rules preserve units, rounding and the source construction station."""
import json
import tempfile
import unittest
from pathlib import Path
import fluid_traits
import heaters

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'
MACHINES = ['heater_oven', 'heater_oilburner', 'heater_electric', 'heater_heatex']


class EnergyHeaters(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {r['id']: r for r in cls.catalog['entries']}
        cls.traits = json.loads((ROOT / 'content/fluid_traits.json').read_text())
        cls.data = json.loads((ROOT / 'content/machine-recipes/heaters.json').read_text())

    def test_blocks_are_registered_machines_with_supported_states(self):
        for name in MACHINES:
            ident = 'ntm:' + name; row = self.rows[ident]
            self.assertIn(ident, self.catalog['machines'])
            self.assertEqual(row['template'], 'minecraft:beehive')
            self.assertEqual(row['logic']['level'], {'prop': 'honey_level', 'max': 5})
            declaration = json.loads((ROOT / f'content/data/ntm/horizon/block/{name}.json').read_text())
            self.assertEqual(declaration['template'], row['template'])
            for loc in ['pt_br', 'en_us']:
                lang = json.loads((ROOT / f'content/assets/ntm/lang/{loc}.json').read_text())
                self.assertTrue(lang['block.ntm.' + name])

    def test_all_four_original_constructions_remain_on_the_anvil(self):
        construction = json.loads((ROOT / 'content/machine-recipes/anvil.json').read_text())['construction']
        for name, tier in zip(MACHINES, [2, 2, 3, 3]):
            matches = [r for r in construction if any(o[0] == 'ntm:' + name for o in r['out'])]
            self.assertEqual(len(matches), 1, name)
            self.assertEqual(matches[0]['tier_lower'], tier)
        for path in (ROOT / 'content/data/ntm/recipe').glob('*.json'):
            self.assertNotIn(json.loads(path.read_text()).get('result', {}).get('id'), ['ntm:' + n for n in MACHINES])

    def test_oven_fuel_timing_and_heat_do_not_use_the_blast_filter(self):
        cfg = self.data['machine_config']['ntm:heater_oven']
        self.assertEqual((cfg['base_heat_tu'], cfg['time_multiplier'], cfg['heat_pull_efficiency']), (500, .125, .5))
        coal = self.data['oven_fuels']['minecraft:coal']
        self.assertEqual((coal['burn_ticks'], coal['heat_tu_per_tick']), (250, 1000))
        lava = self.data['oven_fuels']['minecraft:lava_bucket']
        self.assertEqual(lava['container'], 'minecraft:bucket')
        self.assertEqual((lava['burn_ticks'], lava['heat_tu_per_tick']), (2500, 500))
        self.assertTrue(cfg['fuel_profile_gaps'])
        self.assertIn('ignition', cfg['burning'])

    def test_flammable_uses_final_values_and_final_sources(self):
        final = {f['fluid']: f for f in self.traits['flammable']}
        flare = json.loads((ROOT / 'content/machine-recipes/flare.json').read_text())['fuel_traits']
        self.assertEqual(len(final), 60)
        self.assertEqual({f['fluid']: f['heat_tu_per_bucket'] for f in flare if f['flammable']},
                         {f: t['heat_tu_per_bucket'] for f, t in final.items()})
        for name in ['diesel', 'kerosene', 'balefire']:
            line = int(final[name]['source'].rsplit(':', 1)[1])
            self.assertIn('registerCalculatedFuel', (SOURCE / final[name]['source'].rsplit(':', 1)[0]).read_text().splitlines()[line - 1])

    def test_cooling_keeps_exact_ratios_and_two_efficiencies(self):
        table = {t['fluid']: t for t in self.traits['coolable']}
        self.assertEqual(len(table), 16)
        steam = table['steam']
        self.assertEqual((steam['input_mb'], steam['output'], steam['heat_tu']), (100, ['spentsteam', 1], 200))
        self.assertEqual(steam['efficiencies'], {'TURBINE': 1.0, 'HEATEXCHANGER': .5})
        exchange = {r['name']: r for r in self.data['heat_exchange']}
        self.assertEqual(exchange['heatex.steam']['heat_produced_tu_per_operation'], 100)
        self.assertEqual(exchange['heatex.hotsteam']['heat_produced_tu_per_operation'], 1)
        self.assertNotIn('water', table)
        cfg = self.data['machine_config']['ntm:heater_heatex']
        self.assertEqual(cfg['default_max_operations_per_conversion'], 24000)
        self.assertIn('OPERATIONS, not mB', cfg['conversion'])

    def test_no_generic_power_or_duration_is_invented(self):
        self.assertEqual(self.data['recipes'], [])
        self.assertEqual(self.data['skipped'], [])
        for row in self.data['heat_exchange']:
            self.assertNotIn('power', row)
            self.assertNotIn('duration', row)
        electric = self.data['machine_config']['ntm:heater_electric']
        self.assertIsNone(electric['heat_capacity_tu'])
        self.assertEqual(electric['max_setting'], 10)
        self.assertEqual(electric['consumption_he_per_tick'], 'floor(pow(setting,1.4)*200)')
        oil = self.data['machine_config']['ntm:heater_oilburner']
        self.assertEqual((oil['tank_capacity_mb'], oil['max_burn_mb_per_tick']), (16000, 10))
        self.assertIn('BEFORE multiplication', oil['conversion'])

    def test_references_only_known_fluids_and_provenance(self):
        known = {f['string_id'] for f in json.loads((ROOT / 'content/fluids.json').read_text())['fluids']}
        for t in self.traits['flammable'] + self.traits['coolable']:
            self.assertIn(t['fluid'], known)
            self.assertRegex(t['source'], r'Fluids.java:\d+$')
            if 'output' in t: self.assertIn(t['output'][0], known)

    def test_tables_regenerate_independently_of_directory_and_hash_seed(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory); (out / 'machine-recipes').mkdir()
            # Reuse the existing explicit FuelHandler snapshot, exactly as the pipeline does.
            (out / 'machine-recipes/foundry.json').write_bytes((ROOT / 'content/machine-recipes/foundry.json').read_bytes())
            self.assertEqual(fluid_traits.export(SOURCE, out), self.traits)
            self.assertEqual(heaters.export(SOURCE, out, list(self.rows.values())), self.data)


if __name__ == '__main__': unittest.main()

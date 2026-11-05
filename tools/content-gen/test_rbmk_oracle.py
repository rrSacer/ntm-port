"""T00: replay real Java, verify provenance, numerical boundaries and catalog conservation."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

import reactors_rbmk

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT.parent / 'ntm-src').resolve()
ORACLE = ROOT / 'tools/rbmk-oracle'


class ExecutableRbmkOracle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.checked = reactors_rbmk.load_oracle(SOURCE)
        scratch = Path(os.environ.get('RBMK_ORACLE_SCRATCH', str(SOURCE.parent / '.agent-tmp/rbmk')))
        scratch.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(prefix='java-test-', dir=scratch)
        cls.addClassCleanup(cls.temp.cleanup)
        # This compiles and calls HBM methods; it does not evaluate formulas in Python.
        cls.generated = Path(cls.temp.name) / 'observed.json'
        subprocess.run([sys.executable, str(ORACLE / 'run.py'), '--source', str(SOURCE),
                        '--scratch', str(Path(cls.temp.name) / 'compile'),
                        '--output', str(cls.generated)], check=True, capture_output=True, text=True)
        cls.live = json.loads(cls.generated.read_text())
        with tempfile.TemporaryDirectory() as folder:
            cls.exported = reactors_rbmk.export(SOURCE, Path(folder),
                json.loads((ROOT / 'content/ids.json').read_text())['entries'])

    def test_fixture_replays_exactly(self):
        self.assertEqual(self.live, self.checked)
        self.assertEqual(self.generated.read_bytes(), (ORACLE / 'fixtures/java.json').read_bytes())

    def test_whole_classes_are_byte_identical_to_pin(self):
        for relative in self.live['provenance']['whole_classes']:
            copied = Path(self.temp.name) / 'compile/src/com/hbm' / relative
            original = SOURCE / 'src/main/java/com/hbm' / relative
            self.assertEqual(copied.read_bytes(), original.read_bytes(), relative)
        self.assertEqual(self.live['provenance']['external_jars'], [])

    def test_discriminants_are_observed_not_comment_equations(self):
        probes = {p['name']: p for p in self.live['discriminants']}
        self.assertEqual(probes['log_ten_9_20']['result'], {'value': 10.0, 'bits': '4024000000000000'})
        xenon = probes['xenon_uses_old_poison']['stack']['nbt']
        self.assertEqual(xenon, {'core': 21.0, 'hull': 20.0, 'xenon': 50.5, 'yield': 995.0})
        self.assertEqual(len(self.live['functions']['burn']), 360)
        self.assertEqual(len(self.live['functions']['depletion']), 35)
        # A negative square-root input must remain visible as Java NaN evidence.
        self.assertTrue(any(p['result']['value'] == 'NaN' for p in self.live['functions']['burn']))

    def test_all_rod_defaults_and_setters(self):
        self.assertEqual(len(self.live['rods']), 33)
        default = self.live['defaults']['rod']
        for k, value in {'xenon_generation': .5, 'xenon_burn_divisor': 50,
                         'diffusion': .02, 'heat_per_flux': 1, 'melting_point': 1000,
                         'self_rate': 0, 'yield': 0, 'max_stack_size': 1}.items():
            self.assertEqual(default[k], value, k)
        declared = set(re.findall(r'public ItemRBMKRod (set\w+)\(',
            (SOURCE / 'src/main/java/com/hbm/items/machine/ItemRBMKRod.java').read_text()))
        observed = {p['method'].split('(')[0] for p in self.live['defaults']['setters']}
        self.assertEqual(declared, observed)
        self.assertTrue(all(p['same_instance'] for p in self.live['defaults']['setters']))
        for rod in self.live['rods']:
            self.assertEqual(rod['fresh_stack']['count'], 1)
            self.assertEqual(rod['fresh_stack']['nbt'], {'core': 20.0, 'hull': 20.0, 'yield': rod['yield']})
            self.assertTrue(rod['fresh_tooltip_keys'])
            self.assertEqual(len(rod['burn_heat_trace']), 4)

    def test_nbt_missing_compound_is_different_from_missing_key(self):
        defaults = self.live['defaults']
        self.assertEqual(defaults['nbt_absent_after_get']['nbt']['yield'], 100000000)
        self.assertEqual(defaults['nbt_existing_empty_reads'], {'yield': 0, 'xenon': 0, 'core': 0, 'hull': 0})
        self.assertEqual(defaults['nbt_preserved_by_setter']['nbt']['foreign'], 123)
        self.assertEqual(set(self.exported['rod_nbt']), {'yield', 'xenon', 'core', 'hull'})
        self.assertNotIn('enrichment', self.exported['rod_nbt'])

    def test_dial_effective_values_and_source_quirks(self):
        dials = {d['key']: d for d in self.live['dials']}
        self.assertEqual(len(dials), 25)
        height = dials['KEY_COLUMN_HEIGHT']
        self.assertEqual((height['default'], height['cached_effective']), (4, 3))
        values = {p['input']: p['effective'] for p in height['probes']}
        self.assertEqual((values['-5'], values['1000'], values['bad']), (1, 15, 3))
        special = self.live['dial_special_cases']
        self.assertIn({'name': 'client_surge_reads_passive_cooling', 'server': 7, 'client': 3}, special)
        self.assertIn({'name': 'create_force_reset_cache_before_write', 'raw': '4', 'cached': 15}, special)
        self.assertIn({'name': 'after_refresh', 'cached': 3}, special)

    def test_actual_recipe_registrations_and_missing_states(self):
        recipes = self.live['recipes']
        self.assertEqual(len(recipes['silex_registered']), 295)
        pellet_rows = [r for r in recipes['silex_registered']
                       if r['input'].get('stack', {}).get('item', '').startswith('rbmk_pellet_')]
        self.assertEqual(len(pellet_rows), 255)
        self.assertEqual(len({r['input']['stack']['item'] for r in pellet_rows}), 28)
        self.assertEqual(len(recipes['outgasser_defaults']), 19)
        self.assertEqual(len(recipes['silex_tiny_lookup']), 24)
        missing = {r['pellet']: r['metadata'] for r in recipes['silex_missing_pellet_states']}
        for family in ('lea', 'mea', 'hea241', 'hea242'):
            self.assertEqual(missing['rbmk_pellet_' + family], list(range(10)))
        drx = next(r['recipe'] for r in pellet_rows if r['input']['stack'] ==
                   {'item': 'rbmk_pellet_drx', 'count': 1, 'meta': 0, 'nbt': None})
        self.assertEqual([r['stack']['item'] for r in drx['outputs']], ['undefined'] * 6)

    def test_export_is_observed_data_and_executable_source(self):
        for rod in self.exported['rods']:
            observed = next(r for r in self.live['rods'] if r['variable'] == rod['variable'])
            self.assertTrue(all(observed[k] == v for k, v in rod.items() if k != 'pellet_item'))
        for exported, observed in (('silex_recipes', 'silex_registered'), ('outgasser_recipes', 'outgasser_defaults')):
            self.assertEqual(self.exported[exported], self.live['recipes'][observed])
        self.assertEqual(self.exported['burn_formulas']['LOG_TEN'], 'Math.log10(flux + 1) * 0.5D * reactivity')
        self.assertEqual(self.exported['burn_formulas']['LINEAR'], 'flux / 100D * reactivity')
        self.assertEqual(self.exported['burn_formulas']['QUADRATIC'], 'flux * flux / 10000D * reactivity')

    def test_conditional_recipes_are_nominally_enumerated(self):
        manifest = self.exported['crafting_manifest']
        controls = [r for r in manifest if r['source'].endswith('main/CraftingManager.java')
                    and 'rbmk_control' in r['arguments_java'].split('new Object[]')[0]]
        self.assertEqual(len(controls), 5)
        for row in controls:
            self.assertEqual(row['enabled_default_profile'], 'rbmk_control_reasim' not in row['arguments_java'].split('new Object[]')[0])
            self.assertTrue(row['guards'])
        self.assertTrue(any('inputItemsEx' in r['arguments_java'] for r in manifest))

    def test_catalog_identity_stack_and_previous_content_are_unchanged(self):
        for relative in ('content/ids.json', 'tools/content-gen/rbmk_catalog.py'):
            before = subprocess.check_output(['git', 'show', '81a210d4:' + relative], cwd=ROOT)
            self.assertEqual(before, (ROOT / relative).read_bytes(), relative)
        entries = json.loads((ROOT / 'content/ids.json').read_text())['entries']
        for rod in self.exported['rods']:
            entry = next(r for r in entries if r['id'] == rod['item'])
            self.assertEqual(entry['max_stack_size'], rod['max_stack_size'])
        self.assertEqual(len({r['item'] for r in self.exported['rods']}), 33)


if __name__ == '__main__':
    unittest.main()

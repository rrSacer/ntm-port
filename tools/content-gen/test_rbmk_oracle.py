"""T00: replay real Java, verify provenance, numerical boundaries and catalog conservation."""
import json
from collections import Counter
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

import rbmk_catalog
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
        for exported, observed in (('silex_recipes', 'silex_registered'), ('silex_aliases', 'silex_aliases'), ('outgasser_recipes', 'outgasser_defaults')):
            self.assertEqual(self.exported[exported], self.live['recipes'][observed])
        self.assertEqual(self.exported['burn_formulas']['LOG_TEN'], 'Math.log10(flux + 1) * 0.5D * reactivity')
        self.assertEqual(self.exported['burn_formulas']['LINEAR'], 'flux / 100D * reactivity')
        self.assertEqual(self.exported['burn_formulas']['QUADRATIC'], 'flux * flux / 10000D * reactivity')

    def test_silex_translation_maps_and_real_alias_probes(self):
        aliases = self.live['recipes']['silex_aliases']
        translations = {(r['from']['stack']['item'], r['from']['stack']['meta']):
                        (r['to']['stack']['item'], r['to']['stack']['meta']) for r in aliases['item_translation']}
        self.assertEqual(translations, {
            ('fluid_icon', 29): ('ingot_uranium', 0),
            ('fluid_icon', 30): ('ingot_plutonium', 0),
            ('powder_australium', 0): ('ingot_australium', 0),
            ('powder_lapis', 0): ('minecraft_1_7_10:dye', 4)})
        self.assertEqual({r['from']: r['to'] for r in aliases['dict_translation']},
                         {'dustUranium': 'ingotUranium', 'dustPlutonium': 'ingotPlutonium'})
        self.assertEqual(aliases['all_ingredients_count'], 301)
        expected = [r['input'] for r in self.live['recipes']['silex_registered']]
        expected += [r['from'] for r in aliases['item_translation']]
        expected += [{'ore': r['from']} for r in aliases['dict_translation']]
        key = lambda value: json.dumps(value, sort_keys=True)
        self.assertEqual(Counter(map(key, aliases['all_ingredients'])), Counter(map(key, expected)))
        self.assertEqual(len(aliases['item_alias_probes']), 4)
        for probe in aliases['item_alias_probes']:
            input_key = (probe['input']['item'], probe['input']['meta'])
            target = probe['translated']['stack']
            self.assertEqual((target['item'], target['meta']), translations[input_key])
            self.assertTrue(probe['same_nonnull_recipe_instance'])
            self.assertIsNotNone(probe['recipe'])
        self.assertEqual({p['input']: p['translated'] for p in aliases['dict_alias_probes']},
                         {'dustUranium': 'ingotUranium', 'dustPlutonium': 'ingotPlutonium'})
        for probe in aliases['dict_alias_probes']:
            self.assertTrue(probe['same_nonnull_recipe_instance'])
            self.assertIsNotNone(probe['recipe'])
            self.assertTrue(probe['carrier']['item'].startswith('oracle_probe:'))
        self.assertEqual(len(aliases['ore_membership_fixture']), 4)
        self.assertEqual(aliases['ore_memberships_after_probe'], 0)
        self.assertTrue(aliases['canonical_uranium_lookup_without_fixture_is_null'])
        self.assertFalse(aliases['general_oredict_exercised'])

    def test_conditional_recipes_are_nominally_enumerated(self):
        manifest = self.exported['crafting_manifest']
        controls = [r for r in manifest if r['source'].endswith('main/CraftingManager.java')
                    and 'rbmk_control' in r['arguments_java'].split('new Object[]')[0]]
        self.assertEqual(len(controls), 5)
        for row in controls:
            self.assertEqual(row['enabled_default_profile'], 'rbmk_control_reasim' not in row['arguments_java'].split('new Object[]')[0])
            self.assertTrue(row['guards'])
        self.assertTrue(any('inputItemsEx' in r['arguments_java'] for r in manifest))

    def test_helper_templates_are_separate_and_overloads_are_bound(self):
        templates = {r['id']: r for r in self.exported['crafting_templates']}
        self.assertEqual(len(templates), 2)
        self.assertEqual({r['registration_call']['source_line'] for r in templates.values()}, {243, 246})
        self.assertTrue(all('enabled_default_profile' not in r for r in templates.values()))
        sites = self.exported['crafting_manifest']
        self.assertFalse(any(r['source'].endswith('RodRecipes.java') and r['source_line'] in (243, 246) for r in sites))
        bindings = [r for r in sites if r['kind'] == 'helper_invocation']
        self.assertEqual(len(bindings), 31)
        by_type = Counter()
        for site in bindings:
            template = templates[site['template_id']]
            self.assertEqual(set(site['bindings']), {p['name'] for p in template['parameters']})
            self.assertTrue(site['bindings']['out'].startswith('ModItems.rbmk_fuel_'))
            first = template['parameters'][0]
            self.assertEqual(site['bindings'][first['name']].startswith('ModItems.'), first['type'] == 'Item')
            by_type[first['type']] += 1
        self.assertEqual(by_type, {'DictFrame': 10, 'Item': 21})
        self.assertEqual(self.exported['crafting_manifest_counts'], {
            'lexical_call_sites': 89, 'helper_templates': 2, 'concrete_registration_sites': 87,
            'helper_invocations': 31, 'direct_registration_sites': 56})
        per_file = Counter(Path(r['source']).name for r in sites)
        self.assertEqual(per_file, {'CraftingManager.java': 38, 'RodRecipes.java': 33,
                                    'AssemblyMachineRecipes.java': 2, 'AnvilRecipes.java': 14})

    def test_decorative_rbmk_chain_is_fully_included(self):
        rows = [r for r in self.exported['crafting_manifest'] if 'deco_rbmk' in r['arguments_java']]
        self.assertEqual({r['source_line'] for r in rows}, {788, 789, 790, 791, 989, 990})
        smooth = next(r for r in rows if r['source_line'] == 789)
        self.assertEqual(smooth['arguments_java'],
            'new ItemStack(ModBlocks.deco_rbmk_smooth, 1), new Object[] { "R", \'R\', ModBlocks.deco_rbmk }')
        self.assertTrue(smooth['enabled_default_profile'])

    def test_catalog_identity_stack_and_previous_content_are_unchanged(self):
        # Coordinator's decision of 10/11/2026 (option A): the catalog may grow and other fronts may
        # correct entries outside the RBMK (Onda 1 slice 1 set the Java armor points, durability and
        # equipment gap of the steel, titanium, alloy, cobalt and starmetal sets on purpose; the four simple
        # doors gained their door_sounds). Every entry of the T00 snapshot stays present and in the same
        # relative order; the RBMK's own identities (rbmk_catalog.build: columns, consoles, panels, debris,
        # rods, pellets) stay identical.
        before = json.loads(subprocess.check_output(['git', 'show', '81a210d4:content/ids.json'], cwd=ROOT))['entries']
        entries = json.loads((ROOT / 'content/ids.json').read_text())['entries']
        remaining = iter(e['id'] for e in entries)
        for old in before:
            self.assertIn(old['id'], remaining, old['id'])
        # The eighteen RBMK columns gained their RBMKBase strength in T02 (block_properties hardness and
        # explosion_resistance, checked in test_rbmk_strength): those two keys are the only thing allowed to differ.
        rbmk = {r['id'] if ':' in r['id'] else 'ntm:' + r['id'] for r in rbmk_catalog.build(SOURCE)}
        columns = {'ntm:' + v for v in rbmk_catalog.COLUMNS}
        strength = ('hardness', 'explosion_resistance')
        def same(entry):
            if entry['id'] in columns and 'block_properties' in entry:
                props = {k: v for k, v in entry['block_properties'].items() if k not in strength}
                entry = {k: v for k, v in entry.items() if k != 'block_properties'}
                return {**entry, 'block_properties': props} if props else entry
            return entry
        now = {e['id']: e for e in entries}
        kept = [old for old in before if old['id'] in rbmk]
        self.assertEqual(len(kept), 105)
        for old in kept:
            self.assertEqual(same(old), same(now[old['id']]), old['id'])
        # rbmk_catalog.py only grew (T02 column strength, T03 tooltip keys): every T00 line is still there, in order.
        before_lines = subprocess.check_output(['git', 'show', '81a210d4:tools/content-gen/rbmk_catalog.py'], cwd=ROOT,
                                               text=True).splitlines()
        now_lines = iter((ROOT / 'tools/content-gen/rbmk_catalog.py').read_text().splitlines())
        for line in before_lines:
            self.assertIn(line, now_lines, line)
        for rod in self.exported['rods']:
            entry = next(r for r in entries if r['id'] == rod['item'])
            self.assertEqual(entry['max_stack_size'], rod['max_stack_size'])
        self.assertEqual(len({r['item'] for r in self.exported['rods']}), 33)

    def test_review_fixes_preserve_all_previous_java_observations_and_regenerate_export(self):
        before = json.loads(subprocess.check_output(
            ['git', 'show', '797ed7f6:tools/rbmk-oracle/fixtures/java.json'], cwd=ROOT))
        self.assertEqual(before['provenance']['sources_sha256'], self.live['provenance']['sources_sha256'])
        self.assertEqual({k: v for k, v in before.items() if k not in ('provenance', 'recipes')},
                         {k: v for k, v in self.live.items() if k not in ('provenance', 'recipes')})
        self.assertEqual(before['recipes'],
                         {k: v for k, v in self.live['recipes'].items() if k != 'silex_aliases'})
        self.assertEqual(self.exported, json.loads((ROOT / 'content/machine-recipes/reactors_rbmk.json').read_text()))


if __name__ == '__main__':
    unittest.main()

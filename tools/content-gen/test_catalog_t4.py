#!/usr/bin/env python3
"""Spent-fuel identities used by PUREX, and the metadata-0 recipe lookup."""
import json
import re
import unittest
from pathlib import Path
from bulk_recipes import Resolver, Unsupported

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'
DEPLETED = ['waste_natural_uranium', 'waste_uranium', 'waste_thorium', 'waste_mox', 'waste_plutonium',
            'waste_u233', 'waste_u235', 'waste_schrabidium', 'waste_zfb_mox', 'waste_plate_mox',
            'waste_plate_pu238be', 'waste_plate_pu239', 'waste_plate_ra226be', 'waste_plate_sa326',
            'waste_plate_u233', 'waste_plate_u235']


class SpentFuelIdentities(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        entries = json.loads((ROOT / 'content/ids.json').read_text())['entries']
        cls.rows = {r['source_variable']: r for r in entries if r.get('source_variable') in DEPLETED}

    def test_sixteen_spent_fuel_items_have_names_and_textures(self):
        self.assertEqual(sorted(self.rows), sorted(DEPLETED))
        for var, r in self.rows.items():
            self.assertEqual(r['type'], 'item', var)
            self.assertTrue(r['names']['en_us'].startswith('Depleted'), var)
            self.assertIn('/textures/items/', r['source_texture'])

    def test_purex_consumes_the_declared_identities(self):
        purex = json.loads((ROOT / 'content/machine-recipes/purex.json').read_text())
        self.assertEqual(purex['coverage']['skipped_source_calls'], 5)


class ShapedBlockIdentities(unittest.TestCase):
    """Checks the generated rows against the Java declarations, parsed here without catalog_t4."""

    @classmethod
    def setUpClass(cls):
        import catalog_t4
        cls.java = (SOURCE / 'src/main/java/com/hbm/blocks/ModBlocks.java').read_text()
        cls.java_items = (SOURCE / 'src/main/java/com/hbm/items/ModItems.java').read_text()
        cls.entries = {r['source_variable']: r for r in json.loads((ROOT / 'content/ids.json').read_text())['entries']
                       if r.get('source_variable')}
        cls.shaped = catalog_t4.SHAPED_BLOCKS

    def declaration(self, var):
        for text in (self.java, self.java_items):
            found = re.search(r'^\s*' + var + r'\s*=\s*new\s.*?;', text, re.M | re.S)
            if found: return found.group(0)
        raise AssertionError(f'{var} is not declared in ModBlocks or ModItems')

    def test_every_shaped_block_is_declared_in_java_with_its_texture(self):
        for var, template, gap in self.shaped:
            row = self.entries[var]
            self.assertEqual(row['template'], template, var)
            self.assertEqual(row['behavior_gap'], gap, var)
            self.assertEqual(row['type'], 'block', var)
            self.assertTrue((SOURCE / row['source_texture']).is_file(), var)

    def test_hardness_and_resistance_match_java(self):
        for var, _, _ in self.shaped:
            stmt = self.declaration(var)
            props = self.entries[var]['block_properties']
            self.assertEqual(props['hardness'], float(re.search(r'setHardness\(([\d.]+)F?\)', stmt)[1]), var)
            self.assertEqual(props['explosion_resistance'], float(re.search(r'setResistance\(([\d.]+)F?\)', stmt)[1]), var)

    def test_ladders_are_climbable_and_breakable_by_hand(self):
        for var, _, _ in self.shaped:
            if self.declaration(var).find('BlockNTMLadder') < 0: continue
            self.assertEqual(self.entries[var]['block_properties']['requires_tool'], False, var)
            self.assertEqual(self.entries[var]['template'], 'minecraft:ladder', var)

    def test_spent_fuel_plates_are_hidden_from_creative_as_in_java(self):
        plates = [v for v in self.entries if v.startswith('waste_plate_')]
        self.assertEqual(len(plates), 7)
        for var in plates:
            self.assertTrue(self.entries[var].get('creative_hidden'), var)
            self.assertIn('setCreativeTab(null)', self.declaration(var))


class MetadataZeroLookup(unittest.TestCase):
    def test_single_plain_identity_answers_metadata_zero_only(self):
        resolver = Resolver.__new__(Resolver)
        resolver.byvar = {'red_cable_box': [{'id': 'ntm:red_cable_box', 'type': 'block'}]}
        self.assertEqual(resolver.row('red_cable_box', 0, 'block'), 'ntm:red_cable_box')
        with self.assertRaises(Unsupported):
            resolver.row('red_cable_box', 3, 'block')


if __name__ == '__main__':
    unittest.main()

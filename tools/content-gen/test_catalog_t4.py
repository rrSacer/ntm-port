#!/usr/bin/env python3
"""Spent-fuel identities used by PUREX, and the metadata-0 recipe lookup."""
import json
import unittest
from pathlib import Path
from bulk_recipes import Resolver, Unsupported

ROOT = Path(__file__).resolve().parents[2]
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


class MetadataZeroLookup(unittest.TestCase):
    def test_single_plain_identity_answers_metadata_zero_only(self):
        resolver = Resolver.__new__(Resolver)
        resolver.byvar = {'red_cable_box': [{'id': 'ntm:red_cable_box', 'type': 'block'}]}
        self.assertEqual(resolver.row('red_cable_box', 0, 'block'), 'ntm:red_cable_box')
        with self.assertRaises(Unsupported):
            resolver.row('red_cable_box', 3, 'block')


if __name__ == '__main__':
    unittest.main()

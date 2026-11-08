#!/usr/bin/env python3
"""Check that only the named M1 bridges remain temporary and each one records its reason."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXPECTED_TEMPORARY = {
    'ntm:m1_temp__machine_epress',
    'ntm:m1_temp__machine_battery',
    'ntm:m1_temp__stamp_iron_plate',
    'ntm:m1_temp__stamp_iron_wire',
}


class M1Bridges(unittest.TestCase):
    def setUp(self):
        self.coverage = json.loads((ROOT / 'tools/recipe-gen/coverage.json').read_text())

    def test_temporary_count_is_named_and_justified(self):
        temporary = [r for r in self.coverage['recipes'] if r['temporary']]
        self.assertEqual(self.coverage['temporary_count'], len(temporary))
        self.assertEqual({r['id'] for r in temporary}, EXPECTED_TEMPORARY)
        for record in temporary:
            self.assertIn('java', record['source'].lower(), record['id'])

    def test_temporary_files_match_coverage(self):
        for recipe_id in EXPECTED_TEMPORARY:
            path = ROOT / 'content/data/ntm/recipe' / (recipe_id.split(':', 1)[1] + '.json')
            self.assertTrue(path.is_file(), path)

    def test_no_stale_bridge_files_remain(self):
        stale = [p.name for p in (ROOT / 'content/data/ntm/recipe').glob('m1_temp__*.json')]
        self.assertEqual(sorted(stale), sorted(e.split(':', 1)[1] + '.json' for e in EXPECTED_TEMPORARY))


if __name__ == '__main__':
    unittest.main()

#!/usr/bin/env python3
"""Check Nether and gneiss ore distributions against HbmWorldGen and WorldConfig defaults."""
import unittest
from worldgen import EXTRA, generate


class M1Worldgen(unittest.TestCase):
    def specs(self, prefix):
        return {s['material']: s for s in EXTRA if s['material'].startswith(prefix)}

    def test_nether_veins_match_java(self):
        nether = self.specs('nether_')
        self.assertEqual(set(nether), {'nether_uranium', 'nether_tungsten', 'nether_sulfur', 'nether_fire', 'nether_cobalt', 'nether_coal'})
        # attempts, size, min_y, max_y: generateNether uses nextInt(127) for y 0..126; cobalt is 100 + nextInt(26).
        expected = {
            'nether_uranium': (8, 6, 0, 126),
            'nether_tungsten': (10, 10, 0, 126),
            'nether_sulfur': (26, 12, 0, 126),
            'nether_fire': (24, 6, 0, 126),
            'nether_cobalt': (2, 6, 100, 125),
            'nether_coal': (8, 32, 16, 111),
        }
        for name, (count, size, low, high) in expected.items():
            s = nether[name]
            self.assertEqual((s['attempts_per_chunk'], s['size'], s['min_y'], s['max_y']), (count, size, low, high), name)
            self.assertEqual(s['biomes'], '#minecraft:is_nether', name)
            self.assertEqual(s['targets'], ['netherrack'], name)

    def test_nether_schrabidium_is_not_generated_by_java(self):
        # Only the nuke environment and the smelting table reference ore_nether_schrabidium in 1.7.10.
        self.assertFalse([s for s in EXTRA if s['block'] == 'ntm:ore_nether_schrabidium'])

    def test_gneiss_veins_match_java(self):
        gneiss = self.specs('gneiss_')
        expected = {
            'gneiss_uranium': (21, 6),   # uraniumSpawn 7 x 3
            'gneiss_copper': (36, 6),    # copperSpawn 12 x 3
            'gneiss_asbestos': (6, 6),   # asbestosSpawn 2 x 3
            'gneiss_lithium': (6, 6),    # lithiumSpawn 6
            'gneiss_iron': (25, 6),      # literal 25 in HbmWorldGen
            'gneiss_gold': (10, 6),      # literal 10
            'gneiss_rare': (6, 6),       # rareSpawn 6
        }
        self.assertEqual(set(gneiss), set(expected))
        for name, (count, size) in expected.items():
            s = gneiss[name]
            self.assertEqual((s['attempts_per_chunk'], s['size'], s['min_y'], s['max_y']), (count, size, 30, 39), name)
            self.assertEqual(s['biomes'], '#minecraft:is_overworld', name)
            self.assertEqual(s['targets'], ['ntm:stone_gneiss'], name)

    def test_salts_are_unique(self):
        salts = [s['salt'] for s in EXTRA]
        self.assertEqual(len(salts), len(set(salts)))

    def test_generate_emits_five_files_per_extra_vein(self):
        out = {}
        rows = [{'id': s['block']} for s in EXTRA]
        generate(out.__setitem__, rows, [])
        m1 = [p for p in out if '/m1_nether_' in p or '/m1_gneiss_' in p]
        self.assertEqual(len(m1), 5 * len(EXTRA))
        target = out['data/ntm/worldgen/configured_feature/m1_gneiss_lithium.json']['targets'][0]
        self.assertEqual(target['target']['block'], 'ntm:stone_gneiss')
        self.assertEqual(target['state'], {'id': 'ntm:ore_gneiss_lithium'})
        nether = out['data/ntm/worldgen/structure/m1_nether_uranium.json']
        self.assertEqual(nether['biomes'], '#minecraft:is_nether')

    def test_unknown_block_is_refused(self):
        with self.assertRaises(ValueError):
            generate(lambda p, v: None, [], [dict(material='x', block='ntm:missing', salt=1, attempts_per_chunk=1, size=1, min_y=0, max_y=1)])


if __name__ == '__main__':
    unittest.main()

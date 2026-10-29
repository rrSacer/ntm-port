#!/usr/bin/env python3
"""Press stamp table, source recipe order, Mats WIRE autogen and OreDict contracts."""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from press_recipes import export

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'

EXPECTED_STAMPS = [
    ('ntm:stamp_stone_flat', 'flat', 32),
    ('ntm:stamp_stone_plate', 'plate', 32),
    ('ntm:stamp_stone_wire', 'wire', 32),
    ('ntm:stamp_stone_circuit', 'circuit', 32),
    ('ntm:stamp_iron_flat', 'flat', 64),
    ('ntm:stamp_iron_plate', 'plate', 64),
    ('ntm:stamp_iron_wire', 'wire', 64),
    ('ntm:stamp_iron_circuit', 'circuit', 64),
    ('ntm:stamp_steel_flat', 'flat', 192),
    ('ntm:stamp_steel_plate', 'plate', 192),
    ('ntm:stamp_steel_wire', 'wire', 192),
    ('ntm:stamp_steel_circuit', 'circuit', 192),
    ('ntm:stamp_titanium_flat', 'flat', 256),
    ('ntm:stamp_titanium_plate', 'plate', 256),
    ('ntm:stamp_titanium_wire', 'wire', 256),
    ('ntm:stamp_titanium_circuit', 'circuit', 256),
    ('ntm:stamp_obsidian_flat', 'flat', 512),
    ('ntm:stamp_obsidian_plate', 'plate', 512),
    ('ntm:stamp_obsidian_wire', 'wire', 512),
    ('ntm:stamp_obsidian_circuit', 'circuit', 512),
    ('ntm:stamp_desh_flat', 'flat', 0),
    ('ntm:stamp_desh_plate', 'plate', 0),
    ('ntm:stamp_desh_wire', 'wire', 0),
    ('ntm:stamp_desh_circuit', 'circuit', 0),
    ('ntm:stamp_357', 'c357', 1000),
    ('ntm:stamp_44', 'c44', 1000),
    ('ntm:stamp_9', 'c9', 1000),
    ('ntm:stamp_50', 'c50', 1000),
    ('ntm:stamp_desh_357', 'c357', 0),
    ('ntm:stamp_desh_44', 'c44', 0),
    ('ntm:stamp_desh_9', 'c9', 0),
    ('ntm:stamp_desh_50', 'c50', 0),
]

EXPECTED_WIRES = [
    ('carbon', ['ntm:ingot_graphite'], 'ntm:wire_fine__carbon'),
    ('gold', ['minecraft:gold_ingot'], 'ntm:wire_fine__gold'),
    ('schrabidium', ['ntm:ingot_schrabidium'], 'ntm:wire_fine__schrabidium'),
    ('copper', ['ntm:ingot_copper'], 'ntm:wire_fine__copper'),
    ('tungsten', ['ntm:ingot_tungsten'], 'ntm:wire_fine__tungsten'),
    ('aluminium', ['ntm:ingot_aluminium'], 'ntm:wire_fine__aluminium'),
    ('lead', ['ntm:ingot_lead'], 'ntm:wire_fine__lead'),
    ('zirconium', ['ntm:ingot_zirconium'], 'ntm:wire_fine__zirconium'),
    ('steel', ['ntm:ingot_steel'], 'ntm:wire_fine__steel'),
    ('mingrade', ['ntm:ingot_red_copper'], 'ntm:wire_fine__red_copper'),
    ('magtung', ['ntm:ingot_magnetized_tungsten'], 'ntm:wire_fine__magnetized_tungsten'),
]


class PressContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {row['id']: row for row in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/press.json').read_text())
        cls.by_name = {row['name']: row for row in cls.data['recipes']}

    def test_32_stamps_durabilities_max_damage_and_stamp_book_gap(self):
        self.assertEqual(len(self.data['stamps']), 32)
        self.assertEqual(
            [(s['item'], s['type'], s['durability']) for s in self.data['stamps']],
            EXPECTED_STAMPS,
        )
        for stamp in self.data['stamps']:
            ident = stamp['item']
            durability = stamp['durability']
            self.assertIn(ident, self.rows)
            row = self.rows[ident]
            self.assertEqual(row['max_stack_size'], 1, ident)
            self.assertEqual(row['durability'], {'max_damage': durability, 'wear': durability > 0}, ident)
            if durability > 0:
                self.assertEqual(row.get('max_damage'), durability, ident)
            else:
                self.assertNotIn('max_damage', row, ident)
            item_decl = json.loads((ROOT / f"content/data/ntm/horizon/item/{ident.split(':', 1)[1]}.json").read_text())
            self.assertEqual(item_decl['max_stack_size'], 1, ident)
            self.assertNotIn('max_damage', item_decl, ident)
            self.assertEqual(stamp['unbreakable'], durability == 0)
            self.assertIn('ModItems.java:', stamp['source'])
        self.assertEqual(len(self.data['skipped_stamps']), 1)
        book = self.data['skipped_stamps'][0]
        self.assertEqual(book['variable'], 'ModItems.stamp_book')
        self.assertEqual(book['types'], [f'printing{i}' for i in range(1, 9)])

    def test_coverage_partition_and_source_order(self):
        cov = self.data['coverage']
        self.assertEqual(cov['source_calls'], 38)
        self.assertEqual(cov['expanded'], 48)
        self.assertEqual(cov['emitted'], 47)
        self.assertEqual(cov['skipped'], 1)
        self.assertEqual(cov['stamps'], 32)
        self.assertEqual(cov['skipped_stamps'], 1)
        self.assertEqual(cov['by_stamp'], {
            'flat': {'emitted': 10, 'skipped': 1},
            'plate': {'emitted': 13, 'skipped': 0},
            'wire': {'emitted': 11, 'skipped': 0},
            'circuit': {'emitted': 1, 'skipped': 0},
            'c357': {'emitted': 0, 'skipped': 0},
            'c44': {'emitted': 0, 'skipped': 0},
            'c50': {'emitted': 2, 'skipped': 0},
            'c9': {'emitted': 2, 'skipped': 0},
            **{f'printing{i}': {'emitted': 1, 'skipped': 0} for i in range(1, 9)},
        })
        indices = [r['source_index'] for r in self.data['recipes']]
        self.assertEqual(indices, sorted(indices))
        all_indices = indices + [r['source_index'] for r in self.data['skipped']]
        self.assertEqual(sorted(all_indices), list(range(48)))
        self.assertEqual(len(self.data['source_recipe_order']), 48)
        for row in self.data['recipes'] + self.data['skipped']:
            self.assertEqual(self.data['source_recipe_order'][row['source_index']], row['name'])
            self.assertIn('PressRecipes.java:', row['source'])
        skipped = self.data['skipped'][0]
        self.assertEqual(skipped['source_index'], 6)
        self.assertEqual(skipped['stamp'], 'flat')
        self.assertIn('meteorite_sword_reforged', skipped['reason'])

    def test_flat_plate_casings_circuit_and_printing_recipes(self):
        self.assertEqual(self.by_name['press.0']['out'], ['minecraft:quartz', 1])
        self.assertEqual(self.by_name['press.1']['out'], ['minecraft:lapis_lazuli', 1])
        self.assertEqual(
            self.by_name['press.5']['in'],
            {'ids': ['ntm:coke__coal', 'ntm:coke__lignite', 'ntm:coke__petroleum'], 'count': 1},
        )
        self.assertEqual(self.by_name['press.5']['out'], ['ntm:ingot_graphite', 1])
        self.assertEqual(self.by_name['press.7']['in'], {'ids': ['minecraft:jungle_log'], 'count': 1})
        self.assertEqual(self.by_name['press.7']['out'], ['ntm:ball_resin', 1])
        self.assertEqual(self.by_name['press.8']['out'], ['ntm:briquette__coal', 1])
        self.assertEqual(self.by_name['press.9']['out'], ['ntm:briquette__lignite', 1])
        self.assertEqual(self.by_name['press.10']['out'], ['ntm:briquette__wood', 1])

        plates = [r for r in self.data['recipes'] if r['stamp'] == 'plate']
        self.assertEqual(len(plates), 13)
        self.assertEqual(plates[0]['in']['ids'], ['minecraft:iron_ingot'])
        self.assertEqual(plates[0]['out'], ['ntm:plate_iron', 1])
        self.assertEqual(plates[1]['in']['ids'], ['minecraft:gold_ingot'])
        self.assertEqual(plates[1]['out'], ['ntm:plate_gold', 1])
        # Conservative 1.7.10 OreDict snapshot: copper plate takes ntm:ingot_copper, not modern copper_ingot.
        copper_plate = next(r for r in plates if r['out'][0] == 'ntm:plate_copper')
        self.assertEqual(copper_plate['in']['ids'], ['ntm:ingot_copper'])

        self.assertEqual(self.by_name['press.24']['stamp'], 'c9')
        self.assertEqual(self.by_name['press.24']['out'], ['ntm:casing__small', 4])
        self.assertEqual(self.by_name['press.25']['stamp'], 'c50')
        self.assertEqual(self.by_name['press.25']['out'], ['ntm:casing__large', 2])
        self.assertEqual(self.by_name['press.26']['out'], ['ntm:casing__small_steel', 4])
        self.assertEqual(self.by_name['press.27']['out'], ['ntm:casing__large_steel', 2])

        self.assertEqual(self.by_name['press.39']['stamp'], 'circuit')
        self.assertEqual(self.by_name['press.39']['in']['ids'], ['ntm:billet_silicon'])
        self.assertEqual(self.by_name['press.39']['out'], ['ntm:circuit__silicon', 1])

        for page in range(1, 9):
            rec = self.by_name[f'press.{39 + page}']
            self.assertEqual(rec['stamp'], f'printing{page}')
            self.assertEqual(rec['in'], {'ids': ['minecraft:paper'], 'count': 1})
            self.assertEqual(rec['out'], [f'ntm:page_of___page{page}', 1])

    def test_mats_ordered_wire_autogen_including_carbon_and_gold(self):
        wires = [r for r in self.data['recipes'] if r['stamp'] == 'wire']
        self.assertEqual(len(wires), 11)
        self.assertEqual(
            [(r['expansion']['material'], r['in']['ids'], r['out'][0]) for r in wires],
            EXPECTED_WIRES,
        )
        for rec in wires:
            self.assertEqual(rec['in']['count'], 1)
            self.assertEqual(rec['out'][1], 8)

    def test_all_emitted_ids_exist_and_hashseed_determinism(self):
        vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/items')
        ids = [s['item'] for s in self.data['stamps']]
        for rec in self.data['recipes']:
            self.assertGreater(rec['in']['count'], 0)
            self.assertGreater(rec['out'][1], 0)
            self.assertTrue(rec['in']['ids'])
            ids.extend(rec['in']['ids'])
            ids.append(rec['out'][0])
        for ident in set(ids):
            if ident.startswith('ntm:'):
                self.assertIn(ident, self.rows, ident)
            else:
                self.assertTrue((vanilla / (ident.split(':', 1)[1] + '.json')).is_file(), ident)

        expected = (ROOT / 'content/machine-recipes/press.json').read_bytes()
        with tempfile.TemporaryDirectory(prefix='ntm-press-test-') as tmp:
            out = Path(tmp)
            export(SOURCE, out, list(self.rows.values()))
            self.assertEqual((out / 'machine-recipes/press.json').read_bytes(), expected)

        for seed in ('1', '42'):
            code = (
                "import json, tempfile; from pathlib import Path; import press_recipes; "
                f"root = Path({str(ROOT)!r}); rows = json.loads((root / 'content/ids.json').read_text())['entries']; "
                "tmp = Path(tempfile.mkdtemp(prefix='ntm-press-seed-')); "
                "press_recipes.export(root.parent / 'ntm-src', tmp, rows); "
                "assert (tmp / 'machine-recipes/press.json').read_bytes() == (root / 'content/machine-recipes/press.json').read_bytes()"
            )
            env = {**os.environ, 'PYTHONHASHSEED': seed, 'PYTHONPATH': str(ROOT / 'tools/content-gen')}
            subprocess.run([sys.executable, '-c', code], check=True, env=env, stdout=subprocess.DEVNULL)

        # recipe-gen hands off crates/ntm-core/data/press_recipes.json and reads press_count from press.json.
        manifest = json.loads((ROOT / 'tools/recipe-gen/generated-files.json').read_text())
        self.assertNotIn('crates/ntm-core/data/press_recipes.json', manifest)
        coverage = json.loads((ROOT / 'tools/recipe-gen/coverage.json').read_text())
        self.assertEqual(coverage['press_count'], 47)


if __name__ == '__main__':
    unittest.main()

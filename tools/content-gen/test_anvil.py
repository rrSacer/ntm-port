#!/usr/bin/env python3
"""Anvil block contracts, smithing/mold rules, construction order and IDs."""
import json
import struct
import tempfile
import unittest
import zlib
from collections import Counter
from pathlib import Path
from anvil_catalog import build as build_anvils
from anvil_recipes import construction_recipe, export, smithing_recipe, snapshot
from generate import transform_png

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'
EXPECTED_ANVILS = [
    ('ntm:anvil_iron', 1),
    ('ntm:anvil_lead', 1),
    ('ntm:anvil_steel', 2),
    ('ntm:anvil_desh', 3),
    ('ntm:anvil_ferrouranium', 4),
    ('ntm:anvil_saturnite', 5),
    ('ntm:anvil_bismuth_bronze', 5),
    ('ntm:anvil_arsenic_bronze', 5),
    ('ntm:anvil_schrabidate', 6),
    ('ntm:anvil_dnt', 7),
    ('ntm:anvil_osmiridium', 8),
    ('ntm:anvil_murky', 1916169),
]


class AnvilContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {row['id']: row for row in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/anvil.json').read_text())
        cls.foundry = json.loads((ROOT / 'content/machine-recipes/foundry.json').read_text())
        cls.smithing = {row['name']: row for row in cls.data['smithing']}
        cls.construction = {row['name']: row for row in cls.data['construction']}

    def test_twelve_anvil_blocks_tiers_textures_and_crafting(self):
        self.assertEqual([(a['id'], a['tier']) for a in self.data['anvils']], EXPECTED_ANVILS)
        self.assertEqual(len(build_anvils(SOURCE)), 12)
        for ident, tier in EXPECTED_ANVILS:
            name = ident.split(':', 1)[1]
            row = self.rows[ident]
            self.assertIn(ident, self.catalog['machines'])
            self.assertEqual(row['tier'], tier)
            self.assertEqual(row['template'], 'minecraft:beehive')
            self.assertEqual(row['logic'], {
                'kind': 'machine', 'facing': 'facing', 'active': None,
                'level': {'prop': 'honey_level', 'max': 5},
            })
            block = json.loads((ROOT / f'content/data/ntm/horizon/block/{name}.json').read_text())
            self.assertEqual(block['properties']['hardness'], 5.0)
            self.assertEqual(block['properties']['explosion_resistance'], 100.0)
            self.assertEqual(block['properties']['sounds'], 'minecraft:anvil')
            state = json.loads((ROOT / f'content/assets/ntm/blockstates/{name}.json').read_text())
            self.assertEqual(len(state['variants']), 24)
            loot = json.loads((ROOT / f'content/data/ntm/loot_table/blocks/{name}.json').read_text())
            self.assertIn(ident, json.dumps(loot))
            model = json.loads((ROOT / f'content/assets/ntm/models/block/{name}_0.json').read_text())
            if ident == 'ntm:anvil_murky':
                self.assertEqual(model['textures']['up'], 'ntm:block/anvil_murky')
                self.assertEqual(model['textures']['down'], 'ntm:block/anvil_steel')
                self.assertEqual(model['textures']['north'], 'ntm:block/anvil_steel')
            for face in ('down', 'up', 'north', 'south', 'east', 'west'):
                ns, tex = model['textures'][face].split(':')
                raw = transform_png((ROOT / f'content/assets/{ns}/textures/{tex}.png').read_bytes())
                width, height = struct.unpack('>II', raw[16:24])
                pos, compressed = 8, b''
                while pos < len(raw):
                    length = struct.unpack('>I', raw[pos:pos + 4])[0]
                    if raw[pos + 4:pos + 8] == b'IDAT':
                        compressed += raw[pos + 8:pos + 8 + length]
                    pos += length + 12
                pixels = zlib.decompress(compressed)
                self.assertTrue(all(
                    pixels[y * (width * 4 + 1) + 4 + x * 4] == 255
                    for y in range(height) for x in range(width)
                ), (ident, face))

        bulk = json.loads((ROOT / 'content/bulk-recipe-coverage.json').read_text())
        crafted = {r['result']['id']: r for r in bulk['recipes']}
        for ident in ('ntm:anvil_iron', 'ntm:anvil_lead', 'ntm:anvil_murky', 'ntm:mold_base'):
            self.assertIn(ident, crafted)
        # Higher-tier anvils (steel..osmiridium) require 1 anvil + 10 ingots on tier-1 smithing, never 3x3 crafting.
        for ident, _ in EXPECTED_ANVILS[2:11]:
            self.assertNotIn(ident, crafted)

    def test_source_order_and_coverage_partition(self):
        self.assertEqual(self.data['profile'], {
            'enableLBSM': False, 'enableLBSMUnlockAnvil': False,
            'enableExpensiveMode': False, 'enable528': False, 'external_mods': [],
        })
        self.assertEqual(self.data['coverage'], {
            'smithing': {'source_calls': 41, 'expanded': 58, 'emitted': 36, 'skipped': 22},
            'construction': {'source_calls': 177, 'expanded': 257, 'emitted': 105, 'skipped': 152},
        })
        for section, total in [('smithing', 58), ('construction', 257)]:
            emitted = self.data[section]
            skipped = [row for row in self.data['skipped'] if row['section'] == section]
            self.assertEqual([r['source_index'] for r in emitted], sorted(r['source_index'] for r in emitted))
            seen = [r['source_index'] for r in emitted] + [r['source_index'] for r in skipped]
            self.assertEqual(sorted(seen), list(range(total)))
            self.assertEqual(len(self.data['source_recipe_order'][section]), total)
            for row in emitted + skipped:
                self.assertEqual(self.data['source_recipe_order'][section][row['source_index']], row['name'])
                self.assertIn('AnvilRecipes.java:', row['source'])
            for row in skipped:
                self.assertTrue(row['reason'])

    def test_smithing_upgrades_gunmetal_and_nbt_skips(self):
        expected_targets = [
            ('ntm:anvil_steel', 'ntm:ingot_steel'),
            ('ntm:anvil_desh', 'ntm:ingot_desh'),
            ('ntm:anvil_saturnite', 'ntm:ingot_saturnite'),
            ('ntm:anvil_ferrouranium', 'ntm:ingot_ferrouranium'),
            ('ntm:anvil_bismuth_bronze', 'ntm:ingot_bismuth_bronze'),
            ('ntm:anvil_arsenic_bronze', 'ntm:ingot_arsenic_bronze'),
            ('ntm:anvil_schrabidate', 'ntm:ingot_schrabidate'),
            ('ntm:anvil_dnt', 'ntm:ingot_dineutronium'),
            ('ntm:anvil_osmiridium', 'ntm:ingot_osmiridium'),
        ]
        for base_idx, base_anvil in [(0, 'ntm:anvil_iron'), (9, 'ntm:anvil_lead')]:
            for offset, (target, ingot) in enumerate(expected_targets):
                rec = self.smithing[f'anvil.smithing.{base_idx + offset}']
                self.assertEqual(rec['tier'], 1)
                self.assertEqual(rec['kind'], 'plain')
                self.assertFalse(rec['shapeless'])
                self.assertEqual(rec['consume'], [1, 10])
                self.assertEqual(rec['left'], {'ids': [base_anvil], 'count': 1})
                self.assertEqual(rec['right']['count'], 10)
                self.assertIn(ingot, rec['right']['ids'])
                self.assertEqual(rec['out'], [target, 1])

        gunmetal = self.smithing['anvil.smithing.38']
        self.assertEqual(gunmetal['tier'], 1)
        self.assertEqual(gunmetal['out'], ['ntm:ingot_gunmetal', 1])
        self.assertEqual(gunmetal['consume'], [1, 1])
        self.assertIn('ntm:ingot_copper', gunmetal['left']['ids'])
        self.assertIn('ntm:ingot_aluminium', gunmetal['right']['ids'])

        smithing_skips = Counter(r['reason'] for r in self.data['skipped'] if r['section'] == 'smithing')
        self.assertEqual(smithing_skips['AnvilSmithingHotRecipe requires per-item NBT/state'], 18)
        self.assertEqual(smithing_skips['AnvilSmithingCyanideRecipe requires per-item NBT/state'], 1)
        self.assertEqual(smithing_skips['AnvilSmithingRenameRecipe requires per-item NBT/state'], 1)

    def test_mold_engravings_exact_prefix_and_all_26_molds_covered(self):
        molds = [r for r in self.data['smithing'] if r['kind'] == 'mold']
        self.assertEqual(len(molds), 17)
        for rec in molds:
            self.assertEqual(rec['tier'], 1)
            self.assertFalse(rec['shapeless'])
            self.assertEqual(rec['consume'], [0, 1])
            self.assertTrue(rec['left']['exact'])
            self.assertEqual(rec['right'], {'ids': ['ntm:mold_base'], 'count': 1})

        by_out = {r['out'][0]: r for r in molds}
        # Longer MaterialShapes prefixes (plateTriple, plateSextuple, wireDense) must be rejected.
        for out_id, expected_count in [('ntm:mold__plate', 1), ('ntm:mold__plates', 9)]:
            ids = by_out[out_id]['left']['ids']
            self.assertEqual(by_out[out_id]['left']['count'], expected_count)
            self.assertIn('ntm:plate_iron', ids)
            self.assertFalse(any('plate_cast' in x or 'plate_welded' in x for x in ids))
        self.assertFalse(any('wire_dense' in x for x in by_out['ntm:mold__wire']['left']['ids']))
        self.assertIn('minecraft:iron_ingot', by_out['ntm:mold__ingot']['left']['ids'])
        self.assertEqual(by_out['ntm:mold__plates_cast']['left']['count'], 3)
        self.assertEqual(by_out['ntm:mold__wires_dense']['left']['count'], 9)
        self.assertEqual(by_out['ntm:mold__stamp']['left']['ids'], [
            'ntm:stamp_stone_flat', 'ntm:stamp_iron_flat', 'ntm:stamp_steel_flat',
            'ntm:stamp_titanium_flat', 'ntm:stamp_obsidian_flat',
        ])

        construction_molds = {r['out'][0][0] for r in self.data['construction'] if r['out'][0][0].startswith('ntm:mold__')}
        foundry_molds = {m['item'] for m in self.foundry['molds']}
        self.assertEqual(len(construction_molds), 9)
        self.assertEqual(set(by_out) | construction_molds, foundry_molds)

    def test_construction_tiers_chances_overlays_and_parser_helpers(self):
        plate = self.construction['anvil.construction.4']
        self.assertEqual((plate['tier_lower'], plate['tier_upper'], plate['overlay']), (3, -1, 'SMITHING'))
        self.assertEqual(plate['out'], [['ntm:plate_steel', 1, 1.0]])

        rare = self.construction['anvil.construction.203']
        self.assertEqual((rare['tier_lower'], rare['tier_upper'], rare['overlay']), (2, -1, 'RECYCLING'))
        self.assertEqual(rare['in'], [{'ids': ['ntm:chunk_ore__rare'], 'count': 1}])
        self.assertEqual(rare['out'][:3], [
            ['ntm:fragment_boron', 1, 1.0],
            ['ntm:fragment_boron', 1, 0.5],
            ['ntm:fragment_lanthanium', 1, 0.1],
        ])

        outputs = {o[0] for r in self.data['construction'] for o in r['out']}
        for expected in ('ntm:machine_crucible', 'ntm:heater_firebox', 'ntm:stamp_iron_plate',
                         'ntm:stamp_desh_plate', 'ntm:stamp_9', 'ntm:wire_fine__copper',
                         'ntm:shell__steel', 'ntm:pipe__copper'):
            self.assertIn(expected, outputs)

        resolver, _, shapes, _ = snapshot(SOURCE, list(self.rows.values()))
        shapeless = smithing_recipe(
            'new AnvilSmithingRecipe(2, new ItemStack(ModItems.ingot_gunmetal, 1), '
            'new OreDictStack(CU.ingot(), 2), new OreDictStack(AL.ingot(), 1)).makeShapeless()',
            resolver, shapes,
        )
        self.assertTrue(shapeless['shapeless'])
        self.assertEqual(shapeless['consume'], [2, 1])

        ranged = construction_recipe(
            'new AnvilConstructionRecipe(new AStack[]{new OreDictStack(IRON.ingot(), 2)}, '
            'new AnvilOutput[]{new AnvilOutput(new ItemStack(ModItems.plate_iron, 1), 0.25F)}).setTierRange(2, 5)',
            resolver,
        )
        self.assertEqual((ranged['tier_lower'], ranged['tier_upper'], ranged['overlay']), (2, 5, 'NONE'))
        self.assertEqual(ranged['out'], [['ntm:plate_iron', 1, 0.25]])

    def test_all_emitted_ids_exist_and_export_is_deterministic(self):
        vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/items')
        ids = [a['id'] for a in self.data['anvils']]
        for rec in self.data['smithing']:
            self.assertGreater(rec['left']['count'], 0)
            self.assertGreater(rec['right']['count'], 0)
            self.assertGreater(rec['out'][1], 0)
            ids.extend(rec['left']['ids'] + rec['right']['ids'] + [rec['out'][0]])
        for rec in self.data['construction']:
            self.assertTrue(rec['in'])
            self.assertTrue(rec['out'])
            for stack in rec['in']:
                self.assertGreater(stack['count'], 0)
                self.assertTrue(stack['ids'])
                ids.extend(stack['ids'])
            for ident, count, chance in rec['out']:
                self.assertGreater(count, 0)
                self.assertGreater(chance, 0.0)
                self.assertLessEqual(chance, 1.0)
                ids.append(ident)
        for ident in set(ids):
            if ident.startswith('ntm:'):
                self.assertIn(ident, self.rows, ident)
            else:
                self.assertTrue((vanilla / (ident.split(':', 1)[1] + '.json')).is_file(), ident)

        with tempfile.TemporaryDirectory(prefix='ntm-anvil-test-') as tmp:
            out = Path(tmp)
            export(SOURCE, out, list(self.rows.values()))
            self.assertEqual(
                (out / 'machine-recipes/anvil.json').read_bytes(),
                (ROOT / 'content/machine-recipes/anvil.json').read_bytes(),
            )


if __name__ == '__main__':
    unittest.main()

#!/usr/bin/env python3
"""Bedrock source ordering, identity, yield and datapack contracts."""
import json
import tempfile
import unittest
from pathlib import Path
from bedrock_recipes import export, extract_count
from bedrock_catalog import drillbits
from bedrock_worldgen import generate as deposits

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class BedrockContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {r['id']: r for r in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/bedrock.json').read_text())
        cls.cent = {r['inputItems'][0]['ids'][0]: r for r in cls.data['centrifuge']}

    def test_types_grades_and_metadata_in_source_order(self):
        self.assertEqual([t['suffix'] for t in self.data['types']], ['light', 'heavy', 'rare', 'actinide', 'nonmetal', 'crystal'])
        self.assertEqual(self.data['types'][0]['primary1'], ['iron', 9])
        self.assertEqual(self.data['types'][2]['primary2'], ['rareearth', 5])
        self.assertEqual(self.data['types'][3]['byproductRad'][2], ['u238', 1])
        self.assertEqual(len(self.data['grades']), 26)
        self.assertEqual(self.data['grades'][0]['name'], 'BASE')
        self.assertEqual(self.data['grades'][-1]['name'], 'RAD_WASHED')
        for grade in self.data['grades']:
            self.assertEqual(len(grade['items']), 6)
            for i, ore_type in enumerate(self.data['types']):
                row = self.rows[grade['items'][ore_type['suffix']]]
                self.assertEqual(row['metadata'], grade['ordinal'] << 4 | i)
        self.assertFalse(self.data['missing_fragments'])

    def test_centrifuge_yields_slots_and_floor_ceiling(self):
        self.assertEqual(len(self.cent), 96)
        self.assertEqual(len(self.cent), len(self.data['centrifuge']))
        self.assertEqual(self.cent['ntm:bedrock_ore__base_light']['outputItems'], [['ntm:bedrock_ore__primary_light', 1], ['minecraft:gravel', 1]])
        self.assertEqual(self.cent['ntm:bedrock_ore__base_washed_heavy']['outputItems'], [['ntm:bedrock_ore__primary_heavy', 1], ['ntm:bedrock_ore__primary_heavy', 1], ['minecraft:gravel', 1]])
        self.assertEqual(self.cent['ntm:bedrock_ore__primary_light']['outputItems'], [['ntm:bedrock_ore_fragment__iron', 9], ['ntm:bedrock_ore_fragment__copper', 9]])
        first = self.cent['ntm:bedrock_ore__primary_first_light']['outputItems']
        self.assertEqual(first[:2], [['ntm:bedrock_ore_fragment__iron', 9]] * 2)
        self.assertEqual(extract_count(7, '0.5'), 4)
        self.assertEqual(extract_count(9, '100'), 64)
        for row in self.cent.values(): self.assertLessEqual(len(row['outputItems']), 4)
        self.assertNotIn('ntm:bedrock_ore_base', self.cent)

    def test_missing_fragment_is_explicitly_skipped(self):
        rows = [r for ident, r in self.rows.items() if ident != 'ntm:bedrock_ore_fragment__iron']
        with tempfile.TemporaryDirectory() as directory:
            data = export(SOURCE, Path(directory), rows)
        self.assertEqual(data['coverage']['centrifuge'], 89)
        self.assertEqual(len(data['missing_fragments']), 1)
        skipped = [r for r in data['skipped'] if r['machine'] == 'centrifuge']
        self.assertEqual(len(skipped), 7)
        self.assertTrue(all('bedrock_ore_fragment__iron' in r['reason'] for r in skipped))

    def test_all_ids_and_fragment_packing_preference(self):
        self.assertEqual(len(self.data['fragment_packing']), 45)
        fragments = {ident for ident in self.rows if ident.startswith('ntm:bedrock_ore_fragment__')}
        self.assertEqual({r['input'] for r in self.data['fragment_packing']}, fragments)
        for pack in self.data['fragment_packing']:
            self.assertEqual(pack['count'], 9)
            path = ROOT / 'content/data/ntm/recipe' / (pack['name'] + '.json')
            recipe = json.loads(path.read_text())
            self.assertEqual(recipe['pattern'], ['###'] * 3)
            self.assertEqual(recipe['result'], {'id': pack['output'], 'count': 1})
        packing = {p['input']: p for p in self.data['fragment_packing']}
        self.assertEqual(packing['ntm:bedrock_ore_fragment__iron']['output'], 'ntm:powder_iron')
        for row in [*self.data['centrifuge'], *self.data['assembly']]:
            for ident in [i for stack in row['inputItems'] for i in stack['ids']] + [s[0] for s in row['outputItems']]:
                if ident.startswith('ntm:'): self.assertIn(ident, self.rows)
                self.assertNotIn('ntm:ore_bedrock__', ident)

    def test_machines_deposit_drills_and_fluid(self):
        for name in ['machine_excavator', 'machine_ore_slopper']:
            ident = 'ntm:' + name; row = self.rows[ident]
            self.assertIn(ident, self.catalog['machines'])
            self.assertEqual(row['template'], 'minecraft:beehive')
            self.assertEqual(row['logic']['level'], {'prop': 'honey_level', 'max': 5})
        row = self.rows['ntm:ore_bedrock']
        self.assertNotIn(row['id'], self.catalog['machines'])
        declaration = json.loads((ROOT / 'content/data/ntm/horizon/block/ore_bedrock.json').read_text())
        self.assertEqual(declaration['template'], 'minecraft:bedrock')
        self.assertEqual(declaration['properties']['hardness'], -1)
        self.assertGreaterEqual(declaration['properties']['explosion_resistance'], 3600000)
        self.assertEqual(json.loads((ROOT / 'content/data/ntm/loot_table/blocks/ore_bedrock.json').read_text())['pools'], [])
        bits = drillbits(SOURCE)
        self.assertEqual(self.data['drillbits'], bits)
        self.assertEqual([b['speed'] for b in bits], [1., 1., 1.2, 1.2, 1.5, 1.5, 2., 2., 2.5, 2.5])
        for b in bits: self.assertEqual(self.rows[b['item']]['max_stack_size'], 1)
        fluids = json.loads((ROOT / 'content/fluids.json').read_text())['fluids']
        self.assertEqual(next(f for f in fluids if f['source_variable'] == 'SLOP')['string_id'], 'slop')

    def test_assembly_and_nether_defaults(self):
        self.assertEqual(len(self.data['assembly']), 12)
        self.assertEqual([r['name'] for r in self.data['assembly']][::11], ['ass.excavator', 'ass.slopper'])
        for r in self.data['assembly']:
            self.assertEqual(r['power'], 100)
            self.assertIn(r['duration'], [100, 200])
        self.assertEqual([(r['item'], r['count'], r['weight'], r['tier']) for r in self.data['nether']],
                         [('minecraft:glowstone_dust', 4, 100, 1), ('ntm:powder_fire', 4, 50, 1), ('minecraft:quartz', 4, 100, 1)])
        self.assertFalse(self.data['skipped'])

    def test_datapack_frequency_floor_and_only_bedrock_target(self):
        data = {}; deposits(lambda path, body: data.__setitem__(path, body))
        self.assertEqual(len(data), 10)
        for dimension, y in [('overworld', -64), ('nether', 0)]:
            name = 'bedrock_' + dimension
            feature = data[f'data/ntm/worldgen/configured_feature/{name}.json']
            self.assertEqual(feature['type'], 'minecraft:disk')
            self.assertEqual((feature['radius'], feature['half_height']), (1, 0))
            self.assertEqual(feature['target']['blocks'], ['minecraft:bedrock'])
            placed = data[f'data/ntm/worldgen/placed_feature/{name}.json']['placement']
            self.assertEqual(placed[0], {'type': 'minecraft:rarity_filter', 'chance': 10})
            self.assertEqual(placed[-1]['height']['value']['absolute'], y)
            structure = data[f'data/ntm/worldgen/structure/{name}.json']
            self.assertEqual(structure['biomes'], '#minecraft:is_' + dimension)
        for path, value in data.items(): self.assertEqual(json.loads((ROOT / 'content' / path).read_text()), value)

    def test_regeneration_is_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(export(SOURCE, Path(directory), list(self.rows.values())), self.data)


if __name__ == '__main__': unittest.main()

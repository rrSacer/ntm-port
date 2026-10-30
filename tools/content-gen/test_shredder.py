#!/usr/bin/env python3
"""Source examples, precedence and schema compatibility for shredder content."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from shredder_recipes import export

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class ShredderContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {row['id']: row for row in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/shredder.json').read_text())
        cls.recipes = {row['input']['ids'][0]: row for row in cls.data['recipes']}

    def test_source_precedence_keeps_explicit_outputs(self):
        for ident, output in [('minecraft:diamond_ore', ['ntm:gravel_diamond', 2]),
                              ('minecraft:quartz_block', ['ntm:powder_quartz', 4]),
                              ('ntm:ore_aluminium', ['ntm:chunk_ore__cryolite', 2])]:
            self.assertEqual(self.recipes[ident]['outputs'], [output])
            collision = next(row for row in self.data['collisions'] if row['input'] == ident)
            self.assertEqual(collision['kept']['phase'], 'defaults')
            self.assertEqual(collision['ignored']['phase'], 'post')
            self.assertNotEqual(collision['ignored']['outputs'], [output])

    def test_post_ore_block_and_dust_rules(self):
        expected = {'minecraft:iron_ore': ['ntm:powder_iron', 2], 'minecraft:iron_ingot': ['ntm:powder_iron', 1],
                    'minecraft:iron_block': ['ntm:powder_iron', 9], 'ntm:block_sulfur': ['ntm:sulfur', 4],
                    'ntm:powder_steel_tiny': ['ntm:dust_tiny', 1], 'ntm:powder_steel': ['ntm:dust', 1]}
        for ident, output in expected.items(): self.assertEqual(self.recipes[ident]['outputs'], [output])
        self.assertEqual(self.recipes['ntm:powder_lapis']['outputs'], [['ntm:powder_cobalt_tiny', 1]])
        self.assertFalse(any('Any' in row.get('ore_key', '') for row in self.data['recipes']))
        self.assertNotIn('minecraft:deepslate_iron_ore', self.recipes)

    def test_finite_source_loops_and_metadata(self):
        for ident in ('minecraft:oak_log', 'minecraft:dark_oak_log'):
            self.assertEqual(self.recipes[ident]['outputs'], [['ntm:powder_sawdust', 4]])
        self.assertEqual(self.recipes['minecraft:oak_planks']['outputs'], [['ntm:powder_sawdust', 1]])
        self.assertEqual(self.recipes['minecraft:blue_wool']['outputs'], [['minecraft:string', 4]])
        self.assertEqual(self.recipes['minecraft:quartz_pillar']['outputs'], [['ntm:powder_quartz', 4]])
        self.assertEqual(self.recipes['ntm:ore_bedrock__iron']['outputs'], [['ntm:ore_enriched__iron', 1]])
        for row in self.rows.values():
            if row['source_variable'] != 'bedrock_ore': continue
            self.assertEqual(self.recipes[row['id']]['outputs'], [['minecraft:gravel', 1]])
            self.assertTrue(self.recipes[row['id']]['wildcard_fallback'])

    def test_all_ids_labels_and_skips_are_valid(self):
        self.assertEqual(len(self.recipes), len(self.data['recipes']))
        vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/items')
        for row in self.data['recipes']:
            self.assertEqual(row['input']['count'], 1)
            self.assertTrue(row['label']); self.assertTrue(row['label_en'])
            for ident in row['input']['ids'] + [value[0] for value in row['outputs']]:
                if ident.startswith('ntm:'): self.assertIn(ident, self.rows)
                else: self.assertTrue((vanilla / (ident.split(':')[1] + '.json')).is_file(), ident)
            self.assertGreater(row['outputs'][0][1], 0)
        for row in self.data['skipped']: self.assertTrue(row['reason']); self.assertTrue(row['source'])
        self.assertTrue(any('BobbleType' in row['reason'] for row in self.data['skipped']))

    def test_regeneration_is_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            generated = export(SOURCE, Path(directory), list(self.rows.values()))
            self.assertEqual(generated, self.data)

    def test_blades_and_machine_contract(self):
        self.assertEqual({row['id']: row['durability'] for row in self.data['blades']},
                         {'ntm:blades_steel': 400, 'ntm:blades_titanium': 500, 'ntm:blades_desh': 0})
        row = self.rows['ntm:machine_shredder']
        self.assertIn(row['id'], self.catalog['machines'])
        self.assertEqual(row['template'], 'minecraft:beehive')
        self.assertEqual(row['logic']['level'], {'prop': 'honey_level', 'max': 5})
        states = json.loads((ROOT / 'content/assets/ntm/blockstates/machine_shredder.json').read_text())
        self.assertEqual(len(states['variants']), 24)

    def test_damage_components_are_default_and_desh_never_gets_them(self):
        entries = [{'id': 'blades_' + material, 'source_variable': 'blades_' + material, 'type': 'item',
                    'category': 'component', 'max_stack_size': 1, **({'max_damage': damage} if damage else {})}
                   for material, damage in [('steel', 400), ('titanium', 500), ('desh', 0)]]
        stamp = self.rows['ntm:stamp_357']
        entries.append({'id': 'stamp_357', 'source_variable': 'stamp_357', 'type': 'item',
                        'category': 'component', 'max_stack_size': 1, 'max_damage': stamp['max_damage']})
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory); spec = path / 'catalog.json'; spec.write_text(json.dumps({'entries': entries}))
            for enabled in (False, True):
                out = path / str(enabled)
                command = [sys.executable, str(ROOT / 'tools/content-gen/generate.py'), '--catalog', str(spec), '--output', str(out)]
                if enabled: command.append('--max-damage')
                subprocess.run(command, check=True, capture_output=True)
                for row in entries:
                    declaration = json.loads((out / f'data/ntm/horizon/item/{row["id"]}.json').read_text())
                    self.assertEqual(declaration['max_stack_size'], 1)
                    self.assertNotIn('max_damage', declaration)
                    self.assertEqual(declaration.get('components', {}).get('minecraft:max_damage'), row.get('max_damage'))
                    self.assertEqual(declaration['template'], 'minecraft:iron_ingot')


if __name__ == '__main__': unittest.main()

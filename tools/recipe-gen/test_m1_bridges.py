#!/usr/bin/env python3
"""Check M1 recipes against the 1.7.10 source lines they come from, and the temporary set against coverage.json."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src/src/main/java/com/hbm'
RECIPES = ROOT / 'content/data/ntm/recipe'
EXPECTED_TEMPORARY = {
    'ntm:m1_temp__machine_epress',
    'ntm:m1_temp__machine_battery',
    'ntm:m1_temp__stamp_iron_plate',
    'ntm:m1_temp__stamp_iron_wire',
}

# Each Java-sourced recipe: file, cited line, tokens that must appear on that line, and the JSON it must produce.
# Symbols map as in the Java source: STEEL.plate() -> ntm:plate_steel, CU.plateCast() -> ntm:plate_cast__copper, etc.
JAVA_RECIPES = {
    'machine_wood_burner': ('main/CraftingManager.java', 251,
        ['"PPP", "CFC", "I I"', 'STEEL.plate()', 'ModItems.coil_copper', 'IRON.ingot()', 'Blocks.furnace'],
        {'type': 'minecraft:crafting_shaped', 'category': 'misc', 'pattern': ['PPP', 'CFC', 'I I'],
         'key': {'P': 'ntm:plate_steel', 'C': 'ntm:coil_copper', 'I': 'minecraft:iron_ingot', 'F': 'minecraft:furnace'},
         'result': {'id': 'ntm:machine_wood_burner', 'count': 1}}),
    'machine_electric_furnace': ('main/CraftingManager.java', 230,
        ['"BBB", "WFW", "RRR"', 'BE.ingot()', 'CU.plateCast()', 'ModItems.coil_tungsten', 'Blocks.furnace'],
        {'type': 'minecraft:crafting_shaped', 'category': 'misc', 'pattern': ['BBB', 'WFW', 'RRR'],
         'key': {'B': 'ntm:ingot_beryllium', 'W': 'ntm:plate_cast__copper', 'F': 'minecraft:furnace', 'R': 'ntm:coil_tungsten'},
         'result': {'id': 'ntm:machine_electric_furnace_off', 'count': 1}}),
    'red_cable': ('main/CraftingManager.java', 239,
        ['" W ", "RRR", " W "', 'ModItems.plate_polymer', 'MINGRADE.wireFine()', 'ModBlocks.red_cable, 16'],
        {'type': 'minecraft:crafting_shaped', 'category': 'misc', 'pattern': [' W ', 'RRR', ' W '],
         'key': {'W': 'ntm:plate_polymer', 'R': 'ntm:wire_fine__red_copper'},
         'result': {'id': 'ntm:red_cable', 'count': 16}}),
    'wiring_red_copper': ('main/CraftingManager.java', 539,
        ['"PPP", "PIP", "PPP"', 'STEEL.plate()', 'STEEL.ingot()', 'ModItems.wiring_red_copper'],
        {'type': 'minecraft:crafting_shaped', 'category': 'misc', 'pattern': ['PPP', 'PIP', 'PPP'],
         'key': {'P': 'ntm:plate_steel', 'I': 'ntm:ingot_steel'},
         'result': {'id': 'ntm:wiring_red_copper', 'count': 1}}),
}


class M1Recipes(unittest.TestCase):
    def setUp(self):
        self.coverage = json.loads((ROOT / 'tools/recipe-gen/coverage.json').read_text())

    def test_java_sourced_recipes_match_the_cited_line(self):
        if not SOURCE.is_dir():
            self.skipTest('ntm-src sibling not present')
        for name, (java_file, line, tokens, expected) in JAVA_RECIPES.items():
            source_line = (SOURCE / java_file).read_text().split('\n')[line - 1]
            for token in tokens:
                self.assertIn(token, source_line, f'{name}: {java_file}:{line} lacks {token}')
            produced = json.loads((RECIPES / f'{name}.json').read_text())
            self.assertEqual(produced, expected, name)

    def test_temporary_set_is_named_and_justified(self):
        temporary = [r for r in self.coverage['recipes'] if r['temporary']]
        self.assertEqual(self.coverage['temporary_count'], len(temporary))
        self.assertEqual({r['id'] for r in temporary}, EXPECTED_TEMPORARY)
        for record in temporary:
            self.assertIn('java', record['source'].lower(), record['id'])

    def test_temporary_files_exist(self):
        for recipe_id in EXPECTED_TEMPORARY:
            self.assertTrue((RECIPES / (recipe_id.split(':', 1)[1] + '.json')).is_file(), recipe_id)

    def test_no_stale_bridge_files_remain(self):
        stale = sorted(p.name for p in RECIPES.glob('m1_temp__*.json'))
        self.assertEqual(stale, sorted(e.split(':', 1)[1] + '.json' for e in EXPECTED_TEMPORARY))

    def test_removed_lbsm_only_recipe_is_absent(self):
        self.assertFalse((RECIPES / 'powder_red_copper.json').exists())


if __name__ == '__main__':
    unittest.main()

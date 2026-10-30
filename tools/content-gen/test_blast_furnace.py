#!/usr/bin/env python3
"""Ordered blast recipes, exact metadata and conservative fuel contracts."""
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from blast_furnace_recipes import export, fuel_table, FUEL
from shredder_recipes import ShredderResolver, dictionary

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class BlastFurnaceContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {row['id']: row for row in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/blast_furnace.json').read_text())
        cls.recipes = {row['name']: row for row in cls.data['recipes']}
        cls.resolver = ShredderResolver(SOURCE, list(cls.rows.values()))
        cls.ores, _ = dictionary(cls.resolver)

    def test_order_and_explicit_missing_content(self):
        self.assertEqual(self.data['coverage'], {'source_recipes': 15, 'emitted': 12, 'skipped': 3, 'fuels': 20})
        self.assertEqual([r['source_index'] for r in self.data['recipes']], list(range(9)) + [12, 13, 14])
        self.assertEqual([r['source_index'] for r in self.data['skipped']], [9, 10, 11])
        for row in self.data['recipes'] + self.data['skipped']:
            self.assertEqual(self.data['source_recipe_order'][row['source_index']], row['name'])
        expected = [['ModItems.meteorite_sword_hardened', 'ModItems.meteorite_sword_alloyed'],
                    ['ModItems.ingot_meteorite'], ['ModItems.ingot_meteorite']]
        self.assertEqual([r['missing_items'] for r in self.data['skipped']], expected)

    def test_steel_slag_sand_and_ore_alternatives(self):
        for name, duration, steel, slag in [('steelFromIngot', 800, 2, 1), ('steelFromDust', 800, 2, 1),
                                            ('steelFromOre', 800, 2, 2), ('steelWithFlux', 1200, 3, 2)]:
            row = self.recipes['blast.' + name]
            self.assertEqual(row['duration'], duration)
            self.assertEqual(row['outputItems'], [['ntm:ingot_steel', steel], ['ntm:ingot_raw__slag', slag]])
        inputs = self.recipes['blast.steelFromIngot']['inputItems']
        self.assertEqual(inputs[0]['count'], 2)
        self.assertEqual(set(inputs[1]['ids']), {'minecraft:sand', 'minecraft:red_sand'})
        self.assertIn('minecraft:iron_ore', self.recipes['blast.steelFromOre']['inputItems'][0]['ids'])
        self.assertEqual(self.recipes['blast.mingradeOre']['inputItems'][1]['count'], 6)
        self.assertEqual(self.recipes['blast.firebrick']['outputItems'], [['ntm:ingot_firebrick', 8]])

    def test_all_ids_are_registered(self):
        vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/items')
        for row in self.data['recipes']:
            self.assertTrue(row['label']); self.assertTrue(row['label_en'])
            ids = [ident for stack in row['inputItems'] for ident in stack['ids']]
            ids += [stack[0] for stack in row['outputItems']]
            for ident in ids:
                if ident.startswith('ntm:'): self.assertIn(ident, self.rows)
                else: self.assertTrue((vanilla / (ident.split(':')[1] + '.json')).is_file(), ident)
            self.assertTrue(all(stack['count'] > 0 for stack in row['inputItems']))
            self.assertTrue(all(stack[1] > 0 for stack in row['outputItems']))

    def test_exact_fuels_and_exclusions(self):
        fuels = self.data['fuels']
        for ident, ticks in [('minecraft:coal', 1600), ('minecraft:coal_block', 16000),
                             ('ntm:coke__petroleum', 3200), ('ntm:block_coke__lignite', 32000),
                             ('ntm:briquette__coal', 2000), ('ntm:briquette__lignite', 1600),
                             ('ntm:powder_lignite', 1200), ('ntm:rocket_fuel', 6400),
                             ('ntm:solid_fuel_presto_triplet_bf', 400000)]:
            self.assertEqual(fuels[ident], ticks)
        for ident in ['ntm:briquette__wood', 'ntm:coal_eternal', 'ntm:scrap', 'ntm:coal_infernal',
                      'minecraft:charcoal', 'minecraft:oak_log', 'minecraft:lava_bucket']:
            self.assertNotIn(ident, fuels)
        detail = next(r for r in self.data['fuel_details'] if r['item'] == 'ntm:solid_fuel_bf')
        self.assertEqual(detail['category'], 'balefire')
        self.assertFalse(self.data['fuel_gaps'])

    def test_container_and_unsupported_fuels_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            item_path = 'src/main/java/com/hbm/items/ModItems.java'
            for relative in (FUEL, item_path):
                (source / relative).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy(SOURCE / relative, source / relative)
            items = source / item_path
            items.write_text(items.read_text() + '\nsolid_fuel.setContainerItem(solid_fuel);\n')
            table, _, rejected, _ = fuel_table(source, list(self.rows.values()), self.resolver, self.ores)
            self.assertNotIn('ntm:solid_fuel', table)
            self.assertIn({'item': 'ntm:solid_fuel', 'reason': 'hasContainerItem', 'base_ticks': 3200, 'ore_keys': []}, rejected)
            fuel = source / FUEL
            fuel.write_text(fuel.read_text().replace('return single * 32;', 'return unknownBurnTime();'))
            table, _, _, gaps = fuel_table(source, list(self.rows.values()), self.resolver, self.ores)
            self.assertNotIn('ntm:rocket_fuel', table)
            self.assertTrue(any(r['variable'] == 'rocket_fuel' for r in gaps))

    def test_machine_contract_and_determinism(self):
        row = self.rows['ntm:machine_blast_furnace']
        self.assertIn(row['id'], self.catalog['machines'])
        self.assertEqual(row['template'], 'minecraft:beehive')
        self.assertEqual(row['logic'], {'kind': 'machine', 'active': None, 'facing': 'facing', 'level': {'prop': 'honey_level', 'max': 5}})
        state = json.loads((ROOT / 'content/assets/ntm/blockstates/machine_blast_furnace.json').read_text())
        self.assertEqual(len(state['variants']), 24)
        self.assertEqual(row['names'], {'en_us': 'Blast Furnace', 'pt_br': 'Alto-Forno'})
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(export(SOURCE, Path(directory), list(self.rows.values())), self.data)


if __name__ == '__main__': unittest.main()

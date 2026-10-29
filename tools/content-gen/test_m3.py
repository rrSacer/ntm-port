#!/usr/bin/env python3
"""Source examples and completeness/conservation contracts for M3 exports."""
import json
import unittest
import struct
import zlib
from pathlib import Path
from machine_recipes import integer, MachineResolver, Unsupported
from m3_catalog import fluids
from generate import transform_png

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class M3Contracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {row['id']: row for row in cls.catalog['entries']}
        cls.fluids = {fluid['id']: fluid for fluid in fluids(SOURCE)}
        cls.exports = {machine: json.loads((ROOT / f'content/machine-recipes/{machine}.json').read_text())
                       for machine in ('assembly', 'chemical_plant', 'centrifuge')}

    def test_fluid_registry_and_source_container_restrictions(self):
        self.assertEqual(len(self.fluids), 157)
        self.assertEqual(self.fluids['titanium_tetrachloride']['metadata'], 156)
        self.assertEqual(self.fluids['water']['metadata'], 1)
        runtime = json.loads((ROOT / 'crates/ntm-core/data/fluids.json').read_text())['fluids']
        for fluid in runtime:
            rendering = self.fluids[fluid['string_id']]
            self.assertEqual(rendering['metadata'], fluid['id'])
            self.assertEqual(rendering['temp'], fluid['temp'])
            self.assertEqual(int(rendering['color'], 16), fluid['color'])
            self.assertEqual(rendering['no_container'], fluid.get('no_container', False))
            self.assertEqual(rendering['lead_container'], fluid.get('lead_container', False))
        for fluid in list(self.fluids.values())[1:]:
            for family in ('fluid_tank_lead_full', 'fluid_pack_full', 'fluid_tank_full', 'fluid_barrel_full'):
                ident = 'ntm:' + family + '__' + fluid['id']
                expected = not fluid['no_container'] and (family in ('fluid_tank_lead_full', 'fluid_pack_full') or not fluid['lead_container'])
                self.assertEqual(ident in self.rows, expected, ident)
            for family, descriptor in [('canister_full', 'Canister'), ('gas_full', 'Gastank')]:
                self.assertEqual('ntm:' + family + '__' + fluid['id'] in self.rows, descriptor in fluid['descriptors'])
        for row in self.rows.values():
            if row['category'] != 'fluid_container': continue
            self.assertIn(row['fluid']['empty'], self.rows)
            self.assertEqual(row['metadata'], self.fluids[row['fluid']['id']]['metadata'])

    def test_machine_and_duct_states(self):
        for name in ('machine_assembly_machine', 'machine_chemical_plant', 'machine_centrifuge', 'machine_gascent', 'barrel_steel', 'machine_fluidtank'):
            row = self.rows['ntm:' + name]
            self.assertEqual(row['template'], 'minecraft:beehive')
            state = json.loads((ROOT / f'content/assets/ntm/blockstates/{name}.json').read_text())
            self.assertEqual(len(state['variants']), 24)
            self.assertIn('facing=west,honey_level=5', state['variants'])
        self.assertEqual(self.catalog['ducts'], ['ntm:fluid_duct_neo'])
        self.assertEqual(json.loads((ROOT / 'content/assets/ntm/blockstates/fluid_duct_neo.json').read_text())['variants'].keys(), {''})

    def test_machine_cube_faces_are_opaque(self):
        for name in ('machine_assembly_machine', 'machine_chemical_plant', 'machine_centrifuge', 'machine_gascent', 'barrel_steel', 'machine_fluidtank', 'fluid_duct_neo'):
            suffix = '' if name == 'fluid_duct_neo' else '_0'
            model = json.loads((ROOT / f'content/assets/ntm/models/block/{name}{suffix}.json').read_text())
            for face in ('down', 'up', 'north', 'south', 'east', 'west'):
                namespace, path = model['textures'].get(face, model['textures'].get('all')).split(':')
                raw = transform_png((ROOT / f'content/assets/{namespace}/textures/{path}.png').read_bytes())
                width, height = struct.unpack('>II', raw[16:24]); pos = 8; compressed = b''
                while pos < len(raw):
                    length = struct.unpack('>I', raw[pos:pos + 4])[0]
                    if raw[pos + 4:pos + 8] == b'IDAT': compressed += raw[pos + 8:pos + 8 + length]
                    pos += length + 12
                pixels = zlib.decompress(compressed)
                self.assertTrue(all(pixels[y * (width * 4 + 1) + 4 + x * 4] == 255 for y in range(height) for x in range(width)), (name, face))

    def test_known_assembly_and_chemistry_quantities(self):
        assembly = {recipe['name']: recipe for recipe in self.exports['assembly']['recipes']}
        iron = assembly['ass.plateiron']
        self.assertEqual((iron['duration'], iron['power']), (60, 100))
        self.assertEqual(iron['inputItems'], [{'ids': ['minecraft:iron_ingot'], 'count': 1}])
        self.assertEqual(iron['outputItems'], [['ntm:plate_iron', 1]])
        chemistry = {recipe['name']: recipe for recipe in self.exports['chemical_plant']['recipes']}
        hydrogen = chemistry['chem.hydrogen']
        self.assertEqual(hydrogen['inputItems'], [{'ids': ['minecraft:coal'], 'count': 1}])
        self.assertEqual(hydrogen['inputFluids'], [['water', 8000]])
        self.assertEqual(hydrogen['outputFluids'], [['hydrogen', 500]])

    def test_package_fluid_conservation_and_count(self):
        recipes = {recipe['name']: recipe for recipe in self.exports['assembly']['recipes']}
        packs = [recipe for recipe in recipes.values() if 'expansion' in recipe]
        self.assertEqual(len(packs), 276)
        for fluid in list(self.fluids.values())[1:]:
            if fluid['no_container']: continue
            pack = recipes['ass.packagehbmfluid.' + fluid['id']]
            unpack = recipes['ass.unpackagehbmfluid.' + fluid['id']]
            self.assertEqual(pack['inputFluids'], [[fluid['id'], 32000]])
            self.assertEqual(unpack['outputFluids'], pack['inputFluids'])
            self.assertEqual(pack['outputItems'][0][0], unpack['inputItems'][0]['ids'][0])
            self.assertTrue(pack['label'].startswith('Empacotar: '))
            self.assertTrue(unpack['label'].startswith('Desempacotar: '))

    def test_no_external_or_dynamic_recipe_is_silently_emitted(self):
        assembly = self.exports['assembly']
        self.assertFalse(any(recipe['name'] == 'ass.digimemer' for recipe in assembly['recipes']))
        self.assertTrue(any(recipe['name'] == 'ass.digimemer' for recipe in assembly['skipped']))
        for export in self.exports.values():
            for skipped in export['skipped']:
                self.assertTrue(skipped['reason'])
                self.assertIn('.java:', skipped['source'])
            for recipe in export['recipes']:
                self.assertTrue(recipe['label']); self.assertTrue(recipe['label_en'])
                self.assertNotIn('%s', recipe['label'])
                ingredients = [recipe['input']] if 'input' in recipe else recipe['inputItems']
                for ingredient in ingredients:
                    self.assertGreater(ingredient['count'], 0)
                    self.assertTrue(ingredient['ids'])
                    for ident in ingredient['ids']:
                        if ident.startswith('ntm:'): self.assertIn(ident, self.rows)
                for ident, count in recipe.get('outputs', recipe.get('outputItems', [])):
                    self.assertGreater(count, 0)
                    if ident.startswith('ntm:'): self.assertIn(ident, self.rows)

    def test_safe_literal_arithmetic_and_nondefault_rejection(self):
        self.assertEqual(integer('60 * 20'), 1200)
        self.assertEqual(integer('4_000L'), 4000)
        with self.assertRaises(Unsupported): integer('__import__("os").system("false")')
        resolver = MachineResolver(SOURCE, list(self.rows.values()))
        self.assertEqual(resolver.item('lbs ? new ItemStack(ModItems.powder_gold, 2) : new ItemStack(ModItems.powder_gold, 1)'), ('ntm:powder_gold', 1))
        with self.assertRaises(Unsupported): resolver.fluid('new FluidStack(Fluids.WATER, 1000, 5)')


if __name__ == '__main__': unittest.main()

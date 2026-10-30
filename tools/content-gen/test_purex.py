#!/usr/bin/env python3
"""PUREX block contracts, assembly recipe emission, source recipe order, autoSwitchGroup and determinism."""
import json
import os
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib
from pathlib import Path
from generate import transform_png
from purex_recipes import export

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class PurexContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {row['id']: row for row in cls.catalog['entries']}
        cls.purex = json.loads((ROOT / 'content/machine-recipes/purex.json').read_text())
        cls.by_name = {row['name']: row for row in cls.purex['recipes']}
        cls.skipped_by_name = {row['name']: row for row in cls.purex['skipped']}
        cls.assembly = json.loads((ROOT / 'content/machine-recipes/assembly.json').read_text())
        cls.runtime_fluids = {
            f['string_id'] for f in json.loads((ROOT / 'crates/ntm-core/data/fluids.json').read_text())['fluids']
        }

    def test_machine_purex_block_contract_and_assets(self):
        self.assertIn('ntm:machine_purex', self.rows)
        self.assertIn('ntm:machine_purex', self.catalog['machines'])
        row = self.rows['ntm:machine_purex']
        chem = self.rows['ntm:machine_chemical_plant']
        self.assertEqual(row['type'], 'block')
        self.assertEqual(row['category'], 'machine')
        self.assertEqual(row['template'], 'minecraft:beehive')
        self.assertEqual(row['names'], {'en_us': 'PUREX', 'pt_br': 'PUREX'})
        self.assertEqual(
            row['logic'],
            {'kind': 'machine', 'facing': 'facing', 'active': None, 'level': {'prop': 'honey_level', 'max': 5}},
        )
        self.assertEqual(row['logic'], chem['logic'])

        block_decl = json.loads((ROOT / 'content/data/ntm/horizon/block/machine_purex.json').read_text())
        chem_decl = json.loads((ROOT / 'content/data/ntm/horizon/block/machine_chemical_plant.json').read_text())
        self.assertEqual(block_decl, chem_decl)

        loot = json.loads((ROOT / 'content/data/ntm/loot_table/blocks/machine_purex.json').read_text())
        self.assertEqual(
            loot,
            {
                'type': 'minecraft:block',
                'pools': [{'rolls': 1, 'entries': [{'type': 'minecraft:item', 'name': 'ntm:machine_purex'}]}],
            },
        )

        state = json.loads((ROOT / 'content/assets/ntm/blockstates/machine_purex.json').read_text())
        self.assertEqual(len(state['variants']), 24)
        self.assertIn('facing=west,honey_level=5', state['variants'])

        item_model = json.loads((ROOT / 'content/assets/ntm/items/machine_purex.json').read_text())
        self.assertEqual(item_model, {'model': {'type': 'minecraft:model', 'model': 'ntm:block/machine_purex_0'}})

        for loc in ('en_us', 'pt_br'):
            lang_data = json.loads((ROOT / f'content/assets/ntm/lang/{loc}.json').read_text())
            self.assertEqual(lang_data['block.ntm.machine_purex'], 'PUREX')

        tabs = [
            json.loads(p.read_text())
            for p in sorted((ROOT / 'content/data/ntm/creative_tab').glob('*.json'))
        ]
        machines_tab = next(t for t in tabs if t['title'] == 'itemGroup.ntm.machines')
        self.assertIn('ntm:machine_purex', machines_tab['items'])

        model = json.loads((ROOT / 'content/assets/ntm/models/block/machine_purex_0.json').read_text())
        for face in ('down', 'up', 'north', 'south', 'east', 'west'):
            namespace, path = model['textures'][face].split(':')
            raw = transform_png((ROOT / f'content/assets/{namespace}/textures/{path}.png').read_bytes())
            width, height = struct.unpack('>II', raw[16:24])
            pos = 8
            compressed = b''
            while pos < len(raw):
                length = struct.unpack('>I', raw[pos:pos + 4])[0]
                if raw[pos + 4:pos + 8] == b'IDAT':
                    compressed += raw[pos + 8:pos + 8 + length]
                pos += length + 12
            pixels = zlib.decompress(compressed)
            self.assertTrue(
                all(pixels[y * (width * 4 + 1) + 4 + x * 4] == 255 for y in range(height) for x in range(width)),
                face,
            )

        # Verify visual-gen ownership was respected.
        visual_owned = set(json.loads((ROOT / 'content/.visual-files.json').read_text()))
        content_owned = set(json.loads((ROOT / 'content/.content-gen-files.json').read_text()))
        self.assertEqual(visual_owned & content_owned, set())

    def test_ass_purex_emitted_in_assembly_machine(self):
        by_name = {r['name']: r for r in self.assembly['recipes']}
        self.assertIn('ass.purex', by_name)
        self.assertFalse(any(r['name'] == 'ass.purex' for r in self.assembly['skipped']))
        rec = by_name['ass.purex']
        self.assertEqual((rec['duration'], rec['power']), (300, 100))
        self.assertEqual(rec['outputItems'], [['ntm:machine_purex', 1]])
        self.assertEqual(
            rec['inputItems'],
            [
                {'ids': ['ntm:shell__steel'], 'count': 4},
                {'ids': ['ntm:pipe__rubber'], 'count': 8},
                {'ids': ['ntm:plate_cast__lead'], 'count': 4},
                {'ids': ['ntm:motor_desh'], 'count': 1},
                {'ids': ['ntm:circuit__basic'], 'count': 4},
            ],
        )

    def test_coverage_partition_and_source_order(self):
        self.assertEqual(self.purex['schema'], 1)
        self.assertEqual(self.purex['machine'], 'purex')
        cov = self.purex['coverage']
        self.assertEqual(cov['scanned_source_calls'], 59)
        self.assertEqual(cov['emitted'], 37)
        self.assertEqual(cov['skipped_source_calls'], 22)
        self.assertEqual(
            cov['by_family'],
            {
                'synthesis': {'emitted': 3, 'skipped': 0},
                'pile': {'emitted': 4, 'skipped': 0},
                'zirnox': {'emitted': 0, 'skipped': 9},
                'plate': {'emitted': 0, 'skipped': 7},
                'pwr': {'emitted': 15, 'skipped': 0},
                'molten_salt': {'emitted': 1, 'skipped': 0},
                'watz': {'emitted': 10, 'skipped': 2},
                'icf': {'emitted': 1, 'skipped': 0},
                'vitrification': {'emitted': 0, 'skipped': 3},
                'schrabidium': {'emitted': 3, 'skipped': 1},
            },
        )

        emitted_indices = [r['source_index'] for r in self.purex['recipes']]
        self.assertEqual(emitted_indices, sorted(emitted_indices))
        all_indices = emitted_indices + [r['source_index'] for r in self.purex['skipped']]
        self.assertEqual(sorted(all_indices), list(range(59)))
        self.assertEqual(len(self.purex['source_recipe_order']), 59)
        for row in self.purex['recipes'] + self.purex['skipped']:
            self.assertEqual(self.purex['source_recipe_order'][row['source_index']], row['name'])
            self.assertTrue(row['source'].startswith('PUREXRecipes.java:'), row['source'])

    def test_synthesis_pwr_molten_salt_icf_and_schrab_recipes(self):
        uzh = self.by_name['purex.uzh']
        self.assertEqual((uzh['duration'], uzh['power']), (600, 1000))
        self.assertIsNone(uzh['autoSwitchGroup'])
        self.assertEqual(
            uzh['inputItems'],
            [{'ids': ['ntm:billet_uranium_fuel'], 'count': 1}, {'ids': ['ntm:billet_zirconium'], 'count': 3}],
        )
        self.assertEqual(uzh['inputFluids'], [['nitric_acid', 1000], ['hydrogen', 4000]])
        self.assertEqual(uzh['outputItems'], [['ntm:billet_uzh', 4, 1.0]])
        self.assertEqual(uzh['outputFluids'], [])

        fg = self.by_name['purex.flashgold']
        self.assertEqual(
            fg['inputItems'],
            [{'ids': ['ntm:billet_au198'], 'count': 1}, {'ids': ['ntm:pellet_charged'], 'count': 1}],
        )
        self.assertEqual(fg['inputFluids'], [['amat', 1000]])
        self.assertEqual(fg['outputItems'], [['ntm:billet_balefire_gold', 2, 1.0]])

        fl = self.by_name['purex.flashlead']
        self.assertEqual(
            fl['inputItems'],
            [{'ids': ['ntm:billet_pb209'], 'count': 1}, {'ids': ['ntm:billet_balefire_gold'], 'count': 1}],
        )
        self.assertEqual(fl['inputFluids'], [['amat', 1000]])
        self.assertEqual(fl['outputItems'], [['ntm:billet_flashlead', 1, 1.0]])

        srn = self.by_name['purex.schraranium']
        self.assertEqual(srn['inputItems'], [{'ids':['ntm:ingot_schraranium'],'count':1}])
        self.assertEqual(srn['inputFluids'], [['kerosene',2000],['nitric_acid',1000]])

        pwr_recipes = [r for r in self.purex['recipes'] if r['autoSwitchGroup'] == 'autoswitch.pwr']
        self.assertEqual(len(pwr_recipes), 15)
        for rec in pwr_recipes:
            self.assertEqual((rec['duration'], rec['power']), (100, 2500))
            self.assertEqual(rec['inputFluids'], [['kerosene', 500], ['nitric_acid', 250]])
            self.assertTrue(rec['label'].startswith('Reprocessamento de '), rec['label'])
            self.assertTrue(rec['label_en'].startswith('Reprocessing of '), rec['label_en'])

        pwrmeu = self.by_name['purex.pwrmeu']
        self.assertEqual(pwrmeu['inputItems'], [{'ids': ['ntm:pwr_fuel_depleted__meu'], 'count': 1}])
        self.assertEqual(
            pwrmeu['outputItems'],
            [
                ['ntm:nugget_u238', 3, 1.0],
                ['ntm:nugget_plutonium', 4, 1.0],
                ['ntm:nugget_technetium', 2, 1.0],
                ['ntm:nuclear_waste_tiny', 3, 1.0],
            ],
        )

        salt = self.by_name['purex.thoriumsalt']
        self.assertEqual((salt['duration'], salt['power']), (20, 10000))
        self.assertIsNone(salt['autoSwitchGroup'])
        self.assertEqual(salt['inputItems'], [{'ids': ['ntm:nugget_th232'], 'count': 2}])
        self.assertEqual(salt['inputFluids'], [['thorium_salt_depleted', 16000]])
        self.assertEqual(salt['outputFluids'], [['thorium_salt', 16000]])
        self.assertEqual(
            salt['outputItems'],
            [['ntm:nugget_u233', 1, 0.5], ['ntm:nuclear_waste_tiny', 1, 0.25]],
        )

        icf = self.by_name['purex.icf']
        self.assertEqual((icf['duration'], icf['power']), (300, 10000))
        self.assertIsNone(icf['autoSwitchGroup'])
        self.assertEqual(icf['inputItems'], [{'ids': ['ntm:icf_pellet_depleted'], 'count': 1}])
        self.assertEqual(
            icf['outputItems'],
            [
                ['ntm:icf_pellet_empty', 1, 1.0],
                ['ntm:pellet_charged', 1, 1.0],
                ['ntm:powder_iron', 1, 1.0],
            ],
        )
        self.assertEqual(icf['outputFluids'], [['helium4', 1250]])

        for name, rod in [('purex.schrabpwr', 'ntm:pwr_fuel_depleted__mep'), ('purex.schrabmen', 'ntm:pwr_fuel_depleted__men')]:
            rec = self.by_name[name]
            self.assertEqual((rec['duration'], rec['power']), (200, 50000))
            self.assertEqual(rec['autoSwitchGroup'], 'autoswitch.schrab')
            self.assertEqual(rec['inputItems'], [{'ids': [rod], 'count': 1}])
            self.assertEqual(rec['inputFluids'], [['solvent', 4000], ['schrabidic', 250]])
            self.assertEqual(
                rec['outputItems'],
                [
                    ['ntm:powder_schrabidium', 1, 1.0],
                    ['ntm:nugget_technetium', 3, 1.0],
                    ['ntm:nuclear_waste_tiny', 4, 1.0],
                ],
            )
            self.assertTrue(rec['label'].startswith('Extração de Schrabídio de '), rec['label'])
            self.assertTrue(rec['label_en'].startswith('Schrabidium extraction from '), rec['label_en'])

    def test_naquadria_conditional_and_duplicate_output_preservation(self):
        for name in ('purex.watznaqadah', 'purex.watznaqadria'):
            self.assertNotIn(name, self.by_name)
            self.assertIn(name, self.skipped_by_name)
            self.assertIn('nuggetNaquadria', self.skipped_by_name[name]['reason'])

        # Verify that duplicate outputs (like zirnoxles listing nuclear_waste_tiny twice)
        # remain two separate entries rather than being merged, when the input item exists.
        synthetic_rows = list(self.rows.values()) + [
            {
                'id': 'ntm:waste_schrabidium',
                'source_variable': 'waste_schrabidium',
                'type': 'item',
                'category': 'component',
                'names': {'en_us': 'Depleted Schrabidium ZIRNOX Rod', 'pt_br': 'Haste ZIRNOX de Schrabídio Esgotada'},
            }
        ]
        with tempfile.TemporaryDirectory(prefix='ntm-purex-dup-') as tmp:
            synthetic = export(SOURCE, Path(tmp), synthetic_rows)
            zirnoxles = next(r for r in synthetic['recipes'] if r['name'] == 'purex.zirnoxles')
            self.assertEqual(zirnoxles['autoSwitchGroup'], 'autoswitch.zirnox')
            self.assertEqual(
                zirnoxles['outputItems'],
                [
                    ['ntm:nugget_beryllium', 2, 1.0],
                    ['ntm:nugget_pu239', 1, 1.0],
                    ['ntm:nuclear_waste_tiny', 1, 1.0],
                    ['ntm:nuclear_waste_tiny', 2, 1.0],
                ],
            )

    def test_limits_catalog_references_fluids_and_hashseed_determinism(self):
        vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/items')
        for rec in self.purex['recipes']:
            self.assertLessEqual(len(rec['inputItems']), 3, rec['name'])
            self.assertLessEqual(len(rec['outputItems']), 6, rec['name'])
            self.assertLessEqual(len(rec['inputFluids']), 3, rec['name'])
            self.assertLessEqual(len(rec['outputFluids']), 1, rec['name'])
            self.assertTrue(rec['label'])
            self.assertTrue(rec['label_en'])
            self.assertNotIn('%s', rec['label'])
            self.assertNotIn('%s', rec['label_en'])
            self.assertTrue(rec['autoSwitchGroup'] is None or isinstance(rec['autoSwitchGroup'], str))
            for ing in rec['inputItems']:
                self.assertGreater(ing['count'], 0)
                self.assertTrue(ing['ids'])
                for ident in ing['ids']:
                    if ident.startswith('ntm:'):
                        self.assertIn(ident, self.rows, ident)
                    else:
                        self.assertTrue((vanilla / (ident.split(':', 1)[1] + '.json')).is_file(), ident)
            for ident, count, chance in rec['outputItems']:
                self.assertGreater(count, 0)
                self.assertGreater(chance, 0.0)
                self.assertLessEqual(chance, 1.0)
                if ident.startswith('ntm:'):
                    self.assertIn(ident, self.rows, ident)
                else:
                    self.assertTrue((vanilla / (ident.split(':', 1)[1] + '.json')).is_file(), ident)
            for fid, mb in rec['inputFluids'] + rec['outputFluids']:
                self.assertIn(fid, self.runtime_fluids, (rec['name'], fid))
                self.assertGreater(mb, 0)

        expected = (ROOT / 'content/machine-recipes/purex.json').read_bytes()
        for seed in ('1', '42'):
            code = (
                "import json, tempfile; from pathlib import Path; import purex_recipes; "
                f"root = Path({str(ROOT)!r}); rows = json.loads((root / 'content/ids.json').read_text())['entries']; "
                "tmp = Path(tempfile.mkdtemp(prefix='ntm-purex-seed-')); "
                "purex_recipes.export(root.parent / 'ntm-src', tmp, rows); "
                "assert (tmp / 'machine-recipes/purex.json').read_bytes() == (root / 'content/machine-recipes/purex.json').read_bytes()"
            )
            env = {**os.environ, 'PYTHONHASHSEED': seed, 'PYTHONPATH': str(ROOT / 'tools/content-gen')}
            subprocess.run([sys.executable, '-c', code], check=True, env=env, stdout=subprocess.DEVNULL)
        self.assertTrue(expected)


if __name__ == '__main__':
    unittest.main()

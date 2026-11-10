"""K3 boundaries: source tile contracts, unified door IDs and geometry."""
import json
import unittest
from pathlib import Path
from catalog_k3 import build
from bulk_recipes import Resolver

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class CatalogK3(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.rows=build(SOURCE)

    def test_source_tile_does_not_turn_every_door_into_a_machine(self):
        by={r['id']:r for r in self.rows}
        for name in ['vault_door','blast_door','seal_hatch']:
            self.assertTrue(by[name]['source_has_tile'])
            self.assertEqual(by[name]['template'],'minecraft:beehive')
        for name in ['door_metal','door_office','door_bunker','door_red']:
            self.assertFalse(by[name]['source_has_tile'])
            self.assertEqual(by[name]['template'],'minecraft:oak_door')
            self.assertEqual(by[name]['source_item_variable'],name)
            self.assertNotIn(name+'__block',by)

    def test_simple_doors_keep_the_original_door_sound(self):
        # BlockModDoor plays ntm:block.opendoor opening and closing (docs/sons-hbm.md). It used to be written by
        # hand into the four generated block files and every regeneration took it out: the catalog now declares
        # it (`block_properties`) and the generator emits it, so both ends are checked, the declaration and the
        # block definition the game loads.
        expected={'open':'ntm:block.opendoor','close':'ntm:block.opendoor'}
        by={r['id']:r for r in self.rows}
        for name in ['door_metal','door_office','door_bunker','door_red']:
            self.assertEqual(by[name]['block_properties']['door_sounds'],expected,name)
            block=json.loads((ROOT/f'content/data/ntm/horizon/block/{name}.json').read_text())
            self.assertEqual(block['properties']['door_sounds'],expected,name)

    def test_source_items_and_blocks_resolve_to_one_door_identity(self):
        rows=[{**r,'id':'ntm:'+r['id']} for r in self.rows]
        resolver=Resolver(SOURCE,rows)
        for name in ['door_metal','door_office','door_bunker','door_red']:
            self.assertEqual(resolver.item('ModItems.'+name),('ntm:'+name,1))
            self.assertEqual(resolver.item('ModBlocks.'+name),('ntm:'+name,1))

    def test_png_decoder_stops_at_end_chunk(self):
        from generate import transform_png
        raw=(SOURCE/'src/main/resources/assets/hbm/textures/blocks/block_steel.png').read_bytes()
        self.assertEqual(transform_png(raw),transform_png(raw+b'trailing source metadata'))

    def test_source_assets_and_batch_scope(self):
        self.assertEqual(len({r['id'] for r in self.rows}),len(self.rows))
        for row in self.rows:
            self.assertTrue((SOURCE/row['source_texture']).is_file(),row['id'])
            self.assertFalse(row['id'].startswith(('fusion_','icf_','rbmk_','pwr_','watz_','turret_')))
            for path in row.get('door_textures',{}).values():self.assertTrue((SOURCE/path).is_file())

    def test_door_models_have_all_halves_and_hinges(self):
        for name in ['door_metal','door_office','door_bunker','door_red']:
            data=json.loads((ROOT/f'content/assets/ntm/blockstates/{name}.json').read_text())['variants']
            self.assertEqual(len(data),32)
            keys=','.join(data)
            for state in ['half=upper','half=lower','hinge=right','hinge=left','open=true','open=false']:self.assertIn(state,keys)
            loot=json.loads((ROOT/f'content/data/ntm/loot_table/blocks/{name}.json').read_text())
            self.assertEqual(loot['pools'][0]['entries'][0]['conditions'][0]['properties'],{'half':'lower'})

if __name__=='__main__':unittest.main()

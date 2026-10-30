"""Generic class -> form recognition of the bulk importer (K1)."""
import json
import re
import unittest
from pathlib import Path
import bulk

ROOT=Path(__file__).resolve().parents[2]


class StaticCubeRule(unittest.TestCase):
    def test_cube_class_without_shape_override(self):
        form=bulk.static_cube('BlockSlag',['BlockSlag','BlockBeaconable','BlockBase','Block'],'public boolean isBeaconBase(){}','block_slag',lambda s:False,'block_slag')
        self.assertIn('isBeaconBase',form['behavior_gap'])
        self.assertNotIn('surface_textures',form)

    def test_falling_and_face_suffixes(self):
        have={'x_top','x_side'}
        form=bulk.static_cube('BlockCrate',['BlockCrate','BlockFalling','Block'],'',  'crate',lambda s:s in have,'x')
        self.assertEqual(form['template'],'minecraft:gravel')
        self.assertEqual(form['surface_textures'],{'side':'x_side','top':'x_top','bottom':'x_top'})

    def test_other_forms_stay_out(self):
        for parents,body,name in [(['Spotlight','Block'],'public boolean isOpaqueCube(){}','spotlight'),
                                  (['BlockDoorGeneric','Block'],'','door_x'),
                                  (['BlockPane','Block'],'','pane'),
                                  (['BlockGasFlammable','Block'],'','gas'),
                                  (['RedBarrel','BlockDetonatable','Block'],'','barrel'),
                                  (['BlockOre','Block'],'','fusion_x'),
                                  (['MachineFoo','Block'],'','foo')]:
            self.assertIsNone(bulk.static_cube(parents[0],parents,body,name,lambda s:False,'t'),name)

    def test_catalog_has_the_k1_blocks_and_not_the_other_families(self):
        ids={e['id'] for e in json.loads((ROOT/'tools/content-gen/bulk.json').read_text())['entries']}
        for name in ['crate','crate_lead','lamp_tritium_green_on','reinforced_lamp_off','block_waste','stone_depth','sellafield','asphalt_stairs',
                     'lightstone__tile','platemetal__red','vinyl_tile__small','block_cap__nuka','mush_block','tile_lab']:
            self.assertIn(name,ids,name)
        for name in ['door_metal','spotlight_halogen','gas_radon','red_barrel','conveyor','barbed_wire','ladder_gold','hadron_coil_alloy','pwr_fuel']:
            self.assertNotIn(name,ids,name)

    def test_existing_rows_are_frozen(self):
        rows={e['id']:e for e in json.loads((ROOT/'tools/content-gen/bulk.json').read_text())['entries']}
        self.assertNotIn('source_form',rows['pwr_casing'])


if __name__=='__main__':
    unittest.main()

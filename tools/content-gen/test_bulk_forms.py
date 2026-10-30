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


class InertItemRule(unittest.TestCase):
    def test_use_hook_classes_become_inert_items(self):
        form=bulk.static_item('ItemPill',['ItemPill','ItemFood','Item'],'public ItemStack onItemRightClick(){} public int getMaxItemUseDuration(){}','xanax')
        self.assertIn('onItemRightClick',form['behavior_gap'])
        form=bulk.static_item('ItemFilter',['ItemFilter','Item'],'setMaxDamage(1)','gas_mask_filter')
        self.assertEqual(form['max_stack_size'],1)
        self.assertIn('NBT',bulk.static_item('ItemLemon',['ItemLemon','ItemFood'],'new NBTTagCompound()','lemon')['behavior_gap'])

    def test_equipment_render_and_other_families_stay_out(self):
        for cls,chain,body,name in [('ArmorFSB',['ArmorFSB','ItemArmor'],'','steel_helmet'),('ItemToolAbility',['ItemToolAbility','ItemTool'],'','x_pickaxe'),
                                    ('ItemHot',['ItemHot','Item'],'','ingot_meteorite'),('ItemCustom',['ItemCustom','Item'],'getSubItems','thing'),
                                    ('ItemMissile',['ItemMissile','Item'],'','missile_generic'),('ItemGeigerCounter',['ItemGeigerCounter','Item'],'','geiger_counter'),
                                    ('ItemBattery',['ItemBattery','Item'],'','battery_x')]:
            self.assertIsNone(bulk.static_item(cls,chain,body,name),name)

    def test_catalog_has_the_k2_items(self):
        ids={e['id'] for e in json.loads((ROOT/'tools/content-gen/bulk.json').read_text())['entries']}
        for name in ['lemon','radx','xanax','syringe_metal_stimpak','can_smart','nuke_starter_kit','gas_mask_filter','loot_10','cigarette','cheese','bottle_nuka','plastic_bag']:
            self.assertIn(name,ids,name)
        for name in ['missile_generic','gun_uzi','geiger_counter','t51_helmet','hev_plate','wand_d','mp_thruster_10_solid']:
            self.assertNotIn(name,ids,name)


if __name__=='__main__':
    unittest.main()

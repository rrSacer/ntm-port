"""Source boundaries for the current pile and ZIRNOX content snapshots."""
import json
import tempfile
import unittest
from pathlib import Path
from reactor_catalog import build, fuel_rows, pile_fuels, zirnox_fuels
import reactors_simple

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class SimpleReactors(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        rows=json.loads((ROOT/'content/ids.json').read_text())['entries']
        rows += [{**r,'id':'ntm:'+r['id']} for r in fuel_rows(SOURCE)]
        with tempfile.TemporaryDirectory() as folder:
            cls.data=reactors_simple.export(SOURCE,Path(folder),rows)

    def test_structural_roles_remain_distinct_and_not_craftable(self):
        rows=build(SOURCE)
        internal=[r for r in rows if r['source_variable']=='pile_block']
        self.assertEqual(len(internal),9)
        self.assertEqual({r['metadata'] for r in internal},set(range(9)))
        self.assertTrue(all(r['loot_none'] and r['creative_hidden'] and r['template']=='minecraft:beehive' for r in internal))
        devices=[r for r in rows if r['source_variable']=='pile_device']
        self.assertEqual([r['source_block_metadata_base'] for r in devices],[0,4,8])
        self.assertEqual(len([r for r in rows if r['category']=='machine']),14)

    def test_metadata_and_transmutation(self):
        fuels=pile_fuels(SOURCE)
        self.assertEqual([f['metadata'] for f in fuels],list(range(9)))
        self.assertEqual([fuels[i]['turns_into_metadata'] for i in [3,4,5,7,8]],[4,5,6,8,6])
        self.assertEqual(fuels[0]['life'],0)
        self.assertEqual(fuels[6]['heat_per_neutron'],1.5)
        self.assertEqual(fuels[7]['life'],35000)
        self.assertEqual(self.data['pile']['max_heat_c'],800)

    def test_breeding_is_not_an_active_neighbor(self):
        fuels=self.data['zirnox']['fuels'];by={f['member']:f for f in fuels}
        self.assertEqual([f['member'] for f in fuels if f['breeding']],['TH232','LITHIUM'])
        self.assertEqual(by['TH232']['output'],'ntm:rod_zirnox__thorium_fuel')
        self.assertEqual(by['LITHIUM']['output'],'ntm:rod_zirnox_tritium')
        self.assertEqual(by['NATURAL_URANIUM_FUEL']['max_life'],250000)
        self.assertEqual(by['LES_FUEL']['heat_per_decay'],150)
        self.assertEqual(self.data['zirnox']['life_data']['completion_comparison'],'strict greater than maxLife')

    def test_adjacency_is_reciprocal_and_not_a_rectangular_guess(self):
        graph=self.data['zirnox']['neighbors']
        self.assertEqual(len(graph),24)
        self.assertEqual(graph[0],[1,7]);self.assertEqual(graph[2],[1,9])
        self.assertNotIn(3,graph[2])
        for i,ns in graph.items():
            for j in ns:self.assertIn(i,graph[j])

    def test_textures_and_nbt_are_not_vanilla_damage(self):
        rows=fuel_rows(SOURCE)
        self.assertEqual(len(rows),20)
        for row in rows:
            self.assertTrue((SOURCE/row['source_texture']).is_file())
            self.assertTrue(all(row['names'].values()))
            self.assertNotIn('max_damage',row)
        self.assertEqual(self.data['pile']['life_data']['type'],'double')
        tanks=self.data['zirnox']['tanks']
        self.assertEqual({t['source_fluid']:t['capacity_mb'] for t in tanks},{'SUPERHOTSTEAM':8000,'CARBONDIOXIDE':16000,'WATER':32000})


if __name__=='__main__':unittest.main()

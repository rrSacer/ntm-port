"""Source boundaries for the PWR content (R3): identities, fuel table, recipes, hazards."""
import json
import tempfile
import unittest
from pathlib import Path
import pwr_catalog
import reactors_pwr

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class PwrContent(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows=pwr_catalog.build(SOURCE)
        known=[{'id':'ntm:'+r['id']} for r in cls.rows]
        known+=[{'id':f'ntm:pwr_fuel_{k}__{f["member"].lower()}'} for f in pwr_catalog.fuels(SOURCE) for k in ('hot','depleted')]
        with tempfile.TemporaryDirectory() as folder:
            cls.data=reactors_pwr.export(SOURCE,Path(folder),known)

    def test_homonym_block_and_item_have_distinct_ids(self):
        ids={r['id']:r['type'] for r in self.rows}
        self.assertEqual(ids['pwr_fuel'],'block')
        items=[i for i,t in ids.items() if t=='item' and i.startswith('pwr_fuel__')]
        self.assertEqual(len(items),15)
        self.assertEqual(len(ids),22)
        self.assertEqual(sum(1 for r in self.rows if r['source_variable']=='pwr_fuel'),16)

    def test_structure_rows(self):
        by={r['id']:r for r in self.rows}
        self.assertEqual(by['pwr_controller']['template'],'minecraft:beehive')
        for role,meta in (('internal',0),('port',1)):
            r=by['pwr_block__'+role];self.assertEqual(r['metadata'],meta)
            self.assertTrue(r['creative_hidden'] and r['loot_none'])
        for pillar in pwr_catalog.PILLARS:
            self.assertEqual(set(by[pillar]['surface_textures']),{'side','top','bottom'})

    def test_fuel_table_and_source_quirk(self):
        fuels={f['member']:f for f in self.data['fuels']}
        self.assertEqual(len(fuels),15)
        self.assertEqual((fuels['MEU']['heat_emission'],fuels['MEU']['function'],fuels['MEU']['level'],fuels['MEU']['divisor']),(5.0,'FunctionLogarithmic',600.0,2500.0))
        self.assertEqual(fuels['HES327']['level'],30.0)
        self.assertEqual(fuels['BFB_PU241']['declared_yield'],250000000)
        self.assertTrue(all(f['yield']==1_000_000_000 for f in fuels.values()))
        self.assertEqual(fuels['MOX']['items']['hot'],'ntm:pwr_fuel_hot__mox')
        self.assertEqual([t['capacity_mb'] for t in self.data['controller']['tanks']],[128000,128000])

    def test_recipes_and_hazards(self):
        results={}
        for f in (ROOT/'content/data/ntm/recipe').glob('bulk__*.json'):
            d=json.loads(f.read_text());results.setdefault(d['result']['id'],[]).append(d)
        self.assertEqual(len([k for k in results if k.startswith('ntm:pwr_fuel__')]),15)
        hazards={e['id']:e for e in json.loads((ROOT/'crates/ntm-core/data/hazards.json').read_text())['entries']}
        meu,hot,dep=(hazards[f'ntm:pwr_fuel{k}__meu']['radiation_rad_s'] for k in ('','_hot','_depleted'))
        self.assertEqual((hot,dep),(meu*10,meu*10));self.assertGreater(meu,0)
        self.assertEqual(hazards['ntm:pwr_fuel_hot__meu']['other_hazards'],{'hot':5})
        self.assertEqual(hazards['ntm:pwr_fuel']['radiation_rad_s'],0)


if __name__=='__main__':
    unittest.main()

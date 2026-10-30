"""Source boundaries for the Watz content (R4): identities, pellet table, recipes, hazards."""
import json
import tempfile
import unittest
from pathlib import Path
import watz_catalog
import reactors_watz

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class WatzContent(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows=watz_catalog.build(SOURCE)
        known=[{'id':'ntm:'+r['id']} for r in cls.rows]
        with tempfile.TemporaryDirectory() as folder:
            cls.data=reactors_watz.export(SOURCE,Path(folder),known)

    def test_catalog_partition(self):
        ids=[r['id'] for r in self.rows]
        self.assertEqual(len(ids),29);self.assertEqual(len(set(ids)),29)
        self.assertEqual([r['id'] for r in self.rows if r['category']=='machine'],['watz','watz_pump','struct_watz_core'])
        self.assertEqual([r['id'] for r in self.rows if r['source_variable']=='watz_end'],['watz_end','watz_end__bolted'])
        self.assertEqual(len([i for i in ids if i.startswith('watz_pellet__')]),12)
        self.assertEqual(len([i for i in ids if i.startswith('watz_pellet_depleted__')]),12)
        self.assertTrue(all(r['max_stack_size']==16 for r in self.rows if r['type']=='item'))

    def test_pellet_table(self):
        pellets={p['member']:p for p in self.data['pellets']}
        self.assertEqual(len(pellets),12)
        s=pellets['SCHRABIDIUM']
        self.assertEqual((s['passive'],s['heat_emission'],s['mud_content'],s['burn'],s['heat_divisor']),(2000.0,20.0,0.005,'FunctionLinear(1.5D)','FunctionSqrtFalling(10D)'))
        self.assertEqual(pellets['DU']['absorb'],'FunctionQuadratic(1D, 1D).withDiv(100)')
        self.assertEqual(pellets['NQD']['heat_divisor'],'FunctionSqrt(1D/25D).withOff(25D * 25D)')
        self.assertEqual({p['role'] for p in pellets.values()},{'fuel','absorber'})
        self.assertEqual((self.data['coverage']['fuel'],self.data['coverage']['absorbers']),(9,3))
        self.assertEqual(len(self.data['reactor']['tanks']['shared']),3)

    def test_depleted_icon_is_desaturated(self):
        by={r['id']:r for r in self.rows}
        fresh=by['watz_pellet__les']['tint'];dep=by['watz_pellet_depleted__les']['tint']
        self.assertEqual(fresh,{'light':'ABB4A8','dark':'0C1105'})
        self.assertNotEqual(dep,fresh)
        self.assertEqual(watz_catalog.desaturate(0x818181),0x606060)

    def test_recipes_and_hazards(self):
        results={}
        for f in (ROOT/'content/data/ntm/recipe').glob('bulk__*.json'):
            d=json.loads(f.read_text());results.setdefault(d['result']['id'],[]).append(d)
        pellets=[k for k in results if k.startswith('ntm:watz_pellet__')]
        self.assertEqual(len(pellets),10)
        self.assertNotIn('ntm:watz_pellet__nqd',results);self.assertNotIn('ntm:watz_pellet__nqr',results)
        hazards={e['id']:e for e in json.loads((ROOT/'crates/ntm-core/data/hazards.json').read_text())['entries']}
        self.assertGreater(hazards['ntm:watz_pellet__schrabidium']['radiation_rad_s'],0)
        self.assertEqual(hazards['ntm:watz_pellet_depleted__schrabidium']['radiation_rad_s'],0)
        self.assertEqual(hazards['ntm:watz_pellet__lead']['radiation_rad_s'],0)


if __name__=='__main__':
    unittest.main()

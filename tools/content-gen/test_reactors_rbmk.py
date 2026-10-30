"""Source boundaries for the RBMK content (R2): catalog, rod table, recipes, hazards."""
import json
import tempfile
import unittest
from pathlib import Path
import rbmk_catalog
import reactors_rbmk

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'


class RbmkContent(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows=rbmk_catalog.build(SOURCE)
        known=[{'id':'ntm:'+r['id']} for r in cls.rows]
        with tempfile.TemporaryDirectory() as folder:
            cls.data=reactors_rbmk.export(SOURCE,Path(folder),known)

    def test_catalog_partition(self):
        blocks=[r for r in self.rows if r['type']=='block'];items=[r for r in self.rows if r['type']=='item']
        self.assertEqual((len(blocks),len(items),len(self.rows)),(37,68,105))
        self.assertEqual(len({r['id'] for r in self.rows}),105)
        machines=[r for r in blocks if r['category']=='machine']
        self.assertEqual(len(machines),33)
        self.assertTrue(all(r['template']=='minecraft:beehive' for r in machines))
        self.assertEqual([r['id'] for r in blocks if r['category']!='machine'],['pribris','pribris_burning','pribris_radiating','pribris_digamma'])
        self.assertEqual(len([r for r in items if 'rbmk_fuel_' in r['id']]),33)
        self.assertEqual(len([r for r in items if 'rbmk_pellet_' in r['id']]),32)

    def test_identity_follows_source_registration(self):
        ids={r['id'] for r in self.rows}
        # The hep239 variable is registered under the shorter unlocalized name.
        self.assertIn('rbmk_fuel_hep',ids);self.assertNotIn('rbmk_fuel_hep239',ids)
        self.assertIn('rbmk_pellet_hep239',ids)
        self.assertTrue(all(r['max_stack_size']==1 for r in self.rows if r['id'].startswith('rbmk_fuel_')))

    def test_rod_table(self):
        rods={r['variable']:r for r in self.data['rods']}
        self.assertEqual(len(rods),33)
        ueu=rods['rbmk_fuel_ueu']
        self.assertEqual((ueu['yield'],ueu['func_end'],ueu['function'],ueu['depletion_function'],ueu['heat_per_flux'],ueu['melting_point']),
                         (1e8,15.0,'LOG_TEN','RAISING_SLOPE',0.65,2865.0))
        self.assertEqual(ueu['tint'],0x868D82)
        self.assertEqual(rods['rbmk_fuel_drx']['self_rate'],10.0)
        self.assertIsNone(rods['rbmk_fuel_test']['pellet'])
        self.assertEqual(rods['rbmk_fuel_test']['full_name'],'THE VOICES')
        self.assertEqual(sum(1 for r in rods.values() if r['pellet']),32)
        self.assertEqual(rods['rbmk_fuel_uzh']['heat_coefficient'],{'start':1000.0,'length':500.0})

    def test_enums_and_dials(self):
        self.assertEqual(len(self.data['burn_functions']),9)
        self.assertEqual(self.data['depletion_functions'],['LINEAR','RAISING_SLOPE','BOOSTED_SLOPE','GENTLE_SLOPE','STATIC'])
        self.assertEqual(self.data['burn_formulas']['LOG_TEN'],'log10(x + 1) * reactivity * 50')
        dials={d['key']:d['default'] for d in self.data['dials']}
        self.assertEqual(len(dials),25);self.assertEqual(dials['KEY_COLUMN_HEIGHT'],4);self.assertIs(dials['KEY_REASIM_BOILERS'],False)

    def test_generated_recipes_and_hazards(self):
        recipes={}
        for f in (ROOT/'content/data/ntm/recipe').glob('bulk__*.json'):
            d=json.loads(f.read_text());recipes.setdefault(d['result']['id'],[]).append(d)
        for rod in ['rbmk_fuel_ueu','rbmk_fuel_hes','rbmk_fuel_zfb_am_mix']:
            (r,)=recipes['ntm:'+rod];self.assertEqual(sum(1 for i in r['ingredients'] if 'rbmk_fuel_empty' in json.dumps(i)),1)
            self.assertEqual(len(r['ingredients']),9)
        self.assertNotIn('ntm:rbmk_fuel_drx',recipes);self.assertNotIn('ntm:rbmk_fuel_test',recipes)
        # enable528 defaults off: the base control rods are craftable, the ReaSim ones are not.
        self.assertIn('ntm:rbmk_control',recipes);self.assertIn('ntm:rbmk_console',recipes)
        self.assertNotIn('ntm:rbmk_control_reasim',recipes);self.assertNotIn('ntm:rbmk_control_reasim_auto',recipes)
        hazards={e['id']:e for e in json.loads((ROOT/'crates/ntm-core/data/hazards.json').read_text())['entries']}
        self.assertEqual(hazards['ntm:rbmk_fuel_drx']['other_hazards'].get('digamma'),1/3)
        self.assertGreater(hazards['ntm:rbmk_fuel_ueu']['radiation_rad_s'],0)
        self.assertEqual(hazards['ntm:rbmk_fuel_leaus']['radiation_rad_s'],0)


if __name__=='__main__':
    unittest.main()

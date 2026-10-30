"""RTG Java rounding/config and the radgen's exact independent fuel queues."""
import json
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]


class Radioisotopes(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows={r['id']:r for r in json.loads((ROOT/'content/ids.json').read_text())['entries']}
        cls.data=json.loads((ROOT/'content/machine-recipes/radioisotopes.json').read_text())

    def test_identities_follow_registry_and_metadata(self):
        self.assertIn('ntm:machine_rtg_grey',self.rows)
        self.assertNotIn('ntm:machine_rtg',self.rows)
        for name in ['machine_rtg_grey','machine_radgen']:
            self.assertEqual(self.rows['ntm:'+name]['template'],'minecraft:beehive')
        for family,count in [('short',8),('long',5)]:
            for suffix in ['','_tiny','_depleted','_depleted_tiny']:
                values=[r for r in self.rows.values() if r['source_variable']==f'nuclear_waste_{family}{suffix}']
                self.assertEqual(len(values),count)
                self.assertEqual({r['metadata'] for r in values},set(range(count)))
        depleted=[r for r in self.rows.values() if r['source_variable']=='pellet_rtg_depleted']
        self.assertEqual(len(depleted),6)
        for row in depleted:self.assertEqual(row['crafting_remainder'],'ntm:plate_iron')
        for p in self.data['rtg_pellets']:self.assertEqual(self.rows[p['item']]['max_stack_size'],1)

    def test_rtg_float_rounding_and_config_branches(self):
        pellets={p['item']:p for p in self.data['rtg_pellets']}
        self.assertEqual(pellets['ntm:pellet_rtg']['lifespan_ticks'],631440000)
        self.assertEqual(pellets['ntm:pellet_rtg_lead']['lifespan_ticks'],21600)
        self.assertEqual(pellets['ntm:pellet_rtg_gold']['heat_tu'],{'decay_enabled':200,'decay_disabled':100})
        self.assertEqual(pellets['ntm:pellet_rtg_lead']['heat_tu'],{'decay_enabled':600,'decay_disabled':200})
        cfg=self.data['machine_config']['ntm:machine_rtg_grey']
        self.assertEqual(cfg['heat_max_tu'],{'decay_enabled':600,'decay_disabled':200})
        self.assertEqual((cfg['slots'],cfg['power_max_he'],cfg['he_per_tu']),(15,100000,5))
        self.assertEqual(self.data['config']['defaults'],{'doRTGsDecay':True,'scaleRTGPower':False})

    def test_radgen_fuels_preserve_short_and_long_tiny_ratios(self):
        fuels={r['input']['item']:r for r in self.data['radgen_fuels']}
        self.assertEqual(len(fuels),28)
        for family,power,duration in [('short',1500,36000),('long',500,144000)]:
            full=fuels[f'ntm:nuclear_waste_{family}__uranium235']
            tiny=fuels[f'ntm:nuclear_waste_{family}_tiny__uranium235']
            self.assertEqual((full['power_he_per_tick'],full['duration_ticks']),(power,duration))
            self.assertEqual((tiny['power_he_per_tick'],tiny['duration_ticks']),(power//10,duration//10))
            self.assertEqual(full['output']['item'],f'ntm:nuclear_waste_{family}_depleted__uranium235')
        self.assertIsNone(fuels['ntm:scrap_nuclear']['output'])
        self.assertEqual(fuels['ntm:gem_rad']['output'],{'item':'minecraft:diamond','count':1})
        self.assertEqual(self.data['recipes'],[])
        cfg=self.data['machine_config']['ntm:machine_radgen']
        self.assertEqual((cfg['input_slots'],cfg['output_slots'],cfg['power_max_he']),(12,12,1000000))
        self.assertIn('full storage does not pause',cfg['is_on'])

    def test_radiation_uses_the_helper_and_item_wide_registrations(self):
        hazards={r['id']:r for r in json.loads((ROOT/'crates/ntm-core/data/hazards.json').read_text())['entries']}
        self.assertEqual(hazards['ntm:pellet_rtg']['radiation_rad_s'],15)
        self.assertEqual(hazards['ntm:pellet_rtg_lead']['radiation_rad_s'],15000)
        self.assertEqual(hazards['ntm:pellet_rtg_depleted__neptunium']['radiation_rad_s'],3.75)
        for ident in self.rows:
            if ident.startswith('ntm:nuclear_waste_short__'):self.assertEqual(hazards[ident]['radiation_rad_s'],30)
            if ident.startswith('ntm:nuclear_waste_short_depleted_tiny__'):self.assertEqual(hazards[ident]['radiation_rad_s'],.3)

    def test_recycling_waits_for_the_original_casing_return(self):
        recipes=[json.loads(p.read_text()) for p in (ROOT/'content/data/ntm/recipe').glob('*.json')]
        self.assertFalse(any('pellet_rtg_depleted__' in json.dumps(r.get('ingredients',r.get('key',{}))) for r in recipes))
        gaps=json.loads((ROOT/'content/bulk-recipe-coverage.json').read_text())['gaps']
        self.assertEqual(sum('native crafting remainder required:' in g['reason'] for g in gaps),5)


if __name__=='__main__':unittest.main()

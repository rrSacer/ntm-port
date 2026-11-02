"""The pile's functional dependency uses original HAND_DRILL identities/recipes."""
import json
import unittest
from pathlib import Path
from reactor_catalog import pile_tools

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT.parent/'ntm-src'

class PileTools(unittest.TestCase):
    def test_original_declarations_and_durability(self):
        rows=pile_tools(SOURCE)
        self.assertEqual([r['id'] for r in rows],['hand_drill','hand_drill_desh'])
        self.assertEqual([r['durability']['max_damage'] for r in rows],[100,0])
        self.assertTrue(all(r['max_stack_size']==1 and r['names']['pt_br'] for r in rows))
        normal=json.loads((ROOT/'content/data/ntm/horizon/item/hand_drill.json').read_text())
        desh=json.loads((ROOT/'content/data/ntm/horizon/item/hand_drill_desh.json').read_text())
        self.assertEqual(normal['components']['minecraft:max_damage'],100)
        self.assertNotIn('components',desh)
    def test_original_recipes_reuse_the_exact_plastic_group(self):
        recipes={}
        for f in (ROOT/'content/data/ntm/recipe').glob('*.json'):
            r=json.loads(f.read_text())
            if r['result']['id'].startswith('ntm:hand_drill'):recipes[r['result']['id']]=r
        self.assertEqual(set(recipes),{'ntm:hand_drill','ntm:hand_drill_desh'})
        for r in recipes.values():self.assertEqual(r['pattern'],[' D','S ',' S'])
        plastic=recipes['ntm:hand_drill_desh']['key']['S']
        self.assertTrue(plastic.startswith('#ntm:'))
        path=ROOT/'content/data/ntm/tags/item'/(plastic.removeprefix('#ntm:')+'.json')
        self.assertEqual(set(json.loads(path.read_text())['values']),{'ntm:ingot_polymer','ntm:ingot_bakelite'})
    def test_original_control_tab_and_hazards_are_present(self):
        tab=json.loads((ROOT/'content/data/ntm/creative_tab/control.json').read_text())['items']
        for id in ['ntm:hand_drill','ntm:hand_drill_desh']:self.assertEqual(tab.count(id),1)

if __name__=='__main__':unittest.main()

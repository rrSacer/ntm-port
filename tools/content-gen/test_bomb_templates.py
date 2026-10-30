"""Native nuclear bomb entities need a block-entity template and orientation."""
import json
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
NUKES=['nuke_'+name for name in ['boy','gadget','man','mike','tsar','n2','fstbmb']]


class BombTemplates(unittest.TestCase):
    def test_nuclear_bombs_have_the_native_state_contract(self):
        rows={r['id']:r for r in json.loads((ROOT/'content/ids.json').read_text())['entries']}
        for name in NUKES:
            row=rows['ntm:'+name]
            self.assertEqual(row['template'],'minecraft:beehive')
            self.assertEqual(row['logic'],{'kind':'bomb','powered':None,'facing':'facing','level':{'prop':'honey_level','max':5}})
            declaration=json.loads((ROOT/f'content/data/ntm/horizon/block/{name}.json').read_text())
            self.assertEqual(declaration['template'],'minecraft:beehive')
            states=json.loads((ROOT/f'content/assets/ntm/blockstates/{name}.json').read_text())['variants']
            self.assertEqual(set(states),{f'facing={facing},honey_level={level}' for facing in ['north','east','south','west'] for level in range(6)})
            self.assertEqual({s['model'] for s in states.values()},{'ntm:block/'+name})

    def test_simple_charges_keep_their_redstone_state(self):
        for name in ['tnt_ntm','dynamite','semtex','c4']:
            declaration=json.loads((ROOT/f'content/data/ntm/horizon/block/{name}.json').read_text())
            self.assertEqual(declaration['template'],'minecraft:redstone_lamp')
            states=json.loads((ROOT/f'content/assets/ntm/blockstates/{name}.json').read_text())['variants']
            self.assertEqual(set(states),{'lit=false','lit=true'})


if __name__=='__main__':unittest.main()

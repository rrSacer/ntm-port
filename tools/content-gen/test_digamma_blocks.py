"""content/ digamma blocks against ModBlocks and the two Java classes (Onda 2, sublote 2, machine_fel).

Re-reads `ModBlocks.java`, `DigammaFlame.java` and `DigammaMatter.java` on their own, so a slip in digamma_blocks.py
cannot hide behind the generator.
"""
import json
import os
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main/java/com/hbm'
CONTENT = ROOT / 'content'
MOD_BLOCKS = (SRC / 'blocks/ModBlocks.java').read_text()


def declaration(name):
    return json.loads((CONTENT / f'data/ntm/horizon/block/{name}.json').read_text())


def statement(variable):
    return re.search(rf'\n\s*{variable} = new (\w+)\(([^;]*);', MOD_BLOCKS)


class DigammaBlocks(unittest.TestCase):
    def test_the_flame_is_dead_bush_shaped_light_15_with_no_collision_or_drop(self):
        line = statement('fire_digamma')
        self.assertEqual(line[1], 'DigammaFlame')
        self.assertIn('.setHardness(0.0F).setResistance(150F).setLightLevel(1.0F).setCreativeTab(null)', line[2])
        java = (SRC / 'blocks/bomb/DigammaFlame.java').read_text()
        self.assertIn('return 1;', java[java.index('getRenderType'):])
        self.assertIn('return null;', java[java.index('getCollisionBoundingBoxFromPool'):])
        self.assertIn('World.doesBlockHaveSolidTopSurface(world, x, y - 1, z)', java)
        d = declaration('fire_digamma')
        self.assertEqual(d['template'], 'minecraft:dead_bush')
        self.assertEqual(d['properties'], {'hardness': 0.0, 'explosion_resistance': 150.0, 'requires_tool': False,
                                           'sounds': 'minecraft:stone', 'light': 15, 'occludes': False})
        self.assertEqual(d['collision_shapes'], {'default': []})
        self.assertEqual(d['shapes'], {'default': [[0, 0, 0, 1, 1, 1]]})
        model = json.loads((CONTENT / 'assets/ntm/models/block/fire_digamma.json').read_text())
        self.assertEqual(model['parent'], 'minecraft:block/cross')
        loot = json.loads((CONTENT / 'data/ntm/loot_table/blocks/fire_digamma.json').read_text())
        self.assertEqual(loot['pools'], [])
        self.assertTrue((CONTENT / 'assets/ntm/textures/block/fire_digamma.png.mcmeta').is_file())

    def test_the_matter_is_eight_unbreakable_blocks_without_collision(self):
        line = statement('digamma_matter')
        self.assertEqual(line[1], 'DigammaMatter')
        self.assertIn('.setBlockUnbreakable().setResistance(18000000).setCreativeTab(null)', line[2])
        java = (SRC / 'blocks/bomb/DigammaMatter.java').read_text()
        self.assertIn('if(meta >= 7)', java)
        self.assertIn('10 + rand.nextInt(40)', java)
        for meta in range(8):
            name = 'digamma_matter' if meta == 0 else f'digamma_matter__{meta}'
            d = declaration(name)
            self.assertEqual(d['template'], 'minecraft:glass')
            self.assertEqual(d['properties']['hardness'], -1.0)
            self.assertEqual(d['properties']['explosion_resistance'], 18000000.0)
            self.assertEqual(d['collision_shapes'], {'default': []})
        self.assertFalse((CONTENT / 'data/ntm/horizon/block/digamma_matter__8.json').exists())
        catalog = {e['id']: e for e in json.loads((CONTENT / 'ids.json').read_text())['entries']}
        self.assertEqual([catalog[f'ntm:digamma_matter__{m}']['metadata'] for m in range(1, 8)], list(range(1, 8)))
        self.assertTrue(all(catalog[i].get('creative_hidden') for i in ('ntm:fire_digamma', 'ntm:digamma_matter')))


if __name__ == '__main__':
    unittest.main()

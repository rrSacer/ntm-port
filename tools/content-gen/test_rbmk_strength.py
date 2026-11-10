"""The RBMK columns and their dummy are as strong as the Java's (`RBMKBase()`: setHardness(3F), setResistance(30F)),
read from the source, in the catalog and in every block file the game loads."""
import json
import re
import unittest
from pathlib import Path
import rbmk_catalog

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class RbmkStrength(unittest.TestCase):
    def test_the_base_class_is_3_and_30_and_no_column_overrides_it(self):
        base = (SOURCE / 'src/main/java/com/hbm/blocks/machine/rbmk/RBMKBase.java').read_text()
        ctor = re.search(r'protected RBMKBase\(\)\s*\{(.*?)\}', base, re.S)[1]
        self.assertIn('this.setHardness(3F);', ctor)
        self.assertIn('this.setResistance(30F);', ctor)
        blocks = (SOURCE / 'src/main/java/com/hbm/blocks/ModBlocks.java').read_text()
        for var in rbmk_catalog.COLUMNS:
            line = next(l for l in blocks.splitlines() if re.match(r'\s*' + var + r'\s*=\s*new ', l))
            self.assertNotIn('setHardness', line, var)
            self.assertNotIn('setResistance', line, var)
            self.assertEqual(rbmk_catalog.column_strength(SOURCE, var), (3.0, 18.0), var)

    def test_the_eighteen_columns_and_their_dummy_load_with_it(self):
        catalog = {e['source_variable']: e for e in json.loads((ROOT / 'tools/content-gen/m3.json').read_text())['entries'] if e.get('type') == 'block'}
        visual = json.loads((ROOT / 'tools/visual-gen/visual.json').read_text())
        self.assertEqual(len(rbmk_catalog.COLUMNS), 18)
        for var in rbmk_catalog.COLUMNS:
            self.assertEqual(catalog[var]['block_properties'], {'hardness': 3.0, 'explosion_resistance': 18.0}, var)
        for name in [*rbmk_catalog.COLUMNS, 'dummy_rbmk']:
            props = json.loads((ROOT / f'content/data/ntm/horizon/block/{name}.json').read_text())['properties']
            self.assertEqual((props['hardness'], props['explosion_resistance']), (3.0, 18.0), name)
        self.assertEqual(visual['dummy_variants']['ntm:dummy_rbmk'], {'hardness': 3.0, 'explosion_resistance': 18.0})


if __name__ == '__main__':
    unittest.main()

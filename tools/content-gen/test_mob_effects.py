"""content/ mob effects against HbmPotion.registerPotion and the Java's lang.

Re-reads `HbmPotion.java` on its own: each content effect is the `registerPotion(id, isBad, color, name, x, y)` of
its field (harmful when isBad, the colour, the icon cell), with the Java's `potion.hbm_<field>` as its name.
"""
import json
import os
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
SRC = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main'
CONTENT = ROOT / 'content'
POTIONS = (SRC / 'java/com/hbm/potion/HbmPotion.java').read_text()


def java_lang(name):
    result = {}
    for line in (SRC / f'resources/assets/hbm/lang/{name}.lang').read_text(encoding='utf-8-sig').splitlines():
        if '=' in line and not line.lstrip().startswith('#'):
            k, v = line.split('=', 1)
            result[k.strip()] = v.strip()
    return result


class MobEffects(unittest.TestCase):
    def test_every_effect_is_its_registered_potion(self):
        import mob_effects
        langs = {loc: (java_lang(old), json.loads((CONTENT / f'assets/ntm/lang/{loc}.json').read_text()))
                 for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
        for name, field, bad, color, cell in mob_effects.EFFECTS:
            found = re.search(rf'{field} = registerPotion\(PotionConfig\.\w+, (true|false), 0x([0-9A-Fa-f]+), "potion\.hbm_{field}", (\d+), (\d+)\);', POTIONS)
            self.assertIsNotNone(found, field)
            self.assertEqual((bad, color, cell), (found.group(1) == 'true', int(found.group(2), 16), (int(found.group(3)), int(found.group(4)))), field)
            data = json.loads((CONTENT / f'data/ntm/horizon/mob_effect/{name}.json').read_text())
            self.assertEqual(data, {'category': 'harmful' if bad else 'beneficial', 'color': color}, name)
            self.assertTrue((CONTENT / f'assets/ntm/textures/mob_effect/{name}.png').is_file(), name)
            for loc, (java, port) in langs.items():
                self.assertEqual(port[f'effect.ntm.{name}'], java[f'potion.hbm_{field}'], (loc, name))

    def test_the_radiation_potion_is_an_effect(self):
        data = json.loads((CONTENT / 'data/ntm/horizon/mob_effect/radiation.json').read_text())
        self.assertEqual(data, {'category': 'harmful', 'color': 0x84C128})
        self.assertEqual(json.loads((CONTENT / 'assets/ntm/lang/pt_br.json').read_text())['effect.ntm.radiation'], 'Contaminado')


if __name__ == '__main__':
    unittest.main()

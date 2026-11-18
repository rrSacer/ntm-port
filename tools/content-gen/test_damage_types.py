"""content/ damage types against ModDamageSource and the Java's lang (Onda 1, fatia 5b, decision D2).

Re-reads `ModDamageSource.java` and the 1.7.10 lang files on its own, so a slip in damage_types.py (a lost armor
bypass, a wrong message, a missing `.player` key) cannot hide behind the generator.
"""
import json
import os
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main'
CONTENT = ROOT / 'content'
SOURCE = (SRC / 'java/com/hbm/lib/ModDamageSource.java').read_text()


def java_lang(name):
    result = {}
    for line in (SRC / f'resources/assets/hbm/lang/{name}.lang').read_text(encoding='utf-8-sig').splitlines():
        if '=' in line and not line.lstrip().startswith('#'):
            k, v = line.split('=', 1)
            result[k.strip()] = v.strip()
    return result


class DamageTypes(unittest.TestCase):
    def test_every_emitted_type_is_its_mod_damage_source(self):
        tags = {t: json.loads((CONTENT / f'data/minecraft/tags/damage_type/{t}.json').read_text())
                for t in ('bypasses_armor', 'bypasses_effects', 'bypasses_invulnerability')}
        types = sorted(p.stem for p in (CONTENT / 'data/ntm/damage_type').glob('*.json'))
        self.assertEqual(types, ['digamma', 'radiation'])
        for name in types:
            line = re.search(rf'public static DamageSource \w+ = \(new DamageSource\("{name}"\)\)([^;]*);', SOURCE)
            self.assertIsNotNone(line, name)
            bypasses = '.setDamageBypassesArmor()' in line.group(1)
            data = json.loads((CONTENT / f'data/ntm/damage_type/{name}.json').read_text())
            # 1.7.10 `DamageSource.<init>` sets `hungerDamage = 0.3F` (`ldc 0.3f` in the bytecode of `ro`);
            # `setDamageBypassesArmor` zeroes it.
            self.assertEqual(data, {'message_id': name, 'exhaustion': 0.0 if bypasses else 0.3,
                                    'scaling': 'when_caused_by_living_non_player'
                                    if '.setDifficultyScaled()' in line.group(1) else 'never'})
            for tag, call in [('bypasses_armor', '.setDamageBypassesArmor()'), ('bypasses_effects', '.setDamageIsAbsolute()'),
                              ('bypasses_invulnerability', '.setDamageAllowedInCreativeMode()')]:
                self.assertEqual(f'ntm:{name}' in tags[tag]['values'], call in line.group(1), (name, tag))
        for tag in tags.values():
            self.assertFalse(tag['replace'])
        self.assertEqual(tags['bypasses_armor']['values'], ['ntm:radiation', 'ntm:digamma'])
        self.assertEqual(tags['bypasses_effects']['values'], ['ntm:digamma'])

    def test_radiation_is_the_armor_bypassing_source_at_line_15(self):
        self.assertEqual(SOURCE.splitlines()[14].strip(), 'public static DamageSource radiation = '
                         '(new DamageSource("radiation")).setDamageBypassesArmor();')

    def test_the_death_messages_are_the_java_ones_with_and_without_a_killer(self):
        for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]:
            port = json.loads((CONTENT / f'assets/ntm/lang/{loc}.json').read_text())
            java = java_lang(old)
            for name in (p.stem for p in (CONTENT / 'data/ntm/damage_type').glob('*.json')):
                base = java[f'death.attack.{name}']
                self.assertEqual(port[f'death.attack.{name}'], base, (loc, name))
                # No `.player` in the Java's lang: func_151519_b falls back to the base text.
                self.assertNotIn(f'death.attack.{name}.player', java)
                self.assertEqual(port[f'death.attack.{name}.player'], base, (loc, name))
        pt = json.loads((CONTENT / 'assets/ntm/lang/pt_br.json').read_text())
        self.assertEqual(pt['death.attack.radiation'], '%1$s morreu por envenenamento por radiação.')
        self.assertEqual(pt['death.attack.digamma'], '%1$s entrou no abismo.')


if __name__ == '__main__':
    unittest.main()

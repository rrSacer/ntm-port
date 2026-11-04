"""Every number in crates/ntm-core/data/consumables.json against the Java.

The table is the port's source of truth for item use; this test re-reads the
decompiled-source classes named in each entry's `origin` and compares effects,
durations, heals, doses, containers and caps, so a transcription slip cannot
hide behind a test that repeats the same transcription.
"""
import json
import os
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src'))
JAVA = SOURCE / 'src/main/java/com/hbm'
TABLE = json.loads((ROOT / 'crates/ntm-core/data/consumables.json').read_text())['entries']
IDS = {e['id'] for e in json.loads((ROOT / 'content/ids.json').read_text())['entries']}
SOUNDS = Path(os.environ.get('HORIZON_RESOURCES', Path.home() / 'Games/horizon/.local/horizon-client/26.3')) / 'assets/minecraft/sounds.json'

# 1.7.10 Potion fields to the engine's effect ids.
POTION = {
    'moveSpeed': 'speed', 'moveSlowdown': 'slowness', 'digSpeed': 'haste', 'digSlowdown': 'mining_fatigue',
    'damageBoost': 'strength', 'heal': 'instant_health', 'harm': 'instant_damage', 'jump': 'jump_boost',
    'confusion': 'nausea', 'regeneration': 'regeneration', 'resistance': 'resistance',
    'fireResistance': 'fire_resistance', 'waterBreathing': 'water_breathing', 'invisibility': 'invisibility',
    'blindness': 'blindness', 'nightVision': 'night_vision', 'hunger': 'hunger', 'weakness': 'weakness',
    'poison': 'poison', 'wither': 'wither', 'field_76434_w': 'health_boost', 'field_76444_x': 'absorption',
    'field_76443_y': 'saturation',
}


def read(path):
    return (JAVA / path).read_text(encoding='utf-8')


def product(text):
    """`3 * 60 * 20` or `200`."""
    n = 1
    for part in text.split('*'):
        n *= int(part.strip())
    return n


def blocks(text, head):
    """Bodies of `if(<head>NAME) { ... }`, by NAME, with balanced braces."""
    found = {}
    for match in re.finditer(r'if\(' + re.escape(head) + r'(\w+)\)\s*\{', text):
        depth, i = 1, match.end()
        while depth:
            depth += {'{': 1, '}': -1}.get(text[i], 0)
            i += 1
        found[match.group(1)] = text[match.end():i - 1]
    return found


EFFECT = re.compile(r'new PotionEffect\((?:Potion\.(\w+)|HbmPotion\.(\w+))\.id,\s*([\d\s*]+?),\s*(\d+)(?:,\s*\w+)?\)')


def effects(body):
    out = []
    for vanilla, hbm, ticks, amplifier in EFFECT.findall(body):
        out.append([POTION[vanilla] if vanilla else 'ntm:' + hbm, product(ticks), int(amplifier)])
    return out


def entry(name):
    return TABLE.get('ntm:' + name)


def table_effects(item):
    return [list(e) for e in item.get('effects', [])]


def mod_items(source):
    """`name = new Cls()....setUnlocalizedName("name")` statements of ModItems."""
    return {m.group(1): m.group(2) for m in re.finditer(r'\b(\w+) = (new [^;]*?);', source, re.S) if 'setUnlocalizedName' in m.group(2)}


class ItemEnergy(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = read('items/food/ItemEnergy.java')
        # onEaten only: addInformation repeats the same `this == ModItems.x` tests.
        cls.body = blocks(cls.text[cls.text.index('public ItemStack onEaten'):cls.text.index('public int getMaxItemUseDuration')], 'this == ModItems.')
        cls.mod = read('items/ModItems.java')

    def drinks(self):
        return [i for i, e in TABLE.items() if e['origin'].startswith('ItemEnergy')]

    def test_every_java_drink_is_in_the_table(self):
        java = {m for m in re.findall(r'(\w+) = new ItemEnergy\(\)', self.mod)}
        self.assertEqual({d.removeprefix('ntm:') for d in self.drinks()}, java)

    def test_effects_heal_and_dose(self):
        for item in self.drinks():
            name = item.removeprefix('ntm:')
            row = TABLE[item]
            body = self.body.get(name, '')
            self.assertEqual(table_effects(row), effects(body), name)
            heal = re.findall(r'player\.heal\((\d+)F?\)', body)
            self.assertEqual(row.get('heal', 0), float(heal[0]) if heal else 0, name)
            bypass = re.findall(r'RAD_BYPASS,\s*([\d.]+)F\)', body)
            direct = re.findall(r'incrementRadiation\(player,\s*([\d.]+)F\)', body)
            if bypass:
                self.assertEqual(row['dose'], {'millirad': round(float(bypass[0]) * 1000)}, name)
            elif direct:
                self.assertEqual(row['dose'], {'millirad': round(float(direct[0]) * 1000), 'creative': True}, name)
            else:
                self.assertNotIn('dose', row, name)

    def test_containers_caps_and_the_opener(self):
        decl = mod_items(self.mod)
        for item in self.drinks():
            name = item.removeprefix('ntm:')
            row = TABLE[item]
            text = decl[name]
            gives = [(g['item'].removeprefix('ntm:'), g.get('replace', False)) for g in row.get('gives', [])]
            bottle = re.search(r'makeBottle\((\w+),\s*(\w+)\)', text)
            if 'makeCan()' in text:
                self.assertEqual(gives, [('ring_pull', False), ('can_empty', True)], name)
                self.assertNotIn('requires', row, name)
            elif bottle:
                self.assertEqual(gives, [(bottle.group(2), False), (bottle.group(1), True)], name)
                self.assertEqual(row['requires'], 'ntm:bottle_opener', name)
            else:
                self.assertEqual(gives, [], name)
            self.assertTrue(all(not g.get('creative', True) and g.get('overflow') == 'lost' for g in row.get('gives', [])), name)

    def test_use_shape(self):
        self.assertIn('return 32;', self.text)
        self.assertIn('EnumAction.drink', self.text)
        self.assertRegex(self.text, r'if\(!player\.capabilities\.isCreativeMode\)\s*\{\s*--stack\.stackSize;')
        for item in self.drinks():
            row = TABLE[item]
            self.assertEqual((row['ticks'], row['animation'], row['consume']), (32, 'drink', 'unless_creative'), item)
        # The opener is carried, not held: `inventory.hasItem`.
        self.assertIn('player.inventory.hasItem(ModItems.bottle_opener)', self.text)


class CanteenMangoBdcl(unittest.TestCase):
    def test_canteen(self):
        text = read('items/food/ItemCanteen.java')
        mod = mod_items(read('items/ModItems.java'))['canteen_vodka']
        row = entry('canteen_vodka')
        cooldown = int(product(re.search(r'new ItemCanteen\(([\d\s*]+)\)', mod).group(1)))
        self.assertEqual((row['consume'], row['set_damage']), ('set_damage', cooldown))
        self.assertIn('stack.setItemDamage(stack.getMaxDamage())', text)
        self.assertEqual(int(re.search(r'getMaxItemUseDuration[^}]*return (\d+);', text, re.S).group(1)), row['ticks'])
        self.assertIn('stack.getItemDamage() == 0', text)
        self.assertEqual(row['gate'], 'cooled')
        self.assertEqual(table_effects(row), effects(text))
        self.assertIn('entity.ticksExisted % 20 == 0', text)

    def test_mucho_mango(self):
        text = read('items/food/ItemMuchoMango.java')
        mod = mod_items(read('items/ModItems.java'))['mucho_mango']
        row = entry('mucho_mango')
        hunger = int(re.search(r'new ItemMuchoMango\((\d+)\)', mod).group(1))
        # super(hunger, true): the two-argument ItemFood constructor uses 0.6F.
        self.assertEqual(row['food'], [hunger, 0.6])
        self.assertEqual(row['ticks'], int(re.search(r'getMaxItemUseDuration[^}]*return (\d+);', text, re.S).group(1)))
        ticks = int(re.search(r'new PotionEffect\(Potion\.moveSpeed\.id, (\d+)\)', text).group(1))
        self.assertEqual(table_effects(row), [['speed', ticks, 0]])
        self.assertIn('player.setItemInUse(stack, this.getMaxItemUseDuration(stack))', text)
        self.assertNotIn('canEat', text)

    def test_bdcl_ticks(self):
        text = read('items/food/ItemBDCL.java')
        duration = int(re.search(r'getMaxItemUseDuration[^}]*return (\d+);', text, re.S).group(1))
        skip = re.search(r'if\(count <= (\d+) && count % (\d+) == 0\)', text)
        limit, step = int(skip.group(1)), int(skip.group(2))
        end = int(re.search(r'if\(count == (\d+)\)', text).group(1))
        count, ticks = duration, 0
        while True:
            ticks += 1
            if count == end:
                break
            if count <= limit and count % step == 0:
                count -= 1
            count -= 1
        self.assertEqual(entry('bdcl')['ticks'], ticks)


class ProofBatch(unittest.TestCase):
    def test_cotton_candy(self):
        text = read('items/food/ItemCottonCandy.java')
        mod = mod_items(read('items/ModItems.java'))['cotton_candy']
        row = entry('cotton_candy')
        hunger = int(re.search(r'new ItemCottonCandy\((\d+),', mod).group(1))
        self.assertEqual(row['food'], [hunger, 0.6])
        self.assertEqual(table_effects(row), effects(text))
        self.assertIn('setAlwaysEdible()', text)

    def test_medx(self):
        text = read('items/special/ItemSyringe.java')
        body = re.search(r'if\(this == ModItems\.syringe_metal_medx[^{]*\{\s*if\(!world\.isRemote\)\s*\{(.*?)\n\t\t\t\}', text, re.S).group(1)
        self.assertEqual(table_effects(entry('syringe_metal_medx')), effects(body))

    def test_radaway(self):
        text = read('items/special/ItemSimpleConsumable.java')
        self.assertEqual(entry('radaway')['radaway'], int(re.search(r'doRadaway\(stack, user, (\d+)\)', text).group(1)))


class Sounds(unittest.TestCase):
    @unittest.skipUnless(SOUNDS.is_file(), 'the 26.3 resources are not installed')
    def test_mapped_events_exist_in_the_game(self):
        known = json.loads(SOUNDS.read_text())
        mapped = set(re.findall(r'=> \("minecraft:([\w.]+)"', (ROOT / 'crates/ntm-core/src/consumables.rs').read_text()))
        self.assertGreater(len(mapped), 5)
        self.assertEqual(sorted(m for m in mapped if m not in known), [])


class References(unittest.TestCase):
    def test_every_item_a_row_names_is_in_the_catalog(self):
        for item, row in TABLE.items():
            self.assertIn(item, IDS, item)
            for give in row.get('gives', []):
                self.assertIn(give['item'], IDS, item)
            if row.get('requires'):
                self.assertIn(row['requires'], IDS, item)


if __name__ == '__main__':
    unittest.main()

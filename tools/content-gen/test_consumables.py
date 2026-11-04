"""Every number in crates/ntm-core/data/consumables.json against the Java.

The table is the port's source of truth for item use; this test re-reads the
decompiled-source classes named in each entry's `origin` and compares effects,
durations, heals, doses, containers and caps, so a transcription slip cannot
hide behind a test that repeats the same transcription.
"""
import json
import os
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
SOURCE = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src'))
JAVA = SOURCE / 'src/main/java/com/hbm'
TABLE = json.loads((ROOT / 'crates/ntm-core/data/consumables.json').read_text())['entries']
TABLE.update(json.loads((ROOT / 'crates/ntm-core/data/kits.json').read_text())['entries'])
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
    for match in re.finditer(r'if\(' + re.escape(head) + r'(\w+)(?: && [^{]*?)?\)\s*\{', text):
        depth, i = 1, match.end()
        while depth:
            depth += {'{': 1, '}': -1}.get(text[i], 0)
            i += 1
        found[match.group(1)] = text[match.end():i - 1]
    return found


EFFECT = re.compile(r'new PotionEffect\((?:Potion\.(\w+)|HbmPotion\.(\w+))\.id,\s*([\d\s*]+?)(?:,\s*(\d+)(?:,\s*\w+)?)?\)')
# HbmPotion fields to the content mob effects that stand for them.
HBM = {'radx': 'ntm:radx', 'stability': 'ntm:stability', 'death': 'ntm:death', 'potionsickness': 'ntm:potion_sickness', 'taint': 'ntm:taint'}


def effects(body):
    out = []
    for vanilla, hbm, ticks, amplifier in EFFECT.findall(body):
        out.append([POTION[vanilla] if vanilla else HBM[hbm], product(ticks), int(amplifier or 0)])
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


REMOVE = re.compile(r'removePotionEffect\(Potion\.(\w+)\.id\)')


def method(text, signature, end):
    """The text of the method that starts at `signature` and stops at `end`."""
    return text[text.index(signature):text.index(end, text.index(signature))]


class Pills(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = read('items/food/ItemPill.java')
        cls.body = blocks(method(cls.text, 'protected void onFoodEaten', 'public void addInformation'), 'this == ModItems.')
        cls.mod = mod_items(read('items/ModItems.java'))

    def pills(self):
        return [i for i, e in TABLE.items() if e['origin'].startswith('ItemPill')]

    def test_every_java_pill_is_in_the_table(self):
        java = {n for n, text in self.mod.items() if text.startswith('new ItemPill(')}
        self.assertEqual({p.removeprefix('ntm:') for p in self.pills()}, java)

    def test_use_shape(self):
        self.assertEqual(int(re.search(r'getMaxItemUseDuration[^}]*return (\d+);', self.text, re.S).group(1)), 10)
        self.assertIn('setAlwaysEdible()', self.text)
        # onItemRightClick only asks for potion sickness, never canEat.
        self.assertRegex(self.text, r'if\(!VersatileConfig\.hasPotionSickness\(p_77659_3_\)\)\s*p_77659_3_\.setItemInUse')
        for item in self.pills():
            row = TABLE[item]
            self.assertEqual((row['ticks'], row['animation'], row['consume'], row.get('gate', 'always'), row['sickness_gate'], row['sound']), (10, 'eat', 'always', 'always', True, 'random.burp'), item)
            self.assertNotIn('food', row, item)

    def test_effects_cures_damage_and_dose(self):
        for item in self.pills():
            name = item.removeprefix('ntm:')
            row = TABLE[item]
            body = self.body.get(name, '')
            self.assertEqual(table_effects(row), effects(body), name)
            cure = [POTION[m] for m in REMOVE.findall(body)]
            self.assertEqual(row.get('cure', {}).get('effects', []), cure, name)
            harm = re.search(r'for\(int i = 0; i < (\d+); i\+\+\)\s*player\.attackEntityFrom\([^,]*\?[^,]*:[^,]*,\s*(\d+)\)', body)
            roll = re.search(r'if\(rand\.nextInt\((\d+)\) == 0\) \{\s*player\.attackEntityFrom\(ModDamageSource\.overdose, (\d+)\)', body)
            if harm:
                self.assertEqual(row['hurt'], {'amount': int(harm.group(2)), 'times': int(harm.group(1))}, name)
            elif roll:
                self.assertEqual(row['hurt'], {'amount': int(roll.group(2)), 'chance': int(roll.group(1))}, name)
            else:
                self.assertNotIn('hurt', row, name)
            rad = re.search(r'incrementRadiation\(player, (-?[\d.]+)F\)', body)
            if rad:
                self.assertEqual(row['dose'], {'millirad': round(float(rad.group(1)) * 1000), 'creative': True}, name)
            else:
                self.assertNotIn('dose', row, name)

    def test_the_cure_list_is_the_one_the_pill_and_the_med_bag_share(self):
        self.assertEqual(TABLE['ntm:pill_iodine']['cure'], TABLE['ntm:med_bag']['cure'])


class Syringes(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = read('items/special/ItemSyringe.java')
        cls.body = blocks(method(cls.text, 'public ItemStack onItemRightClick', 'public boolean hitEntity'), 'this == ModItems.')
        cls.mod = mod_items(read('items/ModItems.java'))

    def test_every_registered_syringe_that_is_in_k2_is_in_the_table(self):
        java = {n for n, text in self.mod.items() if text.startswith('new ItemSyringe()') and 'setCreativeTab(null)' not in text and n in {'syringe_metal_stimpak', 'syringe_metal_medx', 'syringe_metal_psycho', 'syringe_metal_super', 'syringe_taint', 'med_bag'}}
        self.assertEqual(java, {'syringe_metal_stimpak', 'syringe_metal_medx', 'syringe_metal_psycho', 'syringe_metal_super', 'syringe_taint', 'med_bag'})
        for name in java:
            self.assertIn('ntm:' + name, TABLE)

    def test_numbers(self):
        for name in ('syringe_metal_stimpak', 'syringe_metal_medx', 'syringe_metal_psycho', 'syringe_metal_super', 'syringe_taint', 'med_bag'):
            row = TABLE['ntm:' + name]
            body = self.body[name]
            self.assertEqual(table_effects(row), effects(body), name)
            heal = re.search(r'player\.heal\((\d+)\)', body)
            if name == 'med_bag':
                self.assertIn('player.setHealth(player.getMaxHealth())', body)
                self.assertGreaterEqual(row['heal'], 100)
                self.assertEqual(row['cure']['effects'], [POTION[m] for m in REMOVE.findall(body)])
            else:
                self.assertEqual(row.get('heal', 0), int(heal.group(1)) if heal else 0, name)
            self.assertEqual(row.get('sickness_gate', False), 'hasPotionSickness' in self.text[self.text.index('this == ModItems.' + name):].split('{')[0], name)
            self.assertEqual(row.get('sound'), 'hbm:item.syringe' if 'hbm:item.syringe' in body else None, name)

    def test_containers(self):
        for name in ('syringe_metal_stimpak', 'syringe_metal_medx', 'syringe_metal_psycho', 'syringe_metal_super'):
            gives = TABLE['ntm:' + name]['gives']
            self.assertEqual([(g['item'], g['replace'], g['overflow']) for g in gives], [('ntm:syringe_metal_empty', True, 'thrown')], name)
        taint = TABLE['ntm:syringe_taint']['gives']
        self.assertEqual([(g['item'], g.get('replace', False)) for g in taint], [('ntm:syringe_metal_empty', False), ('ntm:bottle2_empty', False)])
        self.assertNotIn('gives', TABLE['ntm:med_bag'])


class SimpleConsumables(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = read('items/special/ItemSimpleConsumable.java')
        cls.init = cls.text[cls.text.index('public static void init()'):cls.text.index('public static void effectAntidote')]

    def definition(self, name):
        start = self.init.index('ModItems.' + name + ' = ')
        end = self.init.index('.setUnlocalizedName("' + name + '")', start)
        return self.init[start:end]

    def test_radaway_family(self):
        for name, amount in (('radaway', 140), ('radaway_strong', 350), ('radaway_flush', 500)):
            self.assertIn(f'doRadaway(stack, user, {amount})', self.definition(name), name)
            self.assertEqual(TABLE['ntm:' + name]['radaway'], amount, name)
            self.assertEqual(TABLE['ntm:' + name]['gives'][0]['item'], 'ntm:iv_empty')
        self.assertIn('giveSoundAndDecrement(stack, user, "hbm:item.radaway", new ItemStack(ModItems.iv_empty))', self.text)

    def test_iv_bags(self):
        empty, blood, xp_empty, xp = (self.definition(n) for n in ('iv_empty', 'iv_blood', 'iv_xp_empty', 'iv_xp'))
        self.assertIn('Math.max(user.getHealth() - 5F, 0F)', empty)
        self.assertEqual(TABLE['ntm:iv_empty']['hp_cost'], 5)
        self.assertIn('"hbm:item.syringe", new ItemStack(ModItems.iv_blood)', empty)
        self.assertIn('user.heal(5F)', blood)
        self.assertEqual(TABLE['ntm:iv_blood']['heal'], 5)
        self.assertIn('getTotalExperience(user) >= 100', xp_empty)
        self.assertEqual((TABLE['ntm:iv_xp_empty']['xp_cost'], TABLE['ntm:iv_xp_empty']['gives'][0]['item']), (100, 'ntm:iv_xp'))
        self.assertIn('addExperience(user, 100, false)', xp)
        self.assertIn('"random.orb"', xp)
        self.assertEqual((TABLE['ntm:iv_xp']['xp_gain'], TABLE['ntm:iv_xp']['sound']), (100, 'random.orb'))

    def test_the_three_hbm_syringes(self):
        antidote = method(self.text, 'public static void effectAntidote', 'public static void effectPoison')
        self.assertIn('clearActivePotions()', antidote)
        self.assertIn('hasPotionSickness(affected)) return', antidote)
        self.assertEqual((TABLE['ntm:syringe_antidote']['cure'], TABLE['ntm:syringe_antidote']['sickness_gate']), ('all', True))
        poison = method(self.text, 'public static void effectPoison', 'public static void effectAwesome')
        self.assertIn(', 30)', poison)
        self.assertNotIn('hasPotionSickness', poison)
        self.assertEqual(TABLE['ntm:syringe_poison']['hurt'], {'amount': 30})
        awesome = self.text[self.text.index('public static void effectAwesome'):]
        mine = []
        for vanilla, ticks, amplifier in re.findall(r'new PotionEffect\(Potion\.(\w+)\.id, ([\d\s*]+), (\d+)\)', awesome):
            mine.append([POTION[vanilla], product(ticks), int(amplifier)])
        mine.append(['ntm:radx', 1000, 9])
        self.assertIn('new PotionEffect(HbmPotion.radx.id, 50 * 20, 9)', awesome)
        self.assertEqual(table_effects(TABLE['ntm:syringe_awesome']), mine)
        for name in ('syringe_antidote', 'syringe_poison', 'syringe_awesome'):
            self.assertEqual(TABLE['ntm:' + name]['gives'][0]['item'], 'ntm:syringe_empty')


class Foods(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mod = mod_items(read('items/ModItems.java'))
        cls.lemon = read('items/food/ItemLemon.java')

    def declared(self, name):
        """(heal, saturation modifier, class) as ItemFood computes them."""
        text = self.mod[name]
        found = re.match(r'new (ItemLemon|ItemFood|ItemSoup|ItemWaffle|ItemSchnitzelVegan|ItemCottonCandy|ItemPancake|ItemAppleEuphemium)\((\d+)(?:, ([\d.]+)F?)?(?:, (?:true|false))?\)', text)
        cls, heal, sat = found.groups()
        if cls in ('ItemSoup', 'ItemWaffle', 'ItemSchnitzelVegan', 'ItemCottonCandy') or (cls == 'ItemFood' and sat in (None, 'true', 'false')):
            sat = 0.6  # ItemFood(heal, isWolfFood) uses 0.6F
        return int(heal), float(sat), cls

    def test_every_lemon_and_soup_row_matches_its_constructor(self):
        for item, row in TABLE.items():
            name = item.removeprefix('ntm:')
            if not row['origin'].split('(')[0] in ('ItemLemon', 'ItemSoup', 'ItemFood', 'ItemWaffle', 'ItemSchnitzelVegan', 'ItemPancake', 'ItemAppleEuphemium'):
                continue
            name = 'quesadilla' if name == 'cheese_quesadilla' else name
            heal, sat, cls = self.declared(name)
            self.assertEqual(row['food'], [heal, sat], name)
            self.assertEqual((row['ticks'], row['animation'], row['consume'], row['sound']), (32, 'eat', 'always', 'random.burp'), name)

    def test_gates(self):
        # The only setAlwaysEdible in ItemLemon compares ModItems.med_ipecac
        # inside the constructor, before the static field is assigned: it is
        # null there, so the guard never fires and the two medicines need hunger.
        always = {'cotton_candy', 'apple_euphemium'}
        ctor = re.search(r'if\(this == ModItems\.med_ipecac \|\| this == ModItems\.med_ptsd\) \{\s*this\.setAlwaysEdible\(\);', self.lemon)
        self.assertTrue(ctor)
        self.assertRegex(self.mod['med_ipecac'], r'new ItemLemon\(0, 0, false\)')
        self.assertIn('this.setAlwaysEdible()', read('items/food/ItemCottonCandy.java'))
        self.assertIn('this.setAlwaysEdible()', read('items/food/ItemAppleEuphemium.java'))
        for item, row in TABLE.items():
            name = item.removeprefix('ntm:')
            if row['origin'].split('(')[0] in ('ItemLemon', 'ItemSoup', 'ItemFood', 'ItemWaffle', 'ItemSchnitzelVegan', 'ItemAppleEuphemium'):
                self.assertEqual(row['gate'], 'food_always' if name in always else 'food', name)

    def test_lemon_potion_effects_are_only_the_ones_onFoodEaten_runs(self):
        text = method(self.lemon, 'protected void onFoodEaten', 'public ItemStack onEaten')
        body = blocks(text, 'this == ModItems.')
        self.assertEqual(table_effects(TABLE['ntm:loop_stew']), effects(body['loop_stew']))
        self.assertEqual(table_effects(TABLE['ntm:med_ipecac']), table_effects(TABLE['ntm:med_ptsd']))
        self.assertEqual(table_effects(TABLE['ntm:med_ipecac']), [['hunger', 50, 49]])
        self.assertIn('Potion.hunger.id, 50, 49', text)
        # No super call: glyphid_meat_grilled's setPotionEffect never fires.
        self.assertNotIn('super.onFoodEaten', self.lemon)
        self.assertRegex(self.mod['glyphid_meat_grilled'], r'setPotionEffect\(Potion\.damageBoost\.id, 180, 1, 1F\)')
        self.assertNotIn('effects', TABLE['ntm:glyphid_meat_grilled'])
        self.assertIn('"hbm:entity.vomit"', text)

    def test_the_bowl_comes_back(self):
        self.assertRegex(self.lemon, r'if\(this == ModItems\.loop_stew\)\s*return new ItemStack\(Items\.bowl\)')
        soup = TABLE['ntm:glowing_stew']
        self.assertEqual(soup['gives'][0], {'item': 'minecraft:bowl', 'replace': True, 'overflow': 'thrown', 'creative': True})
        self.assertEqual(TABLE['ntm:loop_stew']['gives'], soup['gives'])

    def test_schnitzel_and_euphemium(self):
        text = read('items/food/ItemSchnitzelVegan.java')
        self.assertEqual(table_effects(TABLE['ntm:schnitzel_vegan']), effects(text))
        self.assertIn('player.setFire(5 * 20)', text)
        self.assertIn('player.motionY = 2', text)
        apple = read('items/food/ItemAppleEuphemium.java')
        parsed = []
        for vanilla, ticks, amp in re.findall(r'new PotionEffect\(Potion\.(\w+)\.id, (\d+), (\d+)\)', apple):
            parsed.append([POTION[vanilla], int(ticks), int(amp)])
        self.assertEqual(table_effects(TABLE['ntm:apple_euphemium']), parsed)

    def test_pancake_message(self):
        text = read('items/food/ItemPancake.java')
        message = re.search(r'new ChatComponentText\(EnumChatFormatting\.YELLOW \+ "([^"]+)"\)', text).group(1)
        self.assertEqual(TABLE['ntm:pancake']['message'], message)
        self.assertEqual(TABLE['ntm:pancake']['gate'], 'never')

    def test_waffle_explodes_in_java_only(self):
        self.assertIn('ExplosionNukeSmall.explode', read('items/food/ItemWaffle.java'))
        self.assertEqual(TABLE['ntm:bomb_waffle']['special'], 'bomb_waffle_blast')


class Cigarettes(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = read('items/special/ItemCigarette.java')
        cls.mod = mod_items(read('items/ModItems.java'))

    def test_shape(self):
        self.assertEqual(int(re.search(r'getMaxItemUseDuration[^}]*return (\d+);', self.text, re.S).group(1)), 30)
        self.assertIn('EnumAction.bow', self.text)
        self.assertIn('stack.stackSize--;', self.text)
        for name in ('cigarette', 'crackpipe'):
            row = TABLE['ntm:' + name]
            self.assertEqual((row['ticks'], row['animation'], row['consume'], row['sound'], row['special']), (30, 'bow', 'always', 'hbm:player.cough', 'smoke'))
        self.assertIn('"hbm:player.cough"', self.text)
        self.assertIn('nbt.setInteger("count", 30)', self.text)

    def test_numbers(self):
        body = blocks(method(self.text, 'public ItemStack onEaten', '@SideOnly'), 'this == ModItems.')
        cig, pipe = body['cigarette'], body['crackpipe']
        self.assertEqual(TABLE['ntm:cigarette']['dose'], {'millirad': round(float(re.search(r'incrementRadiation\(player, ([\d.]+)F\)', cig).group(1)) * 1000), 'creative': True})
        self.assertEqual(table_effects(TABLE['ntm:crackpipe']), effects(pipe))
        self.assertEqual(TABLE['ntm:crackpipe']['heal'], float(re.search(r'player\.heal\(([\d.]+)F\)', pipe).group(1)))
        self.assertNotIn('heal', TABLE['ntm:cigarette'])


class Kits(unittest.TestCase):
    """kits.json is generated; it must be what the generator makes from the Java now."""

    @classmethod
    def setUpClass(cls):
        import kits
        cls.kits = kits
        cls.file = json.loads((ROOT / 'crates/ntm-core/data/kits.json').read_text())['entries']

    def test_the_file_is_what_the_java_makes(self):
        rows = self.kits.extract()
        rows.update(self.kits.loot())
        self.assertEqual(self.file, rows)

    def test_every_catalog_kit_is_present(self):
        text = read('items/special/ItemStarterKit.java')
        kits = {n for n, t in mod_items(read('items/ModItems.java')).items() if t.startswith('new ItemStarterKit()')}
        self.assertEqual({k.removeprefix('ntm:') for k in self.file if self.file[k]['origin'].startswith('ItemStarterKit')}, kits - {'missile_kit'})
        self.assertIn('stack.stackSize--;', text)
        self.assertIn('"hbm:item.unpack"', text)

    def test_nothing_unported_is_in_the_catalog(self):
        # When an item is ported, its name leaves `unported` by regenerating.
        names = {e['source_variable'] for e in json.loads((ROOT / 'content/ids.json').read_text())['entries']}
        for kit, row in self.file.items():
            for entry in row.get('unported', []):
                var = re.match(r'Mod(?:Items|Blocks)\.(\w+)', entry)
                if var and '[' not in entry:
                    self.assertNotIn(var.group(1), names, (kit, entry))

    def test_gives_are_lost_when_they_do_not_fit(self):
        for kit, row in self.file.items():
            for give in row.get('gives', []):
                self.assertEqual(give['overflow'], 'lost', kit)

    def test_loot_crates(self):
        text = read('items/special/ItemLootCrate.java')
        self.assertIn('inventory.addItemStackToInventory(new ItemStack(choose(', text)
        self.assertEqual(self.file['ntm:loot_10']['loot'], [])
        self.assertEqual(self.file['ntm:loot_15']['loot'], [])
        weights = {l['item']: l['weight'] for l in self.file['ntm:loot_misc']['loot']}
        self.assertEqual(weights['ntm:mp_warhead_10_taint'], 1 / 5)
        self.assertEqual(weights['ntm:mp_warhead_15_turbine'], 1 / 100)


class Misc(unittest.TestCase):
    def test_rag(self):
        text = read('items/special/ItemRag.java')
        self.assertIn('player.inventory.addItemStackToInventory(new ItemStack(ModItems.rag_piss))', text)
        row = TABLE['ntm:rag']
        self.assertEqual(row['gives'], [{'item': 'ntm:rag_piss', 'overflow': 'lost', 'creative': True}])
        self.assertEqual(row['consume'], 'always')

    def test_peas(self):
        text = read('items/tool/ItemPeas.java')
        self.assertRegex(text, r'if \(!player\.capabilities\.isCreativeMode\) \{\s*--stack\.stackSize;')
        self.assertEqual(TABLE['ntm:peas']['consume'], 'unless_creative')


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
                self.assertTrue(give['item'].startswith('minecraft:') or give['item'] in IDS, item)
            if row.get('requires'):
                self.assertIn(row['requires'], IDS, item)


if __name__ == '__main__':
    unittest.main()

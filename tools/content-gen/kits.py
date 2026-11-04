"""Starter kits: crates/ntm-core/data/kits.json from ItemStarterKit.java.

Every `player.inventory.addItemStackToInventory(new ItemStack(..))` of a kit
becomes a `gives` row, in source order, with the Java's overflow rule (the
result is ignored, so what does not fit is lost). `giveHaz(world, player, n)`
becomes the `hazmat_kit:n` special. An item the port does not have is listed
in `unported`, by its Java name, and is not given: nothing is invented.
Run `python3 tools/content-gen/kits.py` to rewrite the file; the test compares.
"""
import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JAVA = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main/java/com/hbm'
OUT = ROOT / 'crates/ntm-core/data/kits.json'
EFFECT = {'invisibility': 'invisibility'}
VANILLA = {'Blocks.tnt': 'minecraft:tnt', 'Items.gunpowder': 'minecraft:gunpowder', 'Items.dye': 'minecraft:ink_sac'}


def index():
    entries = json.loads((ROOT / 'content/ids.json').read_text())['entries']
    plain, enum, fluid = {}, {}, {}
    for e in entries:
        var = e.get('source_variable')
        if e.get('source_enum'):
            enum[(var, e['source_enum'])] = e['id']
        elif e.get('source_fluid'):
            fluid[(var, e['source_fluid'])] = e['id']
        else:
            plain.setdefault(var, e['id'])
    return plain, enum, fluid


def resolve(expr, meta, plain, enum, fluid):
    """(port id or None, name for `unported`)."""
    expr = expr.strip()
    inner = re.fullmatch(r'Item\.getItemFromBlock\((.*)\)', expr)
    if inner:
        expr = inner.group(1)
    if expr in VANILLA:
        return VANILLA[expr], expr
    name = expr.split('.', 1)[1] if '.' in expr else expr
    if meta:
        fl = re.fullmatch(r'Fluids\.(\w+)\.getID\(\)', meta)
        if fl:
            return fluid.get((name, fl.group(1))), f'{expr}[{fl.group(1)}]'
        enm = re.fullmatch(r'(\w+)\.(\w+)\.ordinal\(\)', meta)
        if enm:
            key = (name, f'{enm.group(1)}.{enm.group(2)}')
            return enum.get(key), f'{expr}[{enm.group(2)}]'
        return None, f'{expr}[{meta}]'
    return plain.get(name), expr


def blocks(text):
    found = {}
    for m in re.finditer(r'if\(this == ModItems\.(\w+)\)\s*\{', text):
        depth, i = 1, m.end()
        while depth:
            depth += {'{': 1, '}': -1}.get(text[i], 0)
            i += 1
        found[m.group(1)] = text[m.end():i - 1]
    return found


ADD = re.compile(r'addItemStackToInventory\((.*)\);')


def extract():
    text = (JAVA / 'items/special/ItemStarterKit.java').read_text(encoding='utf-8')
    use = text[text.index('public ItemStack onItemRightClick'):text.index('@SideOnly(Side.CLIENT)')]
    mod = (JAVA / 'items/ModItems.java').read_text(encoding='utf-8')
    plain, enum, fluid = index()
    catalog = {e['id'] for e in json.loads((ROOT / 'content/ids.json').read_text())['entries']}
    rows = {}
    for kit, body in blocks(use).items():
        if 'ntm:' + kit not in catalog:
            continue  # missile_kit: the missile family is not imported
        gives, missing, tier = [], [], None
        for line in body.splitlines():
            haz = re.search(r'giveHaz\(world, player, (\d)\)', line)
            if haz:
                tier = int(haz.group(1))
            m = ADD.search(line)
            if not m:
                continue
            arg = m.group(1)
            frame = re.fullmatch(r'DictFrame\.fromOne\((ModItems\.\w+), (\w+\.\w+), (\d+)\)', arg)
            if frame:
                item, name = resolve(frame.group(1), frame.group(2).replace('.', '.') + '.ordinal()', plain, enum, fluid)
                count = int(frame.group(3))
            else:
                parts = re.fullmatch(r'new ItemStack\((.*)\)', arg).group(1)
                depth, cut, fields = 0, 0, []
                for i, ch in enumerate(parts):
                    depth += ch == '('
                    depth -= ch == ')'
                    if ch == ',' and depth == 0:
                        fields.append(parts[cut:i])
                        cut = i + 1
                fields.append(parts[cut:])
                count = int(fields[1]) if len(fields) > 1 else 1
                item, name = resolve(fields[0], fields[2].strip() if len(fields) > 2 else None, plain, enum, fluid)
            if item:
                gives.append({'item': item, 'count': count, 'overflow': 'lost'})
            else:
                missing.append(f'{name} x{count}')
        if kit == 'stealth_boy':
            special = 'stealth_boy'
        else:
            special = f'hazmat_kit:{tier}' if tier is not None else None
        line = next(i for i, l in enumerate(mod.splitlines(), 1) if re.search(rf'\b{kit} = new ItemStarterKit', l))
        row = {'origin': f'ItemStarterKit.onItemRightClick {kit}, ModItems.java:{line}', 'consume': 'always', 'sound': 'hbm:item.unpack'}
        effects = [[EFFECT[v], eval(t), int(a)] for v, t, a in re.findall(r'new PotionEffect\(Potion\.(\w+)\.id, ([\d\s*]+), (\d+), true\)', body)]
        if effects:
            row['effects'] = effects  # ambient: the fourth argument is true
        if gives:
            row['gives'] = gives
        if special:
            row['special'] = special
        if missing:
            row['unported'] = missing
        rows['ntm:' + kit] = row
    return rows


# ItemLootCrate.choose: a part is drawn uniformly and kept with this chance,
# until one is kept; so its weight is the chance.
KEEP = {'COMMON': 1.0, 'UNCOMMON': 1 / 5, 'RARE': 1 / 10, 'EPIC': 1 / 25, 'LEGENDARY': 1 / 50, 'SEWS_CLOTHES_AND_SUCKS_HORSE_COCK': 1 / 100}


def loot():
    """ItemLootCrate lists: `setRarity` adds the part to a list by its type."""
    mod = (JAVA / 'items/ModItems.java').read_text(encoding='utf-8')
    catalog = {e['id'] for e in json.loads((ROOT / 'content/ids.json').read_text())['entries']}
    lists = {'loot_10': [], 'loot_15': [], 'loot_misc': []}
    for m in re.finditer(r'\b(mp_\w+) = new ItemCustomMissilePart\(\)(.*?);', mod, re.S):
        name, body = m.groups()
        rarity = re.search(r'\.setRarity\(Rarity\.(\w+)\)', body)
        if not rarity:
            continue
        fuselage = re.search(r'makeFuselage\(\s*FuelType\.\w+,\s*[\w.]+F?,\s*PartSize\.(\w+),', body)
        if fuselage:
            target = {'SIZE_10': 'loot_10', 'SIZE_15': 'loot_15'}.get(fuselage.group(1))
        else:
            target = 'loot_misc'
        if target:
            lists[target].append((name, KEEP[rarity.group(1)]))
    rows = {}
    for crate, parts in lists.items():
        line = next(i for i, l in enumerate(mod.splitlines(), 1) if re.search(rf'\b{crate} = new ItemLootCrate', l))
        row = {'origin': f'ItemLootCrate.onItemRightClick {crate}, ModItems.java:{line}', 'consume': 'always',
               'loot': [{'item': 'ntm:' + n, 'weight': w} for n, w in parts if 'ntm:' + n in catalog],
               'unported': [n for n, _ in parts if 'ntm:' + n not in catalog]}
        rows['ntm:' + crate] = row
    return rows


if __name__ == '__main__':
    rows = extract()
    rows.update(loot())
    OUT.write_text(json.dumps({'schema': 1, 'entries': rows}, indent=1, ensure_ascii=False) + '\n')

#!/usr/bin/env python3
"""Creative tabs for the NTM content mod (Horizon patch creative-plus).

Each item of the mod goes in the tab the original mod put it in: the
`setCreativeTab(MainRegistry.<tab>)` of its variable in ModItems/ModBlocks
(the id's path before a `__<material>` subtype, or ids.json's
`source_variable`); items the source does not name go by their name's prefix.
Writes `data/ntm/creative_tab/<tab>.json` (the format of
horizon-patches/creative-plus.diff, crates/horizon-client/src/creative/mod_tabs.rs)
and the tabs' titles for the lang files.

  generate.py --mod content|ntm.hmod --ntm-src ../ntm-src --out DIR
  generate.py ... --pack IN.hmod OUT.hmod   # IN plus the tabs, for testing
"""
import argparse, json, re, sys, zipfile
from pathlib import Path

# MainRegistry's tabs, in its order: (tab, our id, icon variable, en_us, pt_br).
TABS = [
    ('partsTab', 'parts', 'ingot_uranium', 'NTM Resources and Parts', 'Recursos e Peças do NTM'),
    ('controlTab', 'control', 'pellet_rtg', 'NTM Machine Items and Fuel', 'Componentes do NTM'),
    ('templateTab', 'templates', 'blueprints', 'NTM Templates', 'Moldes do NTM'),
    ('blockTab', 'blocks', 'ore_uranium', 'NTM Ores and Blocks', 'Minérios e Blocos do NTM'),
    ('machineTab', 'machines', 'pwr_controller', 'NTM Machines', 'Máquinas do NTM'),
    ('nukeTab', 'bombs', 'nuke_man', 'NTM Bombs', 'Bombas do NTM'),
    ('missileTab', 'missiles', 'missile_nuclear', 'NTM Missiles and Satellites', 'Mísseis e Satélites do NTM'),
    ('weaponTab', 'weapons', 'gun_greasegun', 'NTM Weapons and Turrets', 'Armas e Torretas do NTM'),
    ('consumableTab', 'consumables', 'bottle_nuka', 'NTM Consumables and Gear', 'Consumíveis e Equipamentos do NTM'),
]
BY_SOURCE = {t[0]: t for t in TABS}

# Name prefixes for items the source does not declare one by one (materials
# made from the Mats tables, subtypes): first match wins.
PREFIXES = [
    (('ore_', 'block_', 'basalt_', 'stone_', 'deco_', 'brick_', 'concrete', 'glass_', 'sand_', 'gravel_'), 'blocks'),
    (('machine_', 'reactor_', 'pwr_', 'fusion_', 'turbine', 'furnace_', 'struct_'), 'machines'),
    (('stamp_', 'pellet_', 'rod_', 'battery_', 'fuel_', 'canister_', 'cell_', 'upgrade_', 'part_', 'coil_'), 'control'),
    (('nuke_', 'bomb_', 'det_', 'charge_', 'mine_', 'crashed_'), 'bombs'),
    (('missile_', 'sat_', 'launch_'), 'missiles'),
    (('gun_', 'ammo_', 'turret_', 'grenade_'), 'weapons'),
    (('bottle_', 'can_', 'pill_', 'syringe_', 'med_', 'gas_mask', 'hazmat_', 'food_'), 'consumables'),
]


def source_tabs(ntm_src: Path):
    """Variable -> (tab, order) from ModItems and ModBlocks' statements."""
    out = {}
    order = 0
    for rel in ('src/main/java/com/hbm/items/ModItems.java', 'src/main/java/com/hbm/blocks/ModBlocks.java'):
        text = (ntm_src / rel).read_text(encoding='utf-8', errors='replace')
        for stmt in text.split(';'):
            m = re.search(r'(\w+)\s*=\s*new\b', stmt)
            t = re.search(r'setCreativeTab\(\s*MainRegistry\.(\w+)\s*\)', stmt)
            if m and t and t.group(1) in BY_SOURCE and m.group(1) not in out:
                out[m.group(1)] = (BY_SOURCE[t.group(1)][1], order)
                order += 1
    # ItemTooling assigns its tab in the constructor, rather than each
    # registration. Retain that original tab for the imported hand drills.
    tooling=(ntm_src/'src/main/java/com/hbm/items/tool/ItemTooling.java').read_text()
    match=re.search(r'setCreativeTab\(MainRegistry\.(\w+)\)',tooling)
    if match and match[1] in BY_SOURCE:
        items=(ntm_src/'src/main/java/com/hbm/items/ModItems.java').read_text()
        for var in re.findall(r'\b(hand_drill(?:_desh)?)\s*=\s*new ItemTooling\(',items):
            out[var]=(BY_SOURCE[match[1]][1],order);order+=1
    # ItemSimpleConsumable assigns its creative tab in the constructor.
    # Its factory lambdas contain semicolons, so statement splitting cannot
    # recover their assignments. Read only the declared constructor family.
    factory=ntm_src/'src/main/java/com/hbm/items/special/ItemSimpleConsumable.java'
    if factory.exists():
        text=factory.read_text()
        tab=re.search(r'setCreativeTab\(MainRegistry\.(\w+)\)',text)
        if tab and tab[1] in BY_SOURCE:
            for var in re.findall(r'ModItems\.(\w+)\s*=\s*new ItemSimpleConsumable\(',text):
                out[var]=(BY_SOURCE[tab[1]][1],order);order+=1
    return out


def mod_files(src: Path):
    """The mod's files: path -> bytes reader, from a folder or a .hmod."""
    if src.is_dir():
        return {str(p.relative_to(src)): p.read_bytes for p in src.rglob('*') if p.is_file()}
    z = zipfile.ZipFile(src)
    return {n: (lambda n=n: z.read(n)) for n in z.namelist() if not n.endswith('/')}


def mod_items(files):
    """Item ids of the mod: its items and the blocks that have one."""
    items = []
    for name in sorted(files):
        m = re.fullmatch(r'data/(\w+)/horizon/(item|block)/(.+)\.json', name)
        if not m:
            continue
        if m.group(2) == 'block' and json.loads(files[name]()).get('item') is False:
            continue
        items.append(f'{m.group(1)}:{m.group(3)}')
    return items


def tab_of(path, variables, sources):
    for var in (sources.get(path), path, path.split('__')[0]):
        if var and var in variables:
            return variables[var]
    for prefixes, tab in PREFIXES:
        if path.startswith(prefixes):
            return tab, None
    return 'parts', None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--mod', required=True, type=Path, help='content folder or .hmod')
    ap.add_argument('--ntm-src', required=True, type=Path)
    ap.add_argument('--ids', type=Path, help="ids.json (source_variable per id)")
    ap.add_argument('--out', type=Path, help='folder for data/ and lang additions')
    ap.add_argument('--pack', nargs=2, type=Path, metavar=('IN', 'OUT'))
    a = ap.parse_args()

    files = mod_files(a.mod)
    variables = source_tabs(a.ntm_src)
    sources = {}; hidden = set()
    if a.ids and a.ids.exists():
        for e in json.loads(a.ids.read_text()).get('entries', []):
            if e.get('creative_hidden'): hidden.add(e['id'])
            if e.get('source_variable'):
                sources[e['id'].split(':', 1)[1]] = e['source_variable']
    tabs = {t[1]: [] for t in TABS}
    for item in mod_items(files):
        if item in hidden: continue
        path = item.split(':', 1)[1]
        tab, order = tab_of(path, variables, sources)
        tabs[tab].append((order is None, order or 0, path, item))
    generated = {}
    lang = {'en_us': {}, 'pt_br': {}}
    for n, (_, tid, icon, en, pt) in enumerate(TABS):
        entries = sorted(tabs[tid])
        if not entries:
            continue
        ids = [e[3] for e in entries]
        key = f'itemGroup.ntm.{tid}'
        lang['en_us'][key], lang['pt_br'][key] = en, pt
        generated[f'data/ntm/creative_tab/{tid}.json'] = {
            'title': key,
            'fallback': en,
            'icon': f'ntm:{icon}' if f'ntm:{icon}' in ids else ids[0],
            'order': n,
            'items': ids,
        }
    for name, doc in generated.items():
        print(f'{name}: {len(doc["items"])} items', file=sys.stderr)
    if a.out:
        for name, doc in generated.items():
            p = a.out / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(json.dumps(doc, indent=1, ensure_ascii=False) + '\n')
        (a.out / 'lang-additions.json').write_text(json.dumps(lang, indent=1, ensure_ascii=False) + '\n')
    if a.pack:
        src, dst = a.pack
        zin = zipfile.ZipFile(src)
        with zipfile.ZipFile(dst, 'w', zipfile.ZIP_DEFLATED) as zout:
            for info in zin.infolist():
                data = zin.read(info)
                m = re.fullmatch(r'assets/ntm/lang/(\w+)\.json', info.filename)
                if m and m.group(1) in lang:
                    doc = json.loads(data)
                    doc.update(lang[m.group(1)])
                    data = json.dumps(doc, ensure_ascii=False, indent=1).encode()
                zout.writestr(info, data)
            for name, doc in generated.items():
                zout.writestr(zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0)),
                              json.dumps(doc, ensure_ascii=False).encode())


if __name__ == '__main__':
    main()

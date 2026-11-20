#!/usr/bin/env python3
"""Converte as receitas extraídas pelo agente de conteúdo
(content/machine-recipes/{assembly,chemical_plant,centrifuge,shredder,blast_furnace,foundry,anvil,press,purex}.json, schema 1)
para o formato que a lógica embute (crates/ntm-core/data/machines/*.json),
com os nomes pt_BR do content/ids.json para a tela.

Uso: python3 tools/machine-gen/build.py
"""
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parents[2]
ids = json.loads((root / 'content/ids.json').read_text())
names = {e['id']: (e.get('names') or {}).get('pt_br') or e['id'].split(':', 1)[1] for e in ids['entries']}
known = set(names)


def name(i):
    if i in names:
        return names[i]
    return i.split(':', 1)[-1].replace('_', ' ')


def label(r, outs, fouts):
    if outs:
        i, n = outs[0][0], outs[0][1]
        return f"{name(i)} ×{n}" if n > 1 else name(i)
    if fouts:
        f, mb = fouts[0]
        return f"{f.replace('_', ' ')} {mb} mB"
    return r['name']


def ingredient(x, alt=None):
    """`{ids, count}` (schema atual) ou `[id, n]` + alternativas (o primeiro)."""
    if isinstance(x, dict):
        ids, n = x['ids'], x['count']
    else:
        ids, n = (alt or [x[0]]), x[1]
    choices = [a for a in ids if a in known or a.startswith('minecraft:')] or ids[:1]
    return {'ids': choices, 'n': n, 'label': name(choices[0])}


def generic(st, section=None):
    src = json.loads((root / f'content/machine-recipes/{st}.json').read_text())
    if section:
        src = src[section]
    out = []
    for r in src['recipes']:
        alts = r.get('inputItemAlternatives') or [None] * len(r['inputItems'])
        ins = [ingredient(x, a) for x, a in zip(r['inputItems'], alts)]
        outs = [[o[0], o[1], o[2] if len(o) > 2 else 1.0] for o in r['outputItems']]
        out.append({
            'name': r['name'], 'label': r.get('label') or label(r, outs, r['outputFluids']), 'source': r['source'],
            'duration': r.get('duration', 0), 'power': r['power'],
            'in': ins, 'fin': r['inputFluids'], 'out': outs, 'fout': r['outputFluids'],
            'pools': r.get('pools', []),
            **({'group': r['autoSwitchGroup']} if r.get('autoSwitchGroup') else {}),
            **({'productivity': r['productivity']} if r.get('productivity') else {}),
            **({'mout': r['outputMaterials']} if r.get('outputMaterials') else {}),
            **({'periodic': [[p['item'][0], p['item'][1], p['every']] for p in r['periodicOutputItems']]} if r.get('periodicOutputItems') else {}),
            # Lote C do petróleo: intervalo, operações por intervalo, HE por
            # operação (debitado ao operar), catalisador não gasto, calor.
            **({'every': r['interval_ticks']} if r.get('interval_ticks') else {}),
            **({'ops': r['max_operations_per_interval']} if r.get('max_operations_per_interval') else {}),
            **({'he_op': r['energy_per_operation_he']} if r.get('energy_per_operation_he') else {}),
            **({'catalyst': r['catalysts'][0]['item']} if r.get('catalysts') else {}),
            **({'heat': r['heat_required_tu']} if r.get('heat_required_tu') else {}),
        })
    return {'station': st, 'source': f'content/machine-recipes/{st}.json', 'recipes': out, 'skipped': len(src['skipped'])}


def centrifuge():
    src = json.loads((root / 'content/machine-recipes/centrifuge.json').read_text())
    out = []
    for r in src['recipes']:
        ing = ingredient(r['input'], r.get('inputAlternatives'))
        outs = [[o[0], o[1], 1.0] for o in r['outputs']]
        # CentrifugeRecipes não têm duração nem energia: as da máquina
        # (TileEntityMachineCentrifuge: 200 ticks, 200 HE/t). A tela mostra
        # a entrada (a receita é escolhida por ela).
        out.append({'name': r['name'], 'label': ing['label'], 'source': r['source'], 'duration': 200, 'power': 200,
                    'in': [ing], 'fin': [], 'out': outs, 'fout': [], 'pools': []})
    skipped = len(src['skipped'])
    # As linhas de minério de bedrock (CentrifugeRecipes.java:267-288), da
    # etapa 1 da cadeia do bedrock, quando o agente de conteúdo já as extraiu.
    b = root / 'content/machine-recipes/bedrock.json'
    if b.exists():
        bsrc = json.loads(b.read_text())
        for r in bsrc.get('centrifuge', []):
            inp = r.get('input') or r['inputItems'][0]
            ing = ingredient(inp, r.get('inputAlternatives'))
            outs = [[o[0], o[1], o[2] if len(o) > 2 else 1.0] for o in (r.get('outputs') or r['outputItems'])]
            out.append({'name': r['name'], 'label': ing['label'], 'source': r['source'], 'duration': 200, 'power': 200,
                        'in': [ing], 'fin': [], 'out': outs, 'fout': [], 'pools': []})
        skipped += len([x for x in bsrc.get('skipped', []) if 'entrifuge' in json.dumps(x)])
    return {'station': 'centrifuge', 'source': 'content/machine-recipes/centrifuge.json', 'recipes': out, 'skipped': skipped}


def shredder():
    """Uma entrada → (saída, quantidade); a 1ª inserção vence, como no
    `setRecipe` do NTM (o agente de conteúdo já resolveu as colisões). Sem
    receita, a máquina dá sucata (`shredder::SCRAP`)."""
    src = json.loads((root / 'content/machine-recipes/shredder.json').read_text())
    out = {}
    for r in src['recipes']:
        (o, n), = r['outputs']
        for i in r['input']['ids']:
            out.setdefault(i, [o, n])
    blades = [[b['id'], b['durability']] for b in src.get('blades', [])]
    return {'station': 'shredder', 'source': 'content/machine-recipes/shredder.json', 'recipes': out, 'blades': blades,
            'skipped': len(src['skipped'])}


def blast_furnace():
    """Na ordem do NTM (`source_index`: `getRecipe` pega a primeira que
    serve), e o que cada combustível vale (`ModuleBurnTime` do alto-forno)."""
    src = json.loads((root / 'content/machine-recipes/blast_furnace.json').read_text())
    out = []
    for r in sorted(src['recipes'], key=lambda r: r['source_index']):
        out.append({'name': r['name'], 'label': r.get('label') or r['name'], 'source': r['source'], 'duration': r['duration'],
                    'in': [{'ids': i['ids'], 'n': i['count']} for i in r['inputItems']],
                    'out': [[o[0], o[1]] for o in r['outputItems']]})
    return {'station': 'blast_furnace', 'source': 'content/machine-recipes/blast_furnace.json', 'recipes': out, 'fuels': src['fuels'],
            'skipped': len(src['skipped'])}


def foundry():
    """Materiais, item → materiais (antes da conversão), as ligas ativas na
    ordem da fonte, os combustíveis da firebox e os moldes."""
    src = json.loads((root / 'content/machine-recipes/foundry.json').read_text())
    keep = ('id', 'name', 'behavior', 'smelts_into', 'conv', 'colors')
    materials = [{k: m[k] for k in keep} for m in src['materials']]
    alloys = [{'name': a['name'], 'label': name(a['icon']) if a.get('icon') else a['name'], 'frequency': a['frequency'], 'icon': a.get('icon', ''),
               'in': a['in'], 'out': a['out']}
              for a in sorted(src['alloys'], key=lambda a: a['source_index'])]
    molds = [{k: m[k] for k in ('id', 'size', 'name', 'item', 'shape', 'amount', 'cost', 'outputs')} for m in src.get('molds', [])]
    return {'station': 'foundry', 'source': 'content/machine-recipes/foundry.json', 'quanta': src['quanta'], 'materials': materials,
            'items': src['items'], 'alloys': alloys, 'firebox_fuels': src['firebox_fuels'], 'molds': molds,
            'recipes': alloys, 'skipped': len(src.get('skipped', []))}


def anvil():
    """Forja e construção na ordem do NTM (`source_index`: a primeira que
    casa); o rótulo de cada construção é a sua primeira saída."""
    src = json.loads((root / 'content/machine-recipes/anvil.json').read_text())
    side = lambda x: {'ids': x['ids'], 'count': x['count'], 'exact': bool(x.get('exact'))}
    smithing = [{'name': r['name'], 'tier': r['tier'], 'left': side(r['left']), 'right': side(r['right']), 'out': r['out'],
                 'shapeless': r.get('shapeless', False), 'consume': r['consume']}
                for r in sorted(src['smithing'], key=lambda r: r['source_index'])]
    def clabel(r):
        outs = r['out']
        base = label(r, outs, [])
        return base + (f" (+{len(outs) - 1})" if len(outs) > 1 else '')
    construction = [{'name': r['name'], 'label': clabel(r), 'in': [side(i) for i in r['in']], 'out': r['out'],
                     'tier_lower': r['tier_lower'], 'tier_upper': r['tier_upper']}
                    for r in sorted(src['construction'], key=lambda r: r['source_index'])]
    return {'station': 'anvil', 'source': 'content/machine-recipes/anvil.json', 'smithing': smithing, 'construction': construction,
            'recipes': smithing + construction, 'skipped': len(src['skipped'])}



def rotary_furnace():
    src = json.loads((root / 'content/machine-recipes/rotary_furnace.json').read_text())
    out = []
    for r in src['recipes']:
        ins = [ingredient(x) for x in r['inputItems']]
        out.append({
            'name': r['name'],
            'label': r.get('label') or r['name'],
            'source': r['source'],
            'duration': r['duration'],
            'steam': r['steam'],
            'in': ins,
            'fluid': r['inputFluids'][0] if r['inputFluids'] else None,
            'out': r['outputMaterials'][0],
        })
    return {
        'station': 'rotary_furnace',
        'source': 'content/machine-recipes/rotary_furnace.json',
        'config': src['machine_config'],
        'recipes': out,
        'skipped': len(src['skipped']),
    }


def press():
    """As duas prensas (`press_recipes.json`, lido por `recipes::press`):
    as matrizes e, na ordem do NTM, uma receita por alternativa do OreDict
    (a busca pega a primeira que casa). Velocidade e energia das prensas
    são as do M1 (`TileEntityMachinePress`/`EPress`)."""
    src = json.loads((root / 'content/machine-recipes/press.json').read_text())
    stamps = [{'item': s['item'], 'type': s['type'], 'durability': s['durability']} for s in src['stamps']]
    recipes = []
    for r in sorted(src['recipes'], key=lambda r: r['source_index']):
        for i in r['in']['ids']:
            recipes.append({'id': r['name'], 'input': {'item': i, 'count': r['in']['count']},
                            'output': {'item': r['out'][0], 'count': r['out'][1]}, 'stamp': r['stamp']})
    return {'schema': 1, 'stamps': stamps,
            'machines': {'machine_epress': {'energy_per_tick': 100}, 'machine_press': {'max_speed': 400, 'progress_at_max': 25, 'max_press': 200}},
            'recipes': recipes, 'skipped': len(src['skipped'])}


for st, data in [('assembly', generic('assembly')), ('chemical_plant', generic('chemical_plant')), ('centrifuge', centrifuge()), ('shredder', shredder()), ('blast_furnace', blast_furnace()), ('foundry', foundry()), ('anvil', anvil())]:
    p = root / f'crates/ntm-core/data/machines/{st}.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
    print(f"{st}: {len(data['recipes'])} receitas ({data['skipped']} puladas na extração) → {p.relative_to(root)}")

# O PUREX, quando o agente de conteúdo já o extraiu (o formato da planta química).
if (root / 'content/machine-recipes/purex.json').exists():
    data = generic('purex')
    p = root / 'crates/ntm-core/data/machines/purex.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
    print(f"purex: {len(data['recipes'])} receitas ({data['skipped']} puladas na extração) → {p.relative_to(root)}")

# O cristalizador (etapa 2 da cadeia do bedrock), no formato da planta química.
if (root / 'content/machine-recipes/crystallizer.json').exists():
    data = generic('crystallizer')
    p = root / 'crates/ntm-core/data/machines/crystallizer.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
    print(f"crystallizer: {len(data['recipes'])} receitas ({data['skipped']} puladas na extração) → {p.relative_to(root)}")

# Etapas 3 e 4 da cadeia do bedrock: o forno pirolítico, as duas linhas do
# eletrolisador e os dois modos do forno a arco grande; os lotes A e B do
# petróleo: liquefator, solidificador, refinaria, torre de fração, misturador.
for st, section in (('pyrooven', None), ('electrolyser', 'fluid'), ('electrolyser_metal', 'metal'), ('arc_furnace_solid', 'solid'), ('arc_furnace_fluid', 'fluid'),
                    ('liquefaction', None), ('solidification', None),
                    ('refinery', None), ('fraction', None), ('mixer', None),
                    ('cracking', None), ('reforming', None), ('hydrotreating', None),
                    ('vacuum_refinery', None), ('coker', None), ('boiler', None)):
    src = st.split('_metal')[0].replace('_solid', '').replace('_fluid', '')
    if (root / f'content/machine-recipes/{src}.json').exists():
        data = generic(src, section)
        if st == 'arc_furnace_fluid':
            # Scraps melt into their own material (ArcFurnaceRecipes:170-176).
            data['scraps'] = json.loads((root / 'content/machine-recipes/arc_furnace.json').read_text()).get('scrap_default_materials', {})
        p = root / f'crates/ntm-core/data/machines/{st}.json'
        p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
        print(f"{st}: {len(data['recipes'])} receitas ({data['skipped']} puladas na extração) → {p.relative_to(root)}")


if (root / 'content/machine-recipes/rotary_furnace.json').exists():
    data = rotary_furnace()
    p = root / 'crates/ntm-core/data/machines/rotary_furnace.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
    print(f"rotary_furnace: {len(data['recipes'])} receitas ({data['skipped']} puladas na extração) → {p.relative_to(root)}")

# O tamanho da pilha de cada saída dos moldes (`ItemStack.getMaxStackSize`, usado pelo lingotamento): só as que não
# passam de 1 ou de outro valor diferente de 64 (as lâminas e os carimbos têm 1); o resto é 64.
if (root / 'content/machine-recipes/foundry.json').exists() and (root / 'content/ids.json').exists():
    molds = json.loads((root / 'content/machine-recipes/foundry.json').read_text())['molds']
    outs = {item for m in molds for item, _ in m['outputs'].values()}
    ids = {e['id']: e for e in json.loads((root / 'content/ids.json').read_text())['entries']}
    limits = {i: ids[i]['max_stack_size'] for i in sorted(outs) if i in ids and ids[i].get('max_stack_size', 64) != 64}
    p = root / 'crates/ntm-core/data/machines/mold_stack_limits.json'
    p.write_text(json.dumps({'source': 'content/ids.json, the outputs of content/machine-recipes/foundry.json', 'limits': limits}, ensure_ascii=False, indent=0) + '\n')
    print(f"mold_stack_limits: {len(limits)} saídas de molde com pilha diferente de 64 → {p.relative_to(root)}")

# As ferramentas do NTM com a classe "shovel" (`ItemTool.getToolClasses(...).contains("shovel")`, que a pá do lingotamento e dos
# blocos de fundição lê): as `ItemToolAbility` de tipo SHOVEL e as de MINER (`setHarvestLevel("shovel", ...)`) que o conteúdo tem.
# As do vanilla entram pela regra `minecraft:*_shovel`.
ntm_src = Path(os.environ.get('NTM_SRC', root.parent / 'ntm-src'))
mod_items = ntm_src / 'src/main/java/com/hbm/items/ModItems.java'
if (root / 'content/ids.json').exists() and not mod_items.exists():
    mod_items = Path('/mnt/ssd120/sacer/horizon-ntm/ntm-src/src/main/java/com/hbm/items/ModItems.java')
if (root / 'content/ids.json').exists() and mod_items.exists():
    import re
    lines = mod_items.read_text().split('\n')
    shovels = []
    for e in json.loads((root / 'content/ids.json').read_text())['entries']:
        if e.get('type') == 'item' and e.get('source_class', '').startswith('ItemToolAbility'):
            at = int(e['source_location'].split(':')[-1])
            kind = re.search(r'EnumToolType\.(\w+)', ' '.join(lines[at - 1:at + 3]))
            if kind and kind.group(1) in ('SHOVEL', 'MINER'):
                shovels.append(e['id'])
    p = root / 'crates/ntm-core/data/tool_shovels.json'
    p.write_text(json.dumps({'source': 'ModItems.java ItemToolAbility SHOVEL and MINER, the items content/ids.json has', 'shovels': sorted(shovels)}, indent=0) + '\n')
    print(f"tool_shovels: {len(shovels)} ferramentas com a classe shovel → {p.relative_to(root)}")

# A prensa fica fora de machines/: as duas prensas do M1 a leem de data/.
if (root / 'content/machine-recipes/press.json').exists():
    data = press()
    p = root / 'crates/ntm-core/data/press_recipes.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
    print(f"press: {len(data['recipes'])} receitas por item, {len(data['stamps'])} matrizes ({data['skipped']} puladas na extração) → {p.relative_to(root)}")

# Lote 1 de energia: os combustíveis do forno de aquecimento (já com o
# módulo de queima e o timeMult), o calor por balde dos inflamáveis
# (queimador de óleo) e os resfriáveis (trocador de calor; turbinas no lote 2).
if (root / 'content/machine-recipes/heaters.json').exists():
    src = json.loads((root / 'content/machine-recipes/heaters.json').read_text())
    traits = json.loads((root / 'content/fluid_traits.json').read_text())
    fuels = {k: {'ticks': v['burn_ticks'], 'heat': v['heat_tu_per_tick'], 'container': v['container']} for k, v in src['oven_fuels'].items()}
    flammable = {t['fluid']: t['heat_tu_per_bucket'] for t in traits['flammable']}
    coolable = {t['fluid']: {'out': t['output'][0], 'out_mb': t['output'][1], 'in_mb': t['input_mb'], 'heat': t['heat_tu'],
                             'heatex': t['efficiencies'].get('HEATEXCHANGER', 0), 'turbine': t['efficiencies'].get('TURBINE', 0)}
                for t in traits['coolable']}
    data = {'source': 'content/machine-recipes/heaters.json, content/fluid_traits.json', 'oven_fuels': fuels, 'flammable': flammable, 'coolable': coolable}
    p = root / 'crates/ntm-core/data/machines/heaters.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0, sort_keys=True) + '\n')
    print(f"heaters: {len(fuels)} combustíveis, {len(flammable)} inflamáveis, {len(coolable)} resfriáveis → {p.relative_to(root)}")

# Lote 3 de energia: os combustíveis (FT_Combustible: grade e HE por balde),
# a eficiência do diesel por grade e a matriz dos pistões do motor a combustão.
if (root / 'content/machine-recipes/combustion.json').exists():
    src = json.loads((root / 'content/machine-recipes/combustion.json').read_text())
    traits = json.loads((root / 'content/fluid_traits.json').read_text())
    cfg = src['machine_config']
    grades = traits['fuel_grades']
    fuels = {t['fluid']: [t['grade_ordinal'], t['he_per_bucket']] for t in traits['combustible']}
    pistons = {f"ntm:piston_set__{k.lower()}": [v[g] for g in grades] for k, v in cfg['ntm:machine_combustion_engine']['piston_efficiency'].items()}
    diesel = {g: e for g, e in cfg['ntm:machine_diesel']['efficiency_by_grade'].items()}
    data = {'source': 'content/machine-recipes/combustion.json, content/fluid_traits.json', 'grades': grades, 'fuels': fuels,
            'diesel_efficiency': diesel, 'pistons': pistons}
    p = root / 'crates/ntm-core/data/machines/combustion.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0, sort_keys=True) + '\n')
    print(f"combustion: {len(fuels)} combustíveis, {len(pistons)} pistões → {p.relative_to(root)}")

# Os arquivos de máquina (horizon_machine) que o bundle nativo usa sem ler o
# conteúdo em tempo de execução: as regras de cada slot (accept, reject,
# max, output, grid, a posição x/y) e os índices sincronizados com os modelos,
# mais o desenho da tela (título, telas, inventário) que o intérprete lê.
hm = root / 'content/data/ntm/horizon_machine'
if hm.is_dir():
    keep = ('accept', 'reject', 'max', 'output', 'grid', 'x', 'y')
    layout = ('title', 'screen', 'screens', 'inventory')
    files = {}
    for f in sorted(hm.glob('*.json')):
        d = json.loads(f.read_text())
        files[f'ntm:{f.stem}'] = {'slots': [{k: s[k] for k in keep if k in s} for s in d.get('slots', [])], 'synced': d.get('synced', []),
                           **{k: d[k] for k in layout if k in d}}
    p = root / 'crates/ntm-core/data/machine_files.json'
    p.write_text(json.dumps(files, ensure_ascii=False, indent=0, sort_keys=True) + '\n')
    print(f"machine_files: {len(files)} máquinas → {p.relative_to(root)}")

# The combination oven: an item (or an OreDict name's items) into an item and
# a fluid; `dict` rows are the second pass of `getOutput`, after the exact item.
if (root / 'content/machine-recipes/combination.json').exists():
    src = json.loads((root / 'content/machine-recipes/combination.json').read_text())
    rows = [{'ids': r['in']['ids'], **({'dict': r['in']['dict']} if 'dict' in r['in'] else {}),
             'item': r['outputItems'][0] if r['outputItems'] else None,
             'fluid': r['outputFluids'][0] if r['outputFluids'] else None, 'source': r['source'], 'label': r.get('label', r['name'])}
            for r in src['recipes']]
    data = {'source': 'content/machine-recipes/combination.json', 'config': src['machine_config'], 'recipes': rows}
    p = root / 'crates/ntm-core/data/machines/combination.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
    print(f"combination: {len(rows)} receitas → {p.relative_to(root)}")

# The arc welder and the soldering station: the ingredients matched in any
# order (`SerializableRecipe.matchesIngredients`), an optional fluid, an output.
for name in ('arc_welder', 'soldering'):
    f = root / f'content/machine-recipes/{name}.json'
    if not f.exists():
        continue
    src = json.loads(f.read_text())
    def ing(x):
        return {'ids': x['ids'], 'n': x['count']}
    rows = []
    # `getMaxStackSize()` of the product: the output slot stops there (`TileEntityMachineArcWelder.java:191`,
    # `TileEntityMachineSolderingStation.java:166`); the catalog carries it as `max_stack_size`.
    stacks = {e['id']: e['max_stack_size'] for e in ids['entries'] if e.get('max_stack_size')}
    for r in src['recipes']:
        row = {'out': r['outputItems'][0], 'stack': stacks.get(r['outputItems'][0][0], 64), 'duration': r['duration'], 'power': r['power'],
               'fluid': r['inputFluids'][0] if r['inputFluids'] else None, 'source': r['source'], 'label': r.get('label', r['name'])}
        if name == 'arc_welder':
            row['in'] = [ing(x) for x in r['inputItems']]
        else:
            row.update(toppings=[ing(x) for x in r['toppings']], pcb=[ing(x) for x in r['pcb']], solder=[ing(x) for x in r['solder']])
        rows.append(row)
    p = root / f'crates/ntm-core/data/machines/{name}.json'
    p.write_text(json.dumps({'source': f'content/machine-recipes/{name}.json', 'config': src['machine_config'], 'recipes': rows}, ensure_ascii=False, indent=0) + '\n')
    print(f"{name}: {len(rows)} receitas → {p.relative_to(root)}")

# Stateful reactor data is exported once by recipe-gen.
(root / 'crates/ntm-core/data/reactors_simple.json').write_text((root / 'content/machine-recipes/reactors_simple.json').read_text())

# Radioisotopes: keep Java float32/long lifetimes and fuel contracts verbatim.
(root / 'crates/ntm-core/data/radioisotopes.json').write_text((root / 'content/machine-recipes/radioisotopes.json').read_text())

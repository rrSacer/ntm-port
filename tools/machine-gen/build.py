#!/usr/bin/env python3
"""Converte as receitas extraídas pelo agente de conteúdo
(content/machine-recipes/{assembly,chemical_plant,centrifuge,shredder,blast_furnace}.json, schema 1)
para o formato que a lógica embute (crates/ntm-core/data/machines/*.json),
com os nomes pt_BR do content/ids.json para a tela.

Uso: python3 tools/machine-gen/build.py
"""
import json
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


def generic(st):
    src = json.loads((root / f'content/machine-recipes/{st}.json').read_text())
    out = []
    for r in src['recipes']:
        alts = r.get('inputItemAlternatives') or [None] * len(r['inputItems'])
        ins = [ingredient(x, a) for x, a in zip(r['inputItems'], alts)]
        outs = [[o[0], o[1], o[2] if len(o) > 2 else 1.0] for o in r['outputItems']]
        out.append({
            'name': r['name'], 'label': r.get('label') or label(r, outs, r['outputFluids']), 'source': r['source'],
            'duration': r['duration'], 'power': r['power'],
            'in': ins, 'fin': r['inputFluids'], 'out': outs, 'fout': r['outputFluids'],
            'pools': r.get('pools', []),
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
    return {'station': 'centrifuge', 'source': 'content/machine-recipes/centrifuge.json', 'recipes': out, 'skipped': len(src['skipped'])}


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


for st, data in [('assembly', generic('assembly')), ('chemical_plant', generic('chemical_plant')), ('centrifuge', centrifuge()), ('shredder', shredder()), ('blast_furnace', blast_furnace())]:
    p = root / f'crates/ntm-core/data/machines/{st}.json'
    p.write_text(json.dumps(data, ensure_ascii=False, indent=0) + '\n')
    print(f"{st}: {len(data['recipes'])} receitas ({data['skipped']} puladas na extração) → {p.relative_to(root)}")

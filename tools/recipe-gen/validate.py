#!/usr/bin/env python3
"""Check graph integrity, source counts and regeneration without a game."""
import hashlib,json,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[2]
manifest=root/'tools/recipe-gen/generated-files.json'
def snapshot():
    return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in json.loads(manifest.read_text())}
before=snapshot()
subprocess.run(['python3',str(root/'tools/recipe-gen/generate.py')],check=True)
assert before==snapshot(),'Generation must be deterministic'
ids={e['id'] for e in json.loads((root/'content/ids.json').read_text())['entries']}
for rel in json.loads(manifest.read_text()):
    data=json.loads((root/rel).read_text())
    if '/recipe/' in rel:
        assert Path(rel).parent.name=='recipe','Horizon only loads flat recipe directory'
        assert data['result']['count']>0
        for ingredient in [*data.get('key',{}).values(),*data.get('ingredients',[]),data.get('ingredient','')]:
            if ingredient.startswith('ntm:'): assert ingredient in ids,ingredient
            if ingredient.startswith('#ntm:'):
                tag=root/'content/data/ntm/tags/item'/ (ingredient[5:]+'.json')
                assert tag.exists(),tag
                assert all(v in ids for v in json.loads(tag.read_text())['values'])
        assert data['result']['id'] in ids
furnace=json.loads((root/'crates/ntm-core/data/furnace_recipes.json').read_text())
for r in furnace['recipes']:
    for side in ['input','output']:
        item=r[side]['item']
        if item.startswith('ntm:'): assert item in ids,item
press=json.loads((root/'content/machine-recipes/press.json').read_text())
stamp_types={s['type'] for s in press['stamps']} | {t for s in press.get('skipped_stamps',[]) for t in s.get('types',[])}
assert all(s['item'] in ids for s in press['stamps'])
for r in press['recipes']:
    for item in [*r['in']['ids'], r['out'][0]]:
        if item.startswith('ntm:'): assert item in ids,item
    assert r['stamp'] in stamp_types,r['stamp']
print('PASS: deterministic generation, flat paths, item/tag references, machine/stamp integrity')

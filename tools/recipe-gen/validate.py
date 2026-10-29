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
for name in ['press','furnace']:
    recipes=json.loads((root/f'crates/ntm-core/data/{name}_recipes.json').read_text())
    for r in recipes['recipes']:
        for side in ['input','output']:
            item=r[side]['item']
            if item.startswith('ntm:'): assert item in ids,item
    if name=='press':
        assert all(s['item'] in ids for s in recipes['stamps'])
        assert all(r['stamp'] in {s['type'] for s in recipes['stamps']} for r in recipes['recipes'])
print('PASS: deterministic generation, flat paths, item/tag references, machine/stamp integrity')

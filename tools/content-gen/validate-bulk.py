#!/usr/bin/env python3
"""Validate generated assets, recipe graph and hazard/tag agreement."""
import json
import re
from collections import Counter
from functools import cache
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];CONTENT=ROOT/'content'
vanilla=Path('/home/sacer/Games/horizon/.local/horizon-client/26.3')
catalog=json.loads((CONTENT/'ids.json').read_text());rows=catalog['entries'];ids={e['id'] for e in rows}
assert len(ids)==len(rows)

def read(path):return json.loads(path.read_text())
def exists(kind,identity):
    ns,name=identity.split(':',1);path=f'{ns}/{kind}/{name}.json'
    return (CONTENT/'data'/path).is_file() if ns=='ntm' else (vanilla/'data'/path).is_file()
def item(identity):
    if identity.startswith('#'):
        assert exists('tags/item',identity[1:]),identity
    elif identity.startswith('ntm:'):assert identity in ids,identity
    else:
        ns,name=identity.split(':',1);assert (vanilla/'assets'/ns/'items'/(name+'.json')).exists(),identity

@cache
def model(identity):
    if identity in ('builtin/generated','minecraft:builtin/generated','builtin/entity','minecraft:builtin/entity'):return
    ns,name=identity.split(':',1) if ':' in identity else ('minecraft',identity)
    path=(CONTENT if ns=='ntm' else vanilla)/'assets'/ns/'models'/(name+'.json')
    assert path.exists(),path
    obj=read(path)
    for tex in obj.get('textures',{}).values():
        if tex.startswith('#'):continue
        tns,tname=tex.split(':',1) if ':' in tex else ('minecraft',tex)
        target=(CONTENT if tns=='ntm' else vanilla)/'assets'/tns/'textures'/(tname+'.png')
        assert target.exists(),target
    if obj.get('parent'):model(obj['parent'])

for row in rows:
    name=row['id'].split(':')[1]
    for loc in ('en_us','pt_br'):
        text=read(CONTENT/f'assets/ntm/lang/{loc}.json')[('block' if row['type']=='block' else 'item')+'.ntm.'+name]
        assert text and not re.search(r'%[0-9$]*s',text),(row['id'],loc,text)
    if row['type']=='block':
        state=read(CONTENT/f'assets/ntm/blockstates/{name}.json')
        for variants in state.get('variants',{}).values():
            for variant in variants if isinstance(variants,list) else [variants]:model(variant['model'])
        loot=read(CONTENT/f'data/ntm/loot_table/blocks/{name}.json')
        def drops(node):
            if isinstance(node,dict):
                if node.get('type')=='minecraft:item':item(node['name'])
                for value in node.values():drops(value)
            elif isinstance(node,list):
                for value in node:drops(value)
        drops(loot)
    model(read(CONTENT/f'assets/ntm/items/{name}.json')['model']['model'])
for path in (CONTENT/'data/ntm/recipe').glob('*.json'):
    rec=read(path);item(rec['result']['id']);assert 1<=rec['result'].get('count',1)<=64,path
    for ingredient in [*rec.get('key',{}).values(),*rec.get('ingredients',[]),*([rec['ingredient']] if 'ingredient' in rec else [])]:item(ingredient)
for path in (CONTENT/'data/ntm/tags/item').rglob('*.json'):
    for identity in read(path)['values']:item(identity)
hazards=read(ROOT/'crates/ntm-core/data/hazards.json')['entries'];assert {h['id'] for h in hazards}==ids
buckets=read(ROOT/'crates/ntm-core/data/radiation_buckets.json')
for bucket in buckets['buckets']:
    rate=bucket['radiation_millirad_s'];tag=read(CONTENT/'data/ntm/tags/item'/ (bucket['tag'].split(':')[1]+'.json'))
    assert tag['values']==sorted(h['id'] for h in hazards if h['radiation_millirad_s']==rate)
for bucket in buckets['bits']:
    tag=read(CONTENT/'data/ntm/tags/item'/ (bucket['tag'].split(':')[1]+'.json'))
    assert tag['values']==sorted(h['id'] for h in hazards if h['radiation_millirad_s'] & (1<<bucket['bit']))
tabs=list((CONTENT/'data/ntm/creative_tab').glob('*.json'));tab_items=[]
for path in tabs:
    tab=read(path);item(tab['icon']);tab_items.extend(tab['items'])
    for identity in tab['items']:item(identity)
    for loc in ('en_us','pt_br'):assert tab['title'] in read(CONTENT/f'assets/ntm/lang/{loc}.json')
if tabs:assert set(tab_items)==ids and len(tab_items)==len(ids),'creative tabs cover catalog once'
report={'status':'pass','ids':len(ids),'categories':dict(Counter(r['category'] for r in rows)),'blocks':sum(r['type']=='block' for r in rows),'items':sum(r['type']=='item' for r in rows),'recipes':len(list((CONTENT/'data/ntm/recipe').glob('*.json'))),'radiation_rates':len(buckets['buckets']),'radiation_bits':len(buckets['bits']),'creative_tabs':len(tabs),'checks':'ids, source-localized names, model parents/textures, loot item graph, recipe/item/tag graph, exact hazard identity/rate/bit agreement'}
path=ROOT/'content-gen-evidence/bulk/static-validation.json';path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))

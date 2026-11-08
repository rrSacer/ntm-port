#!/usr/bin/env python3
"""Generate M1 recipes; only files in our manifest are removed on regeneration.

Source recipes are curated from NTM 1.7.10, GPL-3.0. No Java execution or
heuristic conversion of metadata. All temporary recipes use the flat m1_temp__ filename prefix.
"""
import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
p = argparse.ArgumentParser()
p.add_argument('--source', type=Path, default=ROOT.parent / 'ntm-src')
a = p.parse_args()
JAVA = a.source / 'src/main/java/com/hbm'
CONTENT = ROOT / 'content'
entries = json.loads((CONTENT / 'ids.json').read_text())['entries']
ids = {e['id'] for e in entries}
files = {}
provenance = []
furnace = []
# Handed over to tools/machine-gen/build.py (fed by content/machine-recipes/press.json).
HANDED_OFF = {'crates/ntm-core/data/press_recipes.json'}

def write(path, obj):
    files[str(path.relative_to(ROOT))] = json.dumps(obj, indent=2, ensure_ascii=False) + '\n'

def ni(name):
    return name if ':' in name or name.startswith('#') else 'ntm:' + name

def check(item):
    if item.startswith('ntm:'):
        assert item in ids, f'Unregistered item {item}'

def tag(item):
    item = ni(item)
    check(item)
    name = 'ingredients/' + item.split(':')[1]
    write(CONTENT / f'data/ntm/tags/item/{name}.json', {'replace': False, 'values': [item]})
    return '#ntm:' + name

def add(name, recipe, source, temporary=False):
    for item in list(recipe.get('key', {}).values()) + recipe.get('ingredients', []) + [recipe.get('ingredient', ''), recipe['result']['id']]:
        if isinstance(item, str): check(item)
    path = ('m1_temp__' if temporary else '') + name
    write(CONTENT / f'data/ntm/recipe/{path}.json', recipe)
    provenance.append({'id': 'ntm:' + path, 'source': source, 'temporary': temporary})

def shaped(name, out, pattern, key, count=1, source='', temporary=False):
    add(name, {'type': 'minecraft:crafting_shaped', 'category': 'misc', 'pattern': pattern, 'key': {k: ni(v) for k,v in key.items()}, 'result': {'id': ni(out), 'count': count}}, source, temporary)

def shapeless(name, out, ingredients, count=1, source='', temporary=False):
    add(name, {'type': 'minecraft:crafting_shapeless', 'category': 'misc', 'ingredients': [ni(i) for i in ingredients], 'result': {'id': ni(out), 'count': count}}, source, temporary)

def pair(big, small, source):
    shaped(big+'_from_'+small, big, ['###']*3, {'#':tag(small)}, source=source)
    shapeless(small+'_from_'+big, small, [big], 9, source)

# Storage and nugget pairs, directly specified in MineralRecipes.java.
special = {'thorium':'ingot_th232', 'lithium':'lithium', 'asbestos':'ingot_asbestos', 'sulfur':'sulfur', 'niter':'niter', 'fluorite':'fluorite'}
for e in entries:
    if e['category'] == 'storage':
        small = special.get(e['material'], 'ingot_'+e['material'])
        if ni(small) in ids: pair(e['id'].split(':')[1], small, 'crafting/MineralRecipes.java:39,227-331')
for mat in ['uranium','th232','lead','beryllium']:
    pair('ingot_'+mat, 'nugget_'+mat, 'crafting/MineralRecipes.java:333-350')
for mat in ['aluminium','lithium','titanium','tungsten']:
    pair('lithium' if mat == 'lithium' else 'ingot_'+mat, 'nugget_raw__'+mat, 'inventory/OreDictManager.java:666-670')
for mat in ['uranium','beryllium']:
    b,i,n = ['%s_%s'%(kind,mat) for kind in ['billet','ingot','nugget']]
    shaped(b+'_from_nuggets', b, ['###','###'], {'#':tag(n)}, source='crafting/MineralRecipes.java:526-536')
    shapeless(n+'_from_billet', n, [b], 6, 'crafting/MineralRecipes.java:528')
    shaped(b+'_from_ingots', b, ['##'], {'#':tag(i)}, 3, 'crafting/MineralRecipes.java:536')
    shapeless(i+'_from_billets', i, [b]*3, 2, 'crafting/MineralRecipes.java:535')

# Parse only simple, unambiguous direct addSmelting calls; exact source counts/XP.
smelt_text = (JAVA / 'crafting/SmeltingRecipes.java').read_text()
pattern = r'GameRegistry.addSmelting\((?:Item.getItemFromBlock\()?Mod(?:Items|Blocks)\.(\w+)\)?, new ItemStack\(ModItems\.(\w+)(?:, (\d+))?\), ([\d.]+)F\);'
for m in re.finditer(pattern, smelt_text):
    src,out,count,xp = m.groups()
    if ni(src) not in ids or ni(out) not in ids: continue
    line = smelt_text[:m.start()].count('\n')+1
    count=int(count or 1)
    for kind,ticks in [('smelting',200),('blasting',100)]:
        add(out+'_from_'+kind+'_'+src, {'type':'minecraft:'+kind,'category':'misc','ingredient':ni(src),'result':{'id':ni(out),'count':count},'experience':float(xp),'cookingtime':ticks}, f'crafting/SmeltingRecipes.java:{line}' + ('; blasting is 26.3 port extension' if kind=='blasting' else ''))
    furnace.append({'id':'ntm:'+src,'input':{'item':ni(src),'count':1},'output':{'item':ni(out),'count':count},'experience':float(xp)})

shaped('machine_press', 'machine_press', ['IRI','IPI','IBI'], {'I':'minecraft:iron_ingot','R':'minecraft:furnace','P':'minecraft:piston','B':'minecraft:iron_block'},source='main/CraftingManager.java:288')
for brick in ['brick','nether_brick']:
    shaped('stamp_iron_flat_from_'+brick,'stamp_iron_flat',['III','SSS'],{'I':'minecraft:'+brick,'S':'minecraft:iron_ingot'},source='main/CraftingManager.java:346-354')

# Java recipes whose ingredients M1 already registers: no bridge needed.
shaped('machine_wood_burner','machine_wood_burner',['PPP','CFC','I I'],{'P':'plate_steel','C':'coil_copper','I':'minecraft:iron_ingot','F':'minecraft:furnace'},source='main/CraftingManager.java:251')
shaped('red_cable','red_cable',[' W ','RRR',' W '],{'W':'plate_polymer','R':'wire_fine__red_copper'},16,'main/CraftingManager.java:239')
shaped('machine_electric_furnace','machine_electric_furnace_off',['BBB','WFW','RRR'],{'B':'ingot_beryllium','W':'plate_cast__copper','F':'minecraft:furnace','R':'coil_tungsten'},source='main/CraftingManager.java:230')
shaped('wiring_red_copper','wiring_red_copper',['PPP','PIP','PPP'],{'P':'plate_steel','I':'ingot_steel'},source='main/CraftingManager.java:539')

# Remaining temporary M1 bridges: each has no Java recipe that M1 can register (see source note).
shaped('machine_epress','machine_epress',['PPP','RMR','WCW'],{'P':'plate_titanium','R':'minecraft:piston','M':'machine_press','W':'wire_fine__red_copper','C':'minecraft:redstone'},source='inventory/recipes/AssemblyMachineRecipes.java:354-356 (assembly machine, not ported; its part_generic/item_expensive inputs have no catalog IDs); temporary crafting bridge',temporary=True)
shaped('machine_battery','machine_battery',['PPP','RLR','WWW'],{'P':'plate_lead','R':'minecraft:redstone','L':'lithium','W':'wire_fine__red_copper'},source='no Java 1.7.10 recipe: MachineBattery is registered in ModBlocks without any addRecipe; M1 design bridge',temporary=True)
for stamp,pattern in [('plate',[' I ',' S ',' I ']),('wire',['I I',' S ','I I'])]:
    shaped('stamp_iron_'+stamp,'stamp_iron_'+stamp,pattern,{'I':'minecraft:iron_ingot','S':'stamp_iron_flat'},source='inventory/recipes/anvil/AnvilRecipes.java:588-589 (anvil construction, anvil not ported); temporary crafting bridge',temporary=True)

# Embed all basic vanilla 26.3 furnace recipes. Use local bundled source data,
# preserving ingredient tags/alternatives by resolving the vanilla tag graph.
vanilla=Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/data/minecraft')
assert (vanilla/'recipe').is_dir(), 'Missing local vanilla 26.3 recipe data'
def expand(ingredient, seen=()):
    if isinstance(ingredient,list): return [v for i in ingredient for v in expand(i,seen)]
    if not ingredient.startswith('#'): return [ingredient]
    if ingredient in seen: raise ValueError('Cyclic item tag '+ingredient)
    ns,name=ingredient[1:].split(':',1)
    f=vanilla.parent/ns/'tags/item'/ (name+'.json')
    if not f.exists(): return []
    return [v for x in json.loads(f.read_text())['values'] for v in expand(x if isinstance(x,str) else x['id'],seen+(ingredient,))]
for f in sorted((vanilla/'recipe').glob('*.json')):
    rec=json.loads(f.read_text())
    if rec.get('type')!='minecraft:smelting': continue
    result=rec['result']
    if isinstance(result,str): result={'id':result}
    for item in sorted(set(expand(rec['ingredient']))):
        furnace.append({'id':'minecraft:'+f.stem+'/'+item.split(':')[1], 'input':{'item':item,'count':1}, 'output':{'item':result['id'],'count':result.get('count',1)},'experience':rec.get('experience',0)})

write(ROOT/'crates/ntm-core/data/furnace_recipes.json',{'schema':1,'machines':{'machine_electric_furnace_off':{'ticks':100,'energy_per_tick':50}},'recipes':furnace})
# Coverage records are intentionally descriptive: many catalog items have no
# crafting recipe (worldgen drops, crystallizer, casting, welding, wiremill).
press_path = CONTENT / 'machine-recipes/press.json'
press_count = len(json.loads(press_path.read_text())['recipes']) if press_path.is_file() else 0
outputs={json.loads(v)['result']['id'] for k,v in files.items() if '/recipe/' in k}
coverage={'catalog_items':len(ids),'vanilla_recipe_count':len(provenance),'temporary_count':sum(x['temporary'] for x in provenance),'press_count':press_count,'furnace_count':len(furnace),'recipes':provenance,'without_vanilla_recipe':sorted(ids-outputs)}
write(ROOT/'tools/recipe-gen/coverage.json',coverage)
manifest=ROOT/'tools/recipe-gen/generated-files.json'
if manifest.exists():
    for rel in json.loads(manifest.read_text()):
        if rel not in files and rel not in HANDED_OFF:
            (ROOT/rel).unlink(missing_ok=True)
for rel,text in files.items():
    dest=ROOT/rel
    dest.parent.mkdir(parents=True,exist_ok=True)
    dest.write_text(text)
manifest.write_text(json.dumps(sorted(files),indent=2)+'\n')
print(json.dumps({k:v for k,v in coverage.items() if k not in ['recipes','without_vanilla_recipe']}))

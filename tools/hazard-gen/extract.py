#!/usr/bin/env python3
"""Extract catalog inventory hazard DATA from NTM constants and registrations.

Only numeric arithmetic is evaluated. Unknown dynamic/NBT hazards are recorded
as gaps; this tool does not modify the Rust radiation rules.
"""
import ast
import json
import re
import sys
from collections import defaultdict
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
SRC=ROOT.parent/'ntm-src'
sys.path.insert(0,str(ROOT/'tools/content-gen'))
from bulk_recipes import Resolver,calls,clean,split,Unsupported
rows=json.loads((ROOT/'content/ids.json').read_text())['entries']
r=Resolver(SRC,rows)
java=SRC/'src/main/java/com/hbm'
registry=clean((java/'hazard/HazardRegistry.java').read_text())
od=clean((java/'inventory/OreDictManager.java').read_text())
constants={}
def number(expr):
    expr=re.sub(r'\bHazardRegistry\.', '',expr)
    expr=re.sub(r'(\d)[Ff]\b',r'\1',expr).replace('_','') if not re.search(r'\b\w+_\w+\b',expr) else re.sub(r'(\d)[Ff]\b',r'\1',expr)
    def evaluate(n):
        if isinstance(n,ast.Constant) and isinstance(n.value,(int,float)):return n.value
        if isinstance(n,ast.Name) and n.id in constants:return constants[n.id]
        if isinstance(n,ast.BinOp) and isinstance(n.op,(ast.Add,ast.Sub,ast.Mult,ast.Div)):
            left,right=evaluate(n.left),evaluate(n.right)
            if isinstance(n.op,ast.Add):return left+right
            if isinstance(n.op,ast.Sub):return left-right
            if isinstance(n.op,ast.Mult):return left*right
            return left/right
        if isinstance(n,ast.UnaryOp) and isinstance(n.op,ast.USub):return -evaluate(n.operand)
        raise ValueError(expr)
    return float(evaluate(ast.parse(expr,mode='eval').body))
for match in re.finditer(r'public static final float (\w+)\s*=\s*([^;]+);',registry):
    try:constants[match[1]]=number(match[2])
    except (ValueError,SyntaxError):pass
rates=defaultdict(dict);sources=defaultdict(list);gaps=[];fuel_targets={}
def put(item,kind,value,source):
    # Hazard source mutexes prevent adding equivalent ore-dictionary registrations.
    # Use the maximum equivalent entry. Explicit direct entries are applied later.
    rates[item][kind]=max(rates[item].get(kind,0),value)
    if source not in sources[item]:sources[item].append(source)
weights={'nugget':'nugget','ingot':'ingot','dustSmall':'powder_tiny','dust':'powder','gem':'gem','crystal':'gem','plate':'plate','plateCast':'plateCast','billet':'billet','block':'block','ore':'ore','oreNether':'ore'}
# Exact registrations in each DictFrame chain, with its own hazard values.
for match in re.finditer(r'^\s*(\w+)\s*((?:\.[^;]*)?);',od,re.M):
    stmt=match[0];frame=match[1]
    hazards={}
    for _,args,method in calls(stmt,r'\.(?:rad|hot|blinding|asbestos|hydro)'):
        try:hazards[{'rad':'radiation','hot':'hot','blinding':'blinding','asbestos':'asbestos','hydro':'hydroactive'}[method[1:]]]=number(args)
        except (ValueError,SyntaxError):pass
    if not hazards:continue
    source='src/main/java/com/hbm/inventory/OreDictManager.java:'+str(od[:match.start()].count('\n')+1)
    for _,args,method in calls(stmt,r'\.\w+'):
        shape=method[1:]
        if shape not in weights:continue
        for expression in split(args):
            try:item,_=r.item(expression,unqualified=True)
            except Unsupported:continue
            for kind,value in hazards.items():put(item,kind,value*constants[weights[shape]],source)
    # Material forms only receive hazards where that shape's ore key is registered
    # by the original frame (hazNugget/hazIngot or a corresponding source item).
    prefix={'nugget':'nugget','ingot':'ingot','wireFine':'wireFine','wireDense':'wireDense','dust':'dust','plate':'plate','plateTriple':'plateCast','block':'block'}
    for key in r.frames.get(frame,[]):
        for form,weight in prefix.items():
            if weight not in weights and weight not in constants:continue
            if not re.search(r'\.'+re.escape(weight)+r'\(',stmt) and not re.search(r'\.haz'+weight.title()+r'\(',stmt):continue
            mult=constants.get(weights.get(weight,weight),1)
            for item in r.ore.get(form+key,[]):
                for kind,value in hazards.items():put(item,kind,value*mult,source)
# Literal HazardSystem.register(... makeData()/addEntry(...)) registrations.
isotope_vars={row['source_variable'] for row in rows if row['source_variable'].startswith(('pellet_rtg','nuclear_waste_short','nuclear_waste_long'))}
def isotope_targets(expression):
    # HazardSystem's Item overload applies to every metadata; ItemStack is
    # exact. Keep this extension scoped to the imported isotope families.
    if expression in isotope_vars:return [row['id'] for row in r.byvar[expression]]
    watz=re.fullmatch(r'DictFrame\.fromOne\(ModItems\.watz_pellet,\s*EnumWatzType\.(\w+)\)',expression)
    if watz:return ['ntm:watz_pellet__'+watz[1].lower()]
    stack=re.match(r'new ItemStack\((\w+),',expression)
    if stack and stack[1] in isotope_vars:
        qualified=expression.replace('new ItemStack('+stack[1]+',','new ItemStack(ModItems.'+stack[1]+',',1)
        return [r.item(qualified)[0]]
    return [r.item(expression,unqualified=True)[0]]

for pos,args,_ in calls(registry,r'HazardSystem\.register'):
    parts=split(args)
    if len(parts)<2:continue
    try:targets=isotope_targets(parts[0])
    except Unsupported:continue
    source='src/main/java/com/hbm/hazard/HazardRegistry.java:'+str(registry[:pos].count('\n')+1)
    for _,args,_ in calls(parts[1],r'(?:makeData|\.addEntry)'):
        parts2=split(args)
        if len(parts2)<2:continue
        hazard=parts2[0].strip().lower()
        try:value=number(parts2[1])
        except (ValueError,SyntaxError):
            gaps.extend({'id':item,'source':source,'expression':args} for item in targets);continue
        for item in targets:put(item,{'hydroactive':'hydroactive'}.get(hazard,hazard),value,source)
# The RTG helper registers fresh radiation, optional HOT and BLINDING, and
# a per-stack depletion modifier. Export the base, never silently zero it.
for pos,args,_ in calls(registry,r'\bregisterRTGPellet'):
    parts=split(args)
    if len(parts)<3 or parts[0] not in isotope_vars:continue
    item=r.item(parts[0],unqualified=True)[0]
    source='src/main/java/com/hbm/hazard/HazardRegistry.java:'+str(registry[:pos].count('\n')+1)
    put(item,'radiation',number(parts[1]),source)
    for kind,expression in zip(['hot','blinding'],parts[3:]):
        value=number(expression)
        if value>0:put(item,kind,value,source)
    gaps.append({'id':item,'source':source,'expression':'HazardModifierRTGRadiation(target='+parts[2]+'): fresh base exported; lifetime-dependent interpolation requires native item data.'})
# The metadata overload supplies its base at argument 2. Restrict to
# ZIRNOX: export both endpoints for the native lifetime modifier.
for pos,args,_ in calls(registry,r'\bregisterOtherFuel'):
    parts=split(args)
    if len(parts)!=5 or parts[0]!='rod_zirnox':continue
    try:
        meta=r.metadata(parts[1],parts[0]);item=r.row(parts[0],meta,'item')
        base=number(parts[2]);target=number(parts[3])
    except (Unsupported,ValueError,SyntaxError):continue
    source='src/main/java/com/hbm/hazard/HazardRegistry.java:'+str(registry[:pos].count('\n')+1)
    put(item,'radiation',base,source)
    if parts[4]=='true':put(item,'blinding',20,source)
    fuel_targets[item]=target
# PWR rods: registerPWRFuel(enum, base) registers fresh=base, hot=10*base plus HOT 5, depleted=10*base.
for pos,args,_ in calls(registry,r'\bregisterPWRFuel'):
    parts=split(args)
    if len(parts)!=2 or not parts[0].startswith('EnumPWRFuel.'):continue
    try:base=number(parts[1])
    except (ValueError,SyntaxError):continue
    suffix=parts[0].split('.',1)[1].lower()
    source='src/main/java/com/hbm/hazard/HazardRegistry.java:'+str(registry[:pos].count('\n')+1)
    for form,factor in (('pwr_fuel',1),('pwr_fuel_hot',10),('pwr_fuel_depleted',10)):
        item='ntm:'+form+'__'+suffix
        if item not in {row['id'] for row in rows}:continue
        put(item,'radiation',base*factor,source)
        if form=='pwr_fuel_hot':put(item,'hot',5,source)
# RBMK rods and pellets: fresh radiation is exported; the depletion
# interpolation (HazardModifierRBMKRadiation) and rod heat (RBMKHot) are NBT.
for pos,args,method in calls(registry,r'\bregisterRBMK(?:Rod|Pellet)?\b'):
    parts=split(args)
    if len(parts)<3:continue
    try:
        item,_=r.item(parts[0],unqualified=True)
        base=number(parts[1]);dep=number(parts[2])
        rest=parts[3:]
        if method=='registerRBMK':hot=rest[0]=='true';blinding=number(rest[2]);digamma=number(rest[3])
        elif method=='registerRBMKPellet':blinding=number(rest[1]) if len(rest)>1 else 0;digamma=number(rest[2]) if len(rest)>2 else 0;hot=False
        else:hot=True;blinding=number(rest[0]) if rest and rest[0] not in ('true','false') else 0;digamma=0
    except (Unsupported,ValueError,SyntaxError,IndexError):continue
    source='src/main/java/com/hbm/hazard/HazardRegistry.java:'+str(registry[:pos].count('\n')+1)
    put(item,'radiation',base,source)
    if blinding>0:put(item,'blinding',blinding,source)
    if digamma>0:put(item,'digamma',digamma,source)
    gaps.append({'id':item,'source':source,'expression':'HazardModifierRBMKRadiation(target='+str(dep)+'): fresh base exported; depletion interpolation requires native item data.'
                 +(' HazardModifierRBMKHot (HOT from rod core heat) is also NBT-driven.' if hot else '')})
# Other fuel helper's first rate is the fresh item's radiation; depletion is NBT.
for pos,args,method in calls(registry,r'\b(?:registerOtherFuel|registerOtherWaste|registerRadSourceWaste)'):
    parts=split(args)
    if len(parts)<2:continue
    try:
        item,_=r.item(parts[0],unqualified=True)
        value=number(parts[1])
    except (Unsupported,ValueError,SyntaxError):continue
    source='src/main/java/com/hbm/hazard/HazardRegistry.java:'+str(registry[:pos].count('\n')+1)
    if method=='registerOtherWaste':value*=0.075
    put(item,'radiation',value,source)
entries=[]
for row in rows:
    data=rates[row['id']];rad=round(data.get('radiation',0),6)
    other={k:v for k,v in data.items() if k!='radiation'}
    entries.append({'id':row['id'],'radiation_rad_s':rad,'radiation_millirad_s':round(rad*1000),**({'fuel_target_rad_s':fuel_targets[row['id']]} if row['id'] in fuel_targets else {}),'other_hazards':other,'sources':sources[row['id']], 'note':'Inventory item; placed-block radiation and dynamic NBT/modifiers are separate.' if rad else 'No resolved RADIATION registration for this catalog identity; not a claim of physical safety.', 'resolution':'resolved source registration' if data else 'no matched source registration'})
output={'schema':1,'source':'HBM NTM 1.7.10 local ntm-src','units':'RAD/s per item (NTM gameplay units)','rules':{'stack':'sum(count * radiation_rad_s)','other_hazards_implemented_m1':False,'zero':'No resolved source registration; dynamic/NBT and missing source registrations are gaps.'},'entries':entries,'extraction_gaps':gaps}
(ROOT/'crates/ntm-core/data/hazards.json').write_text(json.dumps(output,indent=2)+'\n')
buckets=defaultdict(list)
for entry in entries:
    if entry['radiation_millirad_s']>0:buckets[entry['radiation_millirad_s']].append(entry['id'])
owned=[];index=[]
for rate,items in sorted(buckets.items()):
    tag=f'ntm:rad/rate_{rate}';rel=f'data/ntm/tags/item/rad/rate_{rate}.json';path=ROOT/'content'/rel;path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps({'replace':False,'values':sorted(items)},indent=2)+'\n');owned.append(rel)
    index.append({'tag':tag,'radiation_millirad_s':rate,'item_count':len(items)})
bits=[]
for bit in range(31):
    items=sorted(e['id'] for e in entries if e['radiation_millirad_s'] & (1<<bit))
    if not items:continue
    tag=f'ntm:rad/bit_{bit}';rel=f'data/ntm/tags/item/rad/bit_{bit}.json';path=ROOT/'content'/rel;path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps({'replace':False,'values':items},indent=2)+'\n');owned.append(rel)
    bits.append({'tag':tag,'bit':bit,'item_count':len(items)})
manifest=ROOT/'content/.hazard-gen-files.json'
if manifest.exists():
    for rel in json.loads(manifest.read_text()):
        if rel not in owned:(ROOT/'content'/rel).unlink(missing_ok=True)
manifest.write_text(json.dumps(owned,indent=2)+'\n')
(ROOT/'crates/ntm-core/data/radiation_buckets.json').write_text(json.dumps({'schema':1,'grouping':'exact radiation_millirad_s, no rate approximation','buckets':index,'bits':bits},indent=2)+'\n')
print(f'{len(index)} exact radiation buckets')
print(f'{len(entries)} ids, {sum(e["radiation_millirad_s"]>0 for e in entries)} radioactive; {len(gaps)} unresolved explicit expressions')

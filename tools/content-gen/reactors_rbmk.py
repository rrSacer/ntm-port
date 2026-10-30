"""RBMK fuel-rod, enum and dial snapshot; no runtime. Columns/console stay native work."""
import re
from bulk_recipes import clean, split
from bedrock_processing_common import BASE, write
from rbmk_catalog import item_variables

TINT=re.compile(r'int (tint\w+) = (0x[0-9A-Fa-f]+);')
CALL=re.compile(r'\.(set\w+)\(((?:[^()]|\([^()]*\))*)\)')


def num(text):
    return float(re.sub(r'[dDfF]$','',text.strip().replace('_','')))


def rod_table(source,rod_vars,pellet_vars):
    text=clean((source/(BASE+'items/ModItems.java')).read_text())
    tints={m[1]:int(m[2],16) for m in TINT.finditer(text)}
    rods=[]
    for var in rod_vars:
        m=re.search(rf'^\s*{var}\s*=\s*\(ItemRBMKRod\)\s*new ItemRBMKRod\(([^)]*)\)(.*?)\.setTextureName',text,re.M|re.S)
        if not m:raise ValueError('RBMK rod chain drift: '+var)
        row={'variable':var,'pellet':m[1] if m[1] in pellet_vars else None,'full_name':None if m[1] in pellet_vars else m[1].strip('"')}
        for name,args in CALL.findall(m[2]):
            parts=split(args)
            if name=='setUnlocalizedName':continue
            if name=='setYield':row['yield']=num(parts[0])
            elif name=='setStats':row['func_end']=num(parts[0]);row['self_rate']=num(parts[1]) if len(parts)>1 else 0.0
            elif name=='setFunction':row['function']=parts[0].split('.')[-1]
            elif name=='setDepletionFunction':row['depletion_function']=parts[0].split('.')[-1]
            elif name=='setHeat':row['heat_per_flux']=num(parts[0])
            elif name=='setMeltingPoint':row['melting_point']=num(parts[0])
            elif name=='setDiffusion':row['diffusion']=num(parts[0])
            elif name=='setXenon':row['xenon_generation']=num(parts[0]);row['xenon_burn_divisor']=num(parts[1])
            elif name=='setHeatCoeff':row['heat_coefficient']={'start':num(parts[0]),'length':num(parts[1])}
            elif name=='setNeutronTypes':row['neutron_types']={'absorbs':parts[0].split('.')[-1],'releases':parts[1].split('.')[-1]}
            elif name=='setTint':row['tint']=tints[parts[0]] if parts[0] in tints else int(parts[0],16)
            else:raise ValueError('Unmodeled RBMK rod call '+name+' on '+var)
        rods.append(row)
    return rods


def export(source,output,rows):
    known={r['id'] for r in rows}
    rod_vars,pellet_vars=item_variables(source)
    rods=rod_table(source,rod_vars,set(pellet_vars))
    for r in rods:
        r['item']='ntm:'+r['variable'].replace('rbmk_fuel_hep239','rbmk_fuel_hep')
        if r['item'] not in known:raise ValueError('Missing rod id '+r['item'])
        if r['pellet']:
            r['pellet_item']='ntm:'+r['pellet']
            if r['pellet_item'] not in known:raise ValueError('Missing pellet id '+r['pellet'])
    rod=clean((source/(BASE+'items/machine/ItemRBMKRod.java')).read_text())
    burn=re.search(r'enum EnumBurnFunc \{(.*?);',rod,re.S)[1];deplete=re.search(r'enum EnumDepleteFunc \{(.*?);',rod,re.S)[1]
    raw=(source/(BASE+'items/machine/ItemRBMKRod.java')).read_text()
    formulas={m[1]:m[2].strip() for m in re.finditer(r'^\s*(\w+)\(EnumChatFormatting[^\n]*\),?;?\s*//(.*)$',raw.split('enum EnumBurnFunc {',1)[1].split(';\n\n',1)[0],re.M)}
    dials=[]
    dial_text=clean((source/(BASE+'tileentity/machine/rbmk/RBMKDials.java')).read_text()).split('enum RBMKKeys {',1)[1].split(';',1)[0]
    for m in re.finditer(r'(\w+)\("(\w+)",\s*([^)]+)\)',dial_text):
        v=m[3].strip();dials.append({'key':m[1],'game_rule':m[2],'default':True if v=='true' else False if v=='false' else num(v) if re.fullmatch(r'[\d.]+',v) else v})
    return write(output,'reactors_rbmk',{'recipes':[],'skipped':[],
        'rods':rods,'burn_functions':[b.strip().split('(')[0] for b in burn.split('\n') if '(' in b],'burn_formulas':formulas,
        'depletion_functions':[d.strip().rstrip(',').split('//')[0].strip().rstrip(',') for d in deplete.split('\n') if d.strip()],
        'enrichment_formulas':{'LINEAR':'x','STATIC':'1','BOOSTED_SLOPE':'x+sin((x-1)^2*pi)','RAISING_SLOPE':'x+sin(x*pi)/2','GENTLE_SLOPE':'x+sin(x*pi)/3'},
        'xenon':{'generation':'flux*xGen (default 0.5)','burn':'flux^2/xBurn (default 50)'},
        'rod_nbt':{'yield':'double, remaining potential influx','xenon':'double [0,100]','core_heat':'double','hull_heat':'double','enrichment':'yield/rod.yield [0,1]'},
        'pellet_state':{'metadata':'0..9: grade=meta%5 (0 brand new..4 fully depleted), xenon poisoned when meta>=5; five pellet types have xenon disabled (0..4 only)',
                        'ported_as':'one item id per pellet type; the 10-state metadata is native item data'},
        'rod_to_pellet':'RBMKFuelCraftingHandler: a cold (core/hull heat <50) rod alone in the grid yields 8 pellets of its type; meta=4-clamp(ceil(enrichment*5-1),0,4) plus 5 when poison>=0.5; none above 0.99 enrichment. Dynamic: native logic.',
        'dials':dials,
        'runtime_gaps':['Columns, console, crane console, autoloader, panels, boiler/heater/cooler/outgasser/storage logic, flux (RBMKNeutronHandler), heat and meltdown/debris, rod disassembly (RBMKRodDisassemblyHandler), SILEX/fuel pool pellet tables and the dynamic hazard modifiers are native work.'],
        'coverage':{'rods':len(rods),'pellet_rods':sum(1 for r in rods if r['pellet']),'dials':len(dials)}})

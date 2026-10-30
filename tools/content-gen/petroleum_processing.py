#!/usr/bin/env python3
"""Oil conversion cycles, pressures, catalysts and thermal coking defaults."""
import re
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import integer
from bedrock_processing_common import Context, BASE, body, skip, write
from petroleum_recipes import fluid
from pyrooven_recipes import fuel_heat


def settings(ctx,tile):
    path=BASE+'tileentity/machine/'+tile+'.java';text=clean((ctx.source/path).read_text())
    cfg={'source':path,'sources':{}}
    def put(key,pattern,default=None):
        m=re.search(pattern,text)
        if m:
            cfg[key]=integer(m[1]);cfg['sources'][key]=path+':'+str(text[:m.start()].count('\n')+1)
        elif default is not None:cfg[key]=default
        else:raise Unsupported('tile config drift '+tile+':'+key)
    put('interval_ticks',r'getTotalWorldTime\(\) % (\d+) == 0',1)
    put('energy_per_operation_he',r'power -= ([\d_]+)',0)
    put('energy_capacity_he',r'long maxPower = ([\d_]+)',0)
    cfg['tanks']=[]
    for m in re.finditer(r'tanks\[(\d+)\] = new FluidTank\(Fluids\.\w+, ([\d_]+)\)(?:.withPressure\((\d+)\))?',text):
        cfg['tanks'].append({'slot':int(m[1]),'capacity_mb':integer(m[2]),'pressure':int(m[3] or 0),'source':path+':'+str(text[:m.start()].count('\n')+1)})
    cfg['energy_draw']='instant_on_success'
    if 'ModItems.catalytic_converter' in text:
        cfg['catalysts']=[{'item':'ntm:catalytic_converter','count':1,'consumed':False,'slot':10,
                          'source':path+':'+str(text[:text.index('ModItems.catalytic_converter')].count('\n')+1)}]
    if 'for(int i = 0; i < 2; i++)' in text:put('max_operations_per_interval',r'for\(int i = 0; i < (\d+); i\+\+\)')
    return cfg,text


def conversion(ctx,out,name,recipe_class,tile):
    path,text,start,end=body(ctx.source,recipe_class);cfg,tiletext=settings(ctx,'oil/'+tile)
    constants=dict(re.findall(r'final int (\w+)\s*=\s*(\d+)',text))
    rows=[];skipped=[]
    for index,(pos,expr,_) in enumerate(calls(text[start:end],r'\b(?:recipes|cracking)\.put')):
        origin=path+':'+str(text[:start+pos].count('\n')+1);identity=name+'.'+str(index)
        try:
            kind,expression=split(expr);row=ctx.row(identity,origin,index)
            row['inputFluids']=[fluid(ctx,'new FluidStack('+kind+',100)')]
            row['inputFluidPressures']=[2 if name=='vacuum_refinery' else 0]
            outputs=[]
            for _,a,_ in calls(expression,r'new FluidStack'):
                parts=split(a);parts[1]=constants.get(parts[1],parts[1]);f=fluid(ctx,'new FluidStack('+','.join(parts[:2])+')')
                outputs.append((f,int(parts[2]) if len(parts)>2 else 0))
            if name=='hydrotreating':
                f,p=outputs.pop(0);row['inputFluids'].append(f);row['inputFluidPressures'].append(p)
            row['outputFluids']=[f for f,p in outputs if f[0]!='none' and f[1]>0]
            if name=='cracking':
                row['inputFluids'].append(['steam',200]);row['inputFluidPressures'].append(0);row['outputFluids'].append(['spentsteam',2])
            row.update(duration=cfg['interval_ticks'],interval_ticks=cfg['interval_ticks'],power=0,
                       energy_per_operation_he=cfg['energy_per_operation_he'],energy_draw=cfg['energy_draw'])
            if cfg.get('catalysts'):row['catalysts']=cfg['catalysts']
            if 'max_operations_per_interval' in cfg:row['max_operations_per_interval']=cfg['max_operations_per_interval']
            rows.append(ctx.label(row))
        except (Unsupported,KeyError,ValueError,StopIteration) as error:skip(skipped,identity,origin,expr,error)
    return write(out,name,{'machine_config':cfg,'selection':'input tank fluid type',
        'requires_runtime_fields':['energy_per_operation_he','interval_ticks','inputFluidPressures','catalysts','max_operations_per_interval'],
        'recipes':rows,'skipped':skipped,'coverage':{'source_calls':len(rows)+len(skipped),'emitted':len(rows),'skipped':len(skipped)},
        'notes':['power=0 excludes continuous HE/t; energy_per_operation_he is charged atomically on successful cycles.',
                 'duration records the polling interval, not a newly invented continuous crafting duration.',
                 'Cracker executes up to two 100 mB operations per five-tick interval. NONE zero-volume outputs are omitted.']})


def coker(ctx,out):
    path,text,start,end=body(ctx.source,'CokerRecipes');cfg,tiletext=settings(ctx,'oil/TileEntityMachineCoker')
    cfg['energy_draw']='thermal';cfg['heat_capacity_tu']=integer(re.search(r'int maxHeat = ([\d_]+)',tiletext)[1])
    cfg['heat_required_tu']=integer(re.search(r'int processTime = ([\d_]+)',tiletext)[1])
    cfg['diffusion']=float(re.search(r'double diffusion = ([\d.]+)D',tiletext)[1])
    cfg['progress']='Once per tick: burn=floor(heat/100); progress+=burn; heat-=burn. If progress>=processTime perform at most one operation and subtract processTime; no fixed duration.'
    cfg['heat_transfer']='Pull ceil((source_heat - heat)*diffusion) from an IHeatSource below when positive, capped by maxHeat; otherwise heat=max(heat-max(floor(heat/1000),1),0)'
    for key,pattern in [('heat_required_tu','int processTime'),('heat_capacity_tu','int maxHeat'),('diffusion','double diffusion'),('progress','int burn =')]:
        cfg['sources'][key]=cfg['source']+':'+str(tiletext[:tiletext.index(pattern)].count('\n')+1)
    heat=fuel_heat(ctx.source,include_combustion=True);rows=[];skipped=[]
    auto=re.search(r'registerSFAuto\(fluid,\s*([\d_]+L),\s*(DictFrame.fromOne\([^)]+\)),\s*type\)',text)
    for index,(pos,expr,method) in enumerate(calls(text[start:end],r'\b(?:registerAuto|registerSFAuto|registerRecipe)')):
        origin=path+':'+str(text[:start+pos].count('\n')+1);name='coker.'+str(index)
        try:
            args=split(expr);var=args[0];row=ctx.row(name,origin,index)
            if method!='registerRecipe':
                tu=integer(auto[1]) if method=='registerAuto' else integer(args[1])
                item=auto[2] if method=='registerAuto' else args[2];byproduct=args[1] if method=='registerAuto' else args[3]
                amount=int(tu*1000/heat[var])
                if amount>10000:amount-=amount%1000
                elif amount>1000:amount-=amount%100
                elif amount>100:amount-=amount%10
                row['source_auto']={'heat_per_bucket':heat[var],'tu_per_output':tu,'penalty':1,'energy_basis':'max(final FT_Flammable, final FT_Combustible)'}
                row['outputFluids']=[fluid(ctx,f'new FluidStack(Fluids.{byproduct},{max(10,amount//10)})')] if byproduct!='null' else []
            else:
                amount=integer(args[1]);item=args[2]
                if args[3]!='null':row['outputFluids']=[fluid(ctx,args[3])]
            row['inputFluids']=[fluid(ctx,f'new FluidStack(Fluids.{var},{amount})')]
            if item!='null':row['outputItems']=[ctx.stack(item)]
            row.update(heat_required_tu=cfg['heat_required_tu'],power=0)
            rows.append(ctx.label(row))
        except (Unsupported,KeyError,ValueError,StopIteration) as error:skip(skipped,name,origin,expr,error)
    return write(out,'coker',{'machine_config':cfg,'selection':'input tank fluid type; speed derives from current heat',
        'requires_runtime_fields':['heat_required_tu','machine_config.progress','machine_config.heat_transfer'],
        'recipes':rows,'skipped':skipped,'coverage':{'source_calls':len(rows)+len(skipped),'emitted':len(rows),'skipped':len(skipped)}})


def export(source,out,rows):
    ctx=Context(source,rows);result={}
    for name,cls,tile in [('cracking','CrackingRecipes','TileEntityMachineCatalyticCracker'),('reforming','ReformingRecipes','TileEntityMachineCatalyticReformer'),('hydrotreating','HydrotreatingRecipes','TileEntityMachineHydrotreater'),('vacuum_refinery','VacuumRefineryRecipes','TileEntityMachineVacuumDistill')]:
        result[name]=conversion(ctx,out,name,cls,tile)
    result['coker']=coker(ctx,out)
    import boiler_recipes
    result['boiler']=boiler_recipes.export(ctx,out)
    return result

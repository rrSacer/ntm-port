#!/usr/bin/env python3
"""Literal petroleum defaults with runtime selection and overwrite provenance."""
import re
from bulk_recipes import Unsupported, calls, split
from machine_recipes import integer
from bedrock_processing_common import Context, BASE, body, config, skip, write
from pyrooven_recipes import fuel_heat


def machine_config(ctx, tile):
    return config(ctx.source,'tileentity/machine/oil/'+tile+'.java',{
        'power':r'int usageBase = ([\d_]+)', 'duration':r'int processTimeBase = ([\d_]+)',
        'energy_capacity_he':r'long maxPower = ([\d_]+)',
        'tank_capacity_mb':r'tank = new FluidTank\(Fluids.NONE, ([\d_]+)'})


def fluid(ctx, expression):
    args=split(next(calls(expression,r'new FluidStack'))[1])
    if len(args)!=2:raise Unsupported('unexpected FluidStack arguments: '+expression)
    if args[1].startswith('Fluids.'):args.reverse()
    return ctx.fluid('new FluidStack('+','.join(args)+')')


def liquefaction(ctx, output):
    path,text,start,end=body(ctx.source,'LiquefactionRecipes')
    cfg=machine_config(ctx,'TileEntityMachineLiquefactor')
    recipes=[];skipped=[]
    # Explicit legacy identities; never interpret modern melon as melon_slice
    # or a snow layer as the 1.7.10 Blocks.snow full block.
    legacy={'Items.melon':'melon_slice','Blocks.snow':'snow_block',
            'Blocks.double_plant, 1, 0':'sunflower','Blocks.tallgrass, 1, 1':'short_grass',
            'Blocks.tallgrass, 1, 2':'fern'}
    for index,(pos,expression,_) in enumerate(calls(text[start:end],r'recipes\.put')):
        pos+=start;origin=path+':'+str(text[:pos].count('\n')+1);name='liquefaction.'+str(index)
        try:
            args=split(expression);expr=args[0]
            row=ctx.row(name,origin,index)
            if expr.startswith('new ComparableStack'):
                contents=next(calls(expr,r'new ComparableStack'))[1].strip()
                if contents=='Items.fish, 1, wildcard':
                    ids=['minecraft:'+x for x in ['cod','salmon','tropical_fish','pufferfish']]
                    row['match_kind']='wildcard'
                elif contents in legacy:
                    ids=[ctx.resolver.vanilla_item('Items.'+legacy[contents])];row['match_kind']='exact'
                else:
                    ids=ctx.ingredient(expr)['ids'];row['match_kind']='exact'
            else:
                _,ids=ctx.resolver.input('new OreDictStack('+expr+')')
                row['match_kind']='ore';row['source_ore_expression']=expr
            row['inputItems']=[{'ids':ids,'count':1}]
            row['outputFluids']=[fluid(ctx,args[1])]
            row.update(duration=cfg['duration'],power=cfg['power'])
            recipes.append(ctx.label(row))
        except (Unsupported,KeyError,ValueError,StopIteration) as error:
            skip(skipped,name,origin,expression,error);skipped[-1]['source_index']=index
    fallback=text.index('if(stack.getItem() instanceof ItemFood)')
    return write(output,'liquefaction',{'machine_config':cfg,
        'selection':'input exact identity, then item wildcard, then OreDict keys in item registration order, finally ItemFood fallback',
        'selection_order':['exact','wildcard','ore','food_fallback'],
        'recipes':recipes,'skipped':skipped,
        'runtime_gaps':[{'source':path+':'+str(text[:fallback].count('\n')+1),
            'reason':'Dynamic ItemFood fallback requires original item nutrition, which is not available as a complete 1.7.10 registry snapshot; do not substitute modern food values',
            'output_fluid':'salient','amount_formula':'int(food_value * saturation_modifier * 20)'}],
        'coverage':{'source_calls':len(recipes)+len(skipped),'emitted':len(recipes),'skipped':len(skipped),'runtime_gaps':1},
        'notes':['All input stack sizes become one in the source handler.',
                 'Four legacy raw fish variants are one wildcard ingredient; tallgrass, double_plant, snow and melon keep their 1.7.10 identities.',
                 'Array order follows source declarations, not match precedence; selection_order and match_kind are authoritative.']})


def solidification(ctx, output):
    path,text,start,end=body(ctx.source,'SolidificationRecipes')
    cfg=machine_config(ctx,'TileEntityMachineSolidifier')
    constants=dict(re.findall(r'final int (SF_\w+)\s*=\s*(\d+)',text))
    default=re.search(r'registerSFAuto\(fluid,\s*([\d_]+L),\s*ModItems\.(\w+)\)',text)
    penalty=float(re.search(r'double penalty = ([0-9.]+)D',text)[1])
    heat=fuel_heat(ctx.source);active={};skipped=[];superseded=[];total=0
    for index,(pos,expression,method) in enumerate(calls(text[start:end],r'\b(?:registerRecipe|registerSFAuto)')):
        pos+=start;total+=1;origin=path+':'+str(text[:pos].count('\n')+1);name='solidification.'+str(index)
        try:
            args=split(expression);var=args[0].removeprefix('Fluids.')
            row=ctx.row(name,origin,index)
            if method=='registerSFAuto':
                tu=integer(args[1]) if len(args)==3 else integer(default[1])
                item=args[2] if len(args)==3 else 'ModItems.'+default[2]
                amount=int(tu*1000*penalty/heat[var])
                if amount>10000:amount-=amount%1000
                elif amount>1000:amount-=amount%100
                elif amount>100:amount-=amount%10
                amount=max(amount,1)
                row['solid_fuel_auto']={'heat_per_bucket':heat[var],'tu_per_fuel':tu,'penalty':penalty}
            else:
                amount=integer(constants.get(args[1],args[1]));item=args[2]
            row['inputFluids']=[ctx.fluid(f'new FluidStack(Fluids.{var},{amount})')]
            row['outputItems']=[ctx.stack(item)]
            row.update(duration=cfg['duration'],power=cfg['power'])
            ctx.label(row)
            key=row['inputFluids'][0][0]
            if key in active:
                previous=active.pop(key)
                superseded.append({'recipe':previous,'replaced_by':row['name'],
                                   'reason':'Source HashMap.put overwrites the earlier recipe for this fluid'})
            active[key]=row
        except (Unsupported,KeyError,ValueError,StopIteration) as error:
            skip(skipped,name,origin,expression,error);skipped[-1]['source_index']=index
    return write(output,'solidification',{'machine_config':cfg,'selection':'input tank fluid type; last source registration for a fluid wins',
        'recipes':list(active.values()),'skipped':skipped,'superseded':superseded,
        'coverage':{'source_calls':total,'emitted':len(active),'skipped':len(skipped),'superseded':len(superseded)},
        'notes':['The last BALEFIRE registerSFAuto supersedes the earlier 250 mB recipe.',
                 'Final source fuel heat and the solidifier penalty 1.25 are used; this is not the pyrooven bonus 0.5.',
                 'All values are base configuration without upgrades; omissions are catalog gaps, never invented substitutes.']})


def export(source, output, rows):
    ctx=Context(source,rows)
    return {'liquefaction':liquefaction(ctx,output),'solidification':solidification(ctx,output)}


if __name__=='__main__':
    import json
    from pathlib import Path
    root=Path(__file__).resolve().parents[2]
    export(root.parent/'ntm-src',root/'content',json.loads((root/'content/ids.json').read_text())['entries'])

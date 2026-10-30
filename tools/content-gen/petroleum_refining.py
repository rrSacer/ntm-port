#!/usr/bin/env python3
"""Literal refinery fractions and all output-selected mixer alternatives."""
import re
from bulk_recipes import Unsupported, calls, split
from machine_recipes import integer
from bedrock_processing_common import Context, BASE, body, config, skip, write
from petroleum_recipes import fluid


def tile(ctx,relative,specs):
    return config(ctx.source,'tileentity/machine/'+relative+'.java',specs)


def refinery(ctx,out):
    path,text,start,end=body(ctx.source,'RefineryRecipes')
    constants=dict(re.findall(r'final int (\w+)\s*=\s*(\d+)',text))
    cfg=tile(ctx,'oil/TileEntityMachineRefinery',{
        'power':r'this.power -= (\d+);','energy_capacity_he':r'long maxPower = (\d+)',
        'input_mb':r'tanks\[0\].getFill\(\) < (\d+)',
        'solid_every':r'int maxSulfur = (\d+)',
        'input_capacity_mb':r'tanks\[0\] = new FluidTank\(Fluids.HOTOIL, ([\d_]+)',
        'output_capacity_mb':r'tanks\[1\] = new FluidTank\(Fluids.HEAVYOIL, ([\d_]+)'})
    cfg['processing']='refine() is called once per server update; one successful cycle per tick'
    cfg['counter']='One sulfur counter per machine, saved in NBT and preserved on recipe/fluid changes; sulfur increments every successful cycle; at maxSulfur subtract maxSulfur and try to insert the solid; full/incompatible output loses that solid'
    recipes=[];skipped=[]
    for index,(pos,expr,_) in enumerate(calls(text[start:end],r'recipes\.put')):
        origin=path+':'+str(text[:start+pos].count('\n')+1);name='refinery.'+str(index)
        try:
            kind,expression=split(expr)
            args=split(next(calls(expression,r'new RefineryRecipe'))[1])
            row=ctx.row(name,origin,index)
            row['inputFluids']=[fluid(ctx,'new FluidStack('+kind+','+str(cfg['input_mb'])+')')]
            row['outputFluids']=[fluid(ctx,re.sub(r'\b('+ '|'.join(constants)+r')\b',lambda m:constants[m[0]],a)) for a in args[:4]]
            row['periodicOutputItems']=[{'item':ctx.stack(args[4]),'every':cfg['solid_every'],'counter':'sulfur','source':cfg['sources']['solid_every']}]
            row.update(duration=1,power=cfg['power'])
            recipes.append(ctx.label(row))
        except (Unsupported,KeyError,ValueError,StopIteration) as error:skip(skipped,name,origin,expr,error)
    return write(out,'refinery',{'machine_config':cfg,'selection':'input tank fluid type',
        'recipes':recipes,'skipped':skipped,'coverage':{'source_calls':len(recipes)+len(skipped),'emitted':len(recipes),'skipped':len(skipped)},
        'notes':['Four fluid amounts are per 100 mB cycle, not per bucket.',
                 'periodicOutputItems is an exact shared counter, not an output probability; generic outputItems remains empty.']})


def fraction(ctx,out):
    path,text,start,end=body(ctx.source,'FractionRecipes')
    cfg=tile(ctx,'oil/TileEntityMachineFractionTower',{
        'duration':r'getTotalWorldTime\(\) % (\d+) == 0\) fractionate',
        'input_mb':r'tanks\[0\].getFill\(\) >= (\d+)',
        'tank_capacity_mb':r'tanks\[0\] = new FluidTank\(Fluids.HEAVYOIL, (\d+)'})
    cfg['energy']='No power field, energy API or power consumption in source tile'
    recipes=[];skipped=[]
    for index,(pos,expr,_) in enumerate(calls(text[start:end],r'fractions\.put')):
        origin=path+':'+str(text[:start+pos].count('\n')+1);name='fraction.'+str(index)
        try:
            kind,expression=split(expr);row=ctx.row(name,origin,index)
            row['inputFluids']=[fluid(ctx,'new FluidStack('+kind+','+str(cfg['input_mb'])+')')]
            row['outputFluids']=[fluid(ctx,'new FluidStack('+a+')') for _,a,_ in calls(expression,r'new FluidStack')]
            row.update(duration=cfg['duration'],power=0);recipes.append(ctx.label(row))
        except (Unsupported,KeyError,ValueError,StopIteration) as error:skip(skipped,name,origin,expr,error)
    return write(out,'fraction',{'machine_config':cfg,'selection':'input tank fluid type; server ticks divisible by ten',
        'recipes':recipes,'skipped':skipped,'coverage':{'source_calls':len(recipes)+len(skipped),'emitted':len(recipes),'skipped':len(skipped)}})


def mixer(ctx,out):
    path,text,start,end=body(ctx.source,'MixerRecipes')
    cfg=tile(ctx,'TileEntityMachineMixer',{'power':r'this.consumption = (\d+);','energy_capacity_he':r'long maxPower = ([\d_]+)',
        'input_capacity_mb':r'tanks\[0\] = new FluidTank\(Fluids.NONE, ([\d_]+)',
        'output_capacity_mb':r'tanks\[2\] = new FluidTank\(Fluids.NONE, ([\d_]+)'})
    recipes=[];skipped=[];registrations=0;expanded=0
    for index,(pos,expr,_) in enumerate(calls(text[start:end],r'\bregister')):
        registrations+=1;args=split(expr)
        origin=path+':'+str(text[:start+pos].count('\n')+1)
        for alternative,expression in enumerate(args[1:]):
            expanded+=1;name=f'mixer.{index}.{alternative}'
            try:
                amount,duration=split(next(calls(expression,r'new MixerRecipe'))[1])
                row=ctx.row(name,origin,index);row['alternative_index']=alternative
                row['outputFluids']=[fluid(ctx,'new FluidStack('+args[0]+','+amount+')')]
                row.update(duration=integer(duration),power=cfg['power'])
                for _,a,method in calls(expression,r'\.set(?:Stack1|Stack2|Solid)'):
                    if method=='.setSolid':
                        if a=='new ComparableStack(Items.fish, 1, OreDictionary.WILDCARD_VALUE)':
                            ingredient={'ids':['minecraft:'+x for x in ['cod','salmon','tropical_fish','pufferfish']],'count':1}
                        elif a=='new ComparableStack(Blocks.double_plant, 1, 0)':ingredient={'ids':['minecraft:sunflower'],'count':1}
                        else:ingredient=ctx.ingredient(a)
                        row['inputItems'].append(ingredient)
                    else:row['inputFluids'].append(fluid(ctx,a))
                recipes.append(ctx.label(row))
            except (Unsupported,KeyError,ValueError,StopIteration) as error:
                skip(skipped,name,origin,expression,error);skipped[-1].update(source_index=index,alternative_index=alternative)
    return write(out,'mixer',{'machine_config':cfg,'selection':'player selects output tank fluid and recipe index modulo number of alternatives; input tanks remain ordered',
        'recipes':recipes,'skipped':skipped,'coverage':{'source_calls':registrations,'expanded':expanded,'emitted':len(recipes),'skipped':len(skipped)}})


def export(source,out,rows):
    ctx=Context(source,rows)
    return {name:fn(ctx,out) for name,fn in [('refinery',refinery),('fraction',fraction),('mixer',mixer)]}

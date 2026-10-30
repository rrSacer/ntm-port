#!/usr/bin/env python3
"""FT_Heatable steps and the exact first-step boiler eligibility rule."""
import re
from bulk_recipes import Unsupported, calls, clean, split
from machine_recipes import integer
from bedrock_processing_common import BASE, write


def export(ctx,out):
    path=BASE+'inventory/fluid/Fluids.java';text=clean((ctx.source/path).read_text())
    constants={m[1]:float(m[2]) for m in re.finditer(r'double (\w+) = ([\d.]+)D;',text)}
    steps=[];recipes=[];skipped=[];declarations=0
    for match in re.finditer(r'\b(\w+)\.addTraits\(new FT_Heatable\(\)(.*?);',text,re.S):
        declarations+=1;efficiencies={}
        for _,expr,_ in calls(match[2],r'\.setEff'):
            heating,amount=split(expr);efficiencies[heating.split('.')[-1]]=constants[amount] if amount in constants else float(amount.removesuffix('D'))
        for index,(pos,expr,_) in enumerate(calls(match[2],r'\.addStep')):
            origin=path+':'+str(text[:match.start(2)+pos].count('\n')+1)
            try:
                heat,req,output,prod=split(expr);var=match[1]
                item={'source':origin,'input':ctx.fluid(f'new FluidStack(Fluids.{var},{req})'),
                      'output':ctx.fluid(f'new FluidStack(Fluids.{output},{prod})'),
                      'step_index':index,'heat_tu':integer(heat),'efficiencies':efficiencies,
                      'active_boiler':index==0 and efficiencies.get('BOILER',0)>0}
                steps.append(item)
                if item['active_boiler']:
                    row=ctx.row('boiler.'+var.lower(),origin,len(recipes))
                    row['inputFluids']=[item['input']];row['outputFluids']=[item['output']]
                    row['heat_required_tu']=int(max(item['heat_tu']/efficiencies['BOILER'],1));row['power']=0
                    recipes.append(ctx.label(row))
            except (Unsupported,KeyError,ValueError) as error:
                skipped.append({'source':origin,'expression':expr,'reason':str(error)})
    machines={}
    for name,tile in [('machine_boiler','TileEntityHeatBoiler'),('machine_industrial_boiler','TileEntityHeatBoilerIndustrial')]:
        tilepath=BASE+'tileentity/machine/'+tile+'.java';tiletext=clean((ctx.source/tilepath).read_text())
        cfg={'source':tilepath,'sources':{}}
        for key,pattern,parser in [('heat_capacity_tu',r'int maxHeat = ([\d_]+)',integer),('diffusion',r'double diffusion = ([\d.]+)D',float),('input_capacity_mb',r'tanks\[0\] = new FluidTank\(Fluids.WATER, ([\d_]+)',integer)]:
            m=re.search(pattern,tiletext)
            if m is None:raise Unsupported('boiler config drift '+key)
            cfg[key]=parser(m[1]);cfg['sources'][key]=tilepath+':'+str(tiletext[:m.start()].count('\n')+1)
        cfg['output_capacity']='input_capacity_mb * step.amountProduced / step.amountReq'
        cfg['operations_per_tick']='min(inputFill/amountReq, freeOutput/amountProduced, heat/floor(max(heatReq/BOILER_efficiency,1))), integer divisions'
        cfg['heat_transfer']='ceil((source_heat - heat)*diffusion), limited to remaining heat capacity, from IHeatSource below; if not heated cool by max(floor(heat/1000),1)'
        cfg['environmental_heat']='skyLight>7 and TomSaveData.fire>1e-5: heat+=(maxHeat-heat)*0.000005, integer assignment'
        cfg['blocked_output']='small boiler explodes if outputOps==0 and canExplode; industrial boiler does not explode in tryConvert'
        cfg['sources']['conversion']=tilepath+':'+str(tiletext[:tiletext.index('protected void tryConvert()')].count('\n')+1)
        cfg['power']=0;machines['ntm:'+name]=cfg
    return write(out,'boiler',{'machine_config':machines,'selection':'first FT_Heatable step only, with BOILER efficiency >0; convert as many operations as heat and tank capacity permit each tick',
        'requires_runtime_fields':['heat_required_tu','machine_config.operations_per_tick','machine_config.heat_transfer'],
        'heating_steps':steps,'recipes':recipes,'skipped':skipped,
        'coverage':{'heatable_declarations':declarations,'heating_steps':len(steps),'active_boiler_recipes':len(recipes),'skipped':len(skipped)},
        'notes':['Inactive steps are retained for provenance; they must not become selectable boiler recipes.',
                 'No fixed duration or electrical energy conversion is invented; TU and HE remain separate.']})

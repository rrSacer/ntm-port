#!/usr/bin/env python3
"""Source geothermal heat supply and gas-flare traits, without invented cycles."""
import json
import re
from bedrock_processing_common import BASE,Context,write
from bulk_recipes import clean,Unsupported
from pyrooven_recipes import fuel_heat


def location(path,text,needle):
    index=text.index(needle)
    return path+':'+str(text[:index].count('\n')+1)


def hephaestus(ctx,out,steps):
    path=BASE+'tileentity/machine/TileEntityMachineHephaestus.java'
    text=clean((ctx.source/path).read_text());recipes=[]
    for step in steps:
        if step['step_index']!=0 or step['efficiencies'].get('HEATEXCHANGER',0)<=0:continue
        row=ctx.row('hephaestus.'+step['input'][0],step['source'],len(recipes))
        row.update(inputFluids=[step['input']],outputFluids=[step['output']],
                   heat_required_tu=step['heat_tu'],power=0)
        recipes.append(ctx.label(row))
    scan=re.search(r'int height = .*? % (\d+)\).*?int range = (\d+);',text,re.S)
    if not scan:raise Unsupported('Hephaestus scan config drift')
    cfg={'source':path,'input_capacity_mb':int(re.search(r'this.input = new FluidTank\(Fluids.OIL, ([\d_]+)',text)[1].replace('_','')),
         'output_capacity_mb':int(re.search(r'this.output = new FluidTank\(Fluids.HOTOIL, ([\d_]+)',text)[1].replace('_','')),
         'sources':{'tanks':location(path,text,'this.input = new FluidTank'),
                    'scan':location(path,text,'int height ='),
                    'conversion':location(path,text,'protected void heatFluid()'),
                    'eligibility':location(path,text,'protected void setupTanks()'),
                    'heat_sources':location(path,text,'protected int heatFromBlock('),
                    'total_heat':location(path,text,'public int getTotalHeat()')},
         'scan_layers':int(scan[1]),'scan_xz_radius':int(scan[2]),'legacy_min_y':0,
         'scan':'one horizontal plane per tick: height=worldTime%10, y=machineY-1-height; replace that plane heat sum, skip y<0',
         'heat_supply':'sum the 10 cached plane heats each tick; multiply by 3 when worldTime-fissureScanTime<20; heat is not a stored or consumed buffer',
         'operations_per_tick':'min(inputFill/amountReq, freeOutput/amountProduced, totalHeat/heatReq), integer division',
         'efficiency':'HEATEXCHANGER>0 only gates eligibility in setupTanks; conversion uses raw heatReq, without dividing by efficiency',
         'heat_sources':[{'blocks':['minecraft:lava'],'heat_per_block':5},
                         {'source_variable':'volcanic_lava_block','heat_per_block':150,'port_id':None},
                         {'source_variable':'ore_volcano','heat_per_block':300,'port_id':None,'refreshes_fissure_scan_time':True}],
         'runtime_gaps':['Negative-depth scan adaptation is not inferred: legacy y>=0 is retained in the source contract.',
                         'volcanic_lava_block and ore_volcano are absent from this catalog; their source heat contributions are documentary.']}
    return write(out,'hephaestus',{'machine_config':cfg,
        'selection':'input tank fluid; first FT_Heatable step, HEATEXCHANGER efficiency positive',
        'requires_runtime_fields':['heat_required_tu','machine_config.operations_per_tick','machine_config.heat_supply'],
        'recipes':recipes,'skipped':[],
        'coverage':{'heating_steps':len(steps),'emitted':len(recipes),'skipped':0}})


def flare(ctx,out):
    path=BASE+'tileentity/machine/oil/TileEntityMachineGasFlare.java'
    text=clean((ctx.source/path).read_text())
    fluidpath=BASE+'inventory/fluid/Fluids.java';fluidtext=clean((ctx.source/fluidpath).read_text())
    heat=fuel_heat(ctx.source);traits=[]
    for m in re.finditer(r'\b([A-Z][A-Z0-9_]*)\s*=\s*new FluidType\((.*?);',fluidtext,re.S):
        var=m[1]
        if var=='NONE':continue
        ident=ctx.fluid(f'new FluidStack(Fluids.{var},1)')[0]
        gas=bool(re.search(r'\b(?:GASEOUS|EVAP)\b',m[2]));flammable=var in heat
        traits.append({'fluid':ident,'source':fluidpath+':'+str(fluidtext[:m.start()].count('\n')+1),
            'gaseous_or_art':gas,'flammable':flammable,'heat_tu_per_bucket':heat.get(var,0),
            'burn_penalty':5 if gas else 10,'ventable':gas})
    cfg={'source':path,'sources':{'tick':location(path,text,'public void updateEntity()'),
          'tank':location(path,text,'tank = new FluidTank'),
          'vent':location(path,text,'int maxVent ='),
          'generation':location(path,text,'long powerProd =')},
        'energy_capacity_he':int(re.search(r'maxPower = ([\d_]+)',text)[1].replace('_','')),
        'tank_capacity_mb':int(re.search(r'new FluidTank\(Fluids.GAS, ([\d_]+)',text)[1].replace('_','')),'default_fluid':'gas',
        'vent_mb_per_tick':int(re.search(r'int maxVent = (\d+)',text)[1]),
        'burn_mb_per_tick':int(re.search(r'int maxBurn = (\d+)',text)[1]),'speed_multiplier':'1+SPEED upgrade level',
        'default_on':False,'default_burn':False,
        'vent_rule':'when on, tank not empty and not tilted: if burn off or fluid lacks FT_Flammable, vent only GASEOUS or GASEOUS_ART fluids; amount=min(maxVent,tankFill)',
        'burn_rule':'when on and burn enabled and FT_Flammable present: amount=min(maxBurn,tankFill), consume even when energy buffer is full',
        'energy_formula':'base=(heat_tu_per_bucket*amount_mb)//1000; base//=burn_penalty; produced=base+(base*EFFECT_level)//3; power=min(power+produced,100000)',
        'byproduct_rule':'no item/fluid recipe output; source calls fluid release traits for spill/burn, particles, sound and fire damage',
        'runtime_gaps':['Pollution, released fluid traits, tilt, entity fire/damage, sound and particles require server/client logic.']}
    return write(out,'flare',{'machine_config':cfg,'selection':'on/burn switches and fluid traits, not a consumable recipe list',
        'recipes':[],'skipped':[],'fuel_traits':traits,
        'coverage':{'fluids':len(traits),'flammable':sum(t['flammable'] for t in traits),'ventable':sum(t['ventable'] for t in traits),'skipped':0}})


def export(source,out,rows):
    ctx=Context(source,rows);boiler_path=out/'machine-recipes/boiler.json'
    if boiler_path.exists():boiler=json.loads(boiler_path.read_text())
    else:
        import boiler_recipes
        boiler=boiler_recipes.export(ctx,out)
    return {'hephaestus':hephaestus(ctx,out,boiler['heating_steps']),'flare':flare(ctx,out)}

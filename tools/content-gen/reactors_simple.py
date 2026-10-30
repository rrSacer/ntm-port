"""Fuel/heat snapshots for the current graphite pile and ZIRNOX; no runtime."""
import re
from bulk_recipes import clean, split
from bedrock_processing_common import BASE, write
from reactor_catalog import pile_fuels, zirnox_fuels


def export(source, output, rows):
    known={r['id'] for r in rows}
    zpath=BASE+'tileentity/machine/TileEntityReactorZirnox.java'
    ztext=clean((source/zpath).read_text());fuels=zirnox_fuels(source)
    outputs={}
    pattern=r'fuelMap\.put\(new ComparableStack\(ModItems\.rod_zirnox, 1, EnumZirnoxType\.(\w+)\.ordinal\(\)\), new ItemStack\(ModItems\.(\w+)(?:, 1, EnumZirnoxType\.(\w+)\.ordinal\(\))?\)\)'
    for m in re.finditer(pattern,ztext):
        outputs[m[1]]='ntm:'+m[2]+('__'+m[3].lower() if m[3] else '')
    if len(outputs)!=11:raise ValueError('ZIRNOX decay map drift')
    for f in fuels:
        f['item']='ntm:rod_zirnox__'+f['member'].lower();f['output']=outputs[f['member']]
        if f['item'] not in known or f['output'] not in known:raise ValueError('Missing fuel/output: '+str(f))
    neighbors={int(m[1]):[int(x.strip()) for x in m[2].split(',')] for m in re.finditer(r'case (\d+): return new int\[\] \{([^}]+)\}',ztext)}
    if set(neighbors)!=set(range(24)):raise ValueError('ZIRNOX adjacency drift')
    tanks=[]
    for m in re.finditer(r'(steam|carbonDioxide|water) = new FluidTank\(Fluids\.(\w+), (\d+)\)',ztext):
        tanks.append({'tank':m[1],'source_fluid':m[2],'capacity_mb':int(m[3])})
    if len(tanks)!=3:raise ValueError('ZIRNOX tanks drift')
    ppath=BASE+'items/machine/ItemPileRodMK2.java';pfuels=pile_fuels(source)
    for f in pfuels:
        f['item']='ntm:pile_rod__'+f['member'].lower()
        target=f['turns_into_metadata'];f['output']=None if target is None else 'ntm:pile_rod__'+pfuels[target]['member'].lower()
        if f['item'] not in known:raise ValueError('Missing pile fuel')
    corepath=BASE+'tileentity/machine/pile/TileEntityPileCore.java';core=clean((source/corepath).read_text())
    brickpath=BASE+'blocks/machine/pile/BlockPileBrick.java';brick=clean((source/brickpath).read_text())
    limits={k:int(re.search(r'\b'+k+r' = (\d+)',brick)[1]) for k in ['MIN_V_SIZE','MIN_H_SIZE','MAX_V_SIZE','MAX_H_SIZE']}
    return write(output,'reactors_simple',{'recipes':[],'skipped':[],
        'zirnox':{'source':zpath,'fuels':fuels,'neighbors':neighbors,'fuel_slots':24,'tanks':tanks,
            'slots':{'co2_in':24,'water_in':25,'co2_empty':26,'water_empty':27},
            'max_heat':int(re.search(r'maxHeat = (\d+)',ztext)[1]),'max_pressure':int(re.search(r'maxPressure = (\d+)',ztext)[1]),
            'tick_order':'Redstone true latches isOn=true. Load fluid containers; while on, process slots 0..23 in order. Each rod decays once per neighboring NON-breeding rod plus once for itself if not breeding. Each step adds enum heat and life; life>maxLife replaces rod and stops that slot. Earlier replacements can affect later neighbor checks.',
            'life_data':{'key':'life','type':'int','default':0,'completion_comparison':'strict greater than maxLife'},
            'pressure_formula':'2*co2Fill + int(float(heat)*float(co2Fill)/float(co2Max)); evaluated before heat removal.',
            'steam':'When 0<heat<maxHeat and water>0 and CO2>0 and steam not full: if heat>10256, cycle=int(((float(heat)-10256F)/100000F)*min(float(co2Fill)/14000F,1F)*25F*7.5F); subtract cycle water (clamp to zero), add cycle superhotsteam (clamp tank), heat -= int(float(heat)*float(pressure)/1000000F). Source does not cap cycle to water remaining or steam free space. Otherwise eligible heat drops by 10.',
            'failure':'After cooling, pressure>maxPressure OR heat>maxHeat triggers meltdown; equality does not. Destroyed reactor, debris/radiation/explosion require native logic.',
            'special_case':'While on, meteorite_sword_bred becomes meteorite_sword_irradiated; unrelated item content is not synthesized here.',
            'ports':'Four oriented ports at lateral +/-2 and heights +1/+3; water/CO2 receive, superhotsteam sends; no HE generation in this reactor.'},
        'pile':{'source':corepath,'fuel_source':ppath,'fuels':pfuels,'construction_limits':limits,
            'max_heat_c':int(re.search(r'MAX_HEAT = (\d+)',core)[1]),
            'life_data':{'key':'depletion','type':'double','default':0},
            'rod_reaction':'outFlux=source + squirt(inFlux)*reactionMultiplier when multiplier>0; squirt(x)=sqrt(x+1/((x+2)*(x+2)))-1/(x+2). Fuel with life<=0 remains unchanged. Otherwise add outgoing flux to depletion; keep with NBT if total<life, else replace with one turnsInto rod without prior NBT.',
            'channel_heat':'For each rod: outFlux=getReactivity(stack,incomingNeutrons/channel.length); channel heat += outFlux*rod.heatMult; react consumes outFlux (not incoming flux). Flux propagates through neighboring segments/control multipliers.',
            'ventilation':{'fluid':'air','tank_mb':4000,'pressure':1,'channel_capacity_mb':1000,
                'cooling':'For each fueled channel within vertical distance<=1: heat *= (1 - (air/1000)*0.05). Then consume ceil((air/1000)*5) mB from ventilation channel. Vent fills its channel from own tank.'},
            'passive_cooling':'Each fuel channel heat*=0.999, clamped to minimum20; highestHeat>800 triggers failure.',
            'structure':'Hand drill on brick creates current pile core/dummies/edges, height/width5..15; depth probes1..MAX_H_SIZE inclusive and records depth+1 (can reach16), core not on an edge. Drill channels without crossing core/edges/other channels; device and channel orientation/links persist in native data.',
            'interaction':'Loader, vent and control are three source item metas 0/1/2 but block metas 0/4/8 plus orientation. Loader/rod insertion, control extraction and ventilation have direct interactions; no fabricated crafter/GUI.',
            'legacy_excluded':['ItemPileRod','TileEntityPileBase','block_graphite_* functional legacy variants']},
        'runtime_gaps':['Native fuel state/radiation interpolation, source multiblock assembly/conversion/drop behavior, ports, interaction, thermodynamics, failure and faithful models are separate work; stateful content remains Inert.'],
        'coverage':{'pile_fuels':len(pfuels),'zirnox_fuels':len(fuels),'zirnox_conversions':len(outputs),'zirnox_adjacency_slots':len(neighbors)}})

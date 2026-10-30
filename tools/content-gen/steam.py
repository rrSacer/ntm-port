"""Steam generators and condensers: exact ratios plus source runtime rules."""
import re
from bulk_recipes import clean
from bedrock_processing_common import BASE, write
import fluid_traits


def export(source, output, rows):
    traits = fluid_traits.export(source, output)
    def tile(name):
        path = BASE + 'tileentity/machine/TileEntity' + name + '.java'
        return path, clean((source / path).read_text())
    def loc(path, text, needle): return path + ':' + str(text[:text.index(needle)].count('\n') + 1)
    def number(text, name):
        value = re.search(r'\b' + name + r' = ([\d_.]+(?:e\d+)?)[DFL]?\b', text)[1].replace('_', '')
        return float(value) if '.' in value or 'e' in value else int(value)
    configs = {}
    path, text = tile('SteamEngine')
    configs['ntm:machine_steam_engine'] = {
        'source':path, 'input_capacity_mb':number(text,'steamCap'), 'output_capacity_mb':number(text,'ldsCap'),
        'efficiency':number(text,'efficiency'), 'input_policy':'steam only, output spentsteam; types reset each server tick',
        'operations':'min(inputFill/amountReq,freeOutput/amountProduced), integer divisions',
        'energy':'Reset powerBuffer to 0 every tick; powerBuffer+=trunc(ops*trait.heatEnergy*TURBINE_efficiency*machine_efficiency)',
        'sources':{'conversion':loc(path,text,'FT_Coolable trait ='),'tick':loc(path,text,'public void updateEntity()')}}
    basepath, basetext = tile('TurbineBase')
    cycle = [['steam','spentsteam'], ['hotsteam','steam'], ['superhotsteam','hotsteam'], ['ultrahotsteam','superhotsteam']]
    for name, tile_name in [('machine_industrial_turbine','MachineIndustrialTurbine'),('machine_chungus','Chungus')]:
        path, text = tile(tile_name)
        percent = float(re.search(r'double consumptionPercent\(\) \{ return ([\d.]+)D',text)[1])
        cfg = {'source':path, 'base_source':basepath,
               'input_capacity_mb':number(text,'inputTankSize'), 'output_capacity_mb':number(text,'outputTankSize'),
               'efficiency':number(text,'efficiency'), 'consumption_fraction':percent,
               'default_fluids':['steam','spentsteam'], 'lever_cycle':cycle,
               'operations':'inputOps=trunc(min(ceil(inputFill*consumption_fraction),inputFill)/amountReq); outputOps=freeOutput/amountProduced; ops=min(inputOps,outputOps)',
               'selection':'FT_Coolable with positive TURBINE_efficiency*machine_efficiency; otherwise output tank type NONE',
               'energy':'Reset powerBuffer every tick; generation=trunc(int32(ops*trait.heatEnergy)*(TURBINE_efficiency*machine_efficiency))',
               'java_integer_semantics':'ops*trait.heatEnergy multiplies as signed int32 BEFORE the double efficiency; overflow wraps in the source. Large chungus dense-steam operations can overflow; no silent correction is implied.',
               'resizes_on_lever': 'doesResizeCompressor() { return true;' in text,
               'sources':{'conversion':loc(basepath,basetext,'FT_Coolable trait ='), 'cycle':loc(basepath,basetext,'public void onLeverPull()'),
                          'consumption_fraction':loc(path,text,'double consumptionPercent()')}}
        if cfg['resizes_on_lever']:
            cfg['lever_capacity_rule']='steam→hotsteam→superhotsteam→ultrahotsteam divides both capacities by10 at each transition; ultrahotsteam→steam multiplies both by1000'
        if name=='machine_industrial_turbine':
            cfg['flywheel_scale_he']=number(text,'FLYWHEEL_MAX_ENERGY')
            cfg['generation_destination']='Add generation to flywheel_energy, not directly to powerBuffer'
            cfg['flywheel_tick']='maxOps=ceil(inputCapacity*consumption_fraction/amountReq); maxPower=trunc(maxOps*trait.heatEnergy*TURBINE_efficiency*machine_efficiency), updated when generation succeeds. spin=flywheel_energy/FLYWHEEL_MAX_ENERGY; target=min(trunc(max(spin,0.05)*maxPower),flywheel_energy); subtract target and set powerBuffer=target. No explicit spin cap.'
            cfg['sources']['flywheel']=loc(path,text,'public void onServerTick()')
        configs['ntm:'+name]=cfg
    path, text = tile('Condenser')
    configs['ntm:machine_condenser'] = {
        'source':path, 'input_capacity_mb':number(text,'inputTankSize'), 'output_capacity_mb':number(text,'outputTankSize'),
        'input_fluid':'spentsteam', 'output_fluid':'water', 'energy_consumption_he_per_mb':0,
        'operations':'convert=min(inputFill,freeOutput); input-=convert; output+=convert each tick',
        'sources':{'conversion':loc(path,text,'int convert =')}}
    path, text = tile('CondenserPowered')
    configs['ntm:machine_condenser_powered'] = {
        'source':path, 'base_source':configs['ntm:machine_condenser']['source'],
        'input_capacity_mb':number(text,'inputTankSizeP'), 'output_capacity_mb':number(text,'outputTankSizeP'),
        'energy_capacity_he':number(text,'maxPower'), 'energy_consumption_he_per_mb':number(text,'powerConsumption'),
        'input_fluid':'spentsteam', 'output_fluid':'water',
        'operations':'convert=min(inputFill,freeOutput), not limited by affordable HE/mB. Convert the whole amount only if power>=convert*powerConsumption*0.95; then deduct convert*powerConsumption and clamp power to0.',
        'sources':{'condition':loc(path,text,'public boolean extraCondition('), 'consumption':loc(path,text,'public void postConvert(')}}
    turbine = [{'name':'turbine.'+t['fluid'], 'source':t['source'],
                'inputFluids':[[t['fluid'],t['input_mb']]], 'outputFluids':[t['output']],
                'energy_he_per_operation_at_machine_efficiency_1':t['heat_tu']*t['efficiencies']['TURBINE'],
                'trait_efficiency':t['efficiencies']['TURBINE']}
               for t in traits['coolable'] if t['efficiencies']['TURBINE']>0]
    return write(output,'steam',{'machine_config':configs,'recipes':[],'skipped':[],
        'turbine_operations':turbine, 'condensation':{'inputFluids':[['spentsteam',1]],'outputFluids':[['water',1]]},
        'fluid_traits':'../fluid_traits.json',
        'selection':'Algorithmic generators/condensers, not a generic crafter; per-machine fluid selection and rounding are authoritative.',
        'requires_runtime_fields':['machine_config','turbine_operations.energy_he_per_operation_at_machine_efficiency_1','condensation'],
        'coverage':{'machines':len(configs),'turbine_operations':len(turbine),'condensation':1,'skipped':0},
        'runtime_gaps':['Lever modes, tank resizing, flywheel inertia, ports, animation and fluid network require machine logic/visual support.',
                        'Deprecated machine_large_turbine intentionally excluded; no substitute recipe or fixed HE/t is invented.']})

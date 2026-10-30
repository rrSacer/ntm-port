"""Four algorithmic heat sources, without fictitious generic energy recipes."""
import json
import re
from bulk_recipes import clean
from bedrock_processing_common import BASE, write
from machine_recipes import integer
import fluid_traits


def export(source, output, rows):
    traits = fluid_traits.export(source, output)
    def tile(name):
        path = BASE + 'tileentity/machine/TileEntity' + name + '.java'
        text = clean((source / path).read_text())
        return path, text
    def loc(path, text, needle):
        return path + ':' + str(text[:text.index(needle)].count('\n') + 1)
    def number(text, name):
        m = re.search(r'\b' + name + r' = ([\d_.]+)[DFL]?\b', text)
        if not m: raise ValueError('Source heater constant drift: ' + name)
        return float(m[1]) if '.' in m[1] else integer(m[1])
    config = {}
    path, text = tile('HeaterOven'); firepath, firetext = tile('HeaterFirebox')
    modifiers = lambda s: re.findall(r'\.set(\w+(?:Time|Heat)Mod)\(([^)]+)\)', s)
    if modifiers(text) != modifiers(firetext):
        raise ValueError('Oven modifiers differ from reused firebox fuel snapshot')
    foundry_path = output / 'machine-recipes/foundry.json'
    if foundry_path.exists(): foundry = json.loads(foundry_path.read_text())
    else:
        import foundry_recipes
        foundry = foundry_recipes.export(source, output, rows)
    fuel_details = {r['item']: r for r in foundry['fuel_details']}
    base_heat = number(text, 'baseHeat'); time_mult = number(text, 'timeMult')
    oven_fuels = {ident: {'burn_ticks': int(row['ticks'] * time_mult),
                         'heat_tu_per_tick': int(base_heat * fuel_details[ident]['heat_modifier']),
                         'container': row['container'], 'source': fuel_details[ident]['source']}
                  for ident, row in foundry['firebox_fuels'].items()}
    config['ntm:heater_oven'] = {
        'source': path, 'base_heat_tu': base_heat, 'time_multiplier': time_mult,
        'heat_capacity_tu': number(text, 'maxHeatEnergy'), 'heat_pull_efficiency': number(text, 'heatEff'),
        'heat_pull': 'Before burning, pull max(min(heat_below, remaining_capacity),0); consume the full pulled amount below and add trunc(pulled*efficiency)',
        'burning': 'Two input slots, first valid fuel. On ignition consume one item and return its container when the stack becomes empty; credit heat on ignition. Later ticks decrement burnTime only below heat capacity; an active burn credits heat and clamps to capacity.',
        'cooling': 'When inactive: max(heat-max(heat/1000,1),0), integer division',
        'sources': {'pull': loc(path,text,'protected void tryPullHeat()'),
                    'burn': BASE+'tileentity/machine/TileEntityFireboxBase.java:73',
                    'modifiers': loc(path,text,'new ModuleBurnTime()')},
        'fuel_profile_gaps': foundry['fuel_gaps'],
        'runtime_gaps': ['Ashpit/byproducts and soot/smoke effects require their own logic; this contract does not declare them implemented.']}
    path, text = tile('HeaterOilburner')
    config['ntm:heater_oilburner'] = {
        'source':path, 'heat_capacity_tu':number(text,'maxHeatEnergy'),
        'tank_capacity_mb':integer(re.search(r'new FluidTank\(Fluids.HEATINGOIL, ([\d_]+)',text)[1]),
        'default_fluid':'heatingoil', 'default_on':False,
        'default_burn_mb_per_tick':number(text,'burnRate'), 'max_burn_mb_per_tick':number(text,'maxBurnRate'),
        'fuel_traits':'../fluid_traits.json#flammable',
        'conversion':'toBurn=min(burnRate,tankFill); consume toBurn, then heat+=floor(TU_per_bucket/1000)*toBurn. The division occurs BEFORE multiplication. Source may overshoot capacity on this tick; capacity is clamped at the next update.',
        'overdrive':'maxBurnRate=10*(level+1); heatCapacity=500000*(level+1); clamp burnRate and stored heat at start of tick',
        'cooling':'Cool by max(heat/1000,1) only when not burning and below capacity; a valid flammable branch suppresses cooling even with empty tank.',
        'sources':{'tick':loc(path,text,'public void updateEntity()'), 'conversion':loc(path,text,'int toBurn ='),
                   'cooling':loc(path,text,'if(shouldCool)')},
        'runtime_gaps':['OVERDRIVE upgrades, pollution and fluid release effects need runtime support.']}
    path, text = tile('HeaterElectric')
    config['ntm:heater_electric'] = {
        'source':path, 'default_setting':0, 'max_setting':integer(re.search(r'if\(setting > (\d+)\)',text)[1]),
        'heat_capacity_tu':None, 'heat_capacity_note':'No explicit cap in the source; Java int storage',
        'energy_capacity_he':'consumption_at_current_setting * 20',
        'consumption_he_per_tick':'floor(pow(setting,1.4)*200)', 'generation_tu_per_tick':'setting*100',
        'cooling':'First heat=trunc(heat*0.999)',
        'heat_pull':'Then consume all heat from IHeatSource below and add trunc(heat_below*0.85)',
        'activation':'If setting>0 and stored HE>=consumption, deduct HE and add generated TU; source flag isOn is derived, not an independent toggle.',
        'sources':{'tick':loc(path,text,'public void updateEntity()'), 'consumption':loc(path,text,'public long getConsumption()'),
                   'generation':loc(path,text,'public int getHeatGen()'), 'pull':loc(path,text,'protected void tryPullHeat()')}}
    path, text = tile('HeaterHeatex')
    config['ntm:heater_heatex'] = {
        'source':path, 'tank_capacity_mb':integer(re.search(r'Fluids.COOLANT_HOT, ([\d_]+)',text)[1]),
        'default_fluid':'coolant_hot', 'default_max_operations_per_conversion':number(text,'amountToCool'),
        'default_interval_ticks':number(text,'tickDelay'), 'heat_capacity_tu':None,
        'cooling':'First heat=trunc(heat*0.999)',
        'selection':'Input fluid must have FT_Coolable with positive HEATEXCHANGER efficiency; output is coolsTo. Otherwise both tank types become NONE.',
        'conversion':'interval=max(tickDelay,1); only on worldTime%interval==0. ops=min(inputFill/amountReq,freeOutput/amountProduced,amountToCool), integer divisions. Consume and produce fluid, then heat=trunc(heat+heatEnergy*ops*HEATEXCHANGER_efficiency). amountToCool caps OPERATIONS, not mB.',
        'sources':{'selection':loc(path,text,'protected void setupTanks()'), 'conversion':loc(path,text,'protected void tryConvert()'),
                   'cooling':loc(path,text,'this.heatEnergy *= 0.999;')}}
    exchange = [{'name':'heatex.'+t['fluid'], 'source':t['source'],
                 'inputFluids':[[t['fluid'],t['input_mb']]], 'outputFluids':[t['output']],
                 'heat_produced_tu_per_operation':t['heat_tu']*t['efficiencies']['HEATEXCHANGER'],
                 'efficiency':t['efficiencies']['HEATEXCHANGER']}
                for t in traits['coolable'] if t['efficiencies']['HEATEXCHANGER']>0]
    return write(output,'heaters',{'machine_config':config, 'recipes':[], 'skipped':[],
        'oven_fuels':oven_fuels, 'heat_exchange':exchange, 'fluid_traits':'../fluid_traits.json',
        'selection':'Algorithmic heat providers; construction recipes are in anvil.json, not ordinary generic processing.',
        'requires_runtime_fields':['machine_config','oven_fuels','heat_exchange.heat_produced_tu_per_operation'],
        'coverage':{'machines':len(config),'oven_fuels':len(oven_fuels),'heat_exchange':len(exchange),'skipped':0},
        'notes':['TU and HE stay distinct. No fictitious duration, electrical power cost or material outputs are attached to these heat rules.',
                 'Source defaults only; external fuel/config overrides and unverified generic wood fuels remain explicit snapshot gaps.']})

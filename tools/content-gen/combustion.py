"""Fuel-burning generators and their pistons: source constants, fuel grades, derived HE rates."""
import re
from bulk_recipes import clean
from bedrock_processing_common import BASE, write
import fluid_traits


def export(source, output, rows):
    traits = fluid_traits.export(source, output)
    grades = traits['fuel_grades']; fuels = traits['combustible']
    def tile(name):
        path = BASE + 'tileentity/machine/TileEntity' + name + '.java'
        return path, clean((source / path).read_text())
    def loc(path, text, needle): return path + ':' + str(text[:text.index(needle)].count('\n') + 1)
    def number(text, name):
        m = re.search(r'\b' + name + r' = ([\d_.]+)[DFL]?\b', text)
        if not m: raise ValueError('Source combustion constant drift: ' + name)
        value = m[1].replace('_', '')
        return float(value) if '.' in value else int(value)
    def tank_capacity(text, fluid, index=None):
        m = re.search(r'new FluidTank\(Fluids\.' + fluid + r',\s*([\d_]+)', text)
        if not m: raise ValueError('Source tank drift: ' + fluid)
        return int(m[1].replace('_', ''))
    pistons = {}
    pistons_text = clean((source / (BASE + 'items/machine/ItemPistons.java')).read_text())
    for member, values in re.findall(r'^\s*(\w+)\s*\(([\d.,\s]+)\)', pistons_text.split('enum EnumPistonType', 1)[1], re.M):
        row = [float(v) for v in values.split(',')]
        pistons[member] = dict(zip(grades, row + [0.0] * (len(grades) - len(row))))
    if list(pistons) != ['STEEL', 'DURA', 'DESH', 'STARMETAL']: raise ValueError('Piston matrix drift')
    # Fuel grades are indexed by ordinal; a short efficiency row leaves the remaining grades at 0.
    configs = {}

    path, text = tile('MachineDiesel')
    efficiency = {m[1]: float(m[2]) for m in re.finditer(r'fuelEfficiency\.put\(FuelGrade\.(\w+),\s*([\d.]+)D?\)', text)}
    if set(efficiency) != {'MEDIUM', 'HIGH', 'AERO'}: raise ValueError('Diesel efficiency drift')
    diesel_rates = [{'fluid': f['fluid'], 'grade': f['grade'], 'he_per_mb': int(f['he_per_bucket'] // 1000 * efficiency[f['grade']])}
                    for f in fuels if f['grade'] in efficiency]
    configs['ntm:machine_diesel'] = {
        'source': path, 'power_capacity_he': number(text, 'maxPower'), 'fuel_capacity_mb': number(text, 'fuelCap'),
        'default_fluid': 'diesel', 'burn_mb_per_tick': 1, 'efficiency_by_grade': efficiency,
        'explicitly_excluded_grades': ['LOW'], 'zero_efficiency_grades': [g for g in grades if g not in efficiency],
        'fuel_traits': '../fluid_traits.json#combustible',
        'conversion': 'he_per_tick=trunc(floor(he_per_bucket/1000)*efficiency[grade]); LOW and any grade without an efficiency entry yield 0 and are unusable. The bucket division happens BEFORE the multiplication.',
        'operation': 'Only while on, not redstone-powered, acceptable fuel and fill>0: consume exactly 1 mB per tick, then power=min(power+he_per_tick, capacity). A tick that would exceed the cap sets power to the cap.',
        'controls': 'turnOn packet toggles isOn; the machine starts off.',
        'inventory': 'slot0 fluid container input, slot1 empty container output (canister_empty or tank_steel), slot2 battery charged from the buffer, slot3 fluid identifier',
        'derived_he_per_mb': diesel_rates,
        'sources': {'efficiency': loc(path, text, 'fuelEfficiency.put(FuelGrade.MEDIUM'), 'conversion': loc(path, text, 'public static long getHEFromFuel(FluidType type)'),
                    'generate': loc(path, text, 'public void generate()')},
        'runtime_gaps': ['Pollution, sound loop, GUI, ports and the IConfigurableMachine JSON override are runtime concerns and are not claimed.']}

    path, text = tile('MachineCombustionEngine')
    configs['ntm:machine_combustion_engine'] = {
        'source': path, 'power_capacity_he': number(text, 'maxPower'), 'tank_capacity_mb': tank_capacity(text, 'DIESEL'),
        'default_fluid': 'diesel', 'piston_slot': 2,
        'setting_range_gui': [0, 30], 'setting_range_server': 'unclamped: receiveControl stores any integer',
        'fuel_traits': '../fluid_traits.json#combustible', 'piston_efficiency': pistons,
        'piston_item': 'ntm:piston_set', 'piston_metadata': {name: i for i, name in enumerate(pistons)},
        'operation': 'Fuel is tracked in tenths of mB: fill10=fill*10+tenth. When on, setting>0, a piston set sits in slot2, fill10>0 and the tank fluid has FT_Combustible: eff=piston[grade]; if eff>0, toBurn=min(fill10,setting*2); power=trunc(power+toBurn*(he_per_bucket/10000.0)*eff) as a Java compound assignment on a long; fill10-=toBurn; fill=fill10/10, tenth=fill10%10.',
        'he_per_tenth_mb': 'he_per_bucket/10000.0*efficiency; one mB is ten tenths',
        'power_clamp': 'Clamp power to capacity after battery charging each tick; the unclamped value is what batteries receive.',
        'redstone': 'The source does NOT check redstone for the engine.',
        'sources': {'operation': loc(path, text, 'EnumPistonType piston ='), 'ports': loc(path, text, 'public PortDef[] getPorts()'),
                    'controls': loc(path, text, 'public void receiveControl(')},
        'runtime_gaps': ['Piston slot validation, door animation, pollution, sound and GUI are not claimed.']}

    path, text = tile('MachineTurbofan')
    configs['ntm:machine_turbofan'] = {
        'source': path, 'power_capacity_he': number(text, 'maxPower'), 'fuel_capacity_mb': tank_capacity(text, 'KEROSENE'),
        'blood_capacity_mb': tank_capacity(text, 'BLOOD'), 'default_fluid': 'kerosene', 'accepted_grade': 'AERO',
        'fuel_traits': '../fluid_traits.json#combustible',
        'upgrade': {'type': 'AFTERBURN', 'max_level': 3, 'items': ['ntm:upgrade_afterburn_1', 'ntm:upgrade_afterburn_2', 'ntm:upgrade_afterburn_3'], 'override_item': 'ntm:flame_pony', 'override_level': 100},
        'operation': 'burnValue=floor(he_per_bucket/1000) for AERO fuel only. amount=1+afterburner; toBurn=min(amount,fill). With no redstone on any of the four connector positions: fill-=toBurn; output=int32(burnValue*toBurn*(1+min(afterburner/3.0,4))); power+=output.',
        'java_integer_semantics': 'output is a signed int32 Java cast; flame_pony sets afterburner to 100 and can exceed int range with dense fuels.',
        'sources': {'operation': loc(path, text, 'long burnValue = 0;'), 'upgrade': loc(path, text, 'this.afterburner = upgradeManager.getLevel')},
        'runtime_gaps': ['Afterburner damage, entity pushing, blood tank gibbing, particles and sounds are source side effects and are not claimed.']}

    path, text = tile('MachineTurbineGas')
    consumption = {m[1].lower(): float(m[2]) for m in re.finditer(r'fuelMaxCons\.put\(Fluids\.(\w+),\s*([\d.]+)D\)', text)}
    configs['ntm:machine_turbinegas'] = {
        'source': path, 'power_capacity_he': number(text, 'maxPower'),
        'tank_capacities_mb': {'gas': tank_capacity(text, 'GAS'), 'lubricant': tank_capacity(text, 'LUBRICANT'),
                               'water': tank_capacity(text, 'WATER'), 'hotsteam': tank_capacity(text, 'HOTSTEAM')},
        'accepted_grade': 'GAS', 'fuel_traits': '../fluid_traits.json#combustible',
        'max_consumption_mb_per_tick': consumption, 'default_consumption_mb_per_tick': 5,
        'startup': {'rpm_idle': number(text, 'rpmIdle'), 'temp_idle': number(text, 'tempIdle'), 'slider_range': [0, 60]},
        'operation': 'consMax from the table; consumption=consMax*0.05+consMax*throttle/100 with a fractional accumulator, floor() mB removed each tick; one lubricant mB every 10 ticks. energy=floor(he_per_bucket/1000) per mB. instant output ramps toward consMax*energy*(rpm-rpmIdle)/90 and is added to power each tick.',
        'temperature': 'burn_temp=floor(800-e^(-he_per_bucket/100000)*300); the heat exchanger boils floor(consMax*energy*(temp-tempIdle)/220000) water per tick into 10x dense steam, limited by water and steam space.',
        'sources': {'consumption_table': loc(path, text, 'fuelMaxCons.put(Fluids.GAS'), 'power': loc(path, text, 'private void makePower('),
                    'burn_temp': loc(path, text, 'protected int getFluidBurnTemp('), 'startup': loc(path, text, 'private void startup()')},
        'runtime_gaps': ['Startup/shutdown timing, rotor inertia, the random ramp of instant output, ports and sounds need machine logic; the random component is not reproduced here.']}

    return write(output, 'combustion', {
        'machine_config': configs, 'recipes': [], 'skipped': [],
        'fuel_grades': grades, 'fuel_traits': '../fluid_traits.json',
        'selection': 'Algorithmic generators, not generic crafters; the fuel grade and per-machine rounding are authoritative.',
        'requires_runtime_fields': ['machine_config', 'fuel_traits.combustible'],
        'coverage': {'machines': len(configs), 'pistons': len(pistons), 'combustible_fluids': len(fuels), 'skipped': 0},
        'runtime_gaps': ['No fixed HE/t or duration is declared: every rate derives from the combustible trait, the grade efficiency and the piston matrix.']})

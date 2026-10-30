"""PWR fuel and controller snapshot; no runtime. Structure, flux and heat transfer stay native work."""
import re
from bulk_recipes import clean
from bedrock_processing_common import BASE, write
from pwr_catalog import fuels

BONUS={'FunctionLogarithmic':'log10(x/div+1)*level','FunctionSqrt':'squirt(x/div+off)*level, squirt(v)=sqrt(v+1/((v+2)^2))-1/(v+2)'}


def export(source, output, rows):
    known={r['id'] for r in rows}
    table=fuels(source)
    for f in table:
        s=f['member'].lower()
        f['items']={'fresh':f'ntm:pwr_fuel__{s}','hot':f'ntm:pwr_fuel_hot__{s}','depleted':f'ntm:pwr_fuel_depleted__{s}'}
        for item in f['items'].values():
            if item not in known:raise ValueError('Missing PWR fuel id '+item)
        f['yield']=1_000_000_000
    path=BASE+'tileentity/machine/TileEntityPWRController.java'
    text=clean((source/path).read_text())
    tanks=[{'source_fluid':m[1],'capacity_mb':int(m[2].replace('_',''))} for m in re.finditer(r'new FluidTank\(Fluids\.(\w+), ([\d_]+)\)',text)]
    if [t['source_fluid'] for t in tanks]!=['COOLANT','COOLANT_HOT']:raise ValueError('PWR tank drift')
    caps={k:int(re.search(k+r' = ([\d_]+)',text)[1].replace('_','')) for k in ('coreHeatCapacityBase','hullHeatCapacityBase')}
    return write(output,'reactors_pwr',{'recipes':[],'skipped':[],
        'fuels':table,'function_forms':BONUS,
        'source_quirk':'EnumPWRFuel has a 3-argument constructor taking a yield, but it never assigns it: every fuel, BFB_AM_MIX and BFB_PU241 included, keeps yield 1,000,000,000. declared_yield records the ignored argument (250,000,000).',
        'conversion':'fresh -> hot -> depleted by the controller (HeatingType.PWR / FT_PWRModerator tables are in the fluid contracts, not duplicated here).',
        'controller':{'source':path,'tanks':tanks,'heat_capacity':caps,'source_slots':'fuel rods are read from the assembled column blocks (pwr_fuel), not controller slots',
                      'rod_level':'rodLevel/rodTarget percent, default 100'},
        'identities':{'block_pwr_fuel':'ntm:pwr_fuel','item_pwr_fuel':'ntm:pwr_fuel__<enum lower-case>',
                      'note':'Block and item share the variable name pwr_fuel in the source. The block keeps ntm:pwr_fuel; the item subtypes use the pwr_fuel__ prefix like pwr_fuel_hot__/pwr_fuel_depleted__.'},
        'runtime_gaps':['Structure validation and conversion to pwr_block, flux/heat/coolant ticking, control rods, ports and fluid hand-off, failure and the printer GUI are native work; stateful content remains Inert.'],
        'coverage':{'fuels':len(table),'tanks':len(tanks)}})

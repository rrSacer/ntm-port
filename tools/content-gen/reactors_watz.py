"""Watz pellet and reactor snapshot; no runtime. Reaction, shared tanks and structure stay native work."""
import re
from bulk_recipes import clean
from bedrock_processing_common import BASE, write
from watz_catalog import pellets


def export(source, output, rows):
    known={r['id'] for r in rows}
    table=pellets(source)
    for p in table:
        s=p['member'].lower()
        p['items']={'fresh':f'ntm:watz_pellet__{s}','depleted':f'ntm:watz_pellet_depleted__{s}'}
        for item in p['items'].values():
            if item not in known:raise ValueError('Missing Watz pellet id '+item)
        p['mud_content']=p['mud_content_raw']/2.0
        p['role']='absorber' if p['absorb'] else 'fuel'
        p['yield']=500_000_000
    path=BASE+'tileentity/machine/TileEntityWatz.java'
    text=clean((source/path).read_text())
    tanks=[{'source_fluid':m[1],'capacity_mb':int(m[2].replace('_',''))} for m in re.finditer(r'new FluidTank\(Fluids\.(\w+), ([\d_]+)\)',text)]
    if [t['source_fluid'] for t in tanks]!=['COOLANT','COOLANT_HOT','WATZ']*2:raise ValueError('Watz tank drift')
    return write(output,'reactors_watz',{'recipes':[],'skipped':[],
        'pellets':table,
        'function_forms':{'FunctionLinear':'x/div+off times level','FunctionSqrt':'squirt(x/div+off)*level','FunctionSqrtFalling':'sqrt form with level 1/fall, offset fall^2',
                          'FunctionQuadratic':'(x/div+off)^2*level+vOff'},
        'mud':'mudContent = constructor value / 2 per reaction flux; the source label is Poisonous Mud (WATZ fluid).',
        'reactor':{'source':path,'tanks':{'own':tanks[:3],'shared':tanks[3:]},'tank_roles':'COOLANT receives, COOLANT_HOT and WATZ are sent; shared tanks aggregate the stack of reactors',
                   'stack':'Reactors above/below share tanks and reaction; the multiblock body has four extra side volumes.'},
        'structure':{'break_drops':'48 watz_end, 3x64 bolts (DURA), 36 watz_element, 26 watz_cooler and 1 struct_watz_core when a complete Watz breaks (meta>=12 and drop enabled)'},
        'identities':{'watz_end':'ntm:watz_end (meta 0) and ntm:watz_end__bolted (meta 1, source name watz_end_bolted); the screwdriver conversion is native logic'},
        'runtime_gaps':['Multiblock assembly, reaction and heat ticking, shared tanks and mud handling, ports, GUI, the pump, tool conversions and failure are native work; stateful content stays Inert.'],
        'coverage':{'pellets':len(table),'fuel':sum(1 for p in table if p['role']=='fuel'),'absorbers':sum(1 for p in table if p['role']=='absorber'),'tanks':len(tanks)}})

"""Final flammable values and exact cooling operations, separate from IDs."""
import re
from bulk_recipes import calls, clean, split
from bedrock_processing_common import BASE
from generate import write_json
from pyrooven_recipes import fuel_heat
from machine_recipes import integer


def export(source, output):
    path = BASE + 'inventory/fluid/Fluids.java'; text = clean((source / path).read_text())
    def origin(position): return path + ':' + str(text[:position].count('\n') + 1)
    declarations = list(re.finditer(r'\b([A-Z][A-Z0-9_]*)\s*=\s*new FluidType\((.*?);', text, re.S))
    identities = {m[1]: m[1].lower() for m in declarations}
    heat = fuel_heat(source); provenance = {}
    for m in declarations:
        if 'new FT_Flammable(' in m[2]: provenance[m[1]] = origin(m.start())
    for position, args, _ in calls(text, r'\bregisterCalculatedFuel'):
        variable = split(args)[0]
        if variable in heat: provenance[variable] = origin(position)
    flammable = [{'fluid': identities[m[1]], 'heat_tu_per_bucket': heat[m[1]], 'source': provenance[m[1]]}
                 for m in declarations if m[1] in heat]
    constants = {m[1]: float(m[2]) for m in re.finditer(r'double (\w+) = ([\d.]+)D;', text)}
    coolable = []
    for m in re.finditer(r'\b(\w+)\.addTraits\(new FT_Coolable\(([^)]*)\)(.*?);', text, re.S):
        target, req, prod, energy = split(m[2]); efficiencies = {'TURBINE': 0.0, 'HEATEXCHANGER': 0.0}
        for _, args, _ in calls(m[3], r'\.setEff'):
            kind, value = split(args)
            efficiencies[kind.split('.')[-1]] = constants[value] if value in constants else float(value.removesuffix('D'))
        coolable.append({'fluid': identities[m[1]], 'output': [identities[target], integer(prod)],
                         'input_mb': integer(req), 'heat_tu': integer(energy),
                         'efficiencies': efficiencies, 'source': origin(m.start())})
    data = {'schema': 1, 'profile': 'NTM source defaults; external fluid trait JSON/config overrides excluded',
            'units': {'flammable': 'TU/bucket (1000 mB)', 'coolable': 'TU/operation'},
            'flammable': flammable, 'coolable': coolable,
            'deferred_traits': ['FT_Combustible/FuelGrade: energy batch 3'],
            'coverage': {'flammable': len(flammable), 'coolable': len(coolable)}}
    write_json(output / 'fluid_traits.json', data)
    return data

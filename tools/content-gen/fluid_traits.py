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
    grades = {}; combustion = fuel_heat(source, True, grades)
    for m in declarations:
        if 'new FT_Flammable(' in m[2]: provenance[m[1]] = origin(m.start())
    combustible_origin = {m[1]: origin(m.start()) for m in declarations if 'new FT_Combustible(' in m[2]}
    for position, args, _ in calls(text, r'\bregisterCalculatedFuel'):
        variable, _, mult, grade = split(args)[:4]
        if variable in heat: provenance[variable] = origin(position)
        if grade != 'null' and variable in grades and grades[variable] == grade.split('.')[-1] and combustion[variable] > 0:
            combustible_origin[variable] = origin(position)
    flammable = [{'fluid': identities[m[1]], 'heat_tu_per_bucket': heat[m[1]], 'source': provenance[m[1]]}
                 for m in declarations if m[1] in heat]
    grade_order = re.findall(r'^\s*(\w+)\("\w+"\)', clean((source / (BASE + 'inventory/fluid/trait/FT_Combustible.java')).read_text()).split('enum FuelGrade', 1)[1].split('}', 1)[0], re.M)
    if grade_order != ['LOW', 'MEDIUM', 'HIGH', 'AERO', 'GAS']: raise ValueError('FuelGrade drift: ' + repr(grade_order))
    # Java stores the HE per bucket of the last trait written for the class: constructor value, then calculated updates.
    combustible = [{'fluid': identities[m[1]], 'grade': grades[m[1]], 'grade_ordinal': grade_order.index(grades[m[1]]),
                    'he_per_bucket': combustion[m[1]], 'source': combustible_origin[m[1]]}
                   for m in declarations if m[1] in grades]
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
            'units': {'flammable': 'TU/bucket (1000 mB)', 'coolable': 'TU/operation', 'combustible': 'HE/bucket (1000 mB), separate from flammable TU'},
            'fuel_grades': grade_order,
            'flammable': flammable, 'coolable': coolable, 'combustible': combustible,
            'deferred_traits': [],
            'coverage': {'flammable': len(flammable), 'coolable': len(coolable), 'combustible': len(combustible)}}
    write_json(output / 'fluid_traits.json', data)
    return data

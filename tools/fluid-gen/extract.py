#!/usr/bin/env python3
"""Extrai os tipos de fluido do NTM (Fluids.java) para crates/ntm-core/data/fluids.json.

Uso: python3 tools/fluid-gen/extract.py [ntm-src]
O id é a ordem de registro (`Fluids.registerSelf`, uma linha por fluido;
TITANIUM_TETRACHLORIDE declara 156 explicitamente e o extrator confere).
"""
import json, re, sys
from pathlib import Path

src = Path(sys.argv[1] if len(sys.argv) > 1 else '/home/sacer/Projects/horizon-ntm/ntm-src')
java = src / 'src/main/java/com/hbm/inventory/fluid/Fluids.java'
out = Path(__file__).resolve().parents[2] / 'crates/ntm-core/data/fluids.json'

DECL = re.compile(r'^\s*([A-Z0-9_]+)\s*=\s*new FluidType\((?:(\d+),\s*)?"([A-Z0-9_]+)",\s*0x([0-9a-fA-F]+)\s*,\s*(\d+),\s*(\d+),\s*(\d+),\s*EnumSymbol\.([A-Z_]+)\)(.*?);\s*(?://.*)?$')
SIMPLE = {'LIQUID': 'liquid', 'GASEOUS': 'gaseous', 'VISCOUS': 'viscous', 'EVAP': 'evaporates', 'ANTI': 'antimatter',
          'UNSIPHONABLE': 'unsiphonable', 'LEADCON': 'lead_container', 'NOCON': 'no_container', 'PLASMA': 'plasma', 'NOID': 'no_id', 'DELICIOUS': 'delicious'}

fluids = []
lines = java.read_text().splitlines()
for n, line in enumerate(lines, 1):
    m = DECL.match(line)
    if not m:
        continue
    var, forced, name, color, p, f, r, sym, rest = m.groups()
    if var != name:
        raise SystemExit(f'{n}: variável {var} ≠ nome {name}')
    fid = len(fluids)
    if forced is not None and int(forced) != fid:
        raise SystemExit(f'{n}: id forçado {forced}, ordem {fid}')
    t = {'id': fid, 'name': name, 'string_id': name.lower(), 'color': int(color, 16), 'poison': int(p), 'flammability': int(f),
         'reactivity': int(r), 'symbol': sym.lower(), 'temp': 20, 'source': f'Fluids.java:{n}'}
    tm = re.search(r'\.setTemp\((-?[\d_]+)\)', rest)
    if tm:
        t['temp'] = int(tm.group(1).replace('_', ''))
    traits = re.search(r'\.addTraits\((.*)\)', rest)
    traits = traits.group(1) if traits else ''
    for tok, key in SIMPLE.items():
        if re.search(rf'(?<![A-Za-z_]){tok}(?![A-Za-z_(])', traits):
            t[key] = True
    c = re.search(r'FT_Corrosive\((\d+)\)', traits)
    if c:
        t['corrosive'] = int(c.group(1))
    v = re.search(r'FT_VentRadiation\(([\d.]+)F?\)', traits)
    if v:
        t['vent_radiation'] = float(v.group(1))
    if 'FT_Flammable' in traits:
        t['flammable'] = True
    if 'FT_Poison' in traits or 'FT_Toxin' in traits:
        t['toxic'] = True
    if 'CD_Canister' in rest:
        t['canister'] = True
    if 'CD_Gastank' in rest:
        t['gastank'] = True
    fluids.append(t)

assert fluids[0]['name'] == 'NONE' and fluids[1]['name'] == 'WATER', 'ordem inesperada'
assert any(f['name'] == 'TITANIUM_TETRACHLORIDE' and f['id'] == 156 for f in fluids), 'TiCl4 fora do 156'
out.write_text(json.dumps({'source': 'ntm-src src/main/java/com/hbm/inventory/fluid/Fluids.java', 'fluids': fluids}, indent=1, ensure_ascii=False) + '\n')
print(f'{len(fluids)} fluidos → {out}')

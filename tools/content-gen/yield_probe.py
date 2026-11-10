#!/usr/bin/env python3
"""In-game yield probe: ntm veins and vanilla controls per dimension, with distinct biomes per chunk.

Runs the dedicated server through verify-worldgen.py's server() (same pregen as the verifier), then reads the
saves. Writes content-gen-evidence/worldgen/yield-probe-<dimension>.json. Pass --package for the built .hmod.
"""
import argparse, collections, importlib.util, json, sys
from pathlib import Path
HERE = Path(__file__).resolve().parent; ROOT = HERE.parents[1]; E = ROOT / 'content-gen-evidence/worldgen'
def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / f'{name}.py'); m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m); return m
scan = load('world-scan'); verify = load('verify-worldgen')
import worldgen
CONTROLS = {'overworld': ['minecraft:coal_ore'], 'nether': ['minecraft:nether_quartz_ore']}
CONTROL_PREDICTED = {'minecraft:coal_ore': ('overworld', 20, 8.38), 'minecraft:nether_quartz_ore': ('nether', 16, 6.0)}

def probe(world, names, bounds):
    counts = collections.Counter(); per_chunk = collections.Counter(); biomes = {}
    for c in scan.chunks(world):
        cx, cz = c['xPos'], c['zPos']
        if not (bounds[0] <= cx <= bounds[1] and bounds[2] <= cz <= bounds[3]): continue
        found = set()
        for sec in c.get('sections', []):
            for b in sec.get('biomes', {}).get('palette', []): found.add(b if isinstance(b, str) else b.get('value', str(b)))
            for i, name in enumerate(scan.section_states(sec)):
                if name in names:
                    counts[name] += 1; per_chunk[(cx, cz, name)] += 1
        biomes[(cx, cz)] = found
    return counts, per_chunk, biomes

def main():
    ap = argparse.ArgumentParser(); ap.add_argument('dimension', choices=['overworld', 'nether']); ap.add_argument('--package', type=Path, required=True); a = ap.parse_args()
    if a.dimension == 'nether': specs = [s for s in worldgen.EXTRA if s['material'].startswith('nether_')]
    else: specs = json.loads((HERE / 'm1.json').read_text())['worldgen']
    names = {s['block'] for s in specs} | set(CONTROLS[a.dimension])
    world = verify.server(f'probe-{a.dimension}', a.package, a.dimension)
    bounds = (-4, 4, -4, 4)
    counts, per_chunk, biomes = probe(world, names, bounds)
    chunks = len(biomes)
    veins = {}
    for s in specs:
        n = counts[s['block']]
        # blocks per attempt, with the same window method as verify-worldgen.py
        veins[s['block']] = {'blocks': n, 'per_attempt': round(n / (chunks * s['attempts_per_chunk']), 3), 'declared_attempts': s['attempts_per_chunk'], 'size': s['size']}
    controls = {}
    for name in CONTROLS[a.dimension]:
        dim, attempts, predicted = CONTROL_PREDICTED[name]
        controls[name] = {'blocks': counts[name], 'per_chunk': round(counts[name] / chunks, 2), 'predicted_per_chunk_if_full_target': round(attempts * predicted, 2)}
    by_biomes = collections.defaultdict(lambda: collections.Counter()); n_chunks = collections.Counter()
    for (cx, cz), found in biomes.items():
        k = len(found); n_chunks[k] += 1
        for name in names:
            by_biomes[k][name] += per_chunk[(cx, cz, name)]
    dist = {str(k): {'chunks': n_chunks[k], 'veins_blocks_per_chunk': {name: round(by_biomes[k][name] / n_chunks[k], 2) for name in sorted(names)}} for k in sorted(n_chunks)}
    result = {'dimension': a.dimension, 'chunks': chunks, 'veins': veins, 'controls': controls, 'distinct_biomes_per_chunk': dist}
    E.mkdir(parents=True, exist_ok=True)
    (E / f'yield-probe-{a.dimension}.json').write_text(json.dumps(result, indent=2) + '\n'); print(json.dumps(result), flush=True)

if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Executable Java oracle for the world's radiation by chunk (Onda 1, slice 4a).

`ChunkRadiationHandlerSimple`'s `getRadiation`, `setRadiation`, `incrementRad`, `decrementRad` and `updateSystem` are
cut **verbatim** from the HBM source and compiled with minimal stubs: `ChunkCoordIntPair` with the hash of the 1.7.10
client's bytecode (`agu.hashCode`), `MathHelper.clamp_float`, `RadiationConfig`, a world that always holds its blocks.
The handler's own `java.util.HashMap` walks the chunks, so the order the spread and the decay take is the Java's. The
fog (an effect, not the field) is turned off by a world whose chunks never exist.

Runs: seeded increments, decrements, chunk loads and updates; bins made to collide (near the origin and far out, where
hashes pass 2^31), which turn into the trees a full bin becomes, grow, split and take more chunks; a big patch. Each
writes the map as the Java walks it at several points (`dump`), each value's float bits included; the port's
`chunk_radiation` must give the same map in the same order. Two equal full hashes in one tree are left out: the Java
orders them by `System.identityHashCode`, which no run repeats.

Writes tools/chunk-radiation-oracle/fixture.json; `--check` compares (exit 1 when it differs). Needs `java`/`javac`.
  python3 tools/chunk-radiation-oracle/run.py --scratch DIR [--check]
"""
import argparse
import json
import os
import random
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JAVA = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main/java/com/hbm'
FIXTURE = ROOT / 'tools/chunk-radiation-oracle/fixture.json'
SEEDS = [1, 42, 20261111, -5]


def method(text, signature):
    """A method, from its signature to its closing brace at the same depth."""
    start = text.index(signature)
    start = text.rindex('\n', 0, start) + 1
    depth, i = 0, text.index('{', start)
    while True:
        if text[i] == '{':
            depth += 1
        elif text[i] == '}':
            depth -= 1
            if depth == 0:
                return text[start:i + 1]
        i += 1


STUBS = r'''
import java.util.*;
import java.util.Map.Entry;

class ChunkCoordIntPair {
    public final int chunkXPos, chunkZPos;
    public ChunkCoordIntPair(int x, int z) { chunkXPos = x; chunkZPos = z; }
    // agu.hashCode, 1.7.10 client bytecode
    public int hashCode() { int i = 1664525 * chunkXPos + 1013904223; int j = 1664525 * (chunkZPos ^ -559038737) + 1013904223; return i ^ j; }
    public boolean equals(Object o) { if (o == this) return true; if (!(o instanceof ChunkCoordIntPair)) return false; ChunkCoordIntPair p = (ChunkCoordIntPair) o; return p.chunkXPos == chunkXPos && p.chunkZPos == chunkZPos; }
}
class MathHelper { public static float clamp_float(float num, float min, float max) { return num < min ? min : (num > max ? max : num); } }
class RadiationConfig { public static int fogRad = 100; public static int fogCh = 20; }
class World {
    public Random rand = new Random(0);
    public boolean blockExists(int x, int y, int z) { return true; }
    public Chunk getChunkFromBlockCoords(int x, int z) { return new Chunk(); }
}
class Chunk { public boolean isModified; }
'''

ORACLE = r'''
public class ChunkRadiationOracle {
    private Map<World, SimpleRadiationPerWorld> perWorld = new HashMap<>();
    private static final float maxRad = 100_000F;
    public static class SimpleRadiationPerWorld { public Map<ChunkCoordIntPair, Float> radiation = new HashMap<>(); }

%METHODS%

    public static void main(String[] args) throws Exception {
        ChunkRadiationOracle h = new ChunkRadiationOracle();
        SimpleRadiationPerWorld rw = new SimpleRadiationPerWorld();
        World world = new World();
        h.perWorld.put(world, rw);
        java.io.BufferedReader in = new java.io.BufferedReader(new java.io.InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            String[] p = line.split(" ");
            switch (p[0]) {
                // the block coordinates of the chunk's corner: x >> 4, z >> 4 give the chunk back
                case "inc": h.incrementRad(world, Integer.parseInt(p[1]) * 16, 64, Integer.parseInt(p[2]) * 16, Float.intBitsToFloat(Integer.parseInt(p[3]))); break;
                case "dec": h.decrementRad(world, Integer.parseInt(p[1]) * 16, 64, Integer.parseInt(p[2]) * 16, Float.intBitsToFloat(Integer.parseInt(p[3]))); break;
                // receiveChunkLoad: put what the chunk's save holds
                case "load": rw.radiation.put(new ChunkCoordIntPair(Integer.parseInt(p[1]), Integer.parseInt(p[2])), Float.intBitsToFloat(Integer.parseInt(p[3]))); break;
                case "update": h.updateSystem(); break;
                // the map as the Java walks it now
                case "dump":
                    StringBuilder out = new StringBuilder("dump\n");
                    for (Entry<ChunkCoordIntPair, Float> e : rw.radiation.entrySet())
                        out.append(e.getKey().chunkXPos).append(' ').append(e.getKey().chunkZPos).append(' ').append(Float.floatToRawIntBits(e.getValue())).append('\n');
                    System.out.print(out);
                    break;
            }
        }
    }
}
'''


def f32_bits(x):
    import struct
    return struct.unpack('>i', struct.pack('>f', x))[0]


def script(seed):
    """Increments and decrements over a patch of chunks, loads with saved values, and updates between them."""
    rng = random.Random(seed)
    ops = []
    for round in range(20):
        if round == 10:
            ops.append(['dump'])
        for _ in range(rng.randint(1, 8)):
            kind = rng.choice(['inc', 'inc', 'inc', 'dec', 'load'])
            c = [rng.randint(-6, 6), rng.randint(-6, 6)]
            value = rng.choice([rng.uniform(0, 50), rng.uniform(0, 5000), rng.uniform(50_000, 2_000_000), 0.0])
            ops.append([kind, c[0], c[1], f32_bits(value)])
        for _ in range(rng.randint(1, 3)):
            ops.append(['update'])
    return ops + [['dump']]


def spread_hash(x, z):
    """`HashMap.hash` of `agu.hashCode`, unsigned."""
    m = 0xFFFFFFFF
    h = ((1664525 * x + 1013904223) & m) ^ ((1664525 * ((z ^ -559038737) & m) + 1013904223) & m)
    return h ^ (h >> 16)


def colliding(seed, centre=(0, 0)):
    """40 chunks of one bin of a table of 64, loaded in a seeded order among others: the full bin becomes a tree, the
    tree grows (its rotations), each doubling splits it (both halves trees again while over 6, lists when 6 or fewer)
    and more chunks go into the halves; then an update."""
    rng = random.Random(seed)
    near = [(centre[0] + x, centre[1] + z) for x in range(-100, 101) for z in range(-100, 101)]
    keys = [c for c in near if spread_hash(*c) & 63 == 5]
    rng.shuffle(keys)
    # Two equal full hashes in a tree are ordered by System.identityHashCode, which no run repeats: left out.
    keys = list({spread_hash(*c): c for c in reversed(keys)}.values())[::-1][:40]
    others = [c for c in near if spread_hash(*c) & 63 != 5]
    rng.shuffle(others)
    # 20 others first, so the table is at 64 (past 12 entries) when the bin fills; 30 more double it at 48 with the
    # tree in it; the rest of the bin goes into its halves; 150 more double it twice again.
    order = others[:20] + keys[:26] + others[20:50] + keys[26:] + others[50:200]
    ops = []
    for n, (x, z) in enumerate(order):
        ops.append(['load', x, z, f32_bits(rng.choice([0.0, rng.uniform(0, 3000)]))])
        # the walk while the trees grow and split, every few loads
        if n % 7 == 6:
            ops.append(['dump'])
    x, z = rng.choice(keys)
    return ops + [['update'], ['inc', x, z, f32_bits(rng.uniform(0, 500))], ['dump']]


def deep(seed, centre=(0, 0)):
    """47 chunks of one bin and nothing else: the table doubles to 64 under the full bin, the bin becomes a tree and
    takes the rest one by one (each after its parent, the root first: the tree's shape decides the walk), up to 48."""
    rng = random.Random(seed)
    near = [(centre[0] + x, centre[1] + z) for x in range(-100, 101) for z in range(-100, 101)]
    keys = [c for c in near if spread_hash(*c) & 63 == 5]
    rng.shuffle(keys)
    keys = list({spread_hash(*c): c for c in reversed(keys)}.values())[::-1][:47]
    ops = []
    for x, z in keys:
        ops += [['load', x, z, f32_bits(rng.uniform(0, 100))], ['dump']]
    return ops


def patch():
    """A big loaded area, as several players make: the bins of 9 a table of thousands holds."""
    rng = random.Random(11)
    ops = [['load', x, z, f32_bits(rng.choice([0.0, rng.uniform(0, 200)]))] for x in range(-25, 25) for z in range(-25, 25)]
    return ops + [['update'], ['update'], ['dump']]


def run(scratch):
    scratch.mkdir(parents=True, exist_ok=True)
    text = (JAVA / 'handler/radiation/ChunkRadiationHandlerSimple.java').read_text()
    methods = '\n\n'.join(re.sub(r'@Override\s*', '', method(text, sig)) for sig in [
        'public float getRadiation(World world', 'public void setRadiation(World world', 'public void incrementRad(World world',
        'public void decrementRad(World world', 'public void updateSystem()'])
    # The fog is an effect: `chunkExists` false never lets it run; its MainRegistry/NBT stubs are not needed then.
    methods = methods.replace('MainRegistry.proxy.effectNT(data);', '').replace('NBTTagCompound data = new NBTTagCompound();', 'Map<String, Object> data = new HashMap<>();')
    methods = re.sub(r'data\.set(String|Double)\(("\w+"), ([^;]+)\);', r'data.put(\2, \3);', methods)
    methods = methods.replace('world.getChunkProvider().chunkExists(coord.chunkXPos, coord.chunkZPos)', 'false').replace('world.getHeightValue(x, z)', '0')
    (scratch / 'ChunkRadiationOracle.java').write_text(STUBS + ORACLE.replace('%METHODS%', methods))
    subprocess.run(['javac', '-d', str(scratch), str(scratch / 'ChunkRadiationOracle.java')], check=True)
    cases = []
    for seed, ops in [(seed, script(seed)) for seed in SEEDS] + [(f'colliding {n}', colliding(n)) for n in range(7, 11)] + [
        # far out, where hashes pass 2^31 and the tree's signed compare counts
        (f'colliding far {n}', colliding(n, (n * 7919 - 30000, 1_800_000 - n * 104729))) for n in range(3)] + [
        (f'deep {n}', deep(n, (0, 0) if n < 4 else (n * 7919 - 30000, 1_800_000 - n * 104729))) for n in range(8)] + [
        ('patch', patch())]:
        text_ops = '\n'.join(' '.join(str(x) for x in op) for op in ops) + '\n'
        out = subprocess.run(['java', '-cp', str(scratch), 'ChunkRadiationOracle'], input=text_ops, capture_output=True, text=True, check=True).stdout
        # each dump: x, z and the value's bits, flat, in the walk's order
        dumps = [[int(n) & 0xFFFFFFFF if k % 3 == 2 else int(n) for k, n in enumerate(d.split())] for d in out.split('dump\n')[1:]]
        cases.append({'seed': seed, 'ops': ops, 'dumps': dumps})
    return {'source': 'ChunkRadiationHandlerSimple get/set/increment/decrement/updateSystem, verbatim', 'cases': cases}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    fixture = run(args.scratch)
    if args.check:
        if json.loads(FIXTURE.read_text()) != json.loads(json.dumps(fixture)):
            print('fixture differs from the Java oracle', file=sys.stderr)
            sys.exit(1)
        print('fixture matches the Java oracle')
    else:
        FIXTURE.write_text(json.dumps(fixture, separators=(',', ':')) + '\n')
        print(sum(len(d) // 3 for c in fixture['cases'] for d in c['dumps']), 'chunks in', sum(len(c['dumps']) for c in fixture['cases']), 'dumps of', len(fixture['cases']), 'runs')

#!/usr/bin/env python3
"""Executable Java oracle for the step of a walking entity (Onda 1, slice 5d, D3).

The step's arithmetic of `Entity.moveEntity` in the 1.7.10 client (`sa.d(DDD)V`, offsets 1665-1763; the class is
Mojang's and obfuscated, so the lines are transcribed from the bytecode, cast by cast): `dy` is already 0 off a
ladder, `distanceWalkedOnStepModified = (float)((double) Q + (double) MathHelper.sqrt_double(dx² + dy² + dz²) * 0.6)`,
`MathHelper.sqrt_double` being `(float) Math.sqrt`, and a step when `Q > (float) nextStepDistance` (`fcmpl`) on a block
whose material is not air, then `nextStepDistance = (int) Q + 1`. Long walks of irregular moves (walking 0.2158 a tick,
sprinting 0.2806, jumps, ladders, air) are run through it; the port's `walking::accumulate` must give the same steps
and the same float, bit for bit.

Writes tools/step-oracle/fixture.json; `--check` compares (exit 1 when it differs). Needs `java`/`javac`.
  python3 tools/step-oracle/run.py --scratch DIR [--check]
"""
import argparse
import json
import random
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tools/step-oracle/fixture.json'
SEEDS = [1, 42, 20261111, 7]
MOVES = 2000

ORACLE = r"""
import java.io.*;
import java.util.*;

public class StepOracle {
    // MathHelper.sqrt_double (qh.a(D)F).
    static float sqrt_double(double d) { return (float) Math.sqrt(d); }

    public static void main(String[] args) throws Exception {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String line;
        float Q = 0.0F;          // distanceWalkedOnStepModified
        int nextStepDistance = 0;
        int tick = 0;
        StringBuilder steps = new StringBuilder();
        while ((line = in.readLine()) != null) {
            if (line.isEmpty()) continue;
            String[] p = line.split(" ");
            double d23 = Double.parseDouble(p[0]), d25 = Double.parseDouble(p[1]), d27 = Double.parseDouble(p[2]);
            boolean solid = p[3].equals("1");
            // offsets 1695-1728
            Q = (float) ((double) Q + (double) sqrt_double(d23 * d23 + d25 * d25 + d27 * d27) * 0.6D);
            // offsets 1731-1763
            if (Q > (float) nextStepDistance && solid) {
                nextStepDistance = (int) Q + 1;
                steps.append(tick).append(' ');
            }
            tick++;
        }
        System.out.println(steps.toString().trim());
        System.out.println(Float.floatToRawIntBits(Q) + " " + nextStepDistance);
    }
}
"""


def moves(seed):
    """A walk: stretches of walking, sprinting, jumping and standing, ladders (dy kept) and air (no step)."""
    rng = random.Random(seed)
    out = []
    while len(out) < MOVES:
        kind = rng.choice(['walk', 'sprint', 'jump', 'stand', 'ladder', 'air'])
        speed = {'walk': 0.21585, 'sprint': 0.28061, 'jump': 0.3, 'stand': 0.0, 'ladder': 0.0, 'air': 0.25}[kind]
        angle = rng.uniform(0, 6.283185307179586)
        for _ in range(rng.randint(5, 60)):
            jitter = rng.uniform(0.9, 1.1) if speed else 0
            dx, dz = speed * jitter * __import__('math').cos(angle), speed * jitter * __import__('math').sin(angle)
            dy = rng.uniform(0.1, 0.2) if kind == 'ladder' else 0.0
            # Six decimals: short to write, still a double each side reads the same.
            out.append([round(dx, 6), round(dy, 6), round(dz, 6), kind != 'air'])
    return out[:MOVES]


def run(scratch):
    scratch.mkdir(parents=True, exist_ok=True)
    (scratch / 'StepOracle.java').write_text(ORACLE)
    subprocess.run(['javac', '-d', str(scratch), str(scratch / 'StepOracle.java')], check=True)
    cases = []
    for seed in SEEDS:
        walk = moves(seed)
        # repr() round-trips a double exactly; Double.parseDouble reads the same value.
        text = '\n'.join(f'{m[0]!r} {m[1]!r} {m[2]!r} {int(m[3])}' for m in walk) + '\n'
        out = subprocess.run(['java', '-cp', str(scratch), 'StepOracle'], input=text, capture_output=True, text=True, check=True).stdout.splitlines()
        bits, next_step = out[1].split()
        cases.append({'seed': seed, 'moves': walk, 'steps': [int(s) for s in out[0].split()],
                      'walked_bits': int(bits) & 0xFFFFFFFF, 'next_step': int(next_step)})
    return {'source': 'Entity.moveEntity, 1.7.10 client sa.d(DDD)V offsets 1665-1763 (bytecode transcription)', 'cases': cases}


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
        print(sum(len(c['steps']) for c in fixture['cases']), 'steps in', len(fixture['cases']), 'walks')

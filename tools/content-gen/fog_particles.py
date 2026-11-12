#!/usr/bin/env python3
"""The radiation fog's particles (Onda 1, slice 4c): `ParticleRadiationFog`, drawn as the 25 quads of its
`renderParticle`, each a mod particle of the engine (`assets/ntm/particles/rad_fog_<n>.json`).

- Each quad's half size is `new Random(50).nextDouble() * 7.5` as the loop draws it (three `nextGaussian` before
  it, three after; their rejection loop moves the state as the JVM's does). The places are the server's
  (`chunk_radiation::fog_quads`, against the Java oracle).
- 400 ticks (`maxAge`, raised to 400 by `onUpdate`), still (no `moveEntity`), the texture `fog.png` times the colour
  `glColor4f(0.85F, 0.9F, 0.5F, ..)` (the engine draws a mod particle white).
- Decision D1 (A) of 4c: the alpha goes in a straight line from 0.125 to 0, where the Java's is
  `sin(age * PI / 400) * 0.125` (from 0 up to 0.125 at 200 ticks and back): the engine's mod particles interpolate
  their alpha linearly. Also drawn with the world's light, where the Java's is full bright (`setBrightness(240)`).

  python3 tools/content-gen/fog_particles.py
"""
import json
import math
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SRC = ROOT.parent / 'ntm-src' / 'src/main/resources/assets/hbm'
OUT = ROOT / 'content/assets/ntm'
COLOR = (0.85, 0.9, 0.5)


class JavaRandom:
    """`java.util.Random`: its state, `nextDouble` and `nextGaussian`."""

    def __init__(self, seed):
        self.seed = (seed ^ 0x5DEECE66D) & ((1 << 48) - 1)
        self.gaussian = None

    def next(self, bits):
        self.seed = (self.seed * 0x5DEECE66D + 0xB) & ((1 << 48) - 1)
        return self.seed >> (48 - bits)

    def next_double(self):
        return ((self.next(26) << 27) + self.next(27)) * (1.0 / (1 << 53))

    def next_gaussian(self):
        if self.gaussian is not None:
            g, self.gaussian = self.gaussian, None
            return g
        while True:
            v1 = 2 * self.next_double() - 1
            v2 = 2 * self.next_double() - 1
            s = v1 * v1 + v2 * v2
            if 0 < s < 1:
                m = math.sqrt(-2 * math.log(s) / s)
                self.gaussian = v2 * m
                return v1 * m


def sizes():
    rand = JavaRandom(50)
    out = []
    for _ in range(25):
        for _ in range(3):
            rand.next_gaussian()
        out.append(rand.next_double() * 7.5)
        for _ in range(3):
            rand.next_gaussian()
    return out


def main():
    (OUT / 'textures/particle').mkdir(parents=True, exist_ok=True)
    (OUT / 'particles').mkdir(parents=True, exist_ok=True)
    fog = Image.open(SRC / 'textures/particle/fog.png').convert('RGBA')
    r, g, b, a = fog.split()
    tinted = Image.merge('RGBA', [c.point(lambda v, k=k: int(v * k)) for c, k in zip((r, g, b), COLOR)] + [a])
    tinted.save(OUT / 'textures/particle/rad_fog.png')
    for n, size in enumerate(sizes()):
        doc = {'textures': ['ntm:rad_fog'],
               'horizon:behaviour': {'velocity': [0, 0, 0], 'lifetime': 400, 'size': [size, size], 'alpha': [0.125, 0], 'animation': 'first'}}
        (OUT / f'particles/rad_fog_{n}.json').write_text(json.dumps(doc, indent=2) + '\n')
    print(25, 'fog particles')


if __name__ == '__main__':
    main()

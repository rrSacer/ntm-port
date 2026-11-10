"""Vein yield per attempt: Java 1.7.10 WorldGenMinable and the horizon-world 26.3 ore vein port.

Both run on the same JavaRandom with all targets replaceable (netherrack). Used to calibrate
attempts_per_chunk in worldgen.py against the in-game counts (docs/worldgen-m1b.md).
"""
import math, struct

class JavaRandom:
    MULT, ADD, MASK = 0x5DEECE66D, 0xB, (1 << 48) - 1
    def __init__(self, seed): self.s = (seed ^ self.MULT) & self.MASK
    def next(self, bits):
        self.s = (self.s * self.MULT + self.ADD) & self.MASK
        r = self.s >> (48 - bits)
        return r - (1 << bits) if r >= (1 << (bits - 1)) else r
    def nextInt(self, bound):
        r = self.next(31); m = bound - 1
        if bound & m == 0: return (bound * r) >> 31
        u = r
        while True:
            r = u % bound
            if u - r + m < (1 << 31): return r
            u = self.next(31)
    def nextFloat(self): return self.next(24) / float(1 << 24)
    def nextDouble(self): return ((self.next(26) << 27) + self.next(27)) * (1.0 / (1 << 53))

def sin(f): return math.sin(f)
def cos(f): return math.cos(f)

def minable_blocks(rnd, amount, cx, cy, cz):
    """Positions placed by one attempt (Java generate), all replaceable."""
    placed = set()
    f = rnd.nextFloat() * math.pi
    d0 = (cx + 8) + sin(f) * amount / 8.0; d1 = (cx + 8) - sin(f) * amount / 8.0
    d2 = (cz + 8) + cos(f) * amount / 8.0; d3 = (cz + 8) - cos(f) * amount / 8.0
    d4 = cy + rnd.nextInt(3) - 2; d5 = cy + rnd.nextInt(3) - 2
    for l in range(amount + 1):
        d6 = d0 + (d1 - d0) * l / amount; d7 = d4 + (d5 - d4) * l / amount; d8 = d2 + (d3 - d2) * l / amount
        d9 = rnd.nextDouble() * amount / 16.0
        d10 = (sin(l * math.pi / amount) + 1.0) * d9 + 1.0
        d11 = d10
        i1, j1, k1 = math.floor(d6 - d10 / 2), math.floor(d7 - d11 / 2), math.floor(d8 - d10 / 2)
        l1, i2, j2 = math.floor(d6 + d10 / 2), math.floor(d7 + d11 / 2), math.floor(d8 + d10 / 2)
        for x in range(i1, l1 + 1):
            d12 = (x + 0.5 - d6) / (d10 / 2)
            if d12 * d12 >= 1: continue
            for y in range(j1, i2 + 1):
                d13 = (y + 0.5 - d7) / (d11 / 2)
                if d12 * d12 + d13 * d13 >= 1: continue
                for z in range(k1, j2 + 1):
                    d14 = (z + 0.5 - d8) / (d10 / 2)
                    if d12 * d12 + d13 * d13 + d14 * d14 < 1: placed.add((x, y, z))
    return placed

def mean_yield_java(amount, n=20000, seed=1768123):
    rnd = JavaRandom(seed); total = 0
    for _ in range(n):
        total += len(minable_blocks(rnd, amount, 0, 60, 0))
    return total / n



def f32(x): import struct; return struct.unpack('f', struct.pack('f', x))[0]

def place(rnd, size, origin=(0, 60, 0)):
    d = f32(rnd.nextFloat() * math.pi)
    spread = f32(size / 8.0)
    max_radius = math.ceil((size / 16.0 * 2.0 + 1.0) / 2.0)
    ox, oz = float(origin[0]), float(origin[2])
    s, c = math.sin(d), math.cos(d)
    x0, x1 = ox + s * spread, ox - s * spread
    z0, z1 = oz + c * spread, oz - c * spread
    y0 = float(origin[1] + rnd.nextInt(3) - 2)
    y1 = float(origin[1] + rnd.nextInt(3) - 2)
    data = []
    for i in range(size):
        step = f32(i / size)
        xx = x0 + step * (x1 - x0); yy = y0 + step * (y1 - y0); zz = z0 + step * (z1 - z0)
        ss = rnd.nextDouble() * size / 16.0
        sine = math.sin(math.pi * step)
        r = ((sine + 1.0) * ss + 1.0) / 2.0
        data.append([xx, yy, zz, r])
    for i1 in range(size - 1):
        if data[i1][3] <= 0: continue
        for i2 in range(i1 + 1, size):
            if data[i2][3] <= 0: continue
            dx = data[i1][0] - data[i2][0]; dy = data[i1][1] - data[i2][1]; dz = data[i1][2] - data[i2][2]
            dr = data[i1][3] - data[i2][3]
            if dr * dr > dx * dx + dy * dy + dz * dz:
                if dr > 0: data[i2][3] = -1.0
                else: data[i1][3] = -1.0
    placed = set()
    for xx, yy, zz, r in data:
        if r < 0: continue
        x_min, y_min, z_min = math.floor(xx - r), math.floor(yy - r), math.floor(zz - r)
        x_max, y_max, z_max = max(math.floor(xx + r), x_min), max(math.floor(yy + r), y_min), max(math.floor(zz + r), z_min)
        for x in range(x_min, x_max + 1):
            xd = (x + 0.5 - xx) / r
            if xd * xd >= 1: continue
            for y in range(y_min, y_max + 1):
                yd = (y + 0.5 - yy) / r
                if xd * xd + yd * yd >= 1: continue
                for z in range(z_min, z_max + 1):
                    zd = (z + 0.5 - zz) / r
                    if xd * xd + yd * yd + zd * zd < 1: placed.add((x, y, z))
    return placed

def mean_yield_26_3(size, n=20000, seed=1768123):
    rnd = JavaRandom(seed); total = 0
    for _ in range(n):
        total += len(place(rnd, size))
    return total / n


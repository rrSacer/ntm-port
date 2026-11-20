#!/usr/bin/env python3
"""T08 proof (`rbmk-plano` T08): the boiler, heater and cooler on real ducts and barrels, and the ReaSim inlet and
outlet, on a host with a LAN guest, counted before and after and after the world opens again.

- boiler without a loader: water from a barrel by a duct under its core, steam by a duct over its top
  (`getColumnHeight` up) into a barrel; heated by `ntmtest heat`, it turns each mB of water into 100 mB of steam;
- compressor: the guest presses the boiler's button, a dry cold boiler steps its steam grade;
- loaders: one a block under the core, one two under: water by a duct beside the loader;
- cooler: cryogel in by a duct under it, used cryogel out by its top; a column two cells off, made hot, is cooled;
- heater: coolant in, hot coolant out, while it is hot;
- ReaSim (`dialReasimBoilers`): an inlet beside a column's dummy fills its ReaSim water from a barrel, the column boils
  it, an outlet drains the steam into a barrel of super dense steam.

Usage: native_rbmk_t08.py HOST_FOLDER GUEST_FOLDER [reopen]
"""
import json
import re
import struct
import sys
import time
from pathlib import Path
from native_game import Game
from native_stations import fill_barrel, type_duct

HOST, GUEST = 'Host', 'Guest'
Y = 101
B1, B1_IN, B1_SRC, B1_OUT, B1_DST = (40, Y, 8), (40, Y - 1, 8), (41, Y - 1, 8), (40, Y + 4, 8), (41, Y + 4, 8)
B2 = (44, Y, 8)
B3, L3, B3_IN, B3_SRC = (48, Y, 8), (48, Y - 1, 8), (49, Y - 1, 8), (50, Y - 1, 8)
B4, L4, B4_IN, B4_SRC = (52, Y, 8), (52, Y - 2, 8), (53, Y - 2, 8), (54, Y - 2, 8)
COOLER, FAR = (40, Y, 14), (42, Y, 16)
C_IN, C_SRC, C_OUT, C_DST = (40, Y - 1, 14), (41, Y - 1, 14), (40, Y + 4, 14), (41, Y + 4, 14)
HEATER, H_IN, H_SRC, H_OUT, H_DST = (46, Y, 14), (46, Y - 1, 14), (47, Y - 1, 14), (46, Y + 4, 14), (47, Y + 4, 14)
RS, RS_DUMMY = (52, Y, 14), (52, Y + 1, 14)
INLET, I_IN, I_SRC = (53, Y + 1, 14), (54, Y + 1, 14), (55, Y + 1, 14)
OUTLET, O_OUT, O_DST = (51, Y, 14), (50, Y, 14), (49, Y, 14)
INVENTORY = (8, 104)
# GUIRBMKBoiler's compressor: (33, 21, 20 x 64).
COMPRESSOR = (43, 53)


def column(id_):
    return f'ntm:{id_}[facing=north,honey_level=0]'


def bits(v):
    return struct.unpack('<d', struct.pack('<Q', int(v)))[0]


def raw(host, p):
    return json.dumps(host.command(f'data get block {p[0]} {p[1]} {p[2]} ntm')).replace('\\\\"', '"').replace('\\"', '"')


def piped(host, p):
    """A boiler's, heater's or cooler's tanks as the server keeps them: {'input': (fluid, fill), 'output': ...}."""
    text = raw(host, p)
    out = {}
    for k in ('input', 'output'):
        m = re.search(r'"' + k + r'":\{"fluid":"(\w+)","fill":(-?\d+)', text)
        if m:
            out[k] = (m[1], int(m[2]))
    return out


def reasim(host, p):
    text = raw(host, p)
    w, s, h = re.search(r'"reasim_water":(-?\d+)', text), re.search(r'"reasim_steam":(-?\d+)', text), re.search(r'"heat":(\d+)', text)
    return {'water': int(w[1]) if w else None, 'steam': int(s[1]) if s else None, 'heat': bits(h[1]) if h else None}


def port(host, p):
    m = re.search(r'"tank":\{"fluid":"(\w+)","fill":(-?\d+)', raw(host, p))
    return (m[1], int(m[2])) if m else None


def barrel(host, p):
    m = re.search(r'"tank":\{(?:"fluid":"(\w*)",)?"mb":(\d+)', raw(host, p))
    return (m[1] or '', int(m[2])) if m else None


def fill(host, p):
    return (piped(host, p).get('input') or (None, None))[1]


class Proof:
    def __init__(self, host, guest):
        self.host, self.guest = host, guest

    def check(self, name, ok, actual):
        self.host.check(name, bool(ok), actual)

    def wait(self, read, predicate, timeout=40):
        deadline = time.monotonic() + timeout
        got = read()
        while time.monotonic() < deadline:
            got = read()
            if predicate(got):
                return got
            time.sleep(1)
        return got

    def heat(self, p, value, times=1, gap=1.0):
        for _ in range(times):
            self.host.command(f'ntmtest heat {p[0]} {p[1]} {p[2]} {value}')
            time.sleep(gap)

    def stand(self, cell):
        """The host on a stone put at `cell`, within reach of what is around it in the air."""
        self.host.command(f'setblock {cell[0]} {cell[1]} {cell[2]} minecraft:stone')
        self.host.command(f'tp {HOST} {cell[0] + .5} {cell[1] + 1} {cell[2] + .5}')
        time.sleep(1)

    def source(self, at, container):
        self.host.command(f'setblock {at[0]} {at[1]} {at[2]} ntm:barrel_steel[facing=south]')
        time.sleep(.5)
        self.stand((at[0], at[1], at[2] - 2))
        fill_barrel(self.host, at, 1, container)

    def sink(self, at):
        self.host.command(f'setblock {at[0]} {at[1]} {at[2]} ntm:barrel_steel[facing=south]')

    def duct(self, at, fluid):
        self.host.command(f'setblock {at[0]} {at[1]} {at[2]} ntm:fluid_duct_neo')
        time.sleep(.3)
        self.host.command(f'setblock {at[0] + 1} {at[1]} {at[2] - 1} minecraft:stone')
        type_duct(self.host, fluid, [at], (at[0] + 1, at[1] + 1, at[2] - 1))

    def setup(self):
        h = self.host
        h.ctl('wait-ready 120')
        h.close()
        h.command('forceload add 30 0 64 24')
        time.sleep(3)
        h.command('fill 36 97 2 62 108 22 minecraft:air')
        h.command('fill 36 96 2 62 96 22 minecraft:stone')
        for name in (HOST, GUEST):
            h.command(f'gamemode creative {name}')
            h.command(f'effect give {name} minecraft:resistance 1200 4 true')
        for p, b in ((B1, 'rbmk_boiler'), (B2, 'rbmk_boiler'), (B3, 'rbmk_boiler'), (B4, 'rbmk_boiler'), (COOLER, 'rbmk_cooler'),
                     (FAR, 'rbmk_blank'), (HEATER, 'rbmk_heater'), (RS, 'rbmk_blank')):
            h.command(f'setblock {p[0]} {p[1]} {p[2]} {column(b)}')
        # The ReaSim column's first dummy (the inlet beside it finds the core under it), the loaders, the inlet/outlet.
        h.command(f'setblock {RS_DUMMY[0]} {RS_DUMMY[1]} {RS_DUMMY[2]} ntm:dummy_rbmk')
        for p in (L3, L4):
            h.command(f'setblock {p[0]} {p[1]} {p[2]} ntm:rbmk_loader')
        h.command(f'setblock {INLET[0]} {INLET[1]} {INLET[2]} ntm:rbmk_steam_inlet')
        h.command(f'setblock {OUTLET[0]} {OUTLET[1]} {OUTLET[2]} ntm:rbmk_steam_outlet')
        time.sleep(2)
        for at, fluid in ((B1_IN, 'water'), (B1_OUT, 'steam'), (B3_IN, 'water'), (B4_IN, 'water'), (C_IN, 'perfluoromethyl_cold'),
                          (C_OUT, 'perfluoromethyl'), (H_IN, 'coolant'), (H_OUT, 'coolant_hot'), (I_IN, 'water'), (O_OUT, 'superhotsteam')):
            self.duct(at, fluid)
        for at in (B1_DST, C_DST, H_DST, O_DST):
            self.sink(at)
        h.command(f'clear {HOST}')

    def boiler(self):
        h = self.host
        self.source(B1_SRC, 'ntm:fluid_barrel_full__water')
        st = self.wait(lambda: piped(h, B1), lambda s: s.get('input', (0, 0))[1] == 10_000)
        src = barrel(h, B1_SRC)
        self.check('boiler_takes_water_below', st.get('input') == ('WATER', 10_000) and src == ('water', 6_000), [st, src])
        self.heat(B1, 300, 3, 2.0)
        time.sleep(15)
        st, src, dst = piped(h, B1), barrel(h, B1_SRC), barrel(h, B1_DST)
        used = 16_000 - src[1] - st['input'][1]
        self.check('steam_leaves_by_the_top', dst[0] == 'steam' and dst[1] > 0, [st, dst])
        self.check('a_mb_of_water_is_100_of_steam', used > 0 and used * 100 == st['output'][1] + dst[1], {'water_used': used, 'boiler': st, 'barrel': dst, 'source': src})
        self.host.evidence['boiler'] = {'tanks': st, 'sink': dst, 'source': src}

    def open_boiler(self, p):
        """The guest's screen of the boiler at `p`, from a stone in front of it (no floor at the columns' height); the
        compressor's place on the screen."""
        g, h = self.guest, self.host
        g.close()
        h.command(f'setblock {p[0]} {p[1] - 1} {p[2] - 3} minecraft:stone')
        h.command(f'tp {GUEST} {p[0] + .5} {p[1]} {p[2] - 2.5} 0 0')
        time.sleep(1.5)
        for _ in range(3):
            g.ctl(f'interact {p[0]} {p[1]} {p[2]} north')
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline and g.ctl('screen').get('screen') != 'container':
                time.sleep(.2)
            if g.ctl('screen').get('screen') == 'container':
                break
        time.sleep(.8)
        w = next(w for w in g.ctl('ui').get('widgets', []) if w.get('id') == 'slot-9')
        return w['bounds'][0] - INVENTORY[0] + COMPRESSOR[0], w['bounds'][1] - INVENTORY[1] + COMPRESSOR[1]

    def compressor(self):
        g, h = self.guest, self.host
        at = self.open_boiler(B2)
        before = piped(h, B2).get('output')
        g.ctl(f'click-at {at[0]} {at[1]}')
        after = self.wait(lambda: piped(h, B2).get('output'), lambda o: o and o[0] != 'STEAM', 10)
        g.ctl(f'click-at {at[0]} {at[1]}')
        again = self.wait(lambda: piped(h, B2).get('output'), lambda o: o and o[0] == 'SUPERHOTSTEAM', 10)
        g.close()
        self.check('the_guest_compresses', before == ('STEAM', 0) and after == ('HOTSTEAM', 0) and again == ('SUPERHOTSTEAM', 0), [before, after, again])
        # The amount (T08 review P3-2): the boiler that boiled, cold again with water in (heat 50 or less lets it), holds
        # 584 000 mB of steam; one press steps it to hot steam and `setTankType` empties the tank before the `/ 10`:
        # all of it is lost, the water kept.
        full = self.wait(lambda: reasim(h, B1), lambda c: c['heat'] is not None and c['heat'] <= 50, 60)
        before = piped(h, B1)
        at = self.open_boiler(B1)
        g.ctl(f'click-at {at[0]} {at[1]}')
        after = self.wait(lambda: piped(h, B1), lambda s: s.get('output', ('STEAM', 0))[0] != 'STEAM', 10)
        g.close()
        self.check('a_press_loses_all_the_steam', full['heat'] <= 50 and before.get('output') == ('STEAM', 584_000)
                   and after.get('output') == ('HOTSTEAM', 0) and after.get('input') == before.get('input'), [full, before, after])

    def loaders(self):
        h = self.host
        for name, b, src in (('close', B3, B3_SRC), ('classic', B4, B4_SRC)):
            self.source(src, 'ntm:fluid_barrel_full__water')
            st = self.wait(lambda: piped(h, b), lambda s: s.get('input', (0, 0))[1] == 10_000)
            left = barrel(h, src)
            self.check(f'water_by_the_{name}_loader', st.get('input') == ('WATER', 10_000) and left == ('water', 6_000), [st, left])

    def cooler(self):
        h = self.host
        self.source(C_SRC, 'ntm:fluid_barrel_full__perfluoromethyl_cold')
        self.wait(lambda: piped(h, COOLER), lambda s: s.get('input', (0, 0))[1] > 0)
        self.heat(FAR, 900)
        time.sleep(1.5)
        far = reasim(h, FAR)
        self.check('the_cooler_cools_its_5x5', far['heat'] is not None and far['heat'] < 300, far)
        # All the cryogel used: 50 mB cold in, 50 mB used out, a tick.
        done = self.wait(lambda: (barrel(h, C_SRC), piped(h, COOLER), barrel(h, C_DST)),
                         lambda v: v[0][1] == 0 and v[1]['input'][1] < 50, 60)
        src, st, dst = done
        total = src[1] + st['input'][1] + st['output'][1] + dst[1]
        self.check('cryogel_in_is_cryogel_out', total == 16_000 and dst[0] == 'perfluoromethyl' and dst[1] > 0, {'source': src, 'cooler': st, 'sink': dst})
        self.host.evidence['cooler'] = {'tanks': st, 'sink': dst}

    def heater(self):
        h = self.host
        self.source(H_SRC, 'ntm:fluid_barrel_full__coolant')
        self.wait(lambda: piped(h, HEATER), lambda s: s.get('input', (0, 0))[1] > 0)
        self.heat(HEATER, 700, 4, 2.0)
        time.sleep(12)
        src, st, dst = barrel(h, H_SRC), piped(h, HEATER), barrel(h, H_DST)
        total = src[1] + st['input'][1] + st['output'][1] + dst[1]
        self.check('the_heater_heats_coolant', dst[0] == 'coolant_hot' and dst[1] > 0 and total == 16_000, {'source': src, 'heater': st, 'sink': dst})
        self.host.evidence['heater'] = {'tanks': st, 'sink': dst}

    def reasim_ports(self):
        h = self.host
        h.command('ntmrbmk dial dialReasimBoilers true')
        self.source(I_SRC, 'ntm:fluid_barrel_full__water')
        col = self.wait(lambda: reasim(h, RS), lambda c: c['water'] and c['water'] > 0)
        self.check('the_inlet_fills_the_column_beside_its_dummy', col['water'] and col['water'] > 0, [col, port(h, INLET)])
        self.heat(RS, 600, 5, 2.0)
        time.sleep(15)
        src, inl, c, out, dst = barrel(h, I_SRC), port(h, INLET), reasim(h, RS), port(h, OUTLET), barrel(h, O_DST)
        total = src[1] + inl[1] + c['water'] + c['steam'] + out[1] + dst[1]
        self.check('the_outlet_drains_the_steam', dst == ('superhotsteam', dst[1]) and dst[1] > 0 and total == 16_000,
                   {'source': src, 'inlet': inl, 'column': c, 'outlet': out, 'sink': dst})
        self.host.evidence['reasim'] = {'inlet': inl, 'column': c, 'outlet': out, 'sink': dst}

    def kept(self):
        h = self.host
        return {'b1': piped(h, B1), 'b2': piped(h, B2), 'cooler': piped(h, COOLER), 'heater': piped(h, HEATER),
                'inlet': port(h, INLET), 'outlet': port(h, OUTLET), 'rs': {k: v for k, v in reasim(h, RS).items() if k != 'heat'},
                'barrels': {str(p): barrel(h, p) for p in (B1_DST, C_DST, H_DST, O_DST)}}

    def phase(self, name, step):
        """A phase that breaks is a failed check, and the next ones still run."""
        self.host.evidence['phase'] = name
        try:
            step()
        except AssertionError:
            pass  # a failed check, already in the evidence
        except Exception as e:  # noqa: BLE001 - the proof records it
            self.host.evidence['checks'][f'{name}:ran'] = {'ok': False, 'actual': repr(e)}
            self.host.save()

    def run(self):
        self.setup()
        for name, step in (('boiler', self.boiler), ('compressor', self.compressor), ('loaders', self.loaders),
                           ('cooler', self.cooler), ('heater', self.heater), ('reasim', self.reasim_ports)):
            self.phase(name, step)
        # Everything at rest: the same twice in a row, then saved.
        a = self.kept()
        time.sleep(5)
        b = self.kept()
        self.check('at_rest_before_the_save', a == b, [a, b])
        self.host.evidence['kept'] = b
        self.host.ctl('save')
        failed = [k for k, v in self.host.evidence['checks'].items() if not v['ok']]
        if failed:
            raise AssertionError(f'failed: {failed}')
        self.host.evidence['ok'] = True
        self.host.save()

    def reopen(self, before):
        self.host.ctl('wait-ready 120')
        self.host.evidence['phase'] = 'reopen'
        got = self.wait(self.kept, lambda k: k['b1'].get('input') is not None, 60)
        self.check('tanks_survive_a_world_reopen', json.loads(json.dumps(got)) == before, {'before': before, 'after': got})
        self.host.evidence['ok'] = True
        self.host.save()


if __name__ == '__main__':
    if sys.argv[3:] == ['reopen']:
        before = json.loads((Path(sys.argv[1]) / 'native-rbmk-t08-host.json').read_text())['kept']
        Proof(Game(Path(sys.argv[1]), 'rbmk-t08-reopen'), None).reopen(before)
        sys.exit(0)
    host, guest = Game(Path(sys.argv[1]), 'rbmk-t08-host'), Game(Path(sys.argv[2]), 'rbmk-t08-guest')
    Proof(host, guest).run()
    guest.evidence['ok'] = True
    guest.save()

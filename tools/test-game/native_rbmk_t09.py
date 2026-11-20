#!/usr/bin/env python3
"""T09 proof (`rbmk-plano` T09): the RBMK outgasser fed by a funnel, irradiated by a Po-210/Be rod in the channel
beside it, its gas out by a duct to a barrel and its solid taken by the LAN guest from its screen; then the world
reopened.

- a chest over a funnel over the outgasser's top (placed by hand, its dummies standing): a stone (no recipe: it never
  goes in), two coal tars (`oil_tar` meta 2: 100 mB of coal oil each) and two gold ingots (one Au-198 ingot each);
- `dialOutgasserSpeedMod` 10, so the rod's flux gets through 10 000 in seconds;
- every tar and ingot counted where it is (chest, funnel, the two slots) and the products (the gas tank, the barrel,
  the output slot, the guest's inventory): nothing made twice, nothing lost.

Usage: native_rbmk_t09.py HOST_FOLDER GUEST_FOLDER [reopen]
"""
import json
import re
import struct
import sys
import time
from pathlib import Path
from native_game import Game
from native_stations import type_duct

HOST, GUEST = 'Host', 'Guest'
Y = 101
ROD, OUT = (39, Y, 8), (40, Y, 8)
FUNNEL, CHEST = (40, Y + 4, 8), (40, Y + 5, 8)
DUCT, BARREL = (40, Y - 1, 8), (41, Y - 1, 8)
TAR, GOLD, AU198 = 'ntm:oil_tar__coal', 'minecraft:gold_ingot', 'ntm:ingot_au198'


def column(id_):
    return f'ntm:{id_}[facing=north,honey_level=0]'


def bits(v):
    return struct.unpack('<d', struct.pack('<Q', int(v)))[0]


def raw(h, p, path='ntm'):
    return json.dumps(h.command(f'data get block {p[0]} {p[1]} {p[2]} {path}')).replace('\\\\"', '"').replace('\\"', '"')


def outgasser(h):
    text = raw(h, OUT)
    gas = re.search(r'"outgasser":\{"progress":(\d+),"gas":\{"fluid":"(\w+)","fill":(\d+)', text)
    return {'progress': bits(gas[1]) if gas else None, 'gas': (gas[2], int(gas[3])) if gas else None}


def items(h, p):
    """Every stack the block's inventory holds: {id: count}."""
    text = ' '.join(h.command(f'data get block {p[0]} {p[1]} {p[2]} Items').get('chat', []))
    out = {}
    # Each stack's top-level compound (its components may nest braces), its `id` and `count` in whichever order the
    # game writes them.
    depth, start = 0, None
    for i, ch in enumerate(text):
        if ch == '{':
            depth += 1
            if depth == 1:
                start = i
        elif ch == '}' and depth:
            depth -= 1
            if depth == 0 and start is not None:
                top = re.sub(r'\{.*\}', '', text[start + 1:i])
                id_, n = re.search(r'\bid: ?"([^"]+)"', top), re.search(r'\bcount: ?(\d+)', top)
                if id_ and n:
                    out[id_[1]] = out.get(id_[1], 0) + int(n[1])
    return out


def barrel(h):
    m = re.search(r'"tank":\{(?:"fluid":"(\w*)",)?"mb":(\d+)', raw(h, BARREL))
    return (m[1] or '', int(m[2])) if m else None


def held_by(h, name, item):
    """How many `item` the player holds (`clear` counting, taking none)."""
    text = ' '.join(h.command(f'clear {name} {item} 0').get('chat', []))
    m = re.search(r'(\d+)', text)
    return int(m[1]) if m and 'No items' not in text else 0


class Proof:
    def __init__(self, host, guest):
        self.host, self.guest = host, guest

    def check(self, name, ok, actual):
        self.host.check(name, bool(ok), actual)

    def wait(self, read, predicate, timeout=90):
        deadline = time.monotonic() + timeout
        got = read()
        while time.monotonic() < deadline and not predicate(got):
            time.sleep(1)
            got = read()
        return got

    def count(self):
        h = self.host
        chest, funnel, slots = items(h, CHEST), items(h, FUNNEL), items(h, OUT)
        o, b = outgasser(h), barrel(h)
        gas = (o['gas'][1] if o['gas'] and o['gas'][0] == 'COALOIL' else 0) + (b[1] if b and b[0] == 'coaloil' else 0)
        au = slots.get(AU198, 0) + held_by(h, GUEST, AU198)
        tar = chest.get(TAR, 0) + funnel.get(TAR, 0) + slots.get(TAR, 0)
        gold = chest.get(GOLD, 0) + funnel.get(GOLD, 0) + slots.get(GOLD, 0)
        return {'chest': chest, 'funnel': funnel, 'slots': slots, 'outgasser': o, 'barrel': b, 'tar': tar, 'coal_oil': gas,
                'gold': gold, 'au198': au, 'stone_in_slots': slots.get('minecraft:stone', 0)}

    def setup(self):
        h = self.host
        h.ctl('wait-ready 120')
        h.close()
        h.command('forceload add 30 0 64 24')
        time.sleep(3)
        h.command('fill 34 99 2 48 110 14 minecraft:air')
        h.command('fill 34 100 2 48 100 14 minecraft:stone')
        for name in (HOST, GUEST):
            h.command(f'gamemode creative {name}')
            h.command(f'effect give {name} minecraft:resistance 1200 4 true')
        h.command('ntmrbmk dial dialOutgasserSpeedMod 10')
        # The outgasser by hand (its dummies stand: the funnel's port is its top), the channel by command.
        h.command('clear @s')
        h.command('give @s ntm:rbmk_outgasser 1')
        h.command(f'tp @s {OUT[0] + .5} {Y} {OUT[2] + 3.5} 180 45')
        time.sleep(.5)
        self.evidence_place = h.ctl(f'place {OUT[0]} {OUT[1]} {OUT[2]} --item ntm:rbmk_outgasser')
        time.sleep(1)
        h.command(f'setblock {ROD[0]} {ROD[1]} {ROD[2]} {column("rbmk_rod")}')
        time.sleep(1.5)
        h.command('give @s ntm:rbmk_fuel_po210be 1')
        h.command(f'tp @s {ROD[0] + .5} {Y} {ROD[2] - 2.5} 0 0')
        time.sleep(.6)
        h.ctl('select ntm:rbmk_fuel_po210be')
        h.ctl(f'interact {ROD[0]} {ROD[1]} {ROD[2]} north')
        time.sleep(1)
        h.command('clear @s')
        # The gas: a duct under the core to a barrel.
        h.command(f'setblock {DUCT[0]} {DUCT[1]} {DUCT[2]} ntm:fluid_duct_neo')
        h.command(f'setblock {BARREL[0]} {BARREL[1]} {BARREL[2]} ntm:barrel_steel[facing=south]')
        time.sleep(1)
        # The duct is boxed in by the floor and the column: its south face opened to the identifier.
        h.command(f'setblock {DUCT[0]} {DUCT[1]} {DUCT[2] - 1} minecraft:air')
        type_duct(h, 'coaloil', [DUCT], (DUCT[0], Y, DUCT[2] - 3))
        h.command('clear @s')
        # The chest and the funnel last: from now on the funnel feeds.
        h.command(f'setblock {CHEST[0]} {CHEST[1]} {CHEST[2]} minecraft:chest')
        h.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.0 with minecraft:stone 1')
        h.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.1 with {TAR} 2')
        h.command(f'item replace block {CHEST[0]} {CHEST[1]} {CHEST[2]} container.2 with {GOLD} 2')
        h.command(f'setblock {FUNNEL[0]} {FUNNEL[1]} {FUNNEL[2]} minecraft:hopper[facing=down]')
        time.sleep(1)

    def run_it(self):
        h = self.host
        start = self.count()
        self.host.evidence['start'] = start
        self.check('placed_by_hand', 'rbmk_outgasser' in raw(h, OUT, ''), self.evidence_place)
        # Both tars, then both ingots, through.
        done = self.wait(self.count, lambda c: c['coal_oil'] == 200 and c['au198'] == 2 and c['gold'] == 0, 240)
        self.host.evidence['done'] = done
        self.check('tar_makes_coal_oil', done['coal_oil'] == 200 and done['tar'] == 0, done)
        self.check('gold_makes_au198', done['au198'] == 2 and done['gold'] == 0 and done['slots'].get(AU198) == 2, done)
        self.check('the_stone_never_goes_in', done['stone_in_slots'] == 0 and done['chest'].get('minecraft:stone', 0) + done['funnel'].get('minecraft:stone', 0) == 1, done)
        b = self.wait(lambda: barrel(h), lambda b: b and b == ('coaloil', 200), 30)
        self.check('the_gas_leaves_by_the_duct', b == ('coaloil', 200), [b, outgasser(h)])
        # The guest takes the Au-198 out of the output slot (its screen's slot 1: `click-slot 301`, then an empty slot
        # of its inventory).
        g = self.guest
        g.close()
        h.command(f'setblock {OUT[0]} {Y - 1} {OUT[2] + 3} minecraft:stone')
        h.command(f'tp {GUEST} {OUT[0] + .5} {Y} {OUT[2] + 2.5} 180 0')
        time.sleep(1.5)
        for _ in range(3):
            g.ctl(f'interact {OUT[0]} {OUT[1]} {OUT[2]} south')
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline and g.ctl('screen').get('screen') != 'container':
                time.sleep(.2)
            if g.ctl('screen').get('screen') == 'container':
                break
        time.sleep(.8)
        g.ctl('click-slot 301')
        time.sleep(.5)
        g.ctl('click-slot 9')
        time.sleep(1)
        g.close()
        after = self.wait(self.count, lambda c: c['slots'].get(AU198, 0) == 0, 20)
        self.check('the_guest_takes_the_product', held_by(h, GUEST, AU198) == 2 and after['au198'] == 2, after)
        a = self.count()
        time.sleep(4)
        b = self.count()
        self.check('at_rest_before_the_save', a == b, [a, b])
        self.host.evidence['kept'] = {'outgasser': b['outgasser'], 'barrel': b['barrel'], 'slots': b['slots'], 'chest': b['chest'],
                                      'funnel': b['funnel']}
        h.ctl('save')
        failed = [k for k, v in self.host.evidence['checks'].items() if not v['ok']]
        if failed:
            raise AssertionError(f'failed: {failed}')

    def reopen(self, before):
        self.host.ctl('wait-ready 120')
        self.host.evidence['phase'] = 'reopen'
        got = self.wait(lambda: {'outgasser': outgasser(self.host), 'barrel': barrel(self.host), 'slots': items(self.host, OUT),
                                 'chest': items(self.host, CHEST), 'funnel': items(self.host, FUNNEL)},
                        lambda k: k['outgasser']['gas'] is not None, 60)
        self.check('state_survives_a_world_reopen', json.loads(json.dumps(got)) == before, {'before': before, 'after': got})
        self.host.evidence['ok'] = True
        self.host.save()

    def run(self):
        self.setup()
        self.host.evidence['phase'] = 'outgasser'
        self.run_it()
        self.host.evidence['ok'] = True
        self.host.save()


if __name__ == '__main__':
    if sys.argv[3:] == ['reopen']:
        before = json.loads((Path(sys.argv[1]) / 'native-rbmk-t09-host.json').read_text())['kept']
        Proof(Game(Path(sys.argv[1]), 'rbmk-t09-reopen'), None).reopen(before)
        sys.exit(0)
    host, guest = Game(Path(sys.argv[1]), 'rbmk-t09-host'), Game(Path(sys.argv[2]), 'rbmk-t09-guest')
    Proof(host, guest).run()
    guest.evidence['ok'] = True
    guest.save()

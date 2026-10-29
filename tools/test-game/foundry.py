#!/usr/bin/env python3
"""Aceitação da fundição no motor ntm-visual (docs/maquinas.md, "Fundição").

Uso: python3 tools/test-game/foundry.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  fundir   (etapas 1 e 3) uma firebox com carvão, o cadinho em cima (posto
           agachado), um molde de lingote onde cai o resto; 3 lingotes de
           ferro no cadinho derretem, caem no molde e saem lingotes, que o
           jogador retira;
  canais   (etapa 2) no lugar do molde, canal → canal → tanque → saída, e um
           molde num buraco sob a saída; 2 lingotes de ferro chegam ao molde;
  voltar   depois de reabrir o mundo: o calor do cadinho e o metal do tanque.
Grava <pasta>/foundry-evidence.json e capturas foundry-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['fundir']
CTL = ['/home/sacer/Workspace/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'foundry-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
import os
X, Z = int(os.environ.get('FOUNDRY_X', 3560)), -1398
DIRS = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}


def ctl(*args, check=True):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True, timeout=300).stdout.strip()
    try:
        j = json.loads(out.splitlines()[-1]) if out else {}
    except ValueError:
        j = {'raw': out}
    if check and isinstance(j, dict) and j.get('error'):
        raise SystemExit(f'{args}: {j["error"]}')
    return j


def screen():
    return ctl('screen')


def machine_slots(s=None):
    """Os slots da máquina (300+) com item: (slot, item, quantidade)."""
    s = s or screen()
    return [(x['slot'], x['item'], x['count']) for x in s.get('slots', []) if x.get('item') and x['slot'] >= 300]


def data(s=None):
    return (s or screen()).get('data') or (s or screen()).get('values')


def state(x, y, z):
    return ctl('block', x, y, z).get('state', '')


def facing(st):
    for d in DIRS:
        if f'facing={d}' in st:
            return d
    raise SystemExit(f'sem facing: {st}')


def find(block, y0, y1):
    """A multiblock's core (the engine centres it off the clicked spot)."""
    for b in ctl('blocks', X - 4, y0, Z - 4, X + 4, y1, Z + 8).get('blocks', []):
        if b[3].startswith(block + '['):
            return b[:3], b[3]
    raise SystemExit(f'{block} não achado')


def floor_y():
    p = game / f'foundry-y-{X}'
    if p.exists():
        return int(p.read_text())
    ctl('tp', X + 0.5, 120, Z + 6.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


def place(x, y, z, item, sneak=False):
    if state(x, y, z).startswith(item + '['):
        return state(x, y, z)
    ctl('give', item, 1)
    time.sleep(0.5)
    if sneak:
        ctl('key', 'shift', 'down')
        time.sleep(0.3)
    ctl('place', x, y, z, '--item', item)
    if sneak:
        ctl('key', 'shift', 'up')
    time.sleep(1.5)
    ctl('close', check=False)
    return state(x, y, z)


def shift_in(x, y, z, items, stand=None):
    """Abre a máquina e põe `items` (item, n) por shift-clique da hotbar."""
    if stand:
        ctl('tp', stand[0] + 0.5, stand[1], stand[2] + 0.5)
    for item, n in items:
        ctl('give', item, n)
    time.sleep(1)
    ctl('interact', x, y, z)
    time.sleep(2)
    hotbar = {s['item']: s['slot'] for s in screen()['slots'] if s.get('item') and s['slot'] < 9}
    for item, _ in items:
        ctl('click-slot', hotbar[item], 'shift')
        time.sleep(0.7)
    got = machine_slots()
    ctl('close', check=False)
    time.sleep(0.5)
    return got


def take_casts(x, y, z, stand):
    """Retira o lingote do molde (slot 1 da máquina) de perto: quantos saíram."""
    ctl('tp', stand[0] + 0.5, stand[1], stand[2] + 0.5)
    ctl('interact', x, y, z)
    time.sleep(1.5)
    out = [s for s in machine_slots() if s[0] == 301]
    for s in out:
        ctl('click-slot', s[0], 'shift')
        time.sleep(0.5)
    ctl('close', check=False)
    time.sleep(0.3)
    return sum(s[2] for s in out)


def build_furnace(y):
    ctl('fill', X - 10, y, Z - 4, X + 4, y + 6, Z + 8, 'minecraft:air')
    ctl('fill', X - 10, y - 2, Z - 4, X + 4, y - 1, Z + 8, 'minecraft:stone')
    ctl('tp', X + 3.5, y, Z + 0.5)
    ctl('look-at', 'block', X, y, Z, check=False)
    place(X, y, Z, 'ntm:heater_firebox')
    fire = find('ntm:heater_firebox', y, y)
    # Due east of the firebox's core, clicking the block east of it: the
    # engine centres the crucible one block back, over the firebox.
    (fx, _, fz) = fire[0]
    ctl('tp', fx + 4.5, y, fz + 0.5)
    ctl('look-at', 'block', fx + 1, y, fz, check=False)
    place(fx + 1, y + 1, fz, 'ntm:machine_crucible', sneak=True)
    core = find('ntm:machine_crucible', y + 1, y + 1)
    if (core[0][0], core[0][2]) != (fx, fz):
        raise SystemExit(f'o cadinho não ficou sobre a firebox: {core} {fire}')
    return fire, core


def waste(core):
    """Where the waste stream falls: 2 blocks behind the core's facing."""
    dx, dz = DIRS[facing(core[1])]
    return core[0][0] - 2 * dx, core[0][2] - 2 * dz, (dx, dz)


def heat():
    (cx, cy, cz), _ = find('ntm:machine_crucible', floor_y() + 1, floor_y() + 1)
    ctl('tp', cx + 3.5, cy - 1, cz + 0.5)
    ctl('interact', cx + 1, cy, cz)
    time.sleep(1.5)
    s = screen()
    ctl('close', check=False)
    return s.get('data') or s.get('values') or s.get('title')


def fundir():
    y = floor_y()
    fire, core = build_furnace(y)
    wx, wz, (dx, dz) = waste(core)
    # Beside the mold, off the furnace's line.
    stand = (wx + 3 * dz, y, wz + 3 * dx)
    ctl('tp', stand[0] + 0.5, y, stand[2] + 0.5)
    mold = place(wx, y, wz, 'ntm:foundry_mold')
    put_mold = shift_in(wx, y, wz, [('ntm:mold__ingot', 1)])
    # The cores are inside their multiblocks: through the east dummy.
    (fx, _, fz) = fire[0]
    east = (fx + 3, y, fz)
    put_fire = shift_in(fx + 1, y, fz, [('minecraft:coal', 8)], east)
    put_core = shift_in(fx + 1, y + 1, fz, [('minecraft:iron_ingot', 3)], east)
    t0 = time.time()
    taken, shots = 0, 0
    while taken < 3:
        if time.time() - t0 > 240:
            raise SystemExit(f'não terminou: {taken} lingotes')
        time.sleep(4)
        if shots < 3 and time.time() - t0 > 30 + 15 * shots:
            ctl('tp', stand[0] + 0.5, y + 2, stand[2] + 0.5)
            ctl('look-at', 'block', fx + 1, y + 1, fz, check=False)
            time.sleep(1)
            ctl('screenshot', game / f'foundry-fundir-{shots}.png')
            shots += 1
        taken += take_casts(wx, y, wz, stand)
    seconds = time.time() - t0
    counts = ctl('inventory').get('counts', {})
    evidence['fundir'] = {'firebox': fire, 'cadinho': core, 'molde': mold, 'resto_cai_em': [wx, y, wz],
                          'postos': {'molde': put_mold, 'firebox': put_fire, 'cadinho': put_core},
                          'lingotes_retirados': taken, 'segundos': round(seconds, 1),
                          'ferro_no_inventario': counts.get('minecraft:iron_ingot', 0)}
    print('fundir', evidence['fundir'])


def canais():
    y = floor_y()
    core = find('ntm:machine_crucible', y + 1, y + 1)
    _, _, (dx, dz) = waste(core)
    (cx, _, cz) = core[0]
    # Away from the core, from where the waste falls: channel, channel,
    # tank, outlet (facing away), and a mold in a hole under the outlet.
    at = lambda k: (cx - (2 + k) * dx, cz - (2 + k) * dz)
    one = lambda p, yy, b: ctl('fill', p[0], yy, p[1], p[0], yy, p[1], b)
    if not state(at(2)[0], y, at(2)[1]).startswith('ntm:foundry_tank['):
        # The mold of `fundir` comes out as a player breaks it (the mod
        # hears of it), then room past the furnace's box.
        if state(at(0)[0], y, at(0)[1]).startswith('ntm:foundry_mold['):
            ctl('tp', at(0)[0] + 3 * dz + 0.5, y, at(0)[1] + 3 * dx + 0.5)
            ctl('break', at(0)[0], y, at(0)[1])
            time.sleep(2)
        lo = (min(at(6)[0], at(2)[0]), min(at(6)[1], at(2)[1]) - 3)
        hi = (max(at(6)[0], at(2)[0]), max(at(6)[1], at(2)[1]) + 3)
        ctl('fill', lo[0], y, lo[1], hi[0], y + 6, hi[1], 'minecraft:air')
        ctl('fill', lo[0], y - 2, lo[1], hi[0], y - 1, hi[1], 'minecraft:stone')
        one(at(3), y - 1, 'minecraft:air')
        one(at(4), y - 1, 'minecraft:air')  # the pit's open side, to reach the mold
    blocks = {}
    for k, item in ((0, 'ntm:foundry_channel'), (1, 'ntm:foundry_channel'), (2, 'ntm:foundry_tank')):
        blocks[item + str(k)] = place(at(k)[0], y, at(k)[1], item)
    # The outlet faces away from the tank: the player stands past it,
    # looking back at the crucible.
    ox, oz = at(3)
    far = (ox - 2 * dx, y, oz - 2 * dz)
    ctl('tp', far[0] + 0.5, y, far[2] + 0.5)
    ctl('look-at', 'block', cx, y, cz, check=False)
    blocks['outlet'] = place(ox, y, oz, 'ntm:foundry_outlet')
    blocks['mold'] = place(ox, y - 1, oz, 'ntm:foundry_mold')
    shift_in(ox, y - 1, oz, [('ntm:mold__ingot', 1)])
    put_core = shift_in(cx + 1, y + 1, cz, [('minecraft:iron_ingot', 2)], (cx + 3, y, cz))
    t0 = time.time()
    taken, shot = 0, False
    while taken < 2:
        if time.time() - t0 > 240:
            raise SystemExit(f'não terminou: {taken} lingotes; {blocks}')
        time.sleep(4)
        if not shot and time.time() - t0 > 40:
            ctl('tp', at(1)[0] + 4 * dz + 0.5, y + 3, at(1)[1] + 4 * dx + 0.5)
            ctl('look-at', 'block', at(1)[0], y, at(1)[1], check=False)
            time.sleep(1)
            ctl('screenshot', game / 'foundry-canais.png')
            shot = True
        taken += take_casts(ox, y - 1, oz, far)
    evidence['canais'] = {'blocos': blocks, 'postos': put_core, 'lingotes_retirados': taken, 'segundos': round(time.time() - t0, 1)}
    print('canais', evidence['canais'])


def voltar():
    y = floor_y()
    ctl('tp', X + 3.5, y, Z + 3.5)
    time.sleep(2)
    evidence['voltar'] = {'cadinho': heat()}
    print('voltar', evidence['voltar'])


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

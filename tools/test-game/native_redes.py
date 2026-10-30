#!/usr/bin/env python3
"""Aceitação das redes nativas da L3 (docs/nativo.md): energia por cabos e
fluido por dutos, no jogo de tools/build-native.sh.

Uso: python3 tools/test-game/native_redes.py <pasta do jogo> <fase> <horizon-ctl>
O jogo aberto num mundo criativo, com --control-socket <pasta>/control.sock.
Fases:
  energia  uma bateria (modo buffer) enchida pela bateria criativa de um funil,
           três cabos red_cable e uma fornalha elétrica SEM bateria, com
           minério de um funil e um baú embaixo: a fornalha funde só com a
           energia que chega pela rede;
  fluido   dois barris ligados por três dutos: o duto ganha o tipo água com o
           identificador de fluido na mão; o primeiro barril recebe água de
           baldes por um funil e vai para saída pela tela; a água chega ao
           segundo.
Grava <pasta>/native-redes.json.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
phase = sys.argv[2]
CTL = [sys.argv[3], '--socket', str(game / 'control.sock')]
out_path = game / 'native-redes.json'
ev = json.loads(out_path.read_text()) if out_path.exists() else {}
Y = 100


def ctl(*args):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True, timeout=120).stdout.strip()
    try:
        return json.loads(out.splitlines()[-1]) if out else {}
    except ValueError:
        return {'raw': out}


def cmd(c):
    out = ' / '.join(ctl('command', c).get('chat', []))
    time.sleep(0.3)
    return out


def ground(x0, z0, x1, z1):
    """Loaded, cleared, a stone floor; the player south of it."""
    cmd(f'forceload add {x0} {z0} {x1} {z1}')
    time.sleep(2)
    cmd(f'fill {x0} {Y - 4} {z0} {x1} {Y + 3} {z1} minecraft:air')
    cmd(f'fill {x0} {Y - 4} {z0} {x1} {Y - 4} {z1} minecraft:stone')
    cmd(f'tp @s {(x0 + x1) // 2 + 0.5} {Y} {z1 + 0.5}')
    time.sleep(3)


ctl('wait-ready', 120)
if phase == 'energia':
    x, z = 90, 8
    bat, fur = (x, Y, z), (x + 4, Y, z)
    ground(x - 2, z - 2, x + 6, z + 4)
    # The furnace's result falls through a hopper into a chest.
    cmd(f'setblock {fur[0]} {Y - 3} {z} minecraft:chest')
    cmd(f'setblock {fur[0]} {Y - 2} {z} minecraft:hopper[facing=down]')
    cmd(f'setblock {fur[0]} {Y - 1} {z} minecraft:hopper[facing=down]')
    cmd(f'setblock {bat[0]} {Y - 1} {z} minecraft:stone')
    cmd(f'setblock {x + 1} {Y - 1} {z} minecraft:stone')
    cmd(f'setblock {x + 2} {Y - 1} {z} minecraft:stone')
    cmd(f'setblock {x + 3} {Y - 1} {z} minecraft:stone')
    ev['bateria_posta'] = cmd(f'setblock {bat[0]} {Y} {z} ntm:machine_battery[facing=south]')
    for cx in (x + 1, x + 2, x + 3):
        cmd(f'setblock {cx} {Y} {z} ntm:red_cable')
    ev['fornalha_posta'] = cmd(f'setblock {fur[0]} {Y} {z} ntm:machine_electric_furnace_off[facing=south]')
    for p in (bat, fur):
        cmd(f'setblock {p[0]} {Y + 1} {z} minecraft:hopper[facing=down]')
    cmd(f'item replace block {bat[0]} {Y + 1} {z} container.0 with ntm:battery_creative 1')
    cmd(f'item replace block {fur[0]} {Y + 1} {z} container.0 with minecraft:iron_ore 4')
    time.sleep(25)
    ev['energia'] = {
        'bateria': cmd(f'data get block {bat[0]} {Y} {z}'),
        'fornalha': cmd(f'data get block {fur[0]} {Y} {z}'),
        'bau': cmd(f'data get block {fur[0]} {Y - 3} {z} Items'),
    }
elif phase == 'fluido':
    x, z = 100, 8
    a, b = (x, Y, z), (x + 4, Y, z)
    ground(x - 2, z - 2, x + 6, z + 4)
    for cx in range(x, x + 5):
        cmd(f'setblock {cx} {Y - 1} {z} minecraft:stone')
    ev['barris'] = [cmd(f'setblock {p[0]} {Y} {z} ntm:barrel_steel[facing=south]') for p in (a, b)]
    for cx in (x + 1, x + 2, x + 3):
        cmd(f'setblock {cx} {Y} {z} ntm:fluid_duct_neo')
    cmd(f'tp @s {x + 2.5} {Y} {z + 2.5} facing {x + 2.5} {Y} {z + 0.5}')
    time.sleep(2)
    cmd('clear @s')
    cmd('give @s ntm:fluid_identifier_multi[minecraft:custom_data={ntm_fluid:"water"}] 1')
    time.sleep(1)
    ctl('select', 'ntm:fluid_identifier_multi')
    for cx in (x + 1, x + 2, x + 3):
        ctl('interact', cx, Y, z)
        time.sleep(0.5)
    ev['dutos'] = [cmd(f'data get block {cx} {Y} {z} ntm') for cx in (x + 1, x + 2, x + 3)]
    # The first barrel: two buckets of water in through its screen (slot 2,
    # the full container in), then its mode to output (button 0 twice:
    # input, buffer, output).
    cmd('clear @s')
    cmd('give @s minecraft:water_bucket 1')
    time.sleep(1)
    ctl('interact', a[0], Y, z)
    time.sleep(2)
    sc = ctl('screen')
    ev['tela_barril'] = {'tela': sc.get('screen'), 'botoes': len(sc.get('buttons', [])), 'slots_maquina': [s['slot'] for s in sc.get('slots', []) if s['slot'] >= 300]}
    hot = [s['slot'] for s in sc.get('slots', []) if s.get('item') == 'minecraft:water_bucket' and s['slot'] < 36]
    if hot:
        ctl('click-slot', hot[0])
        time.sleep(0.4)
        ctl('click-slot', 302)
        time.sleep(1)
    # The native screen's buttons click by GUI pixel: its top left is the
    # hotbar's first slot less its place in the machine file (inventory x,
    # inventory y + 58); the mode button is at (151, 35), 18 square.
    slot0 = next(w for w in ctl('ui').get('widgets', []) if w.get('id') == 'slot-0')['bounds']
    ox, oy = slot0[0] - 8, slot0[1] - (84 + 58)
    for _ in range(2):
        ctl('click-at', ox + 151 + 9, oy + 35 + 9)
        time.sleep(0.8)
    ctl('screenshot', game / 'native-redes-barril.png')
    ctl('close')
    time.sleep(10)
    ev['fluido'] = {'a': cmd(f'data get block {a[0]} {Y} {z} ntm'), 'b': cmd(f'data get block {b[0]} {Y} {z} ntm')}
out_path.write_text(json.dumps(ev, indent=1, ensure_ascii=False))
print(json.dumps(ev, ensure_ascii=False))

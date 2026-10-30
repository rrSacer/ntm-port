#!/usr/bin/env python3
"""Aceitação da L3 da migração (docs/nativo.md): máquinas rodadas pelo
servidor do ntm-core no server_tick, com o estado no block entity.

Uso: python3 tools/test-game/native_l3.py <pasta do jogo> <fase> <horizon-ctl>
O jogo de tools/build-native.sh aberto num mundo criativo, com
--control-socket <pasta>/control.sock. Fases:
  montar    uma firebox com carvão e uma prensa a carvão com a matriz de
            placa e lingotes de ferro, postas por /setblock e alimentadas por
            funis (as regras dos slots levam cada item ao seu); espera
            e lê o calor, o visual aceso e as placas;
  conferir  depois de fechar e reabrir: o servidor adota as máquinas do
            estado salvo, e a prensa continua.
Grava <pasta>/native-l3.json.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
phase = sys.argv[2]
CTL = [sys.argv[3], '--socket', str(game / 'control.sock')]
out_path = game / 'native-l3.json'
ev = json.loads(out_path.read_text()) if out_path.exists() else {}
Y = 100
FIRE, PRESS = (40, Y, 8), (44, Y, 8)


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


def look(p):
    return {'bloco': ctl('block', *p).get('state'), 'be': cmd(f'data get block {p[0]} {p[1]} {p[2]}')}


ctl('wait-ready', 120)
cmd(f'forceload add {FIRE[0]} {FIRE[2]}')
time.sleep(2)
if phase == 'montar':
    cmd(f'fill {FIRE[0] - 2} {Y - 1} {FIRE[2] - 2} {PRESS[0] + 2} {Y + 3} {FIRE[2] + 4} minecraft:air')
    cmd(f'fill {FIRE[0] - 2} {Y - 1} {FIRE[2] - 2} {PRESS[0] + 2} {Y - 1} {FIRE[2] + 4} minecraft:stone')
    ev['firebox_posta'] = cmd(f'setblock {FIRE[0]} {Y} {FIRE[2]} ntm:heater_firebox[facing=south]')
    ev['prensa_posta'] = cmd(f'setblock {PRESS[0]} {Y} {PRESS[2]} ntm:machine_press[facing=south]')
    cmd(f'tp @s {FIRE[0] + 2.5} {Y} {FIRE[2] + 4.5}')
    time.sleep(3)
    # Funnels feed them (/item doesn't reach a mod block entity's slots): the
    # slots' rules send coal to the fuel, the stamp to its slot (one), the
    # ingots to the input.
    for p in (FIRE, PRESS):
        cmd(f'setblock {p[0]} {Y + 1} {p[2]} minecraft:hopper[facing=down]')
    ev['carvao'] = cmd(f'item replace block {FIRE[0]} {Y + 1} {FIRE[2]} container.0 with minecraft:coal 8')
    cmd(f'item replace block {PRESS[0]} {Y + 1} {PRESS[2]} container.0 with minecraft:coal 8')
    cmd(f'item replace block {PRESS[0]} {Y + 1} {PRESS[2]} container.1 with ntm:stamp_stone_plate 1')
    cmd(f'item replace block {PRESS[0]} {Y + 1} {PRESS[2]} container.2 with minecraft:iron_ingot 8')
    time.sleep(20)
    ev['montar'] = {'firebox': look(FIRE), 'prensa': look(PRESS)}
    ctl('save')
    time.sleep(3)
elif phase == 'conferir':
    cmd(f'tp @s {FIRE[0] + 2.5} {Y} {FIRE[2] + 4.5}')
    time.sleep(4)
    first = look(PRESS)
    time.sleep(15)
    ev['conferir'] = {'prensa_ao_abrir': first, 'prensa_depois': look(PRESS), 'firebox': look(FIRE)}
out_path.write_text(json.dumps(ev, indent=1, ensure_ascii=False))
print(json.dumps(ev, ensure_ascii=False))

#!/usr/bin/env python3
"""Aceitação da L1 da migração (docs/nativo.md): a fornalha elétrica como
block entity nativo, no jogo de tools/build-native.sh.

Uso: python3 tools/test-game/native_l1.py <pasta do jogo> (fase) [horizon-ctl]
O jogo aberto num mundo criativo, com --control-socket <pasta>/control.sock.
Fases:
  montar   põe a fornalha, um funil com 8 minérios de ferro por cima, um com a
           bateria criativa ao lado e, embaixo, um funil para um baú; espera
           e lê o baú e o block entity (estado "ntm" e itens);
  conferir depois de fechar e reabrir o mundo: o block entity tem o mesmo
           estado e o baú os mesmos lingotes, e a fornalha continua fundindo.
Grava <pasta>/native-l1.json.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
phase = sys.argv[2] if len(sys.argv) > 2 else 'montar'
# The horizon-ctl built with the game (tools/build-native.sh).
CTL = [sys.argv[3] if len(sys.argv) > 3 else 'horizon-ctl', '--socket', str(game / 'control.sock')]
out_path = game / 'native-l1.json'
ev = json.loads(out_path.read_text()) if out_path.exists() else {}
F = (8, 100, 8)
x, y, z = F


def ctl(*args):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True, timeout=120).stdout.strip()
    try:
        return json.loads(out.splitlines()[-1]) if out else {}
    except ValueError:
        return {'raw': out}


def cmd(c):
    """The command's answer (horizon-ctl gives the chat it caused)."""
    out = ' / '.join(ctl('command', c).get('chat', []))
    time.sleep(0.3)
    return out


def data_get(px, py, pz, path=''):
    return cmd(f'data get block {px} {py} {pz} {path}'.strip())


ctl('wait-ready', 120)
if phase == 'montar':
    cmd(f'tp @s {x + 0.5} {y + 3} {z + 4.5}')
    time.sleep(5)
    ev['fill'] = cmd(f'fill {x - 2} {y - 3} {z - 2} {x + 2} {y + 2} {z + 2} minecraft:air')
    cmd(f'fill {x - 2} {y - 4} {z - 2} {x + 2} {y - 4} {z + 2} minecraft:stone')
    cmd(f'setblock {x} {y - 3} {z} minecraft:chest')
    cmd(f'setblock {x} {y - 1} {z} minecraft:hopper[facing=down]')
    # The hopper under the furnace feeds the chest under it: a column.
    cmd(f'setblock {x} {y - 2} {z} minecraft:hopper[facing=down]')
    ev['fornalha_posta'] = cmd(f'setblock {x} {y} {z} ntm:machine_electric_furnace_off[facing=north]')
    cmd(f'setblock {x} {y + 1} {z} minecraft:hopper[facing=down]')
    cmd(f'setblock {x + 1} {y} {z} minecraft:hopper[facing=west]')
    cmd(f'item replace block {x} {y + 1} {z} container.0 with minecraft:iron_ore 8')
    cmd(f'item replace block {x + 1} {y} {z} container.0 with ntm:battery_creative 1')
    ev['fornalha_nova'] = data_get(x, y, z)
    time.sleep(25)
    ev['montar'] = {'fornalha': data_get(x, y, z), 'bau': data_get(x, y - 3, z, 'Items')}
    ctl('save')
    time.sleep(3)
elif phase == 'conferir':
    cmd(f'tp @s {x + 0.5} {y + 3} {z + 4.5}')
    time.sleep(3)
    first = data_get(x, y, z)
    time.sleep(15)
    ev['conferir'] = {'fornalha_ao_abrir': first, 'fornalha_depois': data_get(x, y, z), 'bau': data_get(x, y - 3, z, 'Items')}
out_path.write_text(json.dumps(ev, indent=1, ensure_ascii=False))
print(json.dumps(ev, ensure_ascii=False))

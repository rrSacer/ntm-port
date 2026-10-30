#!/usr/bin/env python3
"""Aceitação da L4 no servidor (docs/nativo.md): a radiação do que o jogador
carrega e o detonador, no jogo de tools/build-native.sh.

Uso: python3 tools/test-game/native_l4.py <pasta do jogo> <mundo> <fase> <horizon-ctl>
O jogo aberto num mundo criativo, com --control-socket <pasta>/control.sock.
Fases:
  radiacao   o jogador em sobrevivência com 64 lingotes de urânio (22,4 RAD/s):
             passada a carência de 10 s, a dose cresce e fica guardada no
             estado do mundo (<mundo>/data/ntm/world); depois volta ao criativo;
  detonador  uma carga NTM posta por /setblock: o detonador usado nela a liga,
             usado no ar acende o pavio, e a carga explode. O texto do
             detonador vai ao log do jogo (o servidor não tem como pôr no chat).
Grava <pasta>/native-l4.json.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
world = sys.argv[2]
phase = sys.argv[3]
CTL = [sys.argv[4], '--socket', str(game / 'control.sock')]
out_path = game / 'native-l4.json'
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


def kept():
    f = game / 'saves' / world / 'data' / 'ntm' / 'world'
    return json.loads(f.read_text()) if f.exists() else {}


def log_lines(word):
    log = (game / 'client.log').read_text(errors='replace').splitlines()
    return [l for l in log if word in l][-3:]


ctl('wait-ready', 120)
if phase == 'radiacao':
    cmd('clear @s')
    cmd('gamemode survival')
    cmd('give @s ntm:ingot_uranium 64')
    time.sleep(3)
    ev['carencia'] = kept().get('radiation', {})
    time.sleep(25)
    ev['dose'] = kept().get('radiation', {})
    ev['estado'] = {k: v for k, v in ctl('state').get('player', {}).items() if k in ('health', 'effects', 'game_mode')}
    cmd('clear @s')
    cmd('gamemode creative')
elif phase == 'detonador':
    x, z = 120, 8
    cmd(f'forceload add {x - 4} {z - 4} {x + 4} {z + 6}')
    time.sleep(2)
    cmd(f'fill {x - 4} {Y - 4} {z - 4} {x + 4} {Y + 3} {z + 6} minecraft:air')
    cmd(f'fill {x - 4} {Y - 4} {z - 4} {x + 4} {Y - 1} {z + 6} minecraft:stone')
    ev['carga_posta'] = cmd(f'setblock {x} {Y} {z} ntm:tnt_ntm[lit=false]')
    cmd(f'tp @s {x + 0.5} {Y} {z + 4.5} facing {x + 0.5} {Y} {z + 0.5}')
    time.sleep(2)
    cmd('clear @s')
    cmd('give @s ntm:detonator 1')
    time.sleep(1)
    ctl('select', 'ntm:detonator')
    ctl('interact', x, Y, z)
    time.sleep(1)
    ev['ligado'] = log_lines('Detonador ligado')
    ctl('look', 0, -90)
    ctl('use-item')
    time.sleep(1)
    ev['pavio'] = log_lines('Pavio aceso')
    time.sleep(8)
    ev['depois'] = cmd(f'execute if block {x} {Y} {z} minecraft:air run say vazio') or ctl('block', x, Y, z)
    ev['cratera'] = ctl('block', x, Y - 1, z)
out_path.write_text(json.dumps(ev, indent=1, ensure_ascii=False))
print(json.dumps(ev, ensure_ascii=False))

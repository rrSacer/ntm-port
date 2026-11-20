#!/usr/bin/env python3
"""Aceitação do triturador no motor ntm-visual (docs/maquinas.md, "Triturador").

Uso: python3 tools/test-game/shredder.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  moer     bateria cheia ao lado, minério, lingotes e pedregulho pelas
           entradas, lâminas de aço e desh, até o fim; retira o pó;
  voltar   depois de reabrir o mundo: conteúdo e dano das lâminas ficaram.
Grava <pasta>/shredder-evidence.json e capturas shredder-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['moer']
CTL = ['/home/sacer/Projects/horizon-ntm/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'shredder-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
X, Z = 3530, -1398


def ctl(*args, check=True):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True, timeout=300).stdout.strip()
    try:
        j = json.loads(out.splitlines()[-1]) if out else {}
    except ValueError:
        j = {'raw': out}
    if check and isinstance(j, dict) and j.get('error'):
        raise SystemExit(f'{args}: {j["error"]}')
    return j


def machine_slots():
    """Os slots da máquina (300+) com item: (slot, item, quantidade, dano)."""
    return [(s['slot'], s['item'], s['count'], s.get('damage') or 0) for s in ctl('screen').get('slots', []) if s.get('item') and s['slot'] >= 300]


def floor_y():
    p = game / 'shredder-y'
    if p.exists():
        return int(p.read_text())
    ctl('tp', X + 0.5, 120, Z + 2.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


def moer():
    y = floor_y()
    ctl('fill', X - 3, y, Z - 2, X + 4, y + 3, Z + 2, 'minecraft:air')
    ctl('fill', X - 3, y - 1, Z - 2, X + 4, y - 1, Z + 2, 'minecraft:stone')
    ctl('tp', X + 0.5, y, Z + 1.5)
    for item in ('ntm:machine_battery', 'ntm:machine_shredder'):
        ctl('give', item, 1)
    time.sleep(1)
    ctl('place', X, y, Z, '--item', 'ntm:machine_battery')
    time.sleep(2)
    ctl('close', check=False)  # a tela da bateria, se o "usar" a abriu
    ctl('place', X + 1, y, Z, '--item', 'ntm:machine_shredder')
    time.sleep(2)
    # Bateria: tela antiga (tecla R), +100% de teste, modo saída.
    ctl('look-at-block', X, y, Z)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(3)
    for _ in range(10):
        ctl('click', 'teste')
        time.sleep(0.6)
    ctl('click', 'trocar-modo')
    time.sleep(1)
    ctl('key', 'esc', 'tap')
    time.sleep(1)
    for item, n in (('minecraft:iron_ore', 8), ('minecraft:iron_ingot', 4), ('minecraft:cobblestone', 2), ('ntm:blades_steel', 1), ('ntm:blades_desh', 1)):
        ctl('give', item, n)
    time.sleep(1)
    ctl('interact', X + 1, y, Z)
    time.sleep(2)
    hotbar = {s['item']: s['slot'] for s in ctl('screen')['slots'] if s.get('item') and s['slot'] < 9}
    for item in ('minecraft:iron_ore', 'minecraft:iron_ingot', 'minecraft:cobblestone', 'ntm:blades_steel', 'ntm:blades_desh'):
        ctl('click-slot', hotbar[item], 'shift')
        time.sleep(0.7)
    placed = machine_slots()
    t0 = time.time()
    while any(s[1] == 'minecraft:iron_ore' for s in machine_slots()):
        if time.time() - t0 > 90:
            raise SystemExit(f'não terminou: {machine_slots()}')
        time.sleep(2)
    seconds = time.time() - t0
    ctl('screenshot', game / 'shredder-done.png')
    out = machine_slots()
    dust = [s for s in out if s[1] == 'ntm:powder_iron']
    if dust:
        ctl('click-slot', dust[0][0], 'shift')
        time.sleep(1)
    counts = ctl('inventory').get('counts', {})
    evidence['moer'] = {'postos': placed, 'fim': out, 'segundos': round(seconds, 1), 'po_no_inventario': counts.get('ntm:powder_iron', 0)}
    print('moer', evidence['moer'])
    ctl('key', 'esc', 'tap')


def voltar():
    y = floor_y()
    ctl('interact', X + 1, y, Z)
    time.sleep(2)
    evidence['voltar'] = machine_slots()
    print('voltar', evidence['voltar'])
    ctl('key', 'esc', 'tap')


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

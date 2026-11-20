#!/usr/bin/env python3
"""Aceitação do alto-forno no motor ntm-visual (docs/maquinas.md, "Alto-forno").

Uso: python3 tools/test-game/blastfurnace.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  aco      o multibloco 3×7×3 posto num piso livre; carvão no combustível,
           4 lingotes de ferro e 2 areias nas entradas por shift-clique; sem
           ar quente (o port ainda não produz airblast: o mundo falso testa),
           duas operações de aço; retira o aço;
  voltar   depois de reabrir o mundo: o que ficou nos slots.
Grava <pasta>/blastfurnace-evidence.json e capturas blastfurnace-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['aco']
CTL = ['/home/sacer/Projects/horizon-ntm/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'blastfurnace-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
X, Z = 3540, -1398


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
    """Os slots da máquina (300+) com item: (slot, item, quantidade)."""
    return [(s['slot'], s['item'], s['count']) for s in ctl('screen').get('slots', []) if s.get('item') and s['slot'] >= 300]


def floor_y():
    p = game / 'blastfurnace-y'
    if p.exists():
        return int(p.read_text())
    ctl('tp', X + 0.5, 120, Z + 4.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


def aco():
    y = floor_y()
    ctl('fill', X - 3, y, Z - 3, X + 3, y + 8, Z + 5, 'minecraft:air')
    ctl('fill', X - 3, y - 1, Z - 3, X + 3, y - 1, Z + 5, 'minecraft:stone')
    ctl('tp', X + 0.5, y, Z + 3.5)
    ctl('look', 180, 0)  # para o norte, de frente para o multibloco
    ctl('give', 'ntm:machine_blast_furnace', 1)
    time.sleep(1)
    ctl('place', X, y, Z, '--item', 'ntm:machine_blast_furnace')
    time.sleep(2)
    ctl('close', check=False)
    placed = ctl('block', X, y, Z).get('state', '')
    for item, n in (('minecraft:coal', 2), ('minecraft:iron_ingot', 4), ('minecraft:sand', 2)):
        ctl('give', item, n)
    time.sleep(1)
    ctl('interact', X, y, Z)
    time.sleep(2)
    hotbar = {s['item']: s['slot'] for s in ctl('screen')['slots'] if s.get('item') and s['slot'] < 9}
    for item in ('minecraft:coal', 'minecraft:iron_ingot', 'minecraft:sand'):
        ctl('click-slot', hotbar[item], 'shift')
        time.sleep(0.7)
    put = machine_slots()
    ctl('screenshot', game / 'blastfurnace-working.png')
    t0 = time.time()
    # Duas operações de 800 ticks a 1× (40 s cada a 20 TPS).
    while not any(s[1] == 'ntm:ingot_steel' and s[2] >= 4 for s in machine_slots()):
        if time.time() - t0 > 150:
            raise SystemExit(f'não terminou: {machine_slots()}')
        time.sleep(3)
    seconds = time.time() - t0
    ctl('screenshot', game / 'blastfurnace-done.png')
    out = machine_slots()
    steel = [s for s in out if s[1] == 'ntm:ingot_steel']
    if steel:
        ctl('click-slot', steel[0][0], 'shift')
        time.sleep(1)
    counts = ctl('inventory').get('counts', {})
    evidence['aco'] = {'bloco': placed, 'postos': put, 'fim': out, 'segundos': round(seconds, 1), 'aco_no_inventario': counts.get('ntm:ingot_steel', 0)}
    print('aco', evidence['aco'])
    ctl('key', 'esc', 'tap')


def voltar():
    y = floor_y()
    ctl('interact', X, y, Z)
    time.sleep(2)
    evidence['voltar'] = machine_slots()
    print('voltar', evidence['voltar'])
    ctl('key', 'esc', 'tap')


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

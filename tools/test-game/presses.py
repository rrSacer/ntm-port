#!/usr/bin/env python3
"""Aceitação das prensas e da centrífuga de gás com slots (motor ntm-visual;
docs/maquinas.md, "Prensas e centrífuga de gás com slots").

Uso: python3 tools/test-game/presses.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  prensa    prensa a combustível: carvão, matriz de chapa de ferro e 8
            lingotes de ferro postos nos slots; 8 chapas saem e são
            retiradas; a matriz volta com 8 de desgaste;
  eprensa   prensa elétrica ao lado de uma bateria cheia: matriz de fio com
            62 de desgaste e 5 lingotes de cobre; 2 prensagens a quebram (16
            fios), 3 lingotes ficam;
  gascent   centrífuga de gás ao lado de uma bateria: 2 células de UF6
            despejadas pela tela antiga (tecla R), o upgrade posto no slot
            6; produtos nos slots de saída.
Grava <pasta>/presses-evidence.json e capturas presses-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['prensa', 'eprensa', 'gascent']
CTL = ['/home/sacer/Workspace/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'presses-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
X, Z = 3720, -1398


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


def machine_slots():
    return [(s['slot'], s['item'], s['count'], s.get('damage', 0)) for s in screen().get('slots', []) if s.get('item') and s['slot'] >= 300]


def floor_y():
    p = game / 'presses-y'
    if p.exists():
        return int(p.read_text())
    ctl('tp', X + 0.5, 120, Z + 4.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


def ground():
    y = floor_y()
    ctl('fill', X - 3, y, Z - 3, X + 12, y + 4, Z + 4, 'minecraft:air')
    ctl('fill', X - 3, y - 1, Z - 3, X + 12, y - 1, Z + 4, 'minecraft:stone')


def place(x, item):
    y = floor_y()
    st = ctl('block', x, y, Z).get('state', '')
    if st.startswith(item + '['):
        return st
    ctl('close', check=False)
    ctl('tp', x + 0.5, y, Z + 3.5)
    ctl('give', item, 1)
    time.sleep(0.7)
    ctl('place', x, y, Z, '--item', item)
    time.sleep(1.5)
    ctl('close', check=False)
    return ctl('block', x, y, Z).get('state', '')


def open_at(x):
    y = floor_y()
    ctl('close', check=False)
    ctl('tp', x + 0.5, y, Z + 3.5)
    ctl('interact', x, y, Z)
    time.sleep(1.5)


def close():
    """Close a menu, or the mod's old screen (which `close` leaves open)."""
    ctl('close', check=False)
    if ctl('screen').get('screen') == 'mod':
        ctl('key', 'esc', 'tap')
        time.sleep(0.8)


def old_screen(x):
    """The mod's old screen (key R), for what the slots don't do."""
    y = floor_y()
    close()
    ctl('tp', x + 0.5, y, Z + 3.5)
    ctl('look-at-block', x, y, Z)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    return ctl('ui')


def click_label(start, wait=2.0):
    for w in ctl('ui').get('widgets', []):
        if (w.get('label') or '').startswith(start):
            ctl('click', w['id'])
            time.sleep(wait)
            return
    raise SystemExit(f'sem "{start}": {[w.get("label") for w in ctl("ui").get("widgets", [])]}')


def put(item, n, slot, damage=None):
    """`n` of `item` (a worn one, with `damage`) from the hotbar into
    machine slot `slot` (300 + i)."""
    if damage is None:
        ctl('give', item, n)
    else:
        # A worn one: the vanilla command reads components.
        ctl('command', f'give @s {item}[minecraft:damage={damage}] {n}')
    for _ in range(10):
        time.sleep(0.7)
        hot = [s['slot'] for s in screen()['slots'] if s.get('item') == item and s['slot'] < 36]
        if hot:
            break
    ctl('click-slot', hot[0])
    time.sleep(0.3)
    ctl('click-slot', slot)
    time.sleep(0.5)


def battery(x):
    """A full battery in output mode (old screen; +10% only in creative)."""
    place(x, 'ntm:machine_battery')
    old_screen(x)
    for _ in range(10):
        click_label('Teste: +10%', 0.6)
    click_label('Trocar modo', 1.0)
    close()


def prensa():
    ground()
    ctl('command', 'clear @s', check=False)
    st = place(X, 'ntm:machine_press')
    open_at(X)
    put('minecraft:coal', 4, 300)
    put('ntm:stamp_iron_plate', 1, 301)
    put('minecraft:iron_ingot', 8, 302)
    before = machine_slots()
    ctl('screenshot', game / 'presses-prensa.png')
    t0, taken = time.time(), 0
    while taken < 8 and time.time() - t0 < 240:
        time.sleep(5)
        out = [s for s in machine_slots() if s[0] == 303]
        if out:
            ctl('click-slot', 303, 'shift')
            time.sleep(0.4)
            taken += out[0][2]
    after = machine_slots()
    ctl('close', check=False)
    evidence['prensa'] = {'bloco': st, 'postos': before, 'chapas_retiradas': taken, 'segundos': round(time.time() - t0, 1),
                          'fim': after, 'chapas_no_inventario': ctl('inventory').get('counts', {}).get('ntm:plate_iron', 0)}
    print('prensa', evidence['prensa'])


def eprensa():
    ctl('command', 'clear @s', check=False)
    battery(X + 4)
    st = place(X + 5, 'ntm:machine_epress')
    open_at(X + 5)
    put('ntm:stamp_iron_wire', 1, 301, damage=62)
    put('ntm:ingot_copper', 5, 302)
    before = machine_slots()
    time.sleep(40)
    after = machine_slots()
    ctl('screenshot', game / 'presses-eprensa.png')
    ctl('close', check=False)
    wires = sum(s[2] for s in after if s[1] == 'ntm:wire_fine__copper')
    copper = sum(s[2] for s in after if s[1] == 'ntm:ingot_copper')
    evidence['eprensa'] = {'bloco': st, 'postos': before, 'fim': after,
                           'ok': wires == 16 and copper == 3 and not any(s[0] == 301 for s in after)}
    print('eprensa', evidence['eprensa'])


def gascent():
    ctl('command', 'clear @s', check=False)
    battery(X + 9)
    st = place(X + 10, 'ntm:machine_gascent')
    poured = 0
    for _ in range(2):
        ctl('give', 'ntm:cell_uf6', 1)
        time.sleep(0.7)
        ctl('select', 'ntm:cell_uf6')
        old_screen(X + 10)
        click_label('Despejar UF6/PuF6 da mão')
        poured += 1
        close()
    open_at(X + 10)
    put('ntm:upgrade_gc_speed', 1, 306)
    t0 = time.time()
    while time.time() - t0 < 150:
        time.sleep(5)
        out = [s for s in machine_slots() if 300 <= s[0] <= 303]
        if out:
            break
    after = machine_slots()
    ctl('screenshot', game / 'presses-gascent.png')
    ctl('close', check=False)
    evidence['gascent'] = {'bloco': st, 'celulas': poured, 'fim': after, 'segundos': round(time.time() - t0, 1),
                           'celulas_vazias': ctl('inventory').get('counts', {}).get('ntm:cell_empty', 0)}
    print('gascent', evidence['gascent'])


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

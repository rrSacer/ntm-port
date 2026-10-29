#!/usr/bin/env python3
"""Aceitação da bigorna no motor ntm-visual (docs/maquinas.md, "Bigorna").

Uso: python3 tools/test-game/anvil.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  forjar     uma bigorna de ferro; a bigorna de ferro e 12 lingotes de aço
             nos slots da forja; "Forjar" dá a bigorna de aço, sobram 2;
  construir  uma bigorna de desh (nível 3); a chapa de aço escolhida na lista
             (tela antiga); 20 lingotes de aço na grade; "Construir uma vez"
             faz 1 chapa, com shift faz as outras 19; retira as chapas;
  voltar     depois de reabrir o mundo: o que ficou nos slots, e "Construir
             uma vez" ainda faz a chapa (a receita escolhida voltou).
Grava <pasta>/anvil-evidence.json e capturas anvil-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['forjar', 'construir']
CTL = ['/home/sacer/Workspace/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'anvil-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
X, Z = 3700, -1398
IRON, DESH = (X, Z), (X + 3, Z)


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
    return [(s['slot'], s['item'], s['count']) for s in screen().get('slots', []) if s.get('item') and s['slot'] >= 300]


def floor_y():
    p = game / 'anvil-y'
    if p.exists():
        return int(p.read_text())
    ctl('tp', X + 0.5, 120, Z + 4.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


def open_at(pos):
    y = floor_y()
    ctl('tp', pos[0] + 0.5, y, pos[1] + 2.5)
    ctl('interact', pos[0], y, pos[1])
    time.sleep(1.5)


def put(item, n, slot):
    """`n` of `item` from the hotbar into machine slot `slot` (300 + i)."""
    ctl('give', item, n)
    time.sleep(0.7)
    hot = [s['slot'] for s in screen()['slots'] if s.get('item') == item and s['slot'] < 36]
    ctl('click-slot', hot[0])
    time.sleep(0.3)
    ctl('click-slot', slot)
    time.sleep(0.5)


def place(pos, block):
    y = floor_y()
    st = ctl('block', pos[0], y, pos[1]).get('state', '')
    if st.startswith(block + '['):
        return st
    ctl('tp', pos[0] + 0.5, y, pos[1] + 2.5)
    ctl('give', block, 1)
    time.sleep(0.5)
    ctl('place', pos[0], y, pos[1], '--item', block)
    time.sleep(1.5)
    ctl('close', check=False)
    return ctl('block', pos[0], y, pos[1]).get('state', '')


def ground():
    y = floor_y()
    ctl('fill', X - 3, y, Z - 3, X + 6, y + 4, Z + 4, 'minecraft:air')
    ctl('fill', X - 3, y - 1, Z - 3, X + 6, y - 1, Z + 4, 'minecraft:stone')


def forjar():
    ground()
    st = place(IRON, 'ntm:anvil_iron')
    ctl('command', 'clear @s', check=False)
    open_at(IRON)
    put('ntm:anvil_iron', 1, 300)
    put('ntm:ingot_steel', 12, 301)
    time.sleep(1)
    before = machine_slots()
    ctl('screenshot', game / 'anvil-forja.png')
    ctl('click-button', 'Forjar')
    time.sleep(1.5)
    after = machine_slots()
    ctl('screenshot', game / 'anvil-forjado.png')
    ctl('close', check=False)
    evidence['forjar'] = {'bloco': st, 'antes': before, 'depois': after,
                          'ok': (302, 'ntm:anvil_steel', 1) in after and (301, 'ntm:ingot_steel', 2) in after and not any(s[0] == 300 for s in after)}
    print('forjar', evidence['forjar'])


def pick_plate():
    """The steel plate on the recipe list (button 0 opens the old screen).
    Its rows aren't widgets: clicked at their GUI point (the list's first
    row at y 50, 12 apart, at GUI scale 3), the row from the embedded table
    (the recipes a tier 3 anvil lists, in order)."""
    t = json.loads((Path(__file__).resolve().parents[2] / 'crates/ntm-core/data/machines/anvil.json').read_text())
    listed = [c for c in t['construction'] if c['tier_lower'] <= 3 and (c['tier_upper'] == -1 or 3 <= c['tier_upper'])]
    row = next(i for i, c in enumerate(listed) if c['out'][0][0] == 'ntm:plate_steel' and c['in'][0]['ids'] == ['ntm:ingot_steel'])
    if row > 6:
        raise SystemExit(f'a chapa está na linha {row}, fora da vista')
    ctl('click-button', 'Escolher receita')
    time.sleep(2)
    ctl('screenshot', game / 'anvil-lista.png')
    ctl('click-at', 150, 50 + 12 * row)
    time.sleep(1)
    ctl('close', check=False)
    time.sleep(1)
    return listed[row]['label']


def construir():
    st = place(DESH, 'ntm:anvil_desh')
    ctl('command', 'clear @s', check=False)
    open_at(DESH)
    row = pick_plate()
    open_at(DESH)
    put('ntm:ingot_steel', 20, 303)
    time.sleep(1)
    ctl('click-button', 'Construir uma vez')
    time.sleep(1.5)
    once = machine_slots()
    ctl('key', 'shift', 'down')
    time.sleep(0.2)
    ctl('click-button', 'Construir uma vez')
    ctl('key', 'shift', 'up')
    time.sleep(1.5)
    full = machine_slots()
    ctl('screenshot', game / 'anvil-construido.png')
    plates = [s for s in full if s[1] == 'ntm:plate_steel']
    for s in plates:
        ctl('click-slot', s[0], 'shift')
        time.sleep(0.4)
    ctl('close', check=False)
    counts = ctl('inventory').get('counts', {})
    evidence['construir'] = {'bloco': st, 'receita': row, 'uma': once, 'pacote': full,
                             'chapas_no_inventario': counts.get('ntm:plate_steel', 0)}
    print('construir', evidence['construir'])


def voltar():
    open_at(IRON)
    iron = machine_slots()
    ctl('close', check=False)
    ctl('command', 'clear @s', check=False)
    open_at(DESH)
    put('ntm:ingot_steel', 2, 303)
    time.sleep(1)
    ctl('click-button', 'Construir uma vez')
    time.sleep(1.5)
    desh = machine_slots()
    ctl('close', check=False)
    evidence['voltar'] = {'ferro': iron, 'desh': desh, 'receita_voltou': any(s[1] == 'ntm:plate_steel' for s in desh)}
    print('voltar', evidence['voltar'])


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

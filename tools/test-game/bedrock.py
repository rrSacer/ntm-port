#!/usr/bin/env python3
"""Aceitação da cadeia do bedrock, etapa 1, no motor ntm-visual
(docs/maquinas.md, "Cadeia do bedrock, etapa 1").

Uso: python3 tools/test-game/bedrock.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  escavar     a escavadora posta sobre um depósito de nível 1 (sem fluido;
              actinídeos 2,0, cristalinos 1,0 em (2920, −1886)), com a
              bateria criativa e a broca ferro: cava as 5 camadas sob ela
              (o resto do poço aberto antes, para o teste não levar horas)
              e tira 2 brutos;
  slop        o ore slopper com a bateria criativa e água de um barril na
              sua borda: os 2 brutos viram minérios BASE;
  centrifugar a centrífuga ao lado de uma bateria cheia: BASE de actinídeos
              → PRIMARY → fragmentos de urânio e tório;
  craftar     9 fragmentos de urânio numa bancada: 1 pó de urânio.
O depósito não guarda nada (tudo vem da posição): o teste o põe com fill.
Grava <pasta>/bedrock-evidence.json e capturas bedrock-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['escavar', 'slop', 'centrifugar', 'craftar']
CTL = ['/home/sacer/Workspace/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'bedrock-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
# The deposit's chunk: x 2912..2927, z −1888..−1873. The excavator's core
# lands 3 cells off the click, so a click at z −1876 keeps it in there.
X, Z = 2920, -1876


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
    return [(s['slot'], s['item'], s['count'], s.get('components')) for s in screen().get('slots', []) if s.get('item') and s['slot'] >= 300]


def close():
    ctl('close', check=False)
    for _ in range(4):
        time.sleep(0.8)
        if ctl('screen').get('screen') not in ('mod', 'container'):
            return
        ctl('key', 'esc', 'tap')


def click_label(start, wait=2.0):
    for w in ctl('ui').get('widgets', []):
        if (w.get('label') or '').startswith(start):
            ctl('click', w['id'])
            time.sleep(wait)
            return
    raise SystemExit(f'sem "{start}": {[w.get("label") for w in ctl("ui").get("widgets", [])]}')


def give(item, n):
    ctl('give', item, n)
    for _ in range(10):
        time.sleep(0.5)
        if ctl('inventory').get('counts', {}).get(item, 0) >= n:
            return


def hot(item):
    return [s['slot'] for s in screen()['slots'] if s.get('item') == item and s['slot'] < 36]


def put(item, n, slot):
    give(item, n)
    ctl('click-slot', hot(item)[0])
    time.sleep(0.3)
    ctl('click-slot', slot)
    time.sleep(0.5)


def floor_y():
    p = game / 'bedrock-y'
    if p.exists():
        return int(p.read_text())
    ctl('tp', X + 0.5, 150, Z + 10.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


def find(block, x0, y0, z0, x1, y1, z1):
    for b in ctl('blocks', x0, y0, z0, x1, y1, z1).get('blocks', []):
        if b[3].startswith(block + '['):
            return b[:3]
    raise SystemExit(f'{block} não achado')


def place(x, y, z, item, stand):
    close()
    ctl('tp', stand[0] + 0.5, stand[1], stand[2] + 0.5)
    give(item, 1)
    # A multiblock fills the clicked cell with its own blocks: ctl may call
    # that "did not change"; the caller finds the core.
    ctl('place', x, y, z, '--item', item, check=False)
    time.sleep(2)
    close()


def open_at(x, y, z, stand):
    close()
    ctl('tp', stand[0] + 0.5, stand[1], stand[2] + 0.5)
    ctl('interact', x, y, z)
    time.sleep(1.5)


def battery(x, y, z, stand):
    """A full battery in output mode (old screen; +10% only in creative)."""
    place(x, y, z, 'ntm:machine_battery', stand)
    ctl('look-at-block', x, y, z)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    for _ in range(12):
        click_label('Teste: +10%', 1.0)
    click_label('Trocar modo', 1.0)
    close()


def escavar():
    y = floor_y()
    if not (game / 'bedrock-chao').exists():
        ctl('fill', X - 6, y, Z - 9, X + 6, y + 8, Z + 12, 'minecraft:air')
        ctl('fill', X - 6, y - 1, Z - 9, X + 6, y - 1, Z + 12, 'minecraft:stone')
        (game / 'bedrock-chao').write_text('1')
    ctl('command', 'clear @s', check=False)
    if not any('excavator' in b[3] for b in ctl('blocks', X - 4, y, Z - 6, X + 4, y + 6, Z + 1).get('blocks', [])):
        place(X, y, Z, 'ntm:machine_excavator', (X, y, Z + 3))
    (cx, cy, cz) = find('ntm:machine_excavator', X - 4, y, Z - 4, X + 4, y + 6, Z + 4)
    # The deposit under the core, and the shaft open below its first 5 layers.
    ctl('fill', cx - 1, -63, cz - 1, cx + 1, cy - 9, cz + 1, 'minecraft:air')
    ctl('fill', cx, -64, cz, cx, -64, cz, 'ntm:ore_bedrock')
    # The legs stand on the floor: the screen through a body cell in reach.
    stand = (cx, y, cz + 6)
    open_at(cx, cy, cz + 3, stand)
    put('ntm:battery_creative', 1, 300)
    put('ntm:drillbit__ferro', 1, 304)
    ctl('click-button', 'Broca')
    time.sleep(2)
    ctl('screenshot', game / 'bedrock-escavadora.png')
    t0, raw = time.time(), []
    while time.time() - t0 < 600:
        time.sleep(10)
        open_at(cx, cy, cz + 3, stand)
        raw = [s for s in machine_slots() if s[1] == 'ntm:bedrock_ore_base']
        if sum(s[2] for s in raw) >= 2:
            break
    buf = machine_slots()
    for s in raw:
        ctl('click-slot', s[0], 'shift')
        time.sleep(0.4)
    close()
    inv = ctl('inventory')
    evidence['escavar'] = {'nucleo': [cx, cy, cz], 'buffer': buf, 'segundos': round(time.time() - t0, 1),
                           'brutos': [s for s in inv.get('hotbar', []) + inv.get('main', []) if s and s.get('item') == 'ntm:bedrock_ore_base'],
                           'poco': [ctl('block', cx, yy, cz).get('state') for yy in range(cy - 4, cy - 9, -1)]}
    print('escavar', evidence['escavar'])


def slop():
    y = floor_y()
    # On the stone floor south of the excavator.
    sx, sz = X, Z + 8
    if not any('slopper' in b[3] for b in ctl('blocks', sx - 4, y, sz - 4, sx + 4, y + 4, sz + 4).get('blocks', [])):
        place(sx, y, sz, 'ntm:machine_ore_slopper', (sx, y, sz + 4))
    (ox, oy, oz) = find('ntm:machine_ore_slopper', sx - 4, y, sz - 4, sx + 4, y + 4, sz + 4)
    # Water: a steel barrel just past the bottom layer's east edge.
    cells = [b for b in ctl('blocks', ox - 4, y, oz - 4, ox + 4, y, oz + 4).get('blocks', []) if 'dummy' in b[3] or 'slopper' in b[3]]
    bx = max(c[0] for c in cells) + 1
    if not ctl('block', bx, y, oz).get('state', '').startswith('ntm:barrel_steel'):
        place(bx, y, oz, 'ntm:barrel_steel', (bx + 2, y, oz))
    give('ntm:fluid_barrel_full__water', 1)
    ctl('select', 'ntm:fluid_barrel_full__water')
    close()
    ctl('tp', bx + 2.5, y, oz + 0.5)
    ctl('look-at-block', bx, y, oz)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    click_label('Despejar')
    # Input → Buffer → Output: the barrel gives its water to the slopper.
    while 'Saída' not in ''.join(w.get('label') or '' for w in ctl('ui').get('widgets', [])):
        click_label('Modo:', 1.0)
    close()
    zs = [c[2] for c in cells]
    stand = (ox, y, max(zs) + 3)
    front = (ox, y, max(zs))
    open_at(*front, stand)
    put('ntm:battery_creative', 1, 300)
    raw = hot('ntm:bedrock_ore_base')
    for s in raw:
        ctl('click-slot', s)
        time.sleep(0.3)
        ctl('click-slot', 302)
        time.sleep(0.4)
    t0 = time.time()
    while time.time() - t0 < 180:
        time.sleep(8)
        open_at(*front, stand)
        if not any(s[0] == 302 for s in machine_slots()):
            break
    out = machine_slots()
    ctl('screenshot', game / 'bedrock-slopper.png')
    for s in out:
        if 303 <= s[0] <= 308:
            ctl('click-slot', s[0], 'shift')
            time.sleep(0.4)
    close()
    evidence['slop'] = {'nucleo': [ox, oy, oz], 'saidas': out, 'segundos': round(time.time() - t0, 1),
                        'inventario': ctl('inventory').get('counts', {})}
    print('slop', evidence['slop'])


def centrifuge_run(cx, y, cz, item):
    """`item` (all of it) through the centrifuge; what comes out goes to the
    inventory."""
    stand = (cx, y, cz + 3)
    n = ctl('inventory').get('counts', {}).get(item, 0)
    open_at(cx, y, cz, stand)
    ctl('click-slot', hot(item)[0])
    time.sleep(0.3)
    ctl('click-slot', 300)
    time.sleep(0.5)
    t0 = time.time()
    while time.time() - t0 < 60 + 12 * n:
        time.sleep(5)
        open_at(cx, y, cz, stand)
        if not any(s[0] == 300 for s in machine_slots()):
            break
    time.sleep(11)
    open_at(cx, y, cz, stand)
    for s in machine_slots():
        if s[0] >= 301:
            ctl('click-slot', s[0], 'shift')
            time.sleep(0.4)
    close()


def centrifugar():
    y = floor_y()
    cx, cz = X + 4, Z + 10
    battery(cx + 1, y, cz, (cx + 1, y, cz + 3))
    place(cx, y, cz, 'ntm:machine_centrifuge', (cx, y, cz + 3))
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__base_actinide')
    centrifuge_run(cx, y, cz, 'ntm:bedrock_ore__primary_actinide')
    counts = ctl('inventory').get('counts', {})
    evidence['centrifugar'] = {k: v for k, v in counts.items() if 'bedrock' in k or k in ('minecraft:gravel',)}
    print('centrifugar', evidence['centrifugar'])


def craftar():
    y = floor_y()
    # Room in the hotbar: everything but the uranium fragments out.
    for item in ctl('inventory').get('counts', {}):
        if item != 'ntm:bedrock_ore_fragment__uranium':
            ctl('command', f'clear @s {item}', check=False)
    tx, tz = X + 4, Z + 6
    place(tx, y, tz, 'minecraft:crafting_table', (tx, y, tz + 3))
    open_at(tx, y, tz, (tx, y, tz + 3))
    src = hot('ntm:bedrock_ore_fragment__uranium')
    ctl('click-slot', src[0])
    time.sleep(0.3)
    # One fragment a cell of the 3×3 grid (100..108), the rest back.
    for i in range(9):
        ctl('click-slot', 100 + i, 'right')
        time.sleep(0.2)
    ctl('click-slot', src[0])
    time.sleep(0.3)
    ctl('click-slot', 200, 'shift')
    time.sleep(1)
    close()
    evidence['craftar'] = {'po_de_uranio': ctl('inventory').get('counts', {}).get('ntm:powder_uranium', 0)}
    print('craftar', evidence['craftar'])


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

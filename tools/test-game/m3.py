#!/usr/bin/env python3
"""Verificação em jogo do M3 (docs/maquinas.md, docs/fluidos.md).

Uso: python3 tools/test-game/m3.py <pasta do jogo> [etapa...]
O jogo já aberto num mundo criativo (horizon-ctl open), mudo, em Xvfb
(tools/test-game/run-xvfb.sh). Etapas: assembly, fluids, gascent (padrão:
todas). Grava <pasta>/m3-evidence.json e capturas *.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['assembly', 'fluids', 'gascent']
CTL = ['/home/sacer/Games/horizon/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
evidence = {}


def ctl(*args, check=True):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True, timeout=300).stdout.strip()
    try:
        j = json.loads(out.splitlines()[-1]) if out else {}
    except ValueError:
        j = {'raw': out}
    if check and isinstance(j, dict) and j.get('error'):
        raise SystemExit(f'{args}: {j["error"]}')
    return j


def state():
    return ctl('state')


def inv(item):
    return ctl('inventory').get('counts', {}).get(item, 0)


def chat(cmd, wait=1.5):
    ctl('chat', cmd)
    time.sleep(wait)


def ground(x, z):
    ctl('tp', x + 0.5, 120, z + 0.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    return int(state()['player']['position'][1])


def place(x, y, z, item):
    # Pôr um bloco é o botão de usar: se a mira cai numa máquina, a tela
    # dela abre um instante depois (round trip ao servidor).
    ctl('close', check=False)
    time.sleep(1.2)
    ctl('close', check=False)
    ctl('give', item, 1)
    for _ in range(10):
        if inv(item) > 0:
            break
        time.sleep(0.5)
    for _ in range(6):
        r = ctl('place', x, y, z, '--item', item, check=False)
        if 'mod screen is open' not in str(r.get('error', '')):
            break
        ctl('close', check=False)
        time.sleep(1.5)
    if r.get('error'):
        raise SystemExit(f'place {item}: {r["error"]}')
    return r


def open_at(x, y, z):
    ctl('close', check=False)
    ctl('look-at-block', x, y, z)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    return ctl('ui')


def labels():
    return [w.get('label') or w.get('text') or '' for w in ctl('ui').get('widgets', [])]


def click(label_start, wait=2.5):
    for w in ctl('ui').get('widgets', []):
        if (w.get('label') or '').startswith(label_start):
            ctl('click', w['id'])
            time.sleep(wait)
            return True
    raise SystemExit(f'sem botão "{label_start}": {labels()}')


def maybe(label_start, wait=2.0):
    if any(l.startswith(label_start) for l in labels()):
        click(label_start, wait)


def hold(item, n):
    ctl('close', check=False)
    ctl('give', item, n)
    for _ in range(10):
        if inv(item) > 0:
            break
        time.sleep(0.5)
    ctl('select', item)


def shot(name):
    ctl('screenshot', game / f'{name}.png')


def wait_until(cond, seconds, what):
    t0 = time.time()
    while time.time() - t0 < seconds:
        if cond():
            return time.time() - t0
        time.sleep(2)
    raise SystemExit(f'esperou {seconds}s: {what}')


def battery(x, y, z):
    """Uma bateria cheia em modo saída (o botão +10% só no criativo)."""
    place(x, y, z, 'ntm:machine_battery')
    time.sleep(1.5)
    open_at(x, y, z)
    for _ in range(10):
        click('Teste: +10%', 0.6)
    click('Trocar modo', 1.0)  # Buffer (padrão) → Saída


def row_of(station, name):
    data = Path(__file__).resolve().parents[2] / f'crates/ntm-core/data/machines/{station}.json'
    names = [r['name'] for r in json.loads(data.read_text())['recipes']]
    i = names.index(name)
    assert i < 6, f'{name} na linha {i}: fora das 6 visíveis'
    return i


def pick_row(row):
    """Clica a linha `row` da lista de receitas (o 1º widget da tela)."""
    # O `ui` do horizon-ctl não lista widgets `list`: a lista (6 linhas
    # visíveis: 80 px) fica logo acima do "Voltar", com 6 px de vão.
    ui = ctl('ui')
    back = [w for w in ui.get('widgets', []) if w.get('label') == 'Voltar']
    if not back:
        raise SystemExit(f'sem Voltar: {ui}')
    b = back[0]['bounds']  # [x, y, w, h] em pixels de GUI
    top = b[1] - 6 - 80
    ctl('click-at', b[0] + b[2] / 2, top + 4 + 6 + 12 * row)
    time.sleep(2.5)


def assembly():
    x, z = 3530, -1400
    y = ground(x + 1, z + 3)
    ctl('fill', x - 1, y, z - 3, x + 4, y + 3, z + 3, 'minecraft:air')
    ctl('fill', x - 1, y - 1, z - 3, x + 6, y - 1, z + 3, 'minecraft:stone')  # piso plano
    battery(x, y, z)
    place(x + 1, y, z, 'ntm:machine_assembly_machine')
    time.sleep(1.5)
    open_at(x + 1, y, z)
    click('Escolher receita')
    shot('m3-assembly-list')
    pick_row(row_of('assembly', 'ass.plateiron'))
    shot('m3-assembly-recipe')
    hold('minecraft:iron_ingot', 5)
    open_at(x + 1, y, z)
    click('Pôr itens da mão')
    wait_until(lambda: inv('minecraft:iron_ingot') == 0, 60, 'os lingotes entrarem')
    shot('m3-assembly-working')

    def taken():
        open_at(x + 1, y, z)
        click('Retirar o que ficou pronto', 3.0)
        return inv('ntm:plate_iron') >= 5
    t = wait_until(taken, 150, '5 placas no inventário')
    evidence['assembly'] = {'plates': inv('ntm:plate_iron'), 'seconds_for_5': round(t, 1)}
    print('montadora ok', evidence['assembly'])


def fluids():
    x, z = 3530, -1390
    y = ground(x + 1, z + 3)
    ctl('fill', x - 1, y, z - 3, x + 6, y + 3, z + 3, 'minecraft:air')
    ctl('fill', x - 1, y - 1, z - 3, x + 6, y - 1, z + 3, 'minecraft:stone')  # piso plano
    # bateria | planta química | duto | duto | barril
    battery(x, y, z)
    place(x + 1, y, z, 'ntm:machine_chemical_plant')
    place(x + 2, y, z, 'ntm:fluid_duct_neo')
    place(x + 3, y, z, 'ntm:fluid_duct_neo')
    place(x + 4, y, z, 'ntm:barrel_steel')
    time.sleep(2)
    # Barril: 16 tanques de ar, modo saída.
    for _ in range(16):
        hold('ntm:fluid_tank_full__air', 1)
        open_at(x + 4, y, z)
        click('Despejar o recipiente cheio da mão', 2.0)
    open_at(x + 4, y, z)
    click('Modo', 1.0)  # Entrada → Buffer
    click('Modo', 1.0)  # → Saída
    shot('m3-barrel')
    evidence['air_tanks_back'] = inv('ntm:fluid_tank_empty')
    # Dutos: tipo pelo recipiente da mão.
    for dx in (2, 3):
        hold('ntm:fluid_tank_full__air', 1)
        open_at(x + dx, y, z)
        click('Usar o fluido do recipiente na mão')
    # Planta química: oxigênio (a 1ª receita: ar → oxigênio).
    open_at(x + 1, y, z)
    maybe('Voltar')
    click('Escolher receita')
    pick_row(row_of('chemical_plant', 'chem.oxygen'))
    hold('ntm:gas_empty', 1)

    def filled():
        open_at(x + 1, y, z)
        maybe('Fluidos e recipientes')
        click('Encher o recipiente vazio da mão', 3.0)
        return inv('ntm:gas_full__oxygen') >= 1
    t = wait_until(filled, 240, 'o tanque de gás cheio de oxigênio (1000 mB)')
    open_at(x + 1, y, z)
    maybe('Fluidos e recipientes')
    shot('m3-chemical-fluids')
    evidence['fluids'] = {'oxygen_gas_tanks': inv('ntm:gas_full__oxygen'), 'seconds_to_1000mB': round(t, 1)}
    print('fluidos ok', evidence['fluids'])


def gascent():
    chat('/clear')  # a barra cheia das etapas anteriores
    x, z = 3530, -1380
    y = ground(x + 1, z + 3)
    ctl('fill', x - 1, y, z - 3, x + 3, y + 3, z + 3, 'minecraft:air')
    ctl('fill', x - 1, y - 1, z - 3, x + 6, y - 1, z + 3, 'minecraft:stone')  # piso plano
    battery(x, y, z)
    place(x + 1, y, z, 'ntm:machine_gascent')
    time.sleep(1.5)
    for _ in range(2):
        hold('ntm:cell_uf6', 1)
        open_at(x + 1, y, z)
        click('Despejar UF6/PuF6 da mão')
    shot('m3-gascent')

    def nugget():
        open_at(x + 1, y, z)
        click('Retirar os produtos', 3.0)
        return inv('ntm:nugget_u238') >= 1
    t = wait_until(nugget, 150, 'a pepita de U238')
    evidence['gascent'] = {'u238': inv('ntm:nugget_u238'), 'cells_back': inv('ntm:cell_empty'), 'seconds_first_stage': round(t, 1)}
    print('centrífuga de gás ok', evidence['gascent'])


for s in steps:
    globals()[s]()
(game / 'm3-evidence.json').write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

#!/usr/bin/env python3
"""Aceitação do PUREX no motor ntm-visual (docs/maquinas.md, "PUREX").

Uso: python3 tools/test-game/purex.py <pasta do jogo> [etapa...]
O jogo aberto num mundo criativo pelo tools/test-game/run-visual.sh (Xvfb :15,
mudo), com os mods de tools/build-visual.sh. Etapas:
  pwr      o multibloco 5×5×5 com uma bateria cheia junto da borda; a receita
           do combustível PWR MEU escolhida na lista (botão 0 da tela com
           slots abre a tela antiga); querosene e ácido nítrico despejados
           de barris pela tela antiga; 2 hastes MEU nos slots: 6 pepitas de
           U-238, 8 de plutônio e 4 de tecnécio;
  trocar   uma haste MEN posta: a máquina troca de receita no grupo e dá
           4 pepitas de Pu-239.
Grava <pasta>/purex-evidence.json e capturas purex-*.png.
"""
import json, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:] or ['pwr', 'trocar']
CTL = ['/home/sacer/Projects/horizon-ntm/horizon-visual/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
ev_path = game / 'purex-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}
import os
X, Z = int(os.environ.get('PUREX_X', 3760)), -1398
TABLE = json.loads((Path(__file__).resolve().parents[2] / 'crates/ntm-core/data/machines/purex.json').read_text())


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
    p = game / f'purex-y-{X}'
    if p.exists():
        return int(p.read_text())
    ctl('tp', X + 0.5, 120, Z + 8.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    y = int(ctl('state')['player']['position'][1])
    p.write_text(str(y))
    return y


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


def find(block, y):
    for b in ctl('blocks', X - 6, y, Z - 6, X + 6, y, Z + 6).get('blocks', []):
        if b[3].startswith(block + '['):
            return b[:3]
    raise SystemExit(f'{block} não achado')


def core():
    return find('ntm:machine_purex', floor_y())


def open_slots():
    """The slot screen, through the proxy in front of the core (south)."""
    (cx, y, cz) = core()
    close()
    ctl('tp', cx + 0.5, y, cz + 5.5)
    ctl('interact', cx, y, cz + 2)
    time.sleep(1.5)


def put(item, n, slot):
    give(item, n)
    hot = [s['slot'] for s in screen()['slots'] if s.get('item') == item and s['slot'] < 36]
    ctl('click-slot', hot[0])
    time.sleep(0.3)
    ctl('click-slot', slot)
    time.sleep(0.5)


def pwr():
    y = floor_y()
    ctl('fill', X - 6, y, Z - 6, X + 8, y + 6, Z + 8, 'minecraft:air')
    ctl('fill', X - 6, y - 1, Z - 6, X + 8, y - 1, Z + 8, 'minecraft:stone')
    ctl('command', 'clear @s', check=False)
    # Placed from the south: the engine centres the 5×5×5 off the click.
    ctl('tp', X + 0.5, y, Z + 6.5)
    give('ntm:machine_purex', 1)
    ctl('place', X, y, Z + 2, '--item', 'ntm:machine_purex')
    time.sleep(2)
    close()
    (cx, _, cz) = core()
    # A full battery against the bottom ring's east side, in output mode.
    bx = cx + 3
    ctl('tp', bx + 0.5, y, cz + 3.5)
    give('ntm:machine_battery', 1)
    ctl('place', bx, y, cz, '--item', 'ntm:machine_battery')
    time.sleep(1.5)
    close()
    ctl('look-at-block', bx, y, cz)
    ctl('key', 'r', 'down')
    time.sleep(0.3)
    ctl('key', 'r', 'up')
    time.sleep(2.5)
    for _ in range(12):
        click_label('Teste: +10%', 1.0)
    click_label('Trocar modo', 1.0)
    close()
    # The recipe on the old screen's list (button 0 of the slot screen).
    row = next(i for i, r in enumerate(TABLE['recipes']) if r['name'] == 'purex.pwrmeu')
    open_slots()
    ctl('click-button', 'Escolher receita')
    time.sleep(2)
    ctl('screenshot', game / 'purex-lista.png')
    if row > 6:
        raise SystemExit(f'linha {row} fora da vista')
    ctl('click-at', 150, 50 + 12 * row)
    time.sleep(1.5)
    # Back on the old screen's machine page: its fluids, poured from barrels.
    close()
    for barrel in ('ntm:fluid_barrel_full__kerosene', 'ntm:fluid_barrel_full__nitric_acid'):
        give(barrel, 1)
        ctl('select', barrel)
        open_slots()
        ctl('click-button', 'Escolher receita')
        time.sleep(2)
        click_label('Voltar')
        click_label('Fluidos e recipientes')
        click_label('Despejar o recipiente cheio da mão')
        close()
    open_slots()
    put('ntm:pwr_fuel_depleted__meu', 2, 304)
    ctl('screenshot', game / 'purex-trabalhando.png')
    t0 = time.time()
    while time.time() - t0 < 120:
        time.sleep(4)
        open_slots()  # the old screen may have taken its place
        tc = sum(s[2] for s in machine_slots() if s[1] == 'ntm:nugget_technetium')
        if tc >= 4:
            break
    out = machine_slots()
    ctl('screenshot', game / 'purex-pronto.png')
    close()
    cnt = lambda i: sum(s[2] for s in out if s[1] == i)
    evidence['pwr'] = {'nucleo': core(), 'saidas': out, 'segundos': round(time.time() - t0, 1),
                       'ok': (cnt('ntm:nugget_u238'), cnt('ntm:nugget_plutonium'), cnt('ntm:nugget_technetium')) == (6, 8, 4),
                       'vazios': ctl('inventory').get('counts', {})}
    print('pwr', evidence['pwr'])


def trocar():
    open_slots()
    put('ntm:pwr_fuel_depleted__men', 1, 305)
    t0 = time.time()
    while time.time() - t0 < 60:
        time.sleep(4)
        open_slots()
        if any(s[1] == 'ntm:nugget_pu239' for s in machine_slots()):
            break
    out = machine_slots()
    close()
    evidence['trocar'] = {'saidas': out, 'pu239': sum(s[2] for s in out if s[1] == 'ntm:nugget_pu239'), 'segundos': round(time.time() - t0, 1)}
    print('trocar', evidence['trocar'])


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in (game / 'client.log').read_text(errors='replace')
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

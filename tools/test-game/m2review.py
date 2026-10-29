#!/usr/bin/env python3
"""Verificação em jogo das correções da revisão do M2 (docs/bombas.md, "Revisão").

Uso: python3 tools/test-game/m2review.py <pasta do jogo> <etapa...>
O jogo já aberto num mundo criativo (horizon-ctl open), mudo, em Xvfb
(tools/test-game/run-xvfb.sh). Etapas:
  cargas     3 C4 ligadas pelo mesmo sinal: todas explodem, uma por tick;
  nuke       /ntmbomba teste 40 até o fim;
  sair       /ntmbomba teste 60 e sai do jogo no meio (quit salva);
  voltar     depois de reabrir o mundo: a explosão volta e termina.
Grava <pasta>/m2review-evidence.json e capturas *.png.
"""
import json, re, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1])
steps = sys.argv[2:]
CTL = ['/home/sacer/Games/horizon/target/release/horizon-ctl', '--socket', str(game / 'control.sock')]
LOG = game / 'client.log'
ev_path = game / 'm2review-evidence.json'
evidence = json.loads(ev_path.read_text()) if ev_path.exists() else {}


def ctl(*args, check=True):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True, timeout=300).stdout.strip()
    try:
        j = json.loads(out.splitlines()[-1]) if out else {}
    except ValueError:
        j = {'raw': out}
    if check and isinstance(j, dict) and j.get('error'):
        raise SystemExit(f'{args}: {j["error"]}')
    return j


def log():
    return LOG.read_text(errors='replace')


def chat(cmd, wait=1.5):
    ctl('chat', cmd)
    time.sleep(wait)


def ground(x, z):
    ctl('tp', x + 0.5, 120, z + 0.5)
    time.sleep(4)
    ctl('wait-ready', 120, check=False)
    return int(ctl('state')['player']['position'][1])


def block(x, y, z):
    return ctl('block', x, y, z).get('state', '')


def wait_log(pattern, seconds, start=0):
    t0 = time.time()
    while time.time() - t0 < seconds:
        m = re.search(pattern, log()[start:])
        if m:
            return time.time() - t0, m
        time.sleep(1)
    raise SystemExit(f'esperou {seconds}s por {pattern!r}')


def cargas():
    x, z = 3600, -1400
    y = ground(x + 3, z + 2)  # ao alcance das três
    ctl('fill', x - 2, y, z - 2, x + 8, y + 3, z + 2, 'minecraft:air')
    ctl('fill', x - 4, y - 6, z - 4, x + 10, y - 1, z + 4, 'minecraft:stone')
    for dx in (0, 3, 6):
        ctl('give', 'ntm:c4', 1)
        time.sleep(0.5)
        ctl('close', check=False)
        r = ctl('place', x + dx, y, z, '--item', 'ntm:c4', check=False)
        if r.get('error'):
            raise SystemExit(f'place c4: {r}')
        time.sleep(1.0)
    ctl('tp', x + 3.5, y + 1, z + 10.5)
    time.sleep(2)
    start = len(log())
    # O mesmo sinal para as três: uma fileira de blocos de redstone por cima.
    ctl('fill', x, y + 1, z, x + 6, y + 1, z, 'minecraft:redstone_block')
    t0 = time.time()
    while sum('ntm:c4 at' in l for l in log()[start:].splitlines()) < 3:
        if time.time() - t0 > 30:
            raise SystemExit('as três C4 não explodiram em 30 s')
        time.sleep(1)
    t = time.time() - t0
    lines = [l for l in log()[start:].splitlines() if 'ntm:c4 at' in l]
    gone = [block(x + dx, y, z) for dx in (0, 3, 6)]
    dug = [block(x + dx, y - 2, z) for dx in (0, 3, 6)]
    ctl('tp', x + 3.5, y + 12, z + 12)
    ctl('look-at', x + 3, y - 2, z)
    time.sleep(2)
    ctl('screenshot', game / 'm2review-cargas.png')
    evidence['cargas'] = {'logs': lines, 'blocos': gone, 'abaixo': dug, 'segundos': round(t, 1)}
    print('cargas', evidence['cargas'])


def nuke():
    x, z = 3750, -1400
    ground(x, z)
    chat('/ntmbomba config mundo raiomax 60')
    start = len(log())
    chat('/ntmbomba teste 40')
    t, m = wait_log(r'explosion done: (\S+).*', 300, start)
    ctl('tp', x + 0.5, 160, z + 60)
    ctl('look-at', x, 60, z)
    time.sleep(3)
    ctl('screenshot', game / 'm2review-nuke.png')
    evidence['nuke'] = {'linha': m.group(0)[:300], 'segundos': round(t, 1)}
    print('nuke', evidence['nuke'])


def sair():
    x, z = 3950, -1400
    ground(x, z)
    chat('/ntmbomba config mundo raiomax 60')
    start = len(log())
    chat('/ntmbomba teste 60', wait=0.5)
    _, m = wait_log(r'explosion (w\S+), radius 60', 60, start)
    time.sleep(2)
    evidence['sair'] = {'id': m.group(1)}
    print('sair', evidence['sair'])
    ctl('quit', check=False)


def voltar():
    _, m = wait_log(r'explosion done: (\S+).*', 400)
    evidence['voltar'] = {'retomada': 'explosion resumed' in log() or 'Resumed' in log(), 'linha': m.group(0)[:300]}
    print('voltar', evidence['voltar'])


for s in steps:
    globals()[s]()
evidence['took_too_long'] = 'took too long' in log()
ev_path.write_text(json.dumps(evidence, indent=1, ensure_ascii=False))
print(json.dumps(evidence, ensure_ascii=False))

#!/usr/bin/env python3
"""M3 content-only registry, state, inventory and real block-loot smoke test."""
import json
import socket
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / 'content-gen-evidence/m3'
path_deadline = time.monotonic() + 360
while not (EVIDENCE / 'run-path.txt').exists():
    if time.monotonic() > path_deadline: raise RuntimeError('Launcher has not left the game queue')
    time.sleep(1)
scratch = Path((EVIDENCE / 'run-path.txt').read_text().strip())
results = []; failures = []


def ctl(command):
    with socket.socket(socket.AF_UNIX) as sock:
        sock.settimeout(60); sock.connect(str(scratch / 'control.sock'))
        sock.sendall((command + '\n').encode())
        reply = json.loads(sock.makefile().readline())
    results.append({'command': command, 'reply': reply}); time.sleep(.2)
    if 'error' in reply: raise RuntimeError((command, reply))
    return reply


def command(text):
    reply = ctl('command ' + text)
    if not reply.get('ok'): raise RuntimeError((text, reply))
    return reply


def check(label, value):
    print(label, bool(value), flush=True)
    if not value: failures.append(label)


try:
    deadline = time.monotonic() + 360
    while not (scratch / 'control.sock').exists():
        if time.monotonic() > deadline: raise RuntimeError('Client startup timeout')
        time.sleep(1)
    for _ in range(180):
        if ctl('state').get('screen') == 'playing': break
        time.sleep(1)
    else: raise RuntimeError('World startup timeout')
    command('gamemode creative @s'); command('time set noon'); command('weather clear')
    command('fill -14 100 -14 14 100 14 minecraft:stone')
    command('fill -12 101 -12 12 108 12 minecraft:air')
    command('tp @s 0.5 101 -7.5')
    machines = ['machine_assembly_machine', 'machine_chemical_plant', 'machine_centrifuge', 'machine_gascent', 'barrel_steel', 'machine_fluidtank']
    for index, name in enumerate(machines):
        x = -7 + index * 3
        command(f'setblock {x} 101 0 ntm:{name}[facing=north,honey_level={index}]')
        block = ctl(f'block {x} 101 0')
        check('M3 state ' + name, block.get('block') == 'ntm:' + name and f'honey_level={index}' in block.get('state', ''))
    command('setblock 0 101 3 ntm:fluid_duct_neo')
    check('static duct', ctl('block 0 101 3').get('block') == 'ntm:fluid_duct_neo')
    command('setblock 0 105 -11 minecraft:stone'); command('tp @s 0.5 106 -10.5')
    ctl('look 0 27'); ctl('key f1 tap')
    ctl('screenshot ' + str(EVIDENCE / '01-m3-machines.png')); ctl('key f1 tap')
    command('clear @s')
    items = ['fluid_tank_full__water', 'fluid_barrel_full__sulfuric_acid', 'canister_full__solvent', 'gas_full__hydrogen', 'gas_full__xenon', 'fluid_tank_lead_full__redmud', 'fluid_pack_full__water', 'fluid_tank_full__uf6', 'shell__steel', 'pipe__steel', 'bolt__steel', 'fluorite']
    for name in items: command('give @s ntm:' + name + ' 1')
    inventory = ctl('inventory')['counts']
    check('all 12 representative items', all(inventory.get('ntm:' + name) == 1 for name in items))
    command('gamemode survival @s'); ctl('open-inventory')
    ctl('screenshot ' + str(EVIDENCE / '02-m3-containers.png')); ctl('close')
    command('clear @s'); command('give @s minecraft:netherite_pickaxe 1'); ctl('select 1')
    command('setblock 8 101 -3 ntm:barrel_steel[facing=west,honey_level=5]')
    command('tp @s 8.5 101 -5.5'); ctl('break 8 101 -3')
    command('tp @s 8.5 101 -2.5'); ctl('wait 30')
    check('steel barrel drops itself', ctl('inventory')['counts'].get('ntm:barrel_steel') == 1)
except Exception as error:
    failures.append('exception: ' + str(error))
    raise
finally:
    (EVIDENCE / 'game-validation.json').write_text(json.dumps({'status': 'fail' if failures else 'pass', 'scratch': str(scratch), 'commands': len(results), 'failures': failures, 'results': results}, ensure_ascii=False, indent=2) + '\n')
    if (scratch / 'control.sock').exists(): ctl('quit')
if failures: raise SystemExit(1)

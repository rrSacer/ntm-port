#!/usr/bin/env python3
"""A mined cable leaves its network (the review of round 5, `owned_cells`), in game, read from the server's models:

  cable   a battery that gives (a creative battery in the hopper over it, a redstone block under it), five `ntm:red_cable` in a row
          and a battery that takes, its server model's `power` rising. An excavator (a creative battery and an iron drill bit,
          as native_mining.py) over the row mines the three cables in its footprint: the cells turn to air, the sink's power
          stops rising (the network lost the link: the broken cable is a block change the server heard of, which a commit
          of the mining step must not mark as its own) and the cables left stand.

The other side of `owned_cells`, a cell born with its machine, is the slag tap's own scenarios (`run-foundry-proof.py --scenario
slagtap-pour`): the slag the pour makes keeps its 144 and is not started again as an empty machine.

  python3 tools/test-game/run-mining-cable-game.py --binary B --resources R --bundle H --scratch-root S
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
spec = importlib.util.spec_from_file_location('run_crash_proof', HERE / 'run-crash-proof.py')
rcp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rcp)
from native_game import Game
from native_mining import open_excavator, place_machine, state
from native_recipes import screen_json


def power(g, p):
    text = ' '.join(g.command('data get block %d %d %d ntm' % tuple(p)).get('chat', [])).replace('\\"', '"')
    found = re.search(r'"power":(\d+)', text)
    return int(found.group(1)) if found else None


def server_has_cable(g, p):
    """`ntmtest cable`: the server's own set of cables (NTM_TEST_HOOKS=1), not the world's block."""
    text = ' '.join(g.command('ntmtest cable %d %d %d' % tuple(p)).get('chat', []))
    found = re.search(r'cable=(\d)', text)
    return None if found is None else found.group(1) == '1'


def play(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    x, y, z = 20, 100, 8
    g.ground(x - 6, y, z - 4, width=16, height=18)
    core = place_machine(g, 'machine_excavator', x, y, z)
    cx, cy, cz = core
    row = cy - 4
    g.evidence['core'] = list(core)
    # The row: the giving battery, five cables (x-2..x+2: three in the excavator's footprint, x-1..x+1), the taking battery.
    g.command(f'fill {cx - 5} {row - 1} {cz} {cx + 5} {row - 1} {cz} minecraft:stone')
    g.command(f'fill {cx - 5} {row} {cz - 1} {cx + 5} {row + 3} {cz + 1} minecraft:air')
    source, sink = (cx - 3, row, cz), (cx + 3, row, cz)
    cables = [(cx + d, row, cz) for d in range(-2, 3)]
    g.command('setblock %d %d %d ntm:machine_battery[facing=south]' % source)
    g.command('setblock %d %d %d minecraft:redstone_block' % (source[0], row - 1, source[2]))
    g.command('setblock %d %d %d ntm:machine_battery[facing=south]' % sink)
    for c in cables:
        g.command('setblock %d %d %d ntm:red_cable' % c)
    g.command('setblock %d %d %d minecraft:hopper[facing=down]' % (source[0], row + 1, source[2]))
    g.command('item replace block %d %d %d container.0 with ntm:battery_creative 1' % (source[0], row + 1, source[2]))
    time.sleep(3)
    first = power(g, sink)
    time.sleep(6)
    second = power(g, sink)
    g.evidence['power_connected'] = [first, second]
    g.check('the_network_carries_power', first is not None and second is not None and second > first, [first, second])
    # The excavator over the row, a creative battery and an iron drill bit, started.
    open_excavator(g, core)
    g.put('ntm:battery_creative', 1, 0)
    g.put('ntm:drillbit__ferro', 1, 4)
    screen = screen_json('machine_excavator')
    button = next(b for b in screen['buttons'] if b['id'] == 0)
    g.button(button['rect'], screen['size'])
    time.sleep(.6)
    g.close()
    deadline = time.monotonic() + 180
    mined = lambda: [c for c in cables if state(g, c).startswith('minecraft:air')]
    # The first cable that turns to air, and at once what the server's network says of it: a broken cable is a block change
    # the server hears of in the next tick (a sweep that finds a missing one takes 20 to 40 ticks, a second or two). A mining step
    # that marked its cells as its own would leave it there until then.
    first = None
    while time.monotonic() < deadline and first is None:
        gone = mined()
        if gone:
            first = gone[0]
            held = server_has_cable(g, first)
            g.evidence['server_holds_the_mined_cable'] = held
            g.check('the_server_drops_the_mined_cable_at_once', held is False, held)
        else:
            time.sleep(.05)
    while time.monotonic() < deadline and len(mined()) < 3:
        time.sleep(.5)
    gone = mined()
    g.evidence['mined'] = gone
    g.check('the_cables_in_the_footprint_were_mined', len(gone) == 3 and (cx, row, cz) in gone, gone)
    time.sleep(2)
    after = [power(g, sink)]
    time.sleep(8)
    after.append(power(g, sink))
    g.evidence['power_after'] = after
    g.check('the_network_lost_the_link', after[0] is not None and after[1] == after[0], after)
    left = [c for c in cables if c not in gone and state(g, c).startswith('ntm:red_cable')]
    g.check('the_other_cables_stand', len(left) == 2, left)
    g.ctl('save')
    g.save()


def main():
    if len(sys.argv) > 2 and sys.argv[2] == 'play':
        play(Game(sys.argv[1], 'mining-cable'))
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix='mining-cable-', dir=args.scratch_root))
    print(folder, flush=True)
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='')
    env['NTM_TEST_HOOKS'] = '1'
    for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'), ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
        env[key] = str(folder / sub)
    (folder / 'options.txt').write_text('soundCategory_master:0.0\nmaxFps:30\nenableVsync:false\nguiScale:2\ntutorialStep:none\n')
    (folder / 'horizon-controller.txt').write_text('enabled:false\n')
    reader, writer = os.pipe()
    xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0', '1280x720x24', '-nolisten', 'tcp'],
                            pass_fds=(writer,), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.close(writer)
    with os.fdopen(reader) as pipe:
        env['DISPLAY'] = ':' + pipe.readline().strip()
    args.binary, args.bundle = args.binary.resolve(), args.bundle.resolve()
    world = rcp.World(args, folder, env)
    report = {'binary_sha256': rcp.digest(args.binary), 'bundle_sha256': rcp.digest(args.bundle)}
    try:
        world.launch()
        code = subprocess.run([sys.executable, str(Path(__file__).resolve()), str(folder), 'play'], cwd=folder, env=env, timeout=900).returncode
        world.quit()
        evidence = json.loads((folder / 'native-mining-cable.json').read_text())
        report['checks'] = evidence.get('checks')
        report['evidence'] = {k: evidence.get(k) for k in ('core', 'power_connected', 'mined', 'power_after', 'server_holds_the_mined_cable')}
        report['ok'] = code == 0
    finally:
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (folder / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report.get(k) for k in ('ok', 'checks', 'evidence')}, indent=1), flush=True)
    return 0 if report.get('ok') else 1


if __name__ == '__main__':
    sys.exit(main())

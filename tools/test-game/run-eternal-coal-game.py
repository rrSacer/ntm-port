#!/usr/bin/env python3
"""The eternal coal in game (`FuelHandler`: coal_eternal is its own container item): in a firebox it burns for ever. A firebox
with one `ntm:coal_eternal` lights it (heat rises) and the coal is still in its slot, a stack of 1, minutes later.

  python3 tools/test-game/run-eternal-coal-game.py --binary B --resources R --bundle H --scratch-root S
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
from native_combustion import start_area, machine_state


def play(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    x, y, z = 47, 101, 8
    start_area(g, x - 4, y, z, 12)
    fire = (x, y, z)
    g.command('setblock %d %d %d ntm:heater_firebox[facing=south,honey_level=0]' % fire)
    time.sleep(2)
    g.command('item replace block %d %d %d container.0 with ntm:coal_eternal 1' % fire)
    heats = []
    for _ in range(3):
        time.sleep(15)
        state = machine_state(g, fire)
        found = re.search(r'"heat":(\d+)', state)
        heats.append(int(found.group(1)) if found else None)
    items = ' '.join(g.command('data get block %d %d %d Items' % fire).get('chat', []))
    g.evidence['heats'] = heats
    g.evidence['items'] = items[:300]
    g.check('the_firebox_heats', heats[0] and heats[-1] > heats[0], heats)
    g.check('the_coal_is_still_there', 'ntm:coal_eternal' in items and 'count: 1' in items.replace('b,', ','), items)
    g.ctl('save')
    g.save()


def main():
    if len(sys.argv) > 2 and sys.argv[2] == 'play':
        play(Game(sys.argv[1], 'eternal-coal'))
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix='eternal-coal-', dir=args.scratch_root))
    print(folder, flush=True)
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='')
    env.pop('NTM_TEST_HOOKS', None)
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
        evidence = json.loads((folder / 'native-eternal-coal.json').read_text())
        report['checks'] = evidence.get('checks')
        report['evidence'] = {k: evidence.get(k) for k in ('heats', 'items')}
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

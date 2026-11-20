#!/usr/bin/env python3
"""The wood burner's liquid burn in game (`TileEntityMachineWoodBurner`): a steel barrel of woodoil (four tanks put in by its screen)
behind it, a duct typed woodoil between them: the burner's tank fills through the duct, from the back only; its screen's button 1
switches to fluid and the tank burns 2 mB a tick for 55 HE a mB (woodoil is 110 000 TU a bucket), read from the server's model.

  python3 tools/test-game/run-wood-burner-game.py --binary B --resources R --bundle H --scratch-root S
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
from native_stations import fill_barrel, type_duct
from native_recipes import screen_json


def model_values(g, at):
    state = machine_state(g, at)
    mb = re.search(r'"tank":\{[^}]*"mb":(\d+)', state)
    power = re.search(r'"power":(\d+)', state)
    return (int(mb.group(1)) if mb else None, int(power.group(1)) if power else None)


def play(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    x, y, z = 47, 101, 8
    start_area(g, x - 4, y, z, 12)
    burner, duct, barrel = (x, y, z), (x, y, z + 1), (x, y, z + 2)
    side_duct, side_barrel = (x + 1, y, z), (x + 2, y, z)
    g.command('setblock %d %d %d ntm:machine_wood_burner[facing=north,honey_level=0]' % burner)
    g.command('setblock %d %d %d ntm:fluid_duct_neo' % duct)
    g.command('setblock %d %d %d ntm:barrel_steel[facing=north,honey_level=0]' % barrel)
    g.command('setblock %d %d %d ntm:fluid_duct_neo' % side_duct)
    g.command('setblock %d %d %d ntm:barrel_steel[facing=north,honey_level=0]' % side_barrel)
    time.sleep(2)
    type_duct(g, 'woodoil', [duct, side_duct], (x + 4, y, z + 4))
    fill_barrel(g, barrel, 4, item='ntm:fluid_tank_full__woodoil')
    fill_barrel(g, side_barrel, 4, item='ntm:fluid_tank_full__woodoil')
    time.sleep(8)
    mb, power = model_values(g, burner)
    g.evidence['filled'] = [mb, power]
    g.check('the_back_duct_filled_the_tank_and_the_side_one_did_not', mb == 4000 and power == 0, [mb, power])
    # The screen's button 1: solid / fluid (the burner is a single block here).
    g.close()
    g.command(f'tp @s {x + .5} {y} {z - 3.5}')
    time.sleep(1)
    g.ctl('interact %d %d %d north' % burner)
    time.sleep(1)
    g.check('the_burner_screen_is_open', g.ctl('screen').get('screen') == 'container', g.ctl('screen').get('screen'))
    screen = screen_json('machine_wood_burner')
    button = next(b for b in screen['buttons'] if b['id'] == 1)
    g.button(button['rect'], screen['size'])
    time.sleep(.6)
    g.close()
    time.sleep(6)
    mb2, power2 = model_values(g, burner)
    g.evidence['burning'] = [mb2, power2]
    g.check('it_burns_the_tank', mb2 is not None and mb2 < 4000 and power2 > 0, [mb2, power2])
    # The sum is exact: every mB burnt is 55 HE (the last odd one is 55 too).
    g.check('55_he_a_mb', power2 == 55 * (4000 - mb2), [mb2, power2])
    g.ctl('save')
    g.save()


def main():
    if len(sys.argv) > 2 and sys.argv[2] == 'play':
        play(Game(sys.argv[1], 'wood-burner'))
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix='wood-burner-', dir=args.scratch_root))
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
        evidence = json.loads((folder / 'native-wood-burner.json').read_text())
        report['checks'] = evidence.get('checks')
        report['evidence'] = {k: evidence.get(k) for k in ('filled', 'burning')}
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

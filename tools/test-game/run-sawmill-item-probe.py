#!/usr/bin/env python3
"""D3 of Onda 2, step 1 (the coordinator's condition before the sawmill leaves `Inert`): in game, the sawmill item's
`minecraft:custom_data {"ntm:no_blade":1b}` reaches `native_place_block`, and a broken sawmill drops that variant back
(MachineSawmill.onBlockPlacedBy / getDrops, the Java's damage 1). Until the sawmill keeps `has_blade` in its model
(step 3) the glue keeps what the placed item said by its cell, with NTM_TEST_HOOKS=1 only.

  python3 tools/test-game/run-sawmill-item-probe.py --binary B --resources R --bundle H --scratch-root S

Phases, one session: a plain sawmill placed and broken by a player in survival drops the plain item; the variant placed
and broken drops the variant; that drop picked up and placed again is read as the variant and drops it again; a sawmill
broken in creative drops nothing (the engine's loot rules stay). The glue's log lines (`ntm d3 placed`, `ntm d3 drops`)
are read from the session's log.
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
from native_combustion import start_area
from native_recipes import core_of

SAWMILL = 'ntm:machine_sawmill'
VARIANT = SAWMILL + '[minecraft:custom_data={"ntm:no_blade":1b}]'
SPOTS = {'plain': (47, 101, 8), 'variant': (55, 101, 8), 'again': (63, 101, 8), 'creative': (71, 101, 8)}


def place(g, item, spot):
    x, y, z = spot
    g.command('clear @s')
    g.command('gamemode creative @s')
    g.command(f'give @s {item} 1')
    g.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(1)
    g.ctl(f'place {x} {y} {z} --item {SAWMILL}')
    time.sleep(1)
    return core_of(g, SAWMILL, x, y, z, reach=(3, 3))


def break_by_player(g, core, survival=True):
    g.command('clear @s')
    g.command('gamemode ' + ('survival' if survival else 'creative') + ' @s')
    g.command(f'tp @s {core[0] + .5} {core[1]} {core[2] + 3.5}')
    if survival:
        g.command('give @s minecraft:diamond_pickaxe 1')
        g.ctl('select minecraft:diamond_pickaxe')
    for dx, dy, dz in [(0, 0, 1), (0, 1, 1), (-1, 0, 1), (1, 0, 1), (0, 1, 0), (0, 0, 0)]:
        cell = (core[0] + dx, core[1] + dy, core[2] + dz)
        try:
            g.ctl('break %d %d %d --timeout 30' % cell)
            time.sleep(1.5)
            return cell
        except RuntimeError:
            continue
    return None


def dropped(g, core):
    """The sawmill items lying around `core`, with the components each says it has."""
    out = []
    for e in g.ctl('entities 20')['entities']:
        if e.get('kind') != 'item' or e['item']['item'] != SAWMILL:
            continue
        # Only what this sawmill dropped: the phases stand 8 blocks apart.
        if max(abs(e['pos'][0] - core[0] - .5), abs(e['pos'][2] - core[2] - .5)) > 4:
            continue
        text = ' '.join(g.command(f'data get entity @e[type=minecraft:item,limit=1,sort=nearest,x={e["pos"][0]},y={e["pos"][1]},z={e["pos"][2]}] Item').get('chat', []))
        out.append({'count': e['item']['count'], 'pos': e['pos'], 'no_blade': bool(re.search(r'"?ntm:no_blade"?\s*:\s*1b', text)), 'nbt': text[-300:]})
    return out


def probe(g):
    g.ctl('wait-ready 120')
    start_area(g, 44, 101, 8, 32)
    results = {}
    for phase in ('plain', 'variant'):
        core = place(g, SAWMILL if phase == 'plain' else VARIANT, SPOTS[phase])
        cell = break_by_player(g, core)
        drops = dropped(g, core)
        results[phase] = {'core': core, 'broken': cell, 'drops': drops}
        g.check(f'{phase}_broken', cell is not None, cell)
        g.check(f'{phase}_drops_one', sum(d['count'] for d in drops) == 1, drops)
        g.check(f'{phase}_drop_is_{"variant" if phase == "variant" else "plain"}',
                all(d['no_blade'] == (phase == 'variant') for d in drops), drops)
    # The variant's drop picked up by the player, placed again and broken again.
    drop = results['variant']['drops'][0]['pos']
    g.command('clear @s')
    g.command('gamemode survival @s')
    g.command(f'tp @s {drop[0]} {drop[1]} {drop[2]}')
    time.sleep(2)
    inv = ' '.join(g.command('data get entity @s Inventory').get('chat', []))
    results['picked'] = {'inventory': inv[-400:], 'no_blade': bool(re.search(r'"?ntm:no_blade"?\s*:\s*1b', inv))}
    g.check('picked_up_variant_keeps_its_data', results['picked']['no_blade'], inv[-400:])
    x, y, z = SPOTS['again']
    g.command('gamemode creative @s')
    g.command(f'tp @s {x + .5} {y} {z + 3.5}')
    time.sleep(1)
    g.ctl(f'place {x} {y} {z} --item {SAWMILL}')
    time.sleep(1)
    core = core_of(g, SAWMILL, x, y, z, reach=(3, 3))
    cell = break_by_player(g, core)
    drops = dropped(g, core)
    results['again'] = {'core': core, 'broken': cell, 'drops': drops}
    g.check('again_drops_the_variant', sum(d['count'] for d in drops) == 1 and all(d['no_blade'] for d in drops), drops)
    # Creative: the engine's loot rules stay (nothing dropped).
    core = place(g, VARIANT, SPOTS['creative'])
    cell = break_by_player(g, core, survival=False)
    drops = dropped(g, core)
    results['creative'] = {'core': core, 'broken': cell, 'drops': drops}
    g.check('creative_drops_nothing', cell is not None and not drops, drops)
    g.evidence['results'] = results
    g.save()


def main():
    if len(sys.argv) > 2 and sys.argv[2] == 'probe':
        g = Game(sys.argv[1], 'sawmill-item-probe')
        probe(g)
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix='sawmill-item-', dir=args.scratch_root))
    print(folder, flush=True)
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='',
               NTM_TEST_HOOKS='1')
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
        code = subprocess.run([sys.executable, str(Path(__file__).resolve()), str(folder), 'probe'], cwd=folder, env=env, timeout=900).returncode
        world.quit()
        log = world.log.read_text(errors='replace')
        report['placed'] = re.findall(r'ntm d3 placed .*', log)
        report['drops'] = re.findall(r'ntm d3 drops .*', log)
        evidence = json.loads((folder / 'native-sawmill-item-probe.json').read_text())
        report['checks'] = evidence.get('checks')
        report['results'] = evidence.get('results')
        placed = [bool(re.search(r'no_blade=true', l)) for l in report['placed']]
        report['place_hook_reads_the_data'] = placed == [False, True, True, True]
        report['ok'] = code == 0 and report['place_hook_reads_the_data']
    finally:
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (folder / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report.get(k) for k in ('ok', 'place_hook_reads_the_data', 'placed', 'drops')}, indent=1), flush=True)
    return 0 if report.get('ok') else 1


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""The sawmill in game (Onda 2, step 3 of D3 and the sawmill), one session as a player plays it:

  saw      a sawmill on a firebox fed coal through a hopper (TileEntityHeaterFirebox: coal x2, 200 TU a tick; the sawmill
           takes 10%, under its 300): an oak log put in by hand (MachineSawmill.onBlockActivated) comes out as 6 oak
           planks (TileEntitySawmill.getOutput, 4 planks x 6 / 4), sawdust at the Java's 50%; a click takes them
  overspeed rocket fuel instead (x5, 500 TU a tick: over 300): past 300 hot ticks the blade breaks, one blast at
           [x+0.5, y+1, z+0.5] and then one `ntm:sawblade` one up in front of the core (R4: the blast first), each once
           in the effect trace (NTM_TRACE_EFFECTS=1); no block around changes
  repair   that blade picked up and used on the sawmill: it has its blade again, one fewer in hand

  python3 tools/test-game/run-sawmill-game.py --binary B --resources R --bundle H --scratch-root S
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
from native_recipes import core_of

SAWMILL = 'ntm:machine_sawmill'
BLADE = 'ntm:sawblade'
X, Y, Z = 47, 101, 8
LO, HI = (X - 6, Y - 1, Z - 6), (X + 8, Y + 5, Z + 6)
BLAST = re.compile(r'ntm: effect (\d+)\.(\d+) (explosion \[[^\]]*\] power=\S+ fire=\S+ breaks=\S+|items \[[^\]]*\] \[[^\]]*\])')


def has_blade(g, core):
    found = re.search(r'"has_blade":(true|false)', machine_state(g, core))
    return found is None or found.group(1) == 'true'


def slots(g, core):
    """The sawmill's slots: its block entity's (step 3c), else its model's (a binary from before)."""
    text = ' '.join(g.command('data get block %d %d %d Items' % tuple(core)).get('chat', []))
    found = [(i, n) for n, i in re.findall(r'count: ?(\d+)[^{}]*?id: ?"([a-z_:]+)"', text)]
    return found or re.findall(r'"id":"([a-z_:]+)","count":(\d+)', machine_state(g, core))


def area(g, own):
    out = g.ctl('blocks %d %d %d %d %d %d' % (*LO, *HI))
    return {f'{x},{y},{z}': s for x, y, z, s in out['blocks'] if f'{x},{y},{z}' not in own and not s.startswith(SAWMILL)}


def inventory(g, item):
    inv = g.ctl('inventory')
    stacks = [s for part in ('hotbar', 'main') for s in inv.get(part, []) or []]
    return sum(s.get('count', 0) for s in stacks if s and s.get('item') == item)


def use(g, core, item=None):
    """A click on the first cell of the sawmill a player south of it sees (any cell is the sawmill)."""
    g.command('gamemode survival @s')
    if item:
        g.ctl(f'select {item}')
    g.command(f'tp @s {core[0] + .5} {core[1]} {core[2] + 4.5}')
    time.sleep(.5)
    for dx, dy, dz in [(0, 1, 1), (0, 0, 1), (-1, 1, 1), (1, 1, 1), (0, 1, 0), (0, 0, 0)]:
        try:
            g.ctl('interact %d %d %d south' % (core[0] + dx, core[1] + dy, core[2] + dz))
            g.evidence.setdefault('clicked', []).append([core[0] + dx, core[1] + dy, core[2] + dz])
            break
        except RuntimeError:
            continue
    else:
        raise AssertionError('no cell of the sawmill seen from the south')
    time.sleep(1)
    g.command('gamemode creative @s')


def play(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    start_area(g, X - 4, Y, Z, 16)
    g.command('clear @s')
    g.command(f'give @s {SAWMILL} 1')
    g.command(f'tp @s {X + .5} {Y} {Z + 3.5}')
    time.sleep(1)
    g.ctl(f'place {X} {Y} {Z} --item {SAWMILL}')
    time.sleep(1)
    core = core_of(g, SAWMILL, X, Y, Z, reach=(3, 3))
    g.evidence['core'] = list(core)
    firebox, hopper = (core[0], core[1] - 1, core[2]), (core[0] + 1, core[1] - 1, core[2])
    g.command('setblock %d %d %d ntm:heater_firebox[facing=south,honey_level=0]' % firebox)
    g.command('setblock %d %d %d minecraft:hopper[facing=west]' % hopper)
    own = {'%d,%d,%d' % firebox, '%d,%d,%d' % hopper}
    before = area(g, own)
    g.check('with_its_blade', has_blade(g, core), machine_state(g, core)[:400])
    # Saw: coal, a log by hand.
    g.command('item replace block %d %d %d container.0 with minecraft:coal 8' % hopper)
    time.sleep(8)
    g.command('clear @s')
    g.command('give @s minecraft:oak_log 2')
    use(g, core, 'minecraft:oak_log')
    g.check('the_log_went_in_one', inventory(g, 'minecraft:oak_log') == 1, slots(g, core))
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline and not any(i == 'minecraft:oak_planks' for i, _ in slots(g, core)):
        time.sleep(1)
    sawn = slots(g, core)
    g.evidence['sawn'] = sawn
    g.check('six_planks_out', ('minecraft:oak_planks', '6') in sawn and not any(i == 'minecraft:oak_log' for i, _ in sawn), sawn)
    g.command('clear @s')
    use(g, core)
    taken = {'planks': inventory(g, 'minecraft:oak_planks'), 'sawdust': inventory(g, 'ntm:powder_sawdust')}
    g.evidence['taken'] = taken
    g.check('the_outputs_taken_by_hand', taken['planks'] == 6 and taken['sawdust'] == sum(int(n) for i, n in sawn if i == 'ntm:powder_sawdust')
            and not slots(g, core), [taken, slots(g, core)])
    # Overspeed: rocket fuel, in a fresh firebox (the coal still in the old one keeps the fuel slot: 2000 ticks a coal).
    g.command('setblock %d %d %d minecraft:air' % hopper)
    g.command('setblock %d %d %d minecraft:stone' % firebox)
    time.sleep(1)
    g.command('setblock %d %d %d ntm:heater_firebox[facing=south,honey_level=0]' % firebox)
    g.command('setblock %d %d %d minecraft:hopper[facing=west]' % hopper)
    g.command('kill @e[type=minecraft:item]')
    time.sleep(1)
    g.command('item replace block %d %d %d container.0 with ntm:rocket_fuel 4' % hopper)
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline and has_blade(g, core):
        time.sleep(.5)
    g.check('the_blade_breaks', not has_blade(g, core), machine_state(g, core)[:400])
    g.command('item replace block %d %d %d container.0 with minecraft:air' % hopper)
    time.sleep(3)
    blades = [e for e in g.ctl('entities 30')['entities'] if e.get('kind') == 'item' and e['item']['item'] == BLADE]
    g.evidence['blades'] = blades
    g.check('one_blade_out', sum(e['item']['count'] for e in blades) == 1, blades)
    after = area(g, own)
    changed = sorted(k for k in set(before) | set(after) if before.get(k) != after.get(k))
    g.check('no_block_broken', not changed, changed[:20])
    # Repair: pick the blade up, use it on the sawmill.
    g.command('clear @s')
    g.command('gamemode survival @s')
    b = blades[0]['pos']
    g.command(f'tp @s {b[0]} {b[1]} {b[2]}')
    time.sleep(2)
    g.check('blade_picked_up', inventory(g, BLADE) == 1, inventory(g, BLADE))
    # The firebox cools first: a hot sawmill with its blade back would break it again.
    g.command('setblock %d %d %d minecraft:stone' % firebox)
    time.sleep(1)
    use(g, core, BLADE)
    g.check('the_blade_back', has_blade(g, core) and inventory(g, BLADE) == 0, [has_blade(g, core), inventory(g, BLADE)])
    g.ctl('save')
    g.save()


def main():
    if len(sys.argv) > 2 and sys.argv[2] == 'play':
        play(Game(sys.argv[1], 'sawmill-game'))
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix='sawmill-game-', dir=args.scratch_root))
    print(folder, flush=True)
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='',
               NTM_TRACE_EFFECTS='1')
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
        log = world.log.read_text(errors='replace')
        effects = [{'id': f'{m[1]}.{m[2]}', 'what': m[3].strip()} for m in BLAST.finditer(log)]
        report['effects'] = effects
        evidence = json.loads((folder / 'native-sawmill-game.json').read_text())
        report['checks'] = evidence.get('checks')
        report['evidence'] = {k: evidence.get(k) for k in ('core', 'sawn', 'taken', 'blades')}
        kinds = [e['what'].split(' ')[0] for e in effects]
        report['blast_then_blade_once'] = kinds == ['explosion', 'items'] and len({e['id'] for e in effects}) == 2 \
            and effects[0]['id'].split('.')[0] == effects[1]['id'].split('.')[0] and 'power=5' in effects[0]['what'] \
            and 'fire=false breaks=false' in effects[0]['what'] and BLADE in effects[1]['what']
        report['ok'] = code == 0 and report['blast_then_blade_once']
    finally:
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (folder / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report.get(k) for k in ('ok', 'blast_then_blade_once', 'effects', 'checks')}, indent=1), flush=True)
    return 0 if report.get('ok') else 1


if __name__ == '__main__':
    sys.exit(main())

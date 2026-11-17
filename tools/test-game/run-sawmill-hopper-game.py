#!/usr/bin/env python3
"""The sawmill's hoppers in game (Onda 2, step 3c), one session:

  one_at_a_time  a hopper above a sawmill on a hot firebox, 3 oak logs in it: one log goes in (a stack of one,
                 `isItemValidForSlot`), is sawn into 6 planks, and no other goes in while the outputs are there (the
                 three slots empty only)
  outputs_out    the firebox gone, a hopper below takes the outputs (`canExtractItem`, slots 1 and 2) into a chest;
                 the next log then goes in, and stays: the hopper below never takes the input
  named_by_hand  a log with a name of its own put in by hand into another sawmill keeps its name in the slot (the
                 Java's `copy()`, the `SlotCopy`)

The sawmill's slots are its block entity's (`data get block .. Items`).

  python3 tools/test-game/run-sawmill-hopper-game.py --binary B --resources R --bundle H --scratch-root S
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
LOG, PLANKS, SAWDUST = 'minecraft:oak_log', 'minecraft:oak_planks', 'ntm:powder_sawdust'
X, Y, Z = 40, 101, 8
OTHER = (44, 101, 8)


def entries(text):
    """The top-level compounds of an SNBT list, as text."""
    out, depth, start = [], 0, None
    body = text[text.find('['):] if '[' in text else ''
    for i, c in enumerate(body):
        if c == '{':
            if depth == 0:
                start = i
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0 and start is not None:
                out.append(body[start:i + 1])
    return out


def items(g, p):
    """The stacks a block entity holds: (slot, id, count, text)."""
    text = ' '.join(g.command('data get block %d %d %d Items' % tuple(p)).get('chat', []))
    out = []
    for e in entries(text):
        slot = re.search(r'Slot: ?(\d+)b?', e)
        item = re.search(r'id: ?"([a-z_:]+)"', e)
        count = re.search(r'count: ?(\d+)', e)
        if item:
            out.append((int(slot.group(1)) if slot else -1, item.group(1), int(count.group(1)) if count else 1, e))
    return out


def held(g, p, item):
    return sum(n for _, i, n, _ in items(g, p) if i == item)


def slots(g, core):
    """The sawmill's three slots, by index: (id, count) or None."""
    out = [None, None, None]
    for s, i, n, _ in items(g, core):
        if 0 <= s < 3:
            out[s] = (i, n)
    return out


def place(g, at):
    g.command('gamemode creative @s')
    g.command('clear @s')
    g.command(f'give @s {SAWMILL} 1')
    g.command(f'tp @s {at[0] + .5} {at[1]} {at[2] + 3.5}')
    time.sleep(1)
    g.ctl('place %d %d %d --item %s' % (*at, SAWMILL))
    time.sleep(1)
    g.command('clear @s')
    return core_of(g, SAWMILL, *at, reach=(3, 3))


def wait(cond, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline and not cond():
        time.sleep(.5)
    return cond()


def play(g):
    g.ctl('wait-ready 120')
    g.command('gamemode creative @s')
    start_area(g, X - 4, Y, Z, 16)
    core = place(g, (X, Y, Z))
    g.evidence['core'] = list(core)
    g.check('core_where_planned', tuple(core) == (X, Y, Z), core)
    firebox, feeder = (X, Y - 1, Z), (X + 1, Y - 1, Z)
    above = (X, Y + 1, Z)
    g.command('setblock %d %d %d ntm:heater_firebox[facing=south,honey_level=0]' % firebox)
    g.command('setblock %d %d %d minecraft:hopper[facing=west]' % feeder)
    g.command('item replace block %d %d %d container.0 with minecraft:coal 8' % feeder)
    time.sleep(8)
    # one_at_a_time
    g.command('setblock %d %d %d minecraft:hopper[facing=down]' % above)
    g.command('item replace block %d %d %d container.0 with %s 3' % (*above, LOG))
    g.check('one_log_in', wait(lambda: slots(g, core)[0] == (LOG, 1), 10), [slots(g, core), items(g, above)])
    g.check('two_left_above', held(g, above, LOG) == 2, items(g, above))
    g.check('sawn', wait(lambda: slots(g, core)[1] == (PLANKS, 6), 60), slots(g, core))
    sawn = slots(g, core)
    g.evidence['sawn'] = sawn
    time.sleep(5)
    after = slots(g, core)
    g.evidence['while_outputs'] = {'slots': after, 'above': held(g, above, LOG)}
    g.check('none_in_while_outputs', after[0] is None and held(g, above, LOG) == 2, g.evidence['while_outputs'])
    # outputs_out: cold, a hopper below into a chest.
    g.command('setblock %d %d %d minecraft:stone' % feeder)
    g.command('setblock %d %d %d minecraft:chest' % feeder)
    g.command('setblock %d %d %d minecraft:hopper[facing=east]' % firebox)
    sawdust = sawn[2][1] if sawn[2] and sawn[2][0] == SAWDUST else 0
    out = lambda item: held(g, feeder, item) + held(g, firebox, item)
    g.check('outputs_taken', wait(lambda: out(PLANKS) == 6 and out(SAWDUST) == sawdust, 30),
            [items(g, feeder), items(g, firebox), slots(g, core)])
    g.check('next_log_in', wait(lambda: slots(g, core)[0] == (LOG, 1), 10), slots(g, core))
    time.sleep(6)
    final = {'slots': slots(g, core), 'above': held(g, above, LOG), 'chest_logs': out(LOG)}
    g.evidence['after_outputs'] = final
    g.check('the_input_stays', final['slots'][0] == (LOG, 1) and final['chest_logs'] == 0 and final['above'] == 1, final)
    # named_by_hand: another sawmill, cold, a named log by hand.
    other = place(g, OTHER)
    g.command('clear @s')
    g.command("give @s minecraft:oak_log[minecraft:custom_name='\"Tora\"'] 2")
    g.command('gamemode survival @s')
    g.ctl(f'select {LOG}')
    g.command(f'tp @s {other[0] + .5} {other[1]} {other[2] + 4.5}')
    time.sleep(.5)
    for dx, dy, dz in [(0, 1, 1), (0, 0, 1), (0, 0, 0)]:
        try:
            g.ctl('interact %d %d %d south' % (other[0] + dx, other[1] + dy, other[2] + dz))
            break
        except RuntimeError:
            continue
    time.sleep(1)
    g.command('gamemode creative @s')
    named = [e for s, i, n, e in items(g, other) if s == 0 and i == LOG and n == 1]
    g.evidence['named'] = named
    g.check('named_log_in_whole', bool(named) and 'Tora' in named[0], items(g, other))
    g.ctl('save')
    g.save()


def main():
    if len(sys.argv) > 2 and sys.argv[2] == 'play':
        play(Game(sys.argv[1], 'sawmill-hopper'))
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--scratch-root', type=Path, required=True)
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix='sawmill-hopper-', dir=args.scratch_root))
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
        evidence = json.loads((folder / 'native-sawmill-hopper.json').read_text())
        report['checks'] = evidence.get('checks')
        report['evidence'] = {k: evidence.get(k) for k in ('core', 'sawn', 'while_outputs', 'after_outputs', 'named')}
        report['ok'] = code == 0
    finally:
        if getattr(world, 'client', None) is not None and world.client.poll() is None:
            world.client.kill()
            world.client.wait()
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (folder / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report.get(k) for k in ('ok', 'checks')}, indent=1), flush=True)
    return 0 if report.get('ok') else 1


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""Generate both dimensions in a muted disposable dedicated server and scan the saves."""
import argparse
import collections
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / 'content-gen-evidence/bedrock'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path, required=True)
    parser.add_argument('--horizon-client', default='/home/sacer/Games/horizon/target/release/horizon-client')
    parser.add_argument('--resources', default='/home/sacer/Games/horizon/.local/horizon-client/26.3')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='ntm-bedrock-worldgen-'))
    for folder in ('home', 'game', 'launcher', 'data', 'config'): (root / folder).mkdir()
    (root / 'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\n')
    (root / 'eula.txt').write_text('eula=true\n')
    (root / 'server.properties').write_text('online-mode=false\nserver-port=25741\nlevel-seed=1768123\nview-distance=2\nsimulation-distance=2\nspawn-chunk-radius=0\nhorizon-pregen=true\npause-when-empty-seconds=0\n')
    (EVIDENCE / 'worldgen-home.txt').write_text(str(root) + '\n')
    env = os.environ | {'HOME': str(root / 'home'), 'HORIZON_HOME': str(root / 'game'), 'HORIZON_LAUNCHER_HOME': str(root / 'launcher'),
                        'XDG_DATA_HOME': str(root / 'data'), 'XDG_CONFIG_HOME': str(root / 'config'), 'HORIZON_NO_AUDIO': '1',
                        'HORIZON_NO_UPDATE_CHECK': '1', 'HORIZON_LAUNCHER_NO_SELF_UPDATE': '1', 'WAYLAND_DISPLAY': ''}
    def request(line):
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(45); sock.connect(str(root / 'ctl.sock')); sock.sendall((line + '\n').encode())
            reply = json.loads(sock.makefile().readline())
        with (EVIDENCE / 'worldgen-control.jsonl').open('a') as log: log.write(json.dumps({'command': line, 'reply': reply}) + '\n')
        return reply
    command = [args.horizon_client, '--resources', args.resources, '--dev-mod', str(args.package.resolve()), '--control-socket', str(root / 'ctl.sock'), '--dedicated', '--nogui']
    with (EVIDENCE / 'worldgen-server.log').open('w') as log:
        process = subprocess.Popen(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
        mute = subprocess.Popen(['python3', str(ROOT / 'tools/test-game/mute.py'), str(root)], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(1200):
                if process.poll() is not None: raise RuntimeError('Server exited during startup')
                if (root / 'ctl.sock').exists():
                    try: request('server-status'); break
                    except (OSError, ValueError): pass
                time.sleep(.5)
            else: raise TimeoutError('server startup')
            for dimension in ('overworld', 'the_nether'):
                print(dimension, request('console execute in minecraft:' + dimension + ' run pregen start 64 square 0 0'), flush=True)
                for _ in range(360):
                    time.sleep(1)
                    reply = request('console pregen status')
                    if 'No pregeneration' in str(reply) or '100%' in str(reply) or 'not running' in str(reply).lower(): break
                else: raise TimeoutError('pregeneration ' + dimension)
            print(request('console save-all flush'), flush=True)
            request('console stop'); process.wait(timeout=90)
            if process.returncode: raise RuntimeError('server exit ' + str(process.returncode))
        finally:
            if process.poll() is None: process.terminate(); process.wait(timeout=30)
            if mute.poll() is None: mute.terminate(); mute.wait(timeout=10)
    spec = importlib.util.spec_from_file_location('scan', ROOT / 'tools/content-gen/world-scan.py')
    scan = importlib.util.module_from_spec(spec); spec.loader.exec_module(scan)
    report = {'home': str(root), 'seed': 1768123, 'dimensions': {}}
    for dimension, y in [('overworld', -64), ('the_nether', 0)]:
        world = root / 'world/dimensions/minecraft' / dimension
        ores, chunks = scan.ore_map(world, (-4, 4, -4, 4))
        deposits = sorted([*position] for position, name in ores.items() if name == 'ntm:ore_bedrock')
        report['dimensions'][dimension] = {'chunks': len(chunks), 'deposits': len(deposits), 'y_counts': dict(collections.Counter(p[1] for p in deposits)), 'positions': deposits}
        assert deposits, 'No bedrock ore in ' + dimension
        assert all(p[1] == y for p in deposits), 'Wrong floor in ' + dimension
    (EVIDENCE / 'worldgen.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report), flush=True)


if __name__ == '__main__': main()

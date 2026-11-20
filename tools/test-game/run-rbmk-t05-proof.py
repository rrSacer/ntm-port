#!/usr/bin/env python3
"""T05 proof (`rbmk-plano` T05): two clients, a host with the world open to LAN and a guest that joins it, play the
fuel channel's gestures (native_rbmk_t05.py) with hot and cold rods, in survival and in creative.

Each client goes through agent-run on its own (one game place each). The binary and the resources come from the
environment, so that agent-run around this runner does not take a game place for it:
  NTM_T05_BINARY=PATH NTM_T05_RESOURCES=PATH agent-run python3 tools/test-game/run-rbmk-t05-proof.py --scratch-root PATH
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from native_game import Game

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def wait(game, line, predicate, timeout, what):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            last = game.ctl(line)
            if predicate(last):
                return last
        except (OSError, RuntimeError, KeyError, TypeError):
            pass
        time.sleep(.5)
    raise TimeoutError(f'{what}: {last}')


def free_places(n, timeout=7200):
    import fcntl
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        free = 0
        for i in range(3):
            with open(f'/run/ntm-agent-game.{i}.lock', 'a') as f:
                try:
                    fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    fcntl.flock(f, fcntl.LOCK_UN)
                    free += 1
                except OSError:
                    pass
        if free >= n:
            return
        time.sleep(5)
    raise TimeoutError(f'{n} game places')


class Client:
    def __init__(self, folder, env, binary, resources, bundle, name, world=None):
        self.folder = folder
        folder.mkdir()
        (folder / 'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nmaxFps:30\nenableVsync:false\nguiScale:2\ntutorialStep:none\n')
        (folder / 'horizon-controller.txt').write_text('enabled:false\n')
        (folder / 'horizon-graphics.txt').write_text('workers:2\n')
        env = dict(env)
        for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'), ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
            env[key] = str(folder / sub)
        command = ['/usr/local/bin/agent-run', str(binary), '--resources', str(resources), '--dev-mod', str(bundle),
                   '--control-socket', str(folder / 'control.sock'), '--test-terrain', '--render-distance', '2',
                   '--width', '1280', '--height', '720', '--name', name]
        if world:
            command += ['--world', world, '--creative', '--seed', '1']
        self.log = (folder / 'client-0.log').open('w')
        self.process = subprocess.Popen(command, cwd=folder, env=env, stdout=self.log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 2400
        while not (folder / 'control.sock').exists():
            if self.process.poll() is not None:
                raise RuntimeError(f'{name} exited before opening its socket')
            if time.monotonic() > deadline:
                raise TimeoutError(f'{name} control socket')
            time.sleep(.25)
        self.game = Game(folder, 'runner-' + name.lower())

    def quit(self):
        if self.process.poll() is None:
            try:
                self.game.ctl('quit')
                self.process.wait(timeout=270)
            except Exception:
                self.process.kill()
                self.process.wait()


def main():
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scratch-root', type=Path, required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    args = parser.parse_args()
    binary, resources = Path(os.environ['NTM_T05_BINARY']).resolve(), Path(os.environ['NTM_T05_RESOURCES']).resolve()
    scratch = Path(tempfile.mkdtemp(prefix='rbmk-t05-', dir=args.scratch_root))
    print(scratch, flush=True)
    env = os.environ.copy()
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='',
               HORIZON_TRACE_SOUNDS='ntm:item.upgradeplug')
    reader, writer = os.pipe()
    with (scratch / 'xvfb.log').open('w') as log:
        xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0', '1280x720x24', '-nolisten', 'tcp'],
                                pass_fds=(writer,), stdout=log, stderr=subprocess.STDOUT)
    os.close(writer)
    with os.fdopen(reader) as pipe:
        env['DISPLAY'] = ':' + pipe.readline().strip()
    report = {'ok': False, 'binary': str(binary), 'binary_sha256': digest(binary), 'bundle_sha256': digest(args.bundle)}
    clients = []
    try:
        # Two game places at once (agent-run's locks): wait until two are free, or the guest waits on one while the
        # host's world runs on (a race other agents can still win, then the guest's socket wait covers it).
        free_places(2)
        host = Client(scratch / 'host', env, binary, resources, args.bundle.resolve(), 'Host', 'T05')
        clients.append(host)
        # A loaded machine (llvmpipe, other agents' games) can keep a client from answering for minutes: wait on it.
        wait(host.game, 'status', lambda v: v.get('player') is not None and v.get('pending_chunks') == 0
             and v.get('server', {}).get('loaded') is not False, 2400, 'host world')
        host.game.ctl('lan open')
        port = wait(host.game, 'lan status', lambda v: v.get('open') and v.get('port'), 60, 'lan open')['port']
        guest = Client(scratch / 'guest', env, binary, resources, args.bundle.resolve(), 'Guest')
        clients.append(guest)
        wait(guest.game, 'screen', lambda v: v.get('screen') == 'title', 2400, 'guest title')
        guest.game.ctl(f'connect 127.0.0.1:{port} Guest')
        wait(host.game, 'lan status', lambda v: any(g['name'] == 'Guest' and g.get('placed') for g in v['guests']), 300, 'guest placed')
        wait(guest.game, 'status', lambda v: v['server']['joined'] and v['player'] is not None, 300, 'guest joined')
        played = subprocess.run([sys.executable, str(HERE / 'native_rbmk_t05.py'), str(host.folder), str(guest.folder)],
                                cwd=scratch, env=env, timeout=3600).returncode
        evidence = host.folder / 'native-rbmk-t05-host.json'
        report['evidence'] = json.loads(evidence.read_text()) if evidence.exists() else None
        report['ok'] = played == 0
    except Exception as error:
        report['error'] = repr(error)
    finally:
        (scratch / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        for c in reversed(clients):
            c.quit()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=30)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    checks = (report.get('evidence') or {}).get('checks') or {}
    print(json.dumps({'ok': report['ok'], 'error': report.get('error'), 'checks': sum(c['ok'] for c in checks.values()),
                      'of': len(checks)}), flush=True)
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())

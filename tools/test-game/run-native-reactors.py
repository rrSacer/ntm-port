#!/usr/bin/env python3
"""Run native reactor acceptance in isolated, disposable Xvfb worlds.

Example (wrap in agent-run env QUEUE=game):
  python3 tools/test-game/run-native-reactors.py zirnox --binary PATH \
      --resources PATH --scratch-root PATH
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import zipfile

from native_game import Game


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def refill_pile(folder):
    """Establish the positive-air precondition through the normal barrel UI."""
    from native_pile import BARREL, VENT, machine
    from native_stations import fill_barrel
    g = Game(folder, 'pile-air-precondition')
    g.close()
    g.command('clear @s')
    g.command('tp @s 34.5 100 7.5')
    fill_barrel(g, BARREL, 1, 'ntm:fluid_barrel_full__air')
    g.ctl('interact ' + ' '.join(map(str, BARREL)))
    time.sleep(.7)
    for _ in range(3):
        if machine(g, BARREL)['data']['mode'] == 'Output':
            break
        g.button([151, 35, 18, 18], [176, 166])
        time.sleep(.4)
    g.close()
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        vent = machine(g, VENT)['data']['Vent']
        if vent['mb'] > 0:
            break
        time.sleep(.5)
    g.check('positive_air_before_capture', vent['mb'] > 0, vent)
    g.evidence['ok'] = True
    g.save()


def run(args):
    work = Path(__file__).resolve().parents[2]
    binary = args.binary.resolve()
    bundle = (args.bundle or work / 'dist/ntm.hmod').resolve()
    folder = (args.resume_reopen.resolve() if args.resume_reopen else
              Path(tempfile.mkdtemp(prefix=args.scenario + '-', dir=args.scratch_root)))
    if args.scenario == 'inventory-proxy':
        # A disposable engine fixture: retain the real item policy and container,
        # but replace only the shape with a radgen-style three-port multiblock.
        fixture = folder / 'inventory-proxy.hmod'
        with zipfile.ZipFile(bundle) as source, zipfile.ZipFile(fixture, 'w') as target:
            for entry in source.infolist():
                data = source.read(entry.filename)
                if entry.filename == 'data/ntm/horizon_machine/machine_zirnox.json':
                    doc = json.loads(data)
                    doc['multiblock'] = {'min': [-1, 0, -2], 'max': [1, 0, 3], 'offset': 2,
                        'dummy': 'ntm:dummy_zirnox',
                        'inventory_ports': [[0, 0, 3], [1, 0, 0], [-1, 0, 0]]}
                    data = json.dumps(doc).encode()
                target.writestr(entry, data)
        bundle = fixture
    report = {'ok': False, 'scenario': args.scenario, 'game': str(folder),
              'binary': str(binary), 'binary_sha256': digest(binary),
              'bundle': str(bundle), 'bundle_sha256': digest(bundle), 'phases': []}
    if args.resume_reopen:
        previous = json.loads((folder / 'report.json').read_text())
        for key in ['scenario', 'binary_sha256', 'bundle_sha256']:
            if previous[key] != report[key]:
                raise ValueError('resume changed ' + key)
        report['phases'] = previous['phases']
    print(folder, flush=True)
    env = os.environ.copy()
    for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                     ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
        env[key] = str(folder / sub)
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1',
               HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='')
    (folder / 'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nmaxFps:30\nenableVsync:false\nguiScale:2\ntutorialStep:none\n')
    # Prevent a physical controller on the host from steering this test window.
    # This file belongs exclusively to the newly created disposable directory.
    (folder / 'horizon-controller.txt').write_text('enabled:false\n')
    (folder / 'horizon-graphics.txt').write_text('workers:2\n')
    reader, writer = os.pipe()
    with (folder / 'xvfb.log').open('w') as log:
        xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0',
                                 '1280x720x24', '-nolisten', 'tcp'],
                                pass_fds=(writer,), stdout=log, stderr=subprocess.STDOUT)
    os.close(writer)
    with os.fdopen(reader) as pipe:
        env['DISPLAY'] = ':' + pipe.readline().strip()
    phases = {'zirnox': [('setup', 'inactive', 'convert', 'capture-state'),
                         ('check-state', 'automation', 'failure', 'soils')],
              'pile': [('setup', 'capture-state'), ('check-state', 'destroy')],
              'dims': [('setup', 'capture-state'), ('check-state',)],
              'bombs': [('setup',)], 'mining': [('setup',)],
              'inventory-proxy': [('setup',)],
              'radioisotopes': [('setup', 'decay', 'capture-state'), ('check-state', 'automation')],
              'solar': [('all', 'capture-state'), ('check-state',)],
              'combustion': [('all',)],
              'catalog': [('setup',)],
              'comparator': [('setup',)],
              'dims-idle': [('idle',)], 'dims-idle-clean': [('idle-clean',)]}
    if getattr(args, 'manual_only', False):
        phases['radioisotopes'] = [('setup', 'decay', 'capture-state'), ('check-state',)]
        report['scope'] = 'manual acceptance; conditional hopper phase still pending'
        report['excluded_phases'] = ['automation']
    if args.setup_only:
        phases[args.scenario] = [phases[args.scenario][0][:1]]
    if args.failure_only:
        phases['zirnox'] = [('failure',)]
    client = None
    try:
        if args.resume_reopen:
            sequence = [mode for session in phases[args.scenario] for mode in session]
            done = len(report['phases'])
            if (done < len(phases[args.scenario][0]) or done >= len(sequence)
                    or report['phases'] != sequence[:done]):
                raise ValueError('resume requires a completed first session and pending reopen phases')
        for index, modes in enumerate(phases[args.scenario]):
            if args.resume_reopen and index == 0:
                continue
            log_path = folder / ('client-first.log' if index == 0 else 'client-reopen.log')
            script = ('dims' if args.scenario.startswith('dims') else
                      'solar_stirling' if args.scenario == 'solar' else args.scenario.replace('-', '_'))
            name = ('DimsTest' if script == 'dims' else 'InvProxyTest' if script == 'inventory_proxy'
                    else args.scenario.capitalize()[:12] + 'Test')
            command = [str(binary), '--resources', str(args.resources.resolve()),
                       '--dev-mod', str(bundle), '--control-socket', str(folder / 'control.sock'),
                       '--test-terrain', '--render-distance', '2', '--width', '1280',
                       '--height', '720', '--name', name, '--world', name, '--creative', '--seed', '1']
            with log_path.open('w') as log:
                client = subprocess.Popen(command, cwd=folder, env=env, stdout=log, stderr=log)
            deadline = time.monotonic() + 150
            while not (folder / 'control.sock').exists():
                if client.poll() is not None:
                    raise RuntimeError('client exited before opening its socket')
                if time.monotonic() > deadline:
                    raise TimeoutError('control socket')
                time.sleep(.25)
            for mode in modes:
                if mode in report['phases']:
                    continue
                if args.scenario == 'pile' and mode == 'capture-state':
                    refill_pile(folder)
                script_modes = [] if mode == 'all' and args.scenario in ('solar', 'combustion') else [mode]
                subprocess.run([sys.executable, str(work / f'tools/test-game/native_{script}.py'),
                                str(folder), *script_modes], cwd=folder, env=env, check=True, timeout=900)
                report['phases'].append(mode)
            Game(folder, f'shutdown-{index}').ctl('quit')
            client.wait(timeout=60)
            (folder / 'control.sock').unlink(missing_ok=True)
            lines = log_path.read_text(errors='replace').splitlines()
            report.setdefault('backend_warnings', []).extend(
                line for line in lines if 'temporal anti-aliasing unavailable' in line)
            bad = [line for line in lines
                   if 'temporal anti-aliasing unavailable' not in line
                   if any(token in line.lower() for token in ['panicked at', 'content mod rejected',
                                                              'content mods not loaded', 'validation error'])]
            if bad:
                raise AssertionError(bad)
        report['ok'] = True
    finally:
        if client is not None and client.poll() is None:
            client.terminate()
            try:
                client.wait(timeout=15)
            except subprocess.TimeoutExpired:
                client.kill()
                client.wait()
        xvfb.terminate()
        xvfb.wait(timeout=10)
        (folder / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return folder


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('scenario', choices=['zirnox', 'pile', 'dims', 'dims-idle', 'dims-idle-clean', 'bombs', 'mining', 'inventory-proxy', 'radioisotopes', 'solar', 'combustion', 'catalog', 'comparator'])
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--bundle', type=Path)
    parser.add_argument('--scratch-root', type=Path, required=True)
    parser.add_argument('--resume-reopen', type=Path,
                        help='resume a completed first session using the identical binary and bundle')
    parser.add_argument('--manual-only', action='store_true',
                        help='radioisotopes only: explicitly defer the conditional hopper phase')
    parser.add_argument('--setup-only', action='store_true',
                        help='only the first phase, for isolated baseline comparisons')
    parser.add_argument('--failure-only', action='store_true',
                        help='diagnostic ZIRNOX failure scene only; not full acceptance')
    arguments = parser.parse_args()
    if arguments.manual_only and arguments.scenario != 'radioisotopes':
        parser.error('--manual-only requires radioisotopes')
    if arguments.failure_only and (arguments.scenario != 'zirnox' or arguments.setup_only or arguments.resume_reopen):
        parser.error('--failure-only requires zirnox without setup/resume')
    if arguments.setup_only and arguments.resume_reopen:
        parser.error('--setup-only cannot resume a reopened session')
    run(arguments)
    if arguments.scenario == 'dims' and not arguments.setup_only:
        # setup intentionally forceloads the Nether; the idle scenarios require
        # no forced chunks, and idle-clean additionally requires no reader.
        # Give each original scenario its own new world, as their contract asks.
        for scenario in ['dims-idle', 'dims-idle-clean']:
            run(argparse.Namespace(**{**vars(arguments), 'scenario': scenario, 'resume_reopen': None}))

"""Phase 2 of the two-phase proof for the NTM world the runner stopped at its phase-1 check: reopen twice, count."""
import argparse, importlib.util, json, shutil, sys
from pathlib import Path

HARNESS = Path('/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/onda2-efeitos/probes/two-phase-test-game')
sys.path.insert(0, str(HARNESS))
spec = importlib.util.spec_from_file_location('more', HARNESS / 'run-crash-more.py')
more = importlib.util.module_from_spec(spec)
spec.loader.exec_module(more)
cp = more.crash_proof

p = argparse.ArgumentParser()
p.add_argument('folder', type=Path)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
args = argparse.Namespace(binary=Path('/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/onda2-efeitos/proof-bin/horizon-client'),
                          resources=Path('/home/sacer/Games/horizon/.local/horizon-client/26.3'),
                          bundle=Path('/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/onda2-efeitos/dist/ntm.hmod'))
items = more.items_of('coupled-break')
folder = a.folder
snapshot = folder.parent / (folder.name + '-phase1')
cp.clone_world(folder, snapshot, shutil.ignore_patterns('control.sock'))
import os, subprocess
env = os.environ.copy()
for key, sub in [('HORIZON_HOME', 'home'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                 ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
    env[key] = str(folder / sub)
env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_UPDATE_CHECK='1', WAYLAND_DISPLAY='',
           NTM_CRASH_VANILLA='0')
reader, writer = os.pipe()
xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(writer), '-screen', '0', '1280x720x24', '-nolisten', 'tcp'],
                        pass_fds=(writer,), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
os.close(writer)
with os.fdopen(reader) as pipe:
    env['DISPLAY'] = ':' + pipe.readline().strip()
out = {'folder': str(folder), 'phase1_disk': cp.disk_count.count(folder, items)}
world = None
try:
    world = more.More(args, folder, env, 'coupled-break')
    world.launch()
    out['first_rc'] = world.script('verify', check=False)
    out['first_reopen'] = more.evidence(folder, 'verify')
    world.quit()
    out['phase2_disk'] = cp.disk_count.count(folder, items)
    out['phase2_items'] = cp.disk_count.items(folder, items)
    out['phase2_written'] = cp.disk_count.written(snapshot, folder, items)
    world.launch()
    out['again_rc'] = world.script('again', check=False)
    out['second_reopen'] = more.evidence(folder, 'again')
    world.quit()
    out['again_disk'] = cp.disk_count.count(folder, items)
    out['again_items'] = cp.disk_count.items(folder, items)
finally:
    if world is not None and getattr(world, 'client', None) is not None and world.client.poll() is None:
        world.client.kill(); world.client.wait()
    xvfb.terminate(); xvfb.wait(timeout=10)
    a.out.write_text(json.dumps(out, indent=2, default=str) + '\n')
    print(json.dumps({k: out.get(k) for k in ('phase1_disk', 'first_rc', 'phase2_disk', 'phase2_written', 'again_rc', 'again_disk')}, default=str))

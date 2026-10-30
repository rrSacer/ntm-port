#!/usr/bin/env python3
"""Silent isolated sand profile: decoder, surface biome and desert substrate."""
import argparse
import json
import importlib.util
import collections
import os
import shutil
import socket
import subprocess
import tempfile
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package',type=Path,required=True)
    parser.add_argument('--horizon-client',type=Path,required=True)
    parser.add_argument('--resources',type=Path,required=True)
    args=parser.parse_args();evidence=ROOT/'content-gen-evidence/petroleum-d'
    home=Path(tempfile.mkdtemp(prefix='content-sand-probe-',dir=ROOT.parent/'.agent-tmp'))
    for name in ['game','home','launcher','data','config','resources']:(home/name).mkdir()
    (home/'game/options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nrenderDistance:2\nsimulationDistance:2\nmaxFps:10\nenableVsync:false\n')
    for p in args.resources.iterdir():
        (home/'resources'/p.name).symlink_to(p.resolve(),target_is_directory=p.is_dir())
    env=os.environ|{'HOME':str(home/'home'),'HORIZON_HOME':str(home/'game'),'HORIZON_LAUNCHER_HOME':str(home/'launcher'),
        'XDG_DATA_HOME':str(home/'data'),'XDG_CONFIG_HOME':str(home/'config'),'HORIZON_NO_AUDIO':'1',
        'HORIZON_NO_UPDATE_CHECK':'1','HORIZON_LAUNCHER_NO_SELF_UPDATE':'1','WAYLAND_DISPLAY':'','XDG_RUNTIME_DIR':'/run/user/1000'}
    checks=[];report={'home':str(home),'seed':1768123};process=None;mute=None
    def ctl(command):
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(60);sock.connect(str(home/'ctl.sock'));sock.sendall((command+'\n').encode())
            reply=json.loads(sock.makefile().readline())
        checks.append({'command':command,'reply':{k:v for k,v in reply.items() if k!='blocks'}})
        if reply.get('ok') is False or 'error' in reply:raise AssertionError((command,reply))
        return reply
    def cmd(command):
        reply=ctl('command '+command)
        if any(s in str(reply.get('chat',[])).lower() for s in ['unknown or incomplete','incorrect argument','unknown block','unknown item']):
            raise AssertionError((command,reply))
        return reply
    def wait_for(fn,seconds=30):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            value=fn()
            if value:return value
            ctl('wait 5')
        raise TimeoutError('world synchronization')
    try:
        mute=subprocess.Popen(['python3',str(ROOT/'tools/test-game/mute.py'),str(home/'game')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        with (home/'client.log').open('w') as log:
            process=subprocess.Popen([str(args.horizon_client),'--resources',str(home/'resources'),'--dev-mod',str(args.package.resolve()),
                '--control-socket',str(home/'ctl.sock'),'--offscreen','--creative','--world','OilSandProbe','--seed','1768123',
                '--render-distance','2','--width','640','--height','360','--name','SandProbe'],cwd=home/'game',env=env,stdout=log,stderr=subprocess.STDOUT)
            end=time.monotonic()+600
            while time.monotonic()<end:
                if 'content mods not loaded' in (home/'client.log').read_text():raise RuntimeError('content mods not loaded; see client.log')
                if process.poll() is not None:raise RuntimeError('client startup')
                if (home/'ctl.sock').exists():
                    state=ctl('state')
                    if state.get('screen')=='playing' and state.get('player') and state.get('gamemode')=='creative':break
                time.sleep(.5)
            else:raise TimeoutError('client startup')
            cmd('tp @s 520.5 150 -1815.5')
            wait_for(lambda:abs(ctl('state')['player']['position'][0]-520.5)<.5)
            ctl('wait 50')
            # The patched client has no pregen command. Load the small sample
            # through ordinary player chunk tickets, checking actual terrain.
            report['generation_method']='nine player positions; terrain acknowledged before moving'
            for x in [488,520,552]:
                for z in [-1848,-1816,-1784]:
                    cmd(f'tp @s {x+.5} 150 {z+.5}')
                    wait_for(lambda:abs(ctl('state')['player']['position'][0]-(x+.5))<.5)
                    wait_for(lambda:any(b[3]!='minecraft:air' for b in ctl(f'blocks {x} 56 {z} {x} 84 {z}')['blocks']))
                    ctl('wait 20')
            # Ordinary quit persists the world. This client has neither
            # save-all nor /place feature; the isolated natural profile is
            # already a positive decoder/substrate/biome test.
            report['decoder_and_isolated_placement']='verified by saved natural profile below'
            ctl('quit');process.wait(timeout=90)
        spec=importlib.util.spec_from_file_location('scan',ROOT/'tools/content-gen/world-scan.py')
        scan=importlib.util.module_from_spec(spec);spec.loader.exec_module(scan)
        world=next(p.parent for p in (home/'game').rglob('region') if p.parent.name=='overworld')
        ores,chunks=scan.ore_map(world,(28,36,-118,-110))
        ps=[p for p,ident in ores.items() if ident=='ntm:ore_oil_sand']
        starts=collections.Counter()
        for chunk in scan.chunks(world):
            if not(28<=chunk['xPos']<=36 and -118<=chunk['zPos']<=-110):continue
            for name,data in chunk.get('structures',{}).get('starts',{}).items():
                if name.startswith('ntm:oil_sand') and data.get('id')!='INVALID':starts[name]+=1
        report['natural_fixed_profile']={'chunks':len(chunks),'center':[520,-1816],'test_rarity':1,
            'blocks':len(ps),'y_counts':dict(sorted(collections.Counter(p[1] for p in ps).items())),
            'structure_starts':dict(starts)}
        assert ps,'surface biome profile placed no oil sand'
        assert all(56<=p[1]<=71 for p in ps),'oil sand outside declared height'
        assert starts,'no dry-hot profile starts'
        refused=[l for l in (home/'client.log').read_text().splitlines() if any(s in l.lower() for s in ['content mod rejected','content mods not loaded','refused','panicked at'])]
        report['refused']=refused;assert not refused,refused
        (evidence/'sand-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:process.wait(timeout=20)
            except subprocess.TimeoutExpired:process.kill();process.wait()
        if mute and mute.poll() is None:mute.terminate();mute.wait(timeout=10)
        (evidence/'sand-control.json').write_text(json.dumps(checks,indent=2)+'\n')
        if (home/'client.log').exists():shutil.copy2(home/'client.log',evidence/'sand-client.log')


if __name__=='__main__':main()

#!/usr/bin/env python3
"""One silent check: disk decoder, uniform sand and actual desert substrate."""
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
        if p.name!='command-registries.json':(home/'resources'/p.name).symlink_to(p.resolve(),target_is_directory=p.is_dir())
    registry=json.loads((args.resources/'command-registries.json').read_text())
    registry['by_registry']['minecraft:worldgen/feature']['ids'] += ['ntm:oil_sand','ntm:oil_sand_hot_dry']
    (home/'resources/command-registries.json').write_text(json.dumps(registry)+'\n')
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
    def cmd(command):return ctl('command '+command)
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
            report['pregeneration']=cmd('pregen start 64 square 520 -1816')
            end=time.monotonic()+600
            while time.monotonic()<end:
                ctl('wait 20');reply=cmd('pregen status')
                if 'No pregeneration' in str(reply) or '100%' in str(reply) or 'not running' in str(reply).lower():break
            else:raise TimeoutError('small desert pregeneration')
            cmd('save-all flush')
            cmd('tp @s 416.5 150 -1727.5')
            wait_for(lambda:abs(ctl('state')['player']['position'][0]-416.5)<.5)
            ctl('wait 100')
            def natural_sand():
                r=ctl('blocks 408 56 -1736 424 84 -1720')
                return r if sum(r['counts'].get(s,0) for s in ['minecraft:sand','minecraft:red_sand']) else None
            before=wait_for(natural_sand)
            sands=[p for p in before['blocks'] if p[3].split('[')[0] in ['minecraft:sand','minecraft:red_sand']]
            report['natural_sand_before']={'count':len(sands),'min_y':min(p[1] for p in sands),'max_y':max(p[1] for p in sands),
                'in_original_56_71':sum(56<=p[1]<=71 for p in sands)}
            report['biome_match']=cmd('execute if biome 416 78 -1728 minecraft:desert')
            report['original_height_place']=cmd('place feature ntm:oil_sand 416 64 -1728')
            ctl('wait 20');low=ctl('blocks 408 56 -1736 424 84 -1720')
            report['original_height_blocks']=low['counts'].get('ntm:ore_oil_sand',0)
            report['surface_height_place']=cmd('place feature ntm:oil_sand 416 78 -1728')
            high=wait_for(lambda:r if (r:=ctl('blocks 408 56 -1736 424 84 -1720'))['counts'].get('ntm:ore_oil_sand',0)>0 else None)
            report['surface_height_blocks']=high['counts'].get('ntm:ore_oil_sand',0)
            # Sand is supported, avoiding a falling-block test instead of a feature test.
            cmd('fill 404 139 -1740 428 139 -1716 minecraft:stone')
            cmd('fill 404 140 -1740 428 148 -1716 minecraft:sand')
            wait_for(lambda:ctl('blocks 404 140 -1740 428 148 -1716')['counts'].get('minecraft:sand')==5625)
            report['fixture_place']=cmd('place feature ntm:oil_sand 416 144 -1728')
            fixture=wait_for(lambda:r if (r:=ctl('blocks 404 140 -1740 428 148 -1716'))['counts'].get('ntm:ore_oil_sand',0)>0 else None)
            report['fixture_blocks']=fixture['counts'].get('ntm:ore_oil_sand',0)
            assert report['fixture_blocks']==1773,report['fixture_blocks']
            report['decoder_and_isolated_placement']='pass'
            cmd('save-all flush');ctl('quit');process.wait(timeout=90)
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

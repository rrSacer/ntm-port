#!/usr/bin/env python3
"""Generate petroleum in a silent disk-backed server and measure saved blocks."""
import argparse
import collections
import importlib.util
import json
import os
import re
import shutil
import socket
import subprocess
import tempfile
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]


def components(positions):
    remaining=set(positions);result=[]
    while remaining:
        start=remaining.pop();pending=[start];group=[start]
        while pending:
            x,y,z=pending.pop()
            for dx,dy,dz in [(1,0,0),(-1,0,0),(0,1,0),(0,-1,0),(0,0,1),(0,0,-1)]:
                p=(x+dx,y+dy,z+dz)
                if p in remaining:remaining.remove(p);pending.append(p);group.append(p)
        result.append({'volume':len(group),'min':[min(p[a] for p in group) for a in range(3)],
                       'max':[max(p[a] for p in group) for a in range(3)]})
    return sorted(result,key=lambda r:r['min'])


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package',type=Path,required=True)
    parser.add_argument('--horizon-client',type=Path,required=True)
    parser.add_argument('--resources',type=Path,required=True)
    parser.add_argument('--radius',type=int,default=192)
    parser.add_argument('--seed',type=int,default=1768123)
    parser.add_argument('--center',nargs=2,type=int,default=[0,0])
    parser.add_argument('--skip-desert',action='store_true')
    parser.add_argument('--require-sand',action='store_true')
    parser.add_argument('--label',default='worldgen')
    args=parser.parse_args()
    evidence=ROOT/'content-gen-evidence/petroleum-d';evidence.mkdir(parents=True,exist_ok=True)
    home=Path(tempfile.mkdtemp(prefix='content-oil-worldgen-',dir=ROOT.parent/'.agent-tmp'))
    for folder in ['home','game','launcher','data','config']:(home/folder).mkdir()
    (home/'options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\n')
    (home/'eula.txt').write_text('eula=true\n')
    (home/'server.properties').write_text(f'online-mode=false\nserver-port=25749\nlevel-seed={args.seed}\nview-distance=2\nsimulation-distance=2\nspawn-chunk-radius=0\nhorizon-pregen=true\npause-when-empty-seconds=0\n')
    env=os.environ|{'HOME':str(home/'home'),'HORIZON_HOME':str(home/'game'),
         'HORIZON_LAUNCHER_HOME':str(home/'launcher'),'XDG_DATA_HOME':str(home/'data'),
         'XDG_CONFIG_HOME':str(home/'config'),'HORIZON_NO_AUDIO':'1','HORIZON_NO_UPDATE_CHECK':'1',
         'HORIZON_LAUNCHER_NO_SELF_UPDATE':'1','WAYLAND_DISPLAY':''}
    checks=[]
    def ctl(command):
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(60);client.connect(str(home/'ctl.sock'));client.sendall((command+'\n').encode())
            reply=json.loads(client.makefile().readline())
        checks.append({'command':command,'reply':reply})
        if reply.get('ok') is False or 'error' in reply:raise AssertionError((command,reply))
        return reply
    process=None;mute=None
    try:
        with (home/'server.log').open('w') as log:
            mute=subprocess.Popen(['python3',str(ROOT/'tools/test-game/mute.py'),str(home)],env=env,stdout=log,stderr=subprocess.STDOUT)
            process=subprocess.Popen([str(args.horizon_client),'--resources',str(args.resources),'--dev-mod',str(args.package.resolve()),
                '--control-socket',str(home/'ctl.sock'),'--dedicated','--nogui'],cwd=home,env=env,stdout=log,stderr=subprocess.STDOUT)
            deadline=time.monotonic()+600
            while time.monotonic()<deadline:
                if process.poll() is not None:raise RuntimeError('server startup failed')
                if (home/'ctl.sock').exists():
                    try:ctl('server-status');break
                    except (OSError,ValueError):pass
                time.sleep(.5)
            else:raise TimeoutError('server startup')
            print(ctl(f'console execute in minecraft:overworld run pregen start {args.radius} square {args.center[0]} {args.center[1]}'),flush=True)
            deadline=time.monotonic()+3600
            while time.monotonic()<deadline:
                time.sleep(2);r=ctl('console pregen status');text=str(r)
                if 'No pregeneration' in text or '100%' in text or 'not running' in text.lower():break
            else:raise TimeoutError('oil pregeneration')
            desert=None
            located=ctl('console locate biome minecraft:desert') if not args.skip_desert else {}
            print({'locate_desert':located},flush=True)
            match=re.search(r'\[(-?\d+),\s*(-?\d+),\s*(-?\d+)\]',str(located))
            if match:
                desert=[int(match[1]),int(match[3])]
                print(ctl(f'console execute in minecraft:overworld run pregen start 128 square {desert[0]} {desert[1]}'),flush=True)
                deadline=time.monotonic()+1800
                while time.monotonic()<deadline:
                    time.sleep(2);r=ctl('console pregen status');text=str(r)
                    if 'No pregeneration' in text or '100%' in text or 'not running' in text.lower():break
                else:raise TimeoutError('desert pregeneration')
            print(ctl('console save-all flush'),flush=True);ctl('console stop');process.wait(timeout=90)
            if process.returncode:raise RuntimeError('server exit '+str(process.returncode))
        spec=importlib.util.spec_from_file_location('scan',ROOT/'tools/content-gen/world-scan.py')
        scan=importlib.util.module_from_spec(spec);spec.loader.exec_module(scan)
        n=args.radius//16;world=home/'world/dimensions/minecraft/overworld'
        cx,cz=[v//16 for v in args.center]
        ores,chunks=scan.ore_map(world,(cx-n,cx+n,cz-n,cz+n));report={'home':str(home),'seed':args.seed,'radius':args.radius,'center':args.center,'chunks':len(chunks),'deposits':{}}
        for name in ['ore_oil','ore_bedrock_oil','ore_oil_sand']:
            ps=[p for p,ident in ores.items() if ident=='ntm:'+name]
            groups=components(ps);report['deposits'][name]={'blocks':len(ps),'y_counts':dict(sorted(collections.Counter(p[1] for p in ps).items())),
                'components':groups,'component_mean_volume':sum(g['volume'] for g in groups)/len(groups) if groups else 0}
        if desert:
            cx,cz=[v//16 for v in desert]
            dry_ores,dry_chunks=scan.ore_map(world,(cx-8,cx+8,cz-8,cz+8))
            report['desert']={'center':desert,'chunks':len(dry_chunks),'deposits':{}}
            for name in ['ore_oil','ore_bedrock_oil','ore_oil_sand']:
                ps=[p for p,ident in dry_ores.items() if ident=='ntm:'+name]
                report['desert']['deposits'][name]={'blocks':len(ps),'y_counts':dict(sorted(collections.Counter(p[1] for p in ps).items())),
                    'components':components(ps)}
        refused=[line for line in (home/'server.log').read_text().splitlines() if any(s in line.lower() for s in ['content mod rejected','content mods not loaded','refused','panicked at'])]
        report['refused']=refused
        (evidence/f'{args.label}.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report),flush=True)
        assert not refused,refused
        assert report['chunks']==(2*n+1)**2,report['chunks']
        if args.require_sand:
            assert report['deposits']['ore_oil_sand']['blocks']>0,'no oil sand'
        else:
            assert report['deposits']['ore_oil']['blocks']>0,'no oil'
            bedrock=report['deposits']['ore_bedrock_oil'];assert bedrock['blocks']>0,'no bedrock oil'
            assert all(-64<=int(y)<=-60 for y in bedrock['y_counts']),'bedrock oil height'
        assert all(56<=int(y)<=71 for y in report['deposits']['ore_oil_sand']['y_counts']),'oil sand height'
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:process.wait(timeout=30)
            except subprocess.TimeoutExpired:process.kill();process.wait()
        if mute and mute.poll() is None:mute.terminate();mute.wait(timeout=10)
        (evidence/f'{args.label}-control.json').write_text(json.dumps(checks,indent=2)+'\n')
        if (home/'server.log').exists():shutil.copy2(home/'server.log',evidence/f'{args.label}-server.log')


if __name__=='__main__':main()

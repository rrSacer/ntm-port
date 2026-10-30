#!/usr/bin/env python3
"""Measure oil feature candidates in the same silent, uniform-stone fixture."""
import argparse
import json
import os
import shutil
import socket
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]


def source_volumes():
    # MapGenBubble's strict discrete ellipsoid in a uniform stone substrate.
    return [{'radius':r,'volume':sum(x*x+z*z+3*y*y<r*r/2
             for x in range(-r,r+1) for y in range(-r,r+1) for z in range(-r,r+1))}
            for r in range(8,16)]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package',type=Path,required=True)
    parser.add_argument('--horizon-client',type=Path,required=True)
    parser.add_argument('--resources',type=Path,required=True)
    parser.add_argument('--samples',type=int,default=8)
    parser.add_argument('--label',default='candidate')
    parser.add_argument('--features',nargs='+',default=['oil_test_ore64','oil_test_geode2','oil_test_geode3','oil_test_geode4'])
    args=parser.parse_args()
    evidence=ROOT/'content-gen-evidence/petroleum-d';evidence.mkdir(parents=True,exist_ok=True)
    scratch=Path(tempfile.mkdtemp(prefix='content-oil-feature-probe-',dir=ROOT.parent/'.agent-tmp'))
    for d in ['game','home','launcher','data','config','resources']:(scratch/d).mkdir()
    # The base client omits mod feature ids from its command registry. Extend
    # only the disposable resource view used by this explicit /place fixture;
    # natural-world generation is measured separately with stock resources.
    for child in args.resources.iterdir():
        if child.name!='command-registries.json':(scratch/'resources'/child.name).symlink_to(child.resolve(),target_is_directory=child.is_dir())
    commands=json.loads((args.resources/'command-registries.json').read_text())
    commands['by_registry']['minecraft:worldgen/feature']['ids'] += ['ntm:'+f for f in args.features]
    (scratch/'resources/command-registries.json').write_text(json.dumps(commands)+'\n')
    (scratch/'game/options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nrenderDistance:2\nsimulationDistance:2\nmaxFps:10\nenableVsync:false\n')
    env=os.environ|{'HOME':str(scratch/'home'),'HORIZON_HOME':str(scratch/'game'),
         'HORIZON_LAUNCHER_HOME':str(scratch/'launcher'),'XDG_DATA_HOME':str(scratch/'data'),
         'XDG_CONFIG_HOME':str(scratch/'config'),'HORIZON_LAUNCHER_NO_SELF_UPDATE':'1',
         'HORIZON_NO_UPDATE_CHECK':'1','HORIZON_NO_AUDIO':'1','XDG_RUNTIME_DIR':'/run/user/1000',
         'WAYLAND_DISPLAY':'','SDL_VIDEODRIVER':'x11'}
    records=[];checks=[];process=None;mute=None
    def ctl(command):
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(60);client.connect(str(scratch/'ctl.sock'));client.sendall((command+'\n').encode())
            reply=json.loads(client.makefile().readline())
        summary={k:v for k,v in reply.items() if k!='blocks'}
        checks.append({'command':command,'reply':summary})
        if 'error' in reply or reply.get('ok') is False:raise AssertionError((command,summary))
        return reply
    def cmd(command):
        r=ctl('command '+command)
        if any(s in str(r.get('chat',[])).lower() for s in ['unknown','incorrect','error','failed',"can't find"]):raise AssertionError((command,r))
        return r
    def census():return ctl('blocks -16 84 -16 15 115 15')
    def wait_for(fn,description,seconds=30):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            value=fn()
            if value:return value
            ctl('wait 5')
        raise TimeoutError(description)
    try:
        mute=subprocess.Popen(['python3',str(ROOT/'tools/test-game/mute.py'),str(scratch/'game')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        with (scratch/'client.log').open('w') as log:
            process=subprocess.Popen([str(args.horizon_client),'--resources',str(scratch/'resources'),
                '--dev-mod',str(args.package.resolve()),'--control-socket',str(scratch/'ctl.sock'),'--offscreen','--test-terrain',
                '--creative','--world','OilFeatureProbe','--render-distance','2','--width','640','--height','360','--name','OilProbe'],
                cwd=scratch/'game',env=env,stdout=log,stderr=subprocess.STDOUT)
            end=time.monotonic()+600
            while time.monotonic()<end:
                if process.poll() is not None:raise RuntimeError('client exited during startup')
                if (scratch/'ctl.sock').exists():
                    state=ctl('state')
                    if state.get('screen')=='playing' and state.get('player') and state.get('gamemode')=='creative':break
                time.sleep(.5)
            else:raise TimeoutError('client startup')
            cmd('tp @s 0.5 118 0.5');ctl('wait 30')
            cmd('time set noon');cmd('weather clear')
            for feature in args.features:
                for i in range(args.samples):
                    cmd('fill -16 84 -16 15 115 15 minecraft:stone')
                    wait_for(lambda:census()['counts'].get('minecraft:stone')==32768,'fixture reset')
                    cmd('place feature ntm:'+feature+' 0 100 0')
                    data=wait_for(lambda:(r if r['counts'].get('ntm:ore_oil',0)>0 else None) if (r:=census()) else None,'feature blocks')
                    ctl('wait 20');data=census()
                    positions=[row[:3] for row in data['blocks'] if row[3].split('[')[0]=='ntm:ore_oil']
                    extents=[max(p[a] for p in positions)-min(p[a] for p in positions)+1 for a in range(3)]
                    record={'feature':feature,'sample':i,'volume':len(positions),'extents_xyz':extents,'positions':positions}
                    records.append(record)
                    print(json.dumps({k:v for k,v in record.items() if k!='positions'}),flush=True)
                    (evidence/f'{args.label}-samples.json').write_text(json.dumps(records,indent=2)+'\n')
            # Smoke the new catalog ids while this disposable client is open.
            names=['machine_well','machine_pumpjack','machine_fracking_tower','machine_transformer','ore_oil','ore_oil_empty','ore_bedrock_oil','ore_oil_sand','oil_pipe']
            cmd('clear @s')
            cmd('fill -7 119 -9 7 119 3 minecraft:stone')
            for i,name in enumerate(names):
                state='ntm:'+name+('[facing=north,honey_level=5]' if name.startswith('machine_') else '')
                cmd(f'setblock {i-4} 120 0 {state}')
                wait_for(lambda:ctl(f'block {i-4} 120 0')['block']=='ntm:'+name,'block '+name)
                cmd('give @s ntm:'+name+' 1')
            wait_for(lambda:all(ctl('inventory')['counts'].get('ntm:'+name,0)==1 for name in names),'given oil ids')
            cmd('tp @s 0.5 120 -6.5');ctl('wait 20');ctl('look 0 10');ctl('wait 5')
            ctl('screenshot '+str(evidence/f'{args.label}-blocks.png'))
            ctl('quit');process.wait(timeout=90)
        reference=source_volumes();summary={'source_uniform_stone':reference,'source_mean_volume':statistics.mean(r['volume'] for r in reference),'candidates':{},'home':str(scratch)}
        for f in args.features:
            group=[r for r in records if r['feature']==f]
            summary['candidates'][f]={'samples':len(group),'mean_volume':statistics.mean(r['volume'] for r in group),'min_volume':min(r['volume'] for r in group),'max_volume':max(r['volume'] for r in group),'mean_extents_xyz':[statistics.mean(r['extents_xyz'][a] for r in group) for a in range(3)]}
        text=(scratch/'client.log').read_text()
        refused=[line for line in text.splitlines() if any(s in line.lower() for s in ['content mod rejected','refused','panicked at'])]
        if refused:raise AssertionError(refused)
        summary['refused']=refused
        (evidence/f'{args.label}-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
        print(json.dumps(summary),flush=True)
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:process.wait(timeout=20)
            except subprocess.TimeoutExpired:process.kill();process.wait()
        if mute and mute.poll() is None:mute.terminate();mute.wait(timeout=10)
        (evidence/f'{args.label}-control.json').write_text(json.dumps(checks,indent=2)+'\n')
        if (scratch/'client.log').exists():shutil.copy2(scratch/'client.log',evidence/f'{args.label}-client.log')


if __name__=='__main__':main()

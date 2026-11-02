#!/usr/bin/env python3
"""Disposable, silent content-only client probe for a mass-catalog batch: place every block, hand every item."""
import argparse
import json
import os
import shutil
import socket
import subprocess
import tempfile
import time
from pathlib import Path


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--package',type=Path,required=True)
    parser.add_argument('--horizon-client',type=Path,required=True)
    parser.add_argument('--ids-file',type=Path,required=True,help='JSON {"blocks":[...],"items":[...]} of ids without the ntm: prefix')
    parser.add_argument('--profile',default='catalog')
    args=parser.parse_args()
    root=Path(__file__).resolve().parents[2]
    evidence=root/'content-gen-evidence'/({'bedrock':'bedrock-stages34'}.get(args.profile,args.profile));evidence.mkdir(parents=True,exist_ok=True)
    temporary=root.parent/'.agent-tmp';temporary.mkdir(exist_ok=True)
    scratch=Path(tempfile.mkdtemp(prefix='content-processing-probe-',dir=temporary))
    for d in ['game','home','launcher','data','config']:(scratch/d).mkdir()
    (scratch/'game/options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nrenderDistance:2\nsimulationDistance:2\nmaxFps:15\nenableVsync:false\n')
    (scratch/'game/horizon-graphics.txt').write_text('workers:2\n')
    env={**os.environ,'HOME':str(scratch/'home'),'HORIZON_HOME':str(scratch/'game'),
         'HORIZON_LAUNCHER_HOME':str(scratch/'launcher'),'XDG_DATA_HOME':str(scratch/'data'),
         'XDG_CONFIG_HOME':str(scratch/'config'),'HORIZON_LAUNCHER_NO_SELF_UPDATE':'1',
         'HORIZON_NO_UPDATE_CHECK':'1','HORIZON_NO_AUDIO':'1','XDG_RUNTIME_DIR':'/run/user/1000',
         'WAYLAND_DISPLAY':'','SDL_VIDEODRIVER':'x11'}
    checks=[];process=None;mute=None
    def ctl(command):
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(45);client.connect(str(scratch/'control.sock'))
            client.sendall((command+'\n').encode());reply=json.loads(client.makefile().readline())
        checks.append({'command':command,'reply':reply})
        if 'error' in reply or reply.get('ok') is False:raise AssertionError((command,reply))
        return reply
    def cmd(command):
        reply=ctl('command '+command)
        if any(s in str(reply.get('chat',[])).lower() for s in ['unknown','incorrect','error']):raise AssertionError((command,reply))
        return reply
    try:
        mute=subprocess.Popen(['python3',str(root/'tools/test-game/mute.py'),str(scratch/'game')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        with (scratch/'client.log').open('w') as log:
            process=subprocess.Popen([str(args.horizon_client),'--resources','/home/sacer/Games/horizon/.local/horizon-client/26.3',
                '--dev-mod',str(args.package.resolve()),'--control-socket',str(scratch/'control.sock'),'--offscreen','--test-terrain',
                '--creative','--world','NTMProcessingProbe','--render-distance','2','--width','1280','--height','720','--name','ProcessingProbe'],
                cwd=scratch/'game',env=env,stdout=log,stderr=subprocess.STDOUT)
            deadline=time.monotonic()+600
            while time.monotonic()<deadline:
                if 'content mods not loaded' in (scratch/'client.log').read_text():raise RuntimeError('content mods not loaded; see client.log')
                if process.poll() is not None:raise RuntimeError('client exited during startup')
                if (scratch/'control.sock').exists():
                    state=ctl('state')
                    if state.get('screen')=='playing' and state.get('player') and state.get('gamemode')=='creative':break
                time.sleep(.5)
            else:raise TimeoutError('client did not reach playing')
            cmd('time set noon');cmd('weather clear')
            cmd('fill -12 100 -4 24 100 36 minecraft:stone')
            data=json.loads(args.ids_file.read_text());blocks=data['blocks'];samples=data['items']
            machines=[]
            def at(i):return -8+(i%14)*2,3+(i//14)*2
            for i,name in enumerate(blocks):
                x,z=at(i)
                cmd(f'setblock {x} 101 {z} ntm:{name}')
            for i,name in enumerate(blocks):
                x,z=at(i)
                deadline=time.monotonic()+20
                while True:
                    reply=ctl(f'block {x} 101 {z}')
                    if reply['block']=='ntm:'+name:break
                    if time.monotonic()>deadline:raise AssertionError((name,reply))
                    ctl('wait 2')
            cmd('tp @s 0.5 101 -7.5')
            cmd('clear @s')
            for start in range(0,len(samples),30):
                if start:cmd('clear @s')
                batch=samples[start:start+30]
                for name in batch:cmd('give @s ntm:'+name+' 1')
                deadline=time.monotonic()+20
                while True:
                    counts=ctl('inventory')['counts']
                    if all(counts.get('ntm:'+name,0)==1 for name in batch):break
                    if time.monotonic()>deadline:raise AssertionError(counts)
                    ctl('wait 2')
            # Optional vanilla-door acceptance: the batch must prove item
            # placement creates both halves and a real interaction opens it.
            for i,name in enumerate(data.get('doors',[])):
                x,z=-6+i*4,28
                cmd(f'fill {x-1} 101 {z-1} {x+1} 104 {z+1} minecraft:air')
                cmd(f'tp @s {x+.5} 101 {z-3.5}')
                cmd('clear @s');cmd(f'give @s ntm:{name} 1');ctl('wait 5')
                ctl(f'place {x} 101 {z} --item ntm:{name}')
                ctl('wait 5')
                for y,half in [(101,'lower'),(102,'upper')]:
                    placed=ctl(f'block {x} {y} {z}')
                    assert placed['block']=='ntm:'+name and 'half='+half in placed['state'],placed
                ctl(f'interact {x} 101 {z} north');ctl('wait 5')
                opened=ctl(f'block {x} 101 {z}')
                assert 'open=true' in opened['state'],opened
            ctl('key f1 tap');ctl('open-inventory');ctl('wait 5');ctl('screenshot '+str(evidence/'items.png'));ctl('close')
            ctl('quit');process.wait(timeout=60)
        text=(scratch/'client.log').read_text()
        refused=[line for line in text.splitlines() if any(s in line.lower() for s in ['content mod rejected','content mods not loaded','refused','panicked at'])]
        assert not refused,refused
        (evidence/'game.json').write_text(json.dumps({'home':str(scratch),'blocks':blocks,'item_samples':samples,
            'checks':checks,'refused':refused,'status':'pass'},ensure_ascii=False,indent=2)+'\n')
        print(json.dumps({'status':'pass','home':str(scratch),'checks':len(checks),'items':len(samples)}),flush=True)
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:process.wait(timeout=20)
            except subprocess.TimeoutExpired:process.kill();process.wait()
        if mute and mute.poll() is None:mute.terminate();mute.wait(timeout=10)
        (evidence/'game-checks.json').write_text(json.dumps(checks,ensure_ascii=False,indent=2)+'\n')
        shutil.copy2(scratch/'client.log',evidence/'game-client.log') if (scratch/'client.log').exists() else None


if __name__=='__main__':main()

#!/usr/bin/env python3
"""Disposable, silent content-only client probe for late bedrock/catalog assets."""
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
    parser.add_argument('--profile',choices=['bedrock','petroleum-e','energy-heaters','energy-steam','energy-combustion','energy-solar-stirling'],default='bedrock')
    args=parser.parse_args()
    root=Path(__file__).resolve().parents[2]
    evidence=root/'content-gen-evidence'/({'bedrock':'bedrock-stages34'}.get(args.profile,args.profile));evidence.mkdir(parents=True,exist_ok=True)
    temporary=root.parent/'.agent-tmp';temporary.mkdir(exist_ok=True)
    scratch=Path(tempfile.mkdtemp(prefix='content-processing-probe-',dir=temporary))
    for d in ['game','home','launcher','data','config']:(scratch/d).mkdir()
    (scratch/'game/options.txt').write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\nrenderDistance:2\nsimulationDistance:2\nmaxFps:15\nenableVsync:false\n')
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
            cmd('fill -9 100 -16 16 100 4 minecraft:stone')
            machines=['machine_hephaestus','machine_flare'] if args.profile=='petroleum-e' else ['machine_pyrooven','machine_electrolyser','machine_arc_furnace_large']
            if args.profile=='energy-heaters':machines=['heater_oven','heater_oilburner','heater_electric','heater_heatex']
            if args.profile=='energy-steam':machines=['machine_steam_engine','machine_industrial_turbine','machine_chungus','machine_condenser','machine_condenser_powered']
            if args.profile=='energy-combustion':machines=['machine_diesel','machine_combustion_engine','machine_turbofan','machine_turbinegas']
            if args.profile=='energy-solar-stirling':machines=['machine_solar_boiler','solar_mirror','machine_stirling','machine_stirling_steel','machine_stirling_creative']
            for i,name in enumerate(machines):
                x=-4+i*4
                for y,level in [(101,0),(103,5)]:
                    state=f'ntm:{name}[facing=north,honey_level={level}]'
                    cmd(f'setblock {x} {y} 0 {state}')
                    deadline=time.monotonic()+15
                    while True:
                        reply=ctl(f'block {x} {y} 0')
                        actual=reply['state']
                        if actual.split('[',1)[0]=='ntm:'+name and set(actual.split('[',1)[-1].rstrip(']').split(','))=={'facing=north','honey_level='+str(level)}:break
                        if time.monotonic()>deadline:raise AssertionError((state,reply))
                        ctl('wait 2')
            decor=['plant_flower__'+n for n in ['foxglove','tobacco','nightshade','weed','cd0','cd1']] if args.profile=='petroleum-e' else ['block_graphite','rebar','concrete_rebar']
            if args.profile.startswith('energy-'):decor=[]
            for i,name in enumerate(decor):
                x=-5+i*2 if args.profile=='petroleum-e' else -4+i*4
                if args.profile=='petroleum-e':cmd(f'setblock {x} 100 3 minecraft:grass_block')
                cmd(f'setblock {x} 101 3 ntm:{name}')
                deadline=time.monotonic()+15
                while True:
                    reply=ctl(f'block {x} 101 3')
                    if reply['block']=='ntm:'+name:break
                    if time.monotonic()>deadline:raise AssertionError((name,reply))
                    ctl('wait 2')
            camera_x,camera_z={'energy-solar-stirling':(4.5,-13.5),'energy-steam':(4.5,-13.5),'energy-combustion':(2.5,-11.5)}.get(args.profile,(0.5,-7.5))
            cmd(f'tp @s {camera_x} 101 {camera_z}')
            deadline=time.monotonic()+15
            while True:
                player=ctl('state').get('player')
                if player and abs(player['position'][2]-camera_z)<0.2 and abs(player['position'][1]-101)<0.2:break
                if time.monotonic()>deadline:raise AssertionError(player)
                ctl('wait 2')
            ctl('look 0 -7');ctl('wait 10');ctl('key f1 tap')
            ctl('screenshot '+str(evidence/'machines.png'))
            samples=['arc_electrode__graphite','arc_electrode_burnt__saturnite','scrap_plastic__board_blank',
                'scrap_plastic__card_processor','chemical_dye__black','chemical_dye__white','chemical_dye__lightblue',
                'powder_cement','ingot_semtex','ingot_schraranium','meteorite_sword_treated','meteorite_sword_etched']
            if args.profile=='petroleum-e':samples=['bio_wafer','pill_herbal']+decor
            if args.profile=='energy-heaters':samples=machines
            if args.profile=='energy-steam':samples=machines+['gear_large__iron','gear_large__steel']
            if args.profile=='energy-combustion':samples=machines+['piston_set__'+n for n in ['steel','dura','desh','starmetal']]+['ingot_starmetal']
            if args.profile=='energy-solar-stirling':samples=machines
            cmd('clear @s')
            for name in samples:cmd('give @s ntm:'+name+' 1')
            deadline=time.monotonic()+15
            while True:
                counts=ctl('inventory')['counts']
                if all(counts.get('ntm:'+name,0)==1 for name in samples):break
                if time.monotonic()>deadline:raise AssertionError(counts)
                ctl('wait 2')
            ctl('key f1 tap');ctl('open-inventory');ctl('wait 5');ctl('screenshot '+str(evidence/'items.png'));ctl('close')
            ctl('quit');process.wait(timeout=60)
        text=(scratch/'client.log').read_text()
        refused=[line for line in text.splitlines() if any(s in line.lower() for s in ['content mod rejected','content mods not loaded','refused','panicked at'])]
        assert not refused,refused
        (evidence/'game.json').write_text(json.dumps({'home':str(scratch),'machines':machines,'item_samples':samples,
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

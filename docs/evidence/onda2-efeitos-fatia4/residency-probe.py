from pathlib import Path
from types import SimpleNamespace
import importlib.util,json,os,subprocess,sys,tempfile,time
here=Path('/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/onda2-efeitos/probes/test-game')
sys.path.insert(0,str(here))
from native_game import Game
spec=importlib.util.spec_from_file_location('crash',here/'run-crash-proof.py');mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
root=Path('/mnt/ssd120/sacer/so/r4');root.mkdir(exist_ok=True)
folder=Path(tempfile.mkdtemp(prefix='res-',dir=root))
env=os.environ.copy()
for key,name in [('HORIZON_HOME','home'),('HORIZON_LAUNCHER_HOME','launcher'),('XDG_DATA_HOME','data'),('XDG_CONFIG_HOME','config')]:env[key]=str(folder/name)
env.update(HORIZON_NO_AUDIO='1',HORIZON_NO_UPDATE_CHECK='1',HORIZON_LAUNCHER_NO_SELF_UPDATE='1',WAYLAND_DISPLAY='')
(folder/'options.txt').write_text('maxFps:30\nenableVsync:false\ntutorialStep:none\n')
reader,writer=os.pipe();xvfb=subprocess.Popen(['Xvfb','-displayfd',str(writer),'-screen','0','1280x720x24','-nolisten','tcp'],pass_fds=(writer,),stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);os.close(writer)
with os.fdopen(reader) as f:env['DISPLAY']=':'+f.readline().strip()
args=SimpleNamespace(binary=Path('/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/onda2-efeitos/proof-bin/horizon-client'),resources=Path('/home/sacer/Games/horizon/.local/horizon-client/26.3'),bundle=Path('/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/onda2-efeitos/dist/ntm.hmod'))
world=mod.World(args,folder,env);rows=[]
def loaded(g,p):return g.ctl('block %d %d %d'%p)
def ticking(g,p):return any('TICKING' in x for x in g.command('execute if loaded %d %d %d run say TICKING'%p).get('chat',[]))
try:
 world.launch();g=Game(folder,'residency');g.ctl('wait-ready 120');g.command('forceload add 30 0 64 24');g.command('tp @s 40.5 101 8.5');time.sleep(3);g.command('setblock 47 102 8 minecraft:chest');g.command('item replace block 47 102 8 container.0 with ntm:pile_rod__ra226be 6');g.ctl('save');g.command('forceload remove all')
 for px in [24,8,-8,-24]:
  g.command(f'tp @s {px+.5} 101 8.5');time.sleep(8)
  a,b=loaded(g,(47,102,8)),loaded(g,(49,102,10));rows.append({'player_x':px+.5,'A':a,'B':b,'A_execute_loaded':ticking(g,(47,102,8)),'B_execute_loaded':ticking(g,(49,102,10))});print(json.dumps(rows[-1]),flush=True)
  if a.get('loaded') and not b.get('loaded'):break
 world.quit()
finally:
 if hasattr(world,'client') and world.client.poll() is None:world.client.kill();world.client.wait()
 xvfb.terminate();xvfb.wait(timeout=10)
 out=Path('/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/onda2-efeitos/ntm/docs/evidence/onda2-efeitos-fatia4/residency-probe.json');out.write_text(json.dumps({'folder':str(folder),'rows':rows},indent=2)+'\n')
 print(str(out),flush=True)

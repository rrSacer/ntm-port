#!/usr/bin/env python3
"""Exercise the real dedicated generator in fresh homes, then inspect MCA saves (stdlib)."""
import argparse,collections,hashlib,importlib.util,json,os,socket,subprocess,tempfile,time,shutil
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1];E=ROOT/'content-gen-evidence/worldgen';E.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('scan',HERE/'world-scan.py');scan=importlib.util.module_from_spec(spec);spec.loader.exec_module(scan)
PACKER='/home/sacer/Games/horizon/target/release/cargo-horizon-mod'
def request(root,line,label):
 with socket.socket(socket.AF_UNIX) as sock:
  sock.settimeout(45);sock.connect(str(root/'ctl.sock'));sock.sendall((line+'\n').encode());reply=json.loads(sock.makefile().readline())
 with (E/(label+'-control.jsonl')).open('a') as f:f.write(json.dumps({'command':line,'reply':reply})+'\n')
 return reply

def server(label,package):
 root=Path(tempfile.mkdtemp(prefix='ntm-m1-'+label+'-'))
 for name in ['game','launcher','data','config']:(root/name).mkdir()
 for options in [root/'options.txt',root/'game/options.txt']:options.write_text('soundCategory_master:0.0\nsoundCategory_music:0.0\n')
 (root/'eula.txt').write_text('eula=true\n')
 (root/'server.properties').write_text('online-mode=false\nserver-port=25731\nlevel-seed=1768123\nview-distance=2\nsimulation-distance=2\nspawn-chunk-radius=0\nhorizon-pregen=true\npause-when-empty-seconds=0\n')
 (E/(label+'-home.txt')).write_text(str(root)+'\n')
 env=os.environ|{'HORIZON_HOME':str(root/'game'),'HORIZON_LAUNCHER_HOME':str(root/'launcher'),'XDG_DATA_HOME':str(root/'data'),'XDG_CONFIG_HOME':str(root/'config'),'HORIZON_NO_AUDIO':'1','HORIZON_NO_UPDATE_CHECK':'1','HORIZON_LAUNCHER_NO_SELF_UPDATE':'1'}
 args=['/home/sacer/Games/horizon/target/release/horizon-client','--resources','/home/sacer/Games/horizon/.local/horizon-client/26.3','--dev-mod',str(package),'--control-socket',str(root/'ctl.sock'),'--dedicated','--nogui']
 with (E/(label+'.log')).open('w') as log:
  process=subprocess.Popen(args,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
  try:
   for _ in range(240):
    if process.poll() is not None:raise RuntimeError(f'{label} server exited {process.returncode}')
    if (root/'ctl.sock').exists():
     try:request(root,'server-status',label);break
     except (OSError,ValueError):pass
    time.sleep(.5)
   else:raise TimeoutError('server start')
   print(label,request(root,'console pregen start 128 square 0 0',label),flush=True)
   for _ in range(300):
    time.sleep(1);reply=request(root,'console pregen status',label)
    if 'No pregeneration' in str(reply) or '100%' in str(reply) or 'not running' in str(reply).lower():break
   else:raise TimeoutError('pregen')
   print(label,request(root,'console save-all flush',label),flush=True)
   request(root,'console stop',label);process.wait(timeout=90)
   if process.returncode:raise RuntimeError(f'{label}: exit {process.returncode}')
  finally:
   if process.poll() is None:process.terminate();process.wait(timeout=30)
 return root/'world/dimensions/minecraft/overworld'

def summarize(world,label):
 ores,chunks=scan.ore_map(world,(-4,4,-4,4));entries=sorted([list(p)+[n] for p,n in ores.items()]);counts=dict(collections.Counter(ores.values()))
 result={'seed':1768123,'chunks':len(chunks),'statuses':dict(collections.Counter(s for _,_,s in chunks)),'counts':counts,'sha256':hashlib.sha256(json.dumps(entries).encode()).hexdigest(),'ores':entries}
 (E/(label+'-scan.json')).write_text(json.dumps(result,indent=2)+'\n')
 print(label,'chunks',len(chunks),'counts',counts,flush=True)
 return result

def diagnostic(batch):
 dest=E/f'diagnostic-content-{batch}'
 if dest.exists():shutil.rmtree(dest)
 shutil.copytree(ROOT/'content',dest,ignore=shutil.ignore_patterns('target'))
 wg=dest/'data/ntm/worldgen';shutil.rmtree(wg)
 definitions=[]
 for source in sorted((ROOT/'content/data/ntm/worldgen/configured_feature').glob('m1_*.json')):definitions.append(json.loads(source.read_text()))
 all_definitions=definitions;definitions=definitions[batch*3:(batch+1)*3]
 bands=[]
 for ore in definitions:
  for material in ['stone','deepslate']:
   bands.append({'material':material,'y':-54+10*len(bands),'ores':[ore]})
 for material in ['dirt','andesite','bedrock','air']:
  bands.append({'material':material,'y':-54+10*len(bands),'ores':all_definitions})
 features=[]
 for band in bands:
  material,y=band['material'],band['y']
  fill={'type':'minecraft:replace_single_block','targets':[{'target':{'predicate_type':'minecraft:always_true'},'state':{'id':'minecraft:'+material}}]}
  overlay={'type':'minecraft:overlay','features':[{'feature':fill,'placement':[{'type':'minecraft:offset','x':-4,'y':-4,'z':-4},{'type':'minecraft:cuboid','xz_size':8,'y_size':8}]}]+[{'feature':ore,'placement':[{'type':'minecraft:count','count':4}]} for ore in band['ores']]}
  features.append({'feature':overlay,'placement':[{'type':'minecraft:height_range','height':{'absolute':y}}]})
 def put(kind,value):
  p=wg/kind/'m1_target_probe.json';p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(value,indent=2)+'\n')
 name='ntm:m1_target_probe'
 put('configured_feature',{'type':'minecraft:overlay','features':features})
 put('placed_feature',{'feature':name,'placement':[{'type':'minecraft:offset','x':8,'y':0,'z':8}]})
 put('template_pool',{'fallback':'minecraft:empty','elements':[{'weight':1,'element':{'element_type':'minecraft:feature_pool_element','projection':'rigid','feature':name}}]})
 put('structure',{'type':'minecraft:jigsaw','biomes':'#minecraft:is_overworld','step':'underground_ores','terrain_adaptation':'none','start_pool':name,'size':1,'start_height':{'absolute':32},'use_expansion_hack':False,'max_distance_from_center':16})
 put('structure_set',{'structures':[{'structure':name,'weight':1}],'placement':{'type':'minecraft:random_spread','spacing':2,'separation':1,'salt':2198761,'spread_type':'linear'}})
 subprocess.run([PACKER,'build'],cwd=dest,check=True)
 (E/f'target-bands-{batch}.json').write_text(json.dumps(bands,indent=2)+'\n')
 return dest/'target/horizon-mod/ntm.hmod',bands

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--diagnostic',action='store_true');args=parser.parse_args()
 if args.diagnostic:
  materials=['stone','deepslate','dirt','andesite','bedrock','air']
  counts={m:collections.Counter() for m in materials};outside=[];total_chunks=0
  for batch in range(4):
   package,bands=diagnostic(batch);world=server(f'targets-batch-{batch}',package);ores,chunks=scan.ore_map(world,(-4,4,-4,4));total_chunks+=len(chunks)
   for (x,y,z),name in ores.items():
    found=next((b['material'] for b in bands if abs(y-b['y'])<=4),None)
    if found:counts[found][name]+=1
    else:outside.append([x,y,z,name])
  result={'counts':{m:dict(c) for m,c in counts.items()},'outside_cubes':outside,'chunks_per_world':81,'worlds':4,'total_scanned_chunks':total_chunks}
  result['pass']=not outside and all(not counts[m] for m in materials[2:]) and all(len(counts[m])==11 for m in materials[:2])
  (E/'target-checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
  if not result['pass']:raise RuntimeError('target checks failed')
 else:
  package=ROOT/'content/target/horizon-mod/ntm.hmod'
  first=summarize(server('stable-initial',package),'stable-initial');repeat=summarize(server('stable-repeat',package),'stable-repeat')
  specs=json.loads((HERE/'m1.json').read_text())['worldgen']
  result={'seed':1768123,'chunks':first['chunks'],'same_seed_same_blocks':first['ores']==repeat['ores'],'sha256':first['sha256'],'materials':[]}
  for entry in specs:
   count=first['counts'].get(entry['block'],0);result['materials'].append(dict(entry,count=count,blocks_per_chunk=count/first['chunks'],blocks_per_declared_attempt=count/(first['chunks']*entry['attempts_per_chunk'])))
  result['pass']=result['same_seed_same_blocks'] and all(r['count']>0 for r in result['materials'])
  (E/'generation-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
  if not result['pass']:raise RuntimeError('worldgen checks failed')
if __name__=='__main__':main()

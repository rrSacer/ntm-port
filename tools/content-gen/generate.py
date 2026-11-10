#!/usr/bin/env python3
"""Generate NTM Horizon content from an explicit catalog and the licensed NTM checkout."""
import argparse
import json
import re
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path
import worldgen
import bulk_recipes
from vanilla_references import sound_block, VanillaReferences

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MATERIAL_PT = dict(zip('uranium thorium titanium lead red_copper aluminium tungsten beryllium lithium asbestos fluorite niter sulfur'.split(), ['Urânio', 'Tório', 'Titânio', 'Chumbo', 'Cobre Vermelho', 'Alumínio', 'Tungstênio', 'Berílio', 'Lítio', 'Amianto', 'Fluorita', 'Salitre', 'Enxofre']))
FORM_PT = {'ingot':'Lingote de {}','nugget':'Pepita de {}','plate':'Placa de {}','powder':'Pó de {}','wire':'Fio de {}','wiring':'Bobina de {}','billet':'Tarugo de {}','crystal':'Cristal de {}','ore':'Minério de {}','storage':'Bloco de {}'}
MACHINE_PT = {'machine_wood_burner':'Gerador a Combustão', 'red_cable':'Cabo de Cobre Vermelho', 'machine_battery':'Bloco de Armazenamento de Energia', 'machine_electric_furnace_off':'Fornalha Elétrica', 'machine_press':'Prensa a Combustão', 'machine_epress':'Prensa Elétrica', 'machine_purex':'PUREX'}
MACHINE_TEXTURES = {
 'machine_wood_burner': {'side':'blocks/block_steel', 'top':'blocks/block_steel', 'front':'models/machines/wood_burner'},
 'machine_battery': {'side':'blocks/battery_side_alt','top':'blocks/battery_top','front':'blocks/battery_front_alt'},
 'machine_electric_furnace_off': {'side':'blocks/machine_electric_furnace_side','top':'blocks/machine_electric_furnace_top','bottom':'blocks/machine_electric_furnace_bottom','front':'blocks/machine_electric_furnace_front_off','front_on':'blocks/machine_electric_furnace_front_on'},
 'machine_press': {'side':'blocks/block_steel','top':'blocks/block_steel','front':'blocks/machine_press'},
 'machine_epress': {'side':'blocks/block_steel','top':'blocks/block_steel','front':'blocks/machine_epress'},
 'red_cable': {'side':'blocks/cable_neo','top':'blocks/cable_neo','front':'blocks/cable_neo'},
}


def write_json(path, value):
 path.parent.mkdir(parents=True, exist_ok=True)
 path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')


def lang(path):
 result = {}
 for line in path.read_text(encoding='utf-8-sig').splitlines():
  if '=' in line and not line.lstrip().startswith('#'):
   k,v=line.split('=',1); result[k.strip()]=v.strip()
 return result


def registration(source, variable, kind):
 # Registrations may use the local register(Block) helper or the Forge API.
 call = rf'(?:GameRegistry\.register{kind.title()}\(\s*{variable}\s*,|\bregister\(\s*{variable}\s*\))'
 if not re.search(call,source): raise ValueError(f'No {kind} registration for {variable}')
 assignment = re.search(rf'^\s*(?:ModItems\.)?{variable}\s*=\s*(?:\([A-Za-z0-9_$.]+\)\s*)?new\b', source, re.M)
 if not assignment: raise ValueError(f'No assignment for {variable}')
 # A builder chain may carry `;` inside a trailing `//` comment: end the
 # statement at the first `;` that survives comment stripping.
 lines=[]
 depth=0
 for line in source[assignment.start():].split('\n'):
  code=re.sub(r'\s//.*$','',line)
  lines.append(code)
  structural=re.sub(r'"(?:\\.|[^"\\])*"', '""', code)
  depth+=sum(structural.count(c) for c in '({[')-sum(structural.count(c) for c in ')}]')
  if depth==0 and code.rstrip().endswith(';'):break
 text='\n'.join(lines)
 name=re.search(r'\.(?:setBlockName|setUnlocalizedName)\("([^"]+)"\)',text)
 if not name: raise ValueError(f'No unlocalized name for {variable}')
 tex=re.search(r'\.(?:setBlockTextureName|setTextureName)\(RefStrings.MODID\s*\+\s*":([^"]+)"\)',text)
 return name[1], tex[1] if tex else None


def catalog(src, manifest):
 java=src/'src/main/java/com/hbm'; assets=src/'src/main/resources/assets/hbm'
 sources={k:(java/f'{folder}/Mod{folder.title()}.java').read_text() for k,folder in [('block','blocks'),('item','items')]}
 sources['item']+='\n'+(java/'items/ModItemsArmor.java').read_text()
 sources['item']+='\n'+(java/'items/special/ItemSimpleConsumable.java').read_text()
 langs={loc:lang(assets/f'lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
 mats=(java/'inventory/material/Mats.java').read_text()
 result=[]
 for spec in manifest['entries']:
  row=dict(spec); kind=row['type']; name,texture=registration(sources[kind],row['source_variable'],kind)
  row['id']='ntm:'+row['id']; row['source_registry']='hbm:'+('tile.' if kind=='block' else 'item.')+name
  preset_names=row.get('names',{}); preset_texture=row.get('source_texture')
  row['names']={}; row['name_sources']={}
  if 'metadata' in row and 'subtype_kind' not in row:
   mat_line=next(l for l in mats.splitlines() if re.search(r'\b'+row['source_material']+r'\s*=',l))
   form={'nugget_raw':'NUGGET','wire_fine':'WIRE','wire_dense':'DENSEWIRE','plate_cast':'CASTPLATE','plate_welded':'WELDEDPLATE'}[name]
   if not re.search(r'\b'+form+r'\b',mat_line):raise ValueError(f'Unsupported subtype {row["id"]}')
   meta_expr=re.search(r'makeSmeltable\(([^,]+)',mat_line)[1].strip()
   meta=int(meta_expr) if meta_expr.isdigit() else 30+int(meta_expr.split('+')[1])
   if meta!=row['metadata']:raise ValueError(f'Metadata changed: {row["id"]}')
   colors=re.findall(r'0x([0-9A-Fa-f]{6})',mat_line)
   override=re.search(r'\.aot\(Mats\.'+row['source_material']+r',\s*"([^"]+)"\)',sources['item']) if name=='wire_fine' else None
   if override: texture=override[1]
   else: row['tint']={'light':colors[0],'dark':colors[1]}
  if name in MACHINE_TEXTURES:
   row['textures']=MACHINE_TEXTURES[name]; texture=row['textures']['front']
  else: texture=('blocks/' if kind=='block' else 'items/')+str(texture)
  row['source_texture']=None if row.get('texture_reference') else preset_texture or 'src/main/resources/assets/hbm/textures/'+texture+'.png'
  for loc,translations in langs.items():
   key=('tile.' if kind=='block' else 'item.')+name+'.name'
   value=preset_names.get(loc,translations.get(key))
   if 'metadata' in row and 'subtype_kind' not in row:
    mat=row['material']; material_name=(MATERIAL_PT[mat] if loc=='pt_br' else mat.replace('_',' ').title())
    value=value.replace('%s',material_name) if value else None
   # Some pt_BR entries are absent or still use the English source text.
   if not value or (loc=='pt_br' and loc not in preset_names and value==langs['en_us'].get(key)):
    if loc=='pt_br':
     value=MACHINE_PT.get(name)
     if value is None and row['category']=='stamp':value='Matriz de Ferro ('+name.rsplit('_',1)[1]+')'
     if value is None:
      mat=MATERIAL_PT.get(row.get('material'),row.get('material',name).replace('_',' ').title())
      if name.startswith('plate_cast'):value='Placa Fundida de '+mat
      elif name.startswith('plate_welded'):value='Placa Soldada de '+mat
      elif name.startswith('wire_dense'):value='Fio Denso de '+mat
      else:value=FORM_PT.get(row['category'],'{}').format(mat)
    else:value=name.replace('_',' ').title()
    row['name_sources'][loc]='port translation' if loc=='pt_br' else 'fallback'
   elif loc in preset_names:row['name_sources'][loc]='catalog subtype name derived from NTM localization/render declarations (fallbacks documented in bulk catalog)'
   else: row['name_sources'][loc]=f'assets/hbm/lang/{"en_US" if loc=="en_us" else "pt_BR"}.lang:{key}'
   row['names'][loc]=value
  if not row.get('texture_reference') and not (src/row['source_texture']).is_file():raise ValueError('Missing '+row['source_texture'])
  result.append(row)
 ids=[r['id'] for r in result]
 if len(set(ids))!=len(ids):raise ValueError('Duplicate ID')
 return result


def transform_png(raw, light=None, dark=None, background=None, crop=None, multiply=None):
 """NTM's RGB component remap, using stdlib PNG decoding (8-bit, noninterlaced)."""
 pos=8; chunks={}; compressed=b''
 while pos<len(raw):
  size=struct.unpack('>I',raw[pos:pos+4])[0]; kind=raw[pos+4:pos+8]; data=raw[pos+8:pos+8+size];pos+=size+12
  if kind==b'IDAT':compressed+=data
  else:chunks[kind]=data
  if kind==b'IEND':break  # Source PNGs may have trailing bytes after the image.
 width,height,depth,ctype,_,_,interlace=struct.unpack('>IIBBBBB',chunks[b'IHDR'])
 if depth!=8 or interlace:raise ValueError('Transform needs 8-bit noninterlaced PNG')
 bpp={0:1,2:3,3:1,4:2,6:4}[ctype]; stride=width*bpp; decoded=zlib.decompress(compressed)
 prior=bytearray(stride); output=bytearray(); lo=[int(light[i:i+2],16) for i in (0,2,4)] if light else None;hi=[int(dark[i:i+2],16) for i in (0,2,4)] if dark else None
 for y in range(height):
  offset=y*(stride+1); filt=decoded[offset]; row=bytearray(decoded[offset+1:offset+1+stride])
  for x in range(stride):
   a=row[x-bpp] if x>=bpp else 0;b=prior[x];c=prior[x-bpp] if x>=bpp else 0
   p=a+b-c; pa,pb,pc=abs(p-a),abs(p-b),abs(p-c)
   predictor=(0,a,b,(a+b)//2,a if pa<=pb and pa<=pc else b if pb<=pc else c)[filt]
   row[x]=(row[x]+predictor)&255
  output.append(0)
  for x in range(width):
   pix=row[x*bpp:(x+1)*bpp]
   if ctype==6:r,g,b,alpha=pix
   elif ctype==2:r,g,b=pix;alpha=255
   elif ctype==3:
    index=pix[0];r,g,b=chunks[b'PLTE'][index*3:index*3+3]; alpha=chunks.get(b'tRNS',b'')[index] if index<len(chunks.get(b'tRNS',b'')) else 255
   else:r=g=b=pix[0];alpha=pix[1] if ctype==4 else 255
   rgb=[int(lo[i]+((val-255)/(80-255))*(hi[i]-lo[i]))&255 for i,val in enumerate((r,g,b))] if lo else [r,g,b]
   if multiply:rgb=[v*int(multiply[i*2:i*2+2],16)//255 for i,v in enumerate(rgb)]
   output.extend(rgb+[alpha])
  prior=row
 if crop:
  x0,y0,x1,y1=crop; cropped=bytearray()
  for y in range(y0,y1):cropped.extend(b'\0'+output[y*(width*4+1)+1+x0*4:y*(width*4+1)+1+x1*4])
  width,height=x1-x0,y1-y0;output=cropped
 if background:
  # Composite an icon over the steel casing; its alpha must not punch holes in a full cube.
  bg=transform_png(background)
  bw,bh=struct.unpack('>II',bg[16:24]);pos=8;compressed=b''
  while pos<len(bg):
   n=struct.unpack('>I',bg[pos:pos+4])[0]
   if bg[pos+4:pos+8]==b'IDAT':compressed+=bg[pos+8:pos+8+n]
   pos+=n+12
  backing=zlib.decompress(compressed)
  for y in range(height):
   for x in range(width):
    p=y*(width*4+1)+1+x*4;b=(y*bh//height)*(bw*4+1)+1+(x*bw//width)*4;alpha=output[p+3]
    for c in range(3):output[p+c]=(output[p+c]*alpha+backing[b+c]*(255-alpha))//255
    output[p+3]=255
 def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(bytes(output),9))+chunk(b'IEND',b'')


def generate(src,out,rows,distributions,max_damage=True):
 """Generate fallbacks, yielding claimed paths to visual-gen, including during cleanup."""
 ownership=out/'.visual-files.json'
 visual_owned=set(json.loads(ownership.read_text())) if ownership.exists() else set()
 generated=[]
 def emit_bytes(rel,data):
  if rel in visual_owned:return
  path=out/rel;path.parent.mkdir(parents=True,exist_ok=True)
  path.write_bytes(data);generated.append(rel)
 def emit(rel,value):
  emit_bytes(rel,(json.dumps(value,ensure_ascii=False,indent=2)+'\n').encode('utf-8'))
 def texture(source,tint=None,suffix=''):
  relative=source.removeprefix('src/main/resources/assets/hbm/textures/').removesuffix('.png')
  relative=relative.replace('blocks/','block/',1) if relative.startswith('blocks/') else relative.replace('items/','item/',1) if relative.startswith('items/') else 'block/'+relative
  relative=relative.lower()
  dest='assets/ntm/textures/'+relative+suffix+'.png'
  data=(src/source).read_bytes()
  if tint:data=transform_png(data,**tint)
  emit_bytes(dest,data)
  # Preserve animation metadata when copying an animated source.
  mcmeta=src/(source+'.mcmeta')
  if mcmeta.exists():emit_bytes(dest+'.mcmeta',mcmeta.read_bytes())
  return 'ntm:'+relative+suffix
 emit_bytes('horizon-mod.toml',('id = "ntm"\nname = "HBM Nuclear Tech Mod — Content"\nversion = "0.2.0"\napi = "0.2"\nside = "both"\ncapabilities = ["registries"]\n').encode('utf-8'))
 emit('data/ntm/horizon/block/logic_token.json',{'template':'minecraft:composter','item':False})
 emit('assets/ntm/models/block/logic_token.json',{'parent':'minecraft:block/cube_all','textures':{'all':'minecraft:block/bedrock'}})
 emit('assets/ntm/blockstates/logic_token.json',{'variants':{'':{'model':'ntm:block/logic_token'}}})
 languages={loc:{} for loc in ('en_us','pt_br')};blocks=[]
 for row in rows:
  name=row['id'].split(':')[1];kind=row['type']; machine=row['category']=='machine'
  tex=row.get('texture_reference') or texture(row['source_texture'],row.get('tint'),'__'+row.get('material',name) if 'tint' in row else '')
  if row.get('texture_crop'):
   dest='assets/ntm/textures/item/'+name+'_frame.png'
   emit_bytes(dest,transform_png((src/row['source_texture']).read_bytes(),crop=row['texture_crop']));tex='ntm:item/'+name+'_frame'
  if row.get('background_texture'):
   dest='assets/ntm/textures/block/'+name+'_composite.png'
   emit_bytes(dest,transform_png((src/row['source_texture']).read_bytes(),background=(src/row['background_texture']).read_bytes()));tex='ntm:block/'+name+'_composite'
  if row.get('icon_cube'):
   dest='assets/ntm/textures/block/'+name+'_opaque.png'
   emit_bytes(dest,transform_png((src/row['source_texture']).read_bytes(),background=(src/'src/main/resources/assets/hbm/textures/blocks/block_steel.png').read_bytes()));tex='ntm:block/'+name+'_opaque'
  if kind=='item':
   declaration={'template':row.get('template','minecraft:iron_ingot'),'max_stack_size':row.get('max_stack_size',64)}
   if max_damage and row.get('max_damage'):
    if declaration['max_stack_size']!=1:raise ValueError('Durable item must have stack size 1: '+row['id'])
    declaration['components']={'minecraft:max_damage':row['max_damage']}
   if row.get('rarity'):declaration['rarity']=row['rarity']
   if row.get('components'):declaration.setdefault('components',{}).update(row['components'])
   emit(f'data/ntm/horizon/item/{name}.json',declaration)
   emit(f'assets/ntm/models/item/{name}.json',{'parent':'minecraft:item/generated','textures':{'layer0':tex}})
   if row.get('overlay_textures'):
    layers={'layer0':tex}
    for layer,overlay in enumerate(row['overlay_textures'],1):layers['layer'+str(layer)]=texture(overlay['source'],overlay.get('tint'),'__'+name if overlay.get('tint') else '')
    emit(f'assets/ntm/models/item/{name}.json',{'parent':'minecraft:item/generated','textures':layers})
   model='ntm:item/'+name
  else:
   blocks.append(row['id'])
   template=row.get('template','minecraft:beehive' if machine else 'minecraft:iron_block' if row['category'] in ('storage','resource_block') else 'minecraft:stone')
   properties={'hardness':5.0,'explosion_resistance':10.0,'requires_tool':True,'sounds':'minecraft:iron_block' if machine or row['category']=='storage' else 'minecraft:stone','light':0,**row.get('block_properties',{})}
   properties['sounds']=sound_block(properties['sounds'])
   emit(f'data/ntm/horizon/block/{name}.json',{'template':template,'properties':properties,'item':True})
   if machine:
    textures={face:texture('src/main/resources/assets/hbm/textures/'+path+'.png') for face,path in row['textures'].items()}
    if row.get('opaque_machine_textures'):
     backing=(src/'src/main/resources/assets/hbm/textures/blocks/block_steel.png').read_bytes()
     for face,path in row['textures'].items():
      dest=f'assets/ntm/textures/block/{name}_{face}_opaque.png'
      emit_bytes(dest,transform_png((src/('src/main/resources/assets/hbm/textures/'+path+'.png')).read_bytes(),background=backing))
      textures[face]=f'ntm:block/{name}_{face}_opaque'
    if name in ('machine_press','machine_epress','machine_wood_burner'):
     dest=f'assets/ntm/textures/block/{name}_front.png'
     raw=(src/row['source_texture']).read_bytes()
     backing=(src/'src/main/resources/assets/hbm/textures/blocks/block_steel.png').read_bytes()
     emit_bytes(dest,transform_png(raw,background=backing,crop=(0,32,64,64) if name=='machine_wood_burner' else None))
     textures['front']=f'ntm:block/{name}_front'
    # A small meter gives every numeric state visible meaning, including TESR approximations.
    for level in range(6):
     front=textures.get('front_on',textures['front']) if level else textures['front']
     model_tex={'particle':textures['side'],'down':textures.get('bottom',textures['top']),'up':textures['top'],'north':front,'south':textures['side'],'east':textures['side'],'west':textures['side']}
     model={'textures':model_tex,'elements':[{'from':[0,0,0],'to':[16,16,16],'faces':{face:{'texture':'#'+face,'cullface':face} for face in ('down','up','north','south','east','west')}}]}
     if level:
      model['textures']['meter']='minecraft:block/redstone_block'
      model['elements'].append({'from':[2,1,-0.02],'to':[2+12*level/5,2,0],'faces':{'north':{'texture':'#meter'}}})
     emit(f'assets/ntm/models/block/{name}_{level}.json',model)
    variants={f'facing={facing},honey_level={level}':{'model':f'ntm:block/{name}_{level}','y':rotation} for facing,rotation in [('north',0),('east',90),('south',180),('west',270)] for level in range(6)}
    model=f'ntm:block/{name}_0'
   else:
    emit(f'assets/ntm/models/block/{name}.json',{'parent':'minecraft:block/cube_all','textures':{'all':tex}})
    model='ntm:block/'+name; variants={'':{'model':model}}
    if row.get('cross_model'):
     emit(f'assets/ntm/models/block/{name}.json',{'parent':'minecraft:block/cross','textures':{'cross':tex}})
    if row.get('surface_textures'):
     faces={k:texture(v) for k,v in row['surface_textures'].items()}
     emit(f'assets/ntm/models/block/{name}.json',{'parent':'minecraft:block/cube_bottom_top','textures':faces})
    if row['category']=='bomb':
     variants=({f'facing={facing},honey_level={level}':{'model':model,'y':rotation}
                for facing,rotation in [('north',0),('east',90),('south',180),('west',270)] for level in range(6)}
               if template=='minecraft:beehive' else {f'lit={lit}':{'model':model} for lit in ('false','true')})
    if template in ('minecraft:oak_log','minecraft:quartz_pillar'):
     faces={k:texture(v) for k,v in row['surface_textures'].items()}
     emit(f'assets/ntm/models/block/{name}.json',{'parent':'minecraft:block/cube_column','textures':{'side':faces['side'],'end':faces['top']}})
     variants={'axis=y':{'model':model},'axis=x':{'model':model,'x':90,'y':90},'axis=z':{'model':model,'x':90}}
    if template=='minecraft:stone_stairs':
     vanilla=Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft')
     variants=json.loads((vanilla/'blockstates/stone_stairs.json').read_text())['variants']
     for variant in variants.values():variant['model']=variant['model'].replace('minecraft:block/stone_stairs','ntm:block/'+name)
     for suffix,parent in [('', 'stairs'),('_inner','inner_stairs'),('_outer','outer_stairs')]:
      emit(f'assets/ntm/models/block/{name}{suffix}.json',{'parent':'minecraft:block/'+parent,'textures':{'side':tex,'top':tex,'bottom':tex}})
    if template=='minecraft:stone_slab':
     for suffix,parent in [('', 'slab'),('_top','slab_top'),('_double','cube_all')]:
      emit(f'assets/ntm/models/block/{name}{suffix}.json',{'parent':'minecraft:block/'+parent,'textures':{'side':tex,'top':tex,'bottom':tex,'all':tex}})
     variants={f'type={kind}':{'model':'ntm:block/'+name+suffix} for kind,suffix in [('bottom',''),('top','_top'),('double','_double')]}
    if row.get('door_textures'):
     # Preserve vanilla door geometry/state mapping; only replace its texture
     # pair. Powered has no model distinction, as in the vanilla blockstate.
     door_tex={face:texture(path) for face,path in row['door_textures'].items()}
     resources=VanillaReferences().resources/'assets/minecraft'
     variants=json.loads((resources/'blockstates/oak_door.json').read_text())['variants']
     for variant in variants.values():
      suffix=variant['model'].split('oak_door',1)[1]
      original=json.loads((resources/('models/block/oak_door'+suffix+'.json')).read_text())
      original['textures']=door_tex
      emit(f'assets/ntm/models/block/{name}{suffix}.json',original)
      variant['model']='ntm:block/'+name+suffix
     model='ntm:block/'+name+'_bottom_left'
    if template=='minecraft:snow':
     variants={}
     for layers in range(1,9):
      snow_model=f'ntm:block/{name}_{layers}';emit(f'assets/ntm/models/block/{name}_{layers}.json',{'parent':f'minecraft:block/snow_height{layers*2}' if layers<8 else 'minecraft:block/cube_all','textures':{'texture':tex,'all':tex}});variants[f'layers={layers}']={'model':snow_model}
     model=f'ntm:block/{name}_1'
   emit(f'assets/ntm/blockstates/{name}.json',{'variants':variants})
   drop={'cluster_iron':'ntm:crystal_iron','cluster_copper':'ntm:crystal_copper','cluster_titanium':'ntm:crystal_titanium','cluster_aluminium':'ntm:crystal_aluminium','ore_depth_cinnebar':'ntm:cinnebar','ore_depth_zirconium':'ntm:nugget_zirconium','ore_depth_borax':'ntm:powder_borax','ore_depth_nether_neodymium':'ntm:fragment_neodymium','ore_alexandrite':'ntm:gem_alexandrite','ore_nether_coal':'ntm:coal_infernal','ore_nether_sulfur':'ntm:sulfur','ore_gneiss_asbestos':'ntm:ingot_asbestos','ore_cobalt':'ntm:fragment_cobalt','ore_nether_cobalt':'ntm:fragment_cobalt','ore_cinnebar':'ntm:cinnebar','ore_coltan':'ntm:fragment_coltan','ore_lignite':'ntm:lignite','ore_rare':'ntm:chunk_ore__rare','ore_gneiss_rare':'ntm:chunk_ore__rare','ore_fluorite':'ntm:fluorite','ore_niter':'ntm:niter','ore_sulfur':'ntm:sulfur','ore_asbestos':'ntm:ingot_asbestos'}.get(name,row['id'])
   drop=row.get('drop',drop)
   entry={'type':'minecraft:item','name':drop}
   if row.get('drop_modifiers'):entry['modifier']=row['drop_modifiers']
   ranges={'ore_depth_cinnebar':(2,4),'ore_depth_zirconium':(2,3),'ore_depth_nether_neodymium':(2,3),'ore_fluorite':(2,4),'ore_niter':(2,4),'ore_sulfur':(2,4),'ore_nether_sulfur':(2,4),'ore_cobalt':(4,9),'ore_nether_cobalt':(5,12)}
   if name in ranges:
    low,high=ranges[name];entry['modifier']=[{'type':'minecraft:set_count','count':{'type':'minecraft:uniform','min':low,'max':high}}]
   if name=='ore_nether_fire':entry={'type':'minecraft:alternatives','children':[{'type':'minecraft:item','name':'ntm:ingot_phosphorus','conditions':[{'condition':'minecraft:random_chance','chance':0.1}]},{'type':'minecraft:item','name':'ntm:powder_fire'}]}
   if template=='minecraft:stone_slab':entry['modifier']=[{'type':'minecraft:set_count','count':2,'conditions':[{'condition':'minecraft:block_state_property','block':row['id'],'properties':{'type':'double'}}]}]
   if row.get('door_textures'):entry['conditions']=[{'condition':'minecraft:block_state_property','block':row['id'],'properties':{'half':'lower'}}]
   emit(f'data/ntm/loot_table/blocks/{name}.json',{'type':'minecraft:block','pools':row.get('loot_pools',[] if row.get('loot_none') else [{'rolls':1,'entries':[entry]}])})
  if row.get('item_texture'):
   emit(f'assets/ntm/models/item/{name}.json',{'parent':'minecraft:item/generated','textures':{'layer0':texture(row['item_texture'])}})
   model='ntm:item/'+name
  emit(f'assets/ntm/items/{name}.json',{'model':{'type':'minecraft:model','model':model}})
  for loc in languages:languages[loc][('block.' if kind=='block' else 'item.')+'ntm.'+name]=row['names'][loc]
 if any(row['id']=='ntm:machine_armor_table' for row in rows):
  emit('data/ntm/horizon/menu/armor_table.json',{'slots':14,'data_slots':0})
  gui='src/main/resources/assets/hbm/textures/gui/machine/gui_armor_modifier.png'
  emit_bytes('assets/ntm/textures/gui/machine/gui_armor_modifier.png',(src/gui).read_bytes())
  languages['en_us']['container.ntm.armor_table']='Armor Modification Table'
  languages['pt_br']['container.ntm.armor_table']='Mesa de Modificação de Armadura'
  import rbmk_catalog
  rbmk_catalog.tooltip_lang(src,languages)
 import mob_effects
 mob_effects.generate(src,emit,emit_bytes,languages,transform_png,{loc:lang(src/f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]})
 for loc,value in languages.items():
  # Preserve visual-gen's dummy_machine lang key across content regenerations when dummy_machine is active.
  existing_lang=out/f'assets/ntm/lang/{loc}.json'
  if 'data/ntm/horizon/block/dummy_machine.json' in visual_owned and existing_lang.is_file():
   prior_lang=json.loads(existing_lang.read_text())
   if 'block.ntm.dummy_machine' in prior_lang:value['block.ntm.dummy_machine']=prior_lang['block.ntm.dummy_machine']
  emit(f'assets/ntm/lang/{loc}.json',value)
 emit('data/minecraft/tags/block/mineable/pickaxe.json',{'replace':False,'values':blocks})
 emit('data/minecraft/tags/block/needs_iron_tool.json',{'replace':False,'values':[r['id'] for r in rows if r['category']=='ore']})
 emit('data/ntm/tags/item/fluid_identifiers.json',{'replace':False,'values':[r['id'] for r in rows if r.get('fluid_identifier')]})
 worldgen.generate(emit,rows,distributions)
 bulk_recipes.generate(src,out,rows,emit)
 import bedrock_recipes
 bedrock_recipes.generate_crafting(src,rows,emit)
 # Only remove files this generator owned previously. Never delete sibling pipeline output.
 previous=out/'.content-gen-files.json'
 if previous.exists():
  for old in json.loads(previous.read_text()):
   if old not in generated and old not in visual_owned and (out/old).is_file():(out/old).unlink()
 write_json(previous,sorted(set(generated)))
 return len(set(generated))


def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--source',type=Path,default=ROOT.parent/'ntm-src')
 parser.add_argument('--output',type=Path,default=ROOT/'content')
 parser.add_argument('--catalog',type=Path,default=HERE/'m1.json')
 parser.add_argument('--ids-only',action='store_true')
 parser.add_argument('--max-damage',action='store_true',default=True,help='Compatibility flag: durable items emit minecraft:max_damage components by default')
 args=parser.parse_args()
 if args.catalog.resolve()==(HERE/'m1.json').resolve():
  import m3_catalog
  m3_catalog.build(args.source,args.output)
 manifest=json.loads(args.catalog.read_text())
 if args.catalog.resolve()==(HERE/'m1.json').resolve() and (HERE/'bulk.json').exists():
  for extension in ('bulk.json','bombs.json','m3.json','item_hooks.json'):
   if not (HERE/extension).exists():continue
   bulk=json.loads((HERE/extension).read_text())
   original_ids={e['id'] for e in manifest['entries']}
   if extension in ('bombs.json','m3.json','item_hooks.json'):
    for spec in bulk['entries']:
     previous=next((r for r in manifest['entries'] if r['id']==spec['id']),None)
     if previous:
      if previous['type']!=spec['type']:raise ValueError('Cross-kind public ID collision: '+spec['id'])
      previous.update(spec)
      # A null in an override row removes the field (a gap the hooks closed).
      for key,value in spec.items():
       if value is None:previous.pop(key,None)
     else:manifest['entries'].append(spec)
   else:manifest['entries'] += [e for e in bulk['entries'] if e['id'] not in original_ids]
 rows=catalog(args.source,manifest)
 bulk_recipes.annotate(args.source,rows)

 for row in rows:
  if 'sounds' in row.get('block_properties', {}):
   row['block_properties']['sounds']=sound_block(row['block_properties']['sounds'])
  if row['category']=='machine':row['logic']={'kind':'machine','facing':'facing','active':None,'level':{'prop':'honey_level','max':5}}
  elif row['category']=='cable':row['logic']={'kind':'cable'}
  elif row['category']=='duct':row['logic']={'kind':'duct'}
 write_json(args.output/'ids.json',{'schema':1,'namespace':'ntm','subtype_convention':'<original_registry_path>__<material>; source_registry and metadata retain 1.7.10 identity','ducts':[r['id'] for r in rows if r['category']=='duct'],'bombs':[r['id'] for r in rows if r['category']=='bomb'],'machines':[r['id'] for r in rows if r['category']=='machine'],'internal_blocks':[{'id':'ntm:logic_token','type':'block','category':'internal','item':False,'template':'minecraft:composter','logic':{'level':{'prop':'level','max':8}}}],'worldgen':manifest.get('worldgen',[]),'worldgen_gaps':manifest.get('worldgen_gaps',{}),'entries':rows})
 count=0 if args.ids_only else generate(args.source,args.output,rows,manifest.get('worldgen',[]),max_damage=args.max_damage)
 if not args.ids_only and args.catalog.resolve()==(HERE/'m1.json').resolve():
  import machine_recipes
  machine_recipes.export(args.source,args.output,rows)
 recipe_generator=HERE.parent/'recipe-gen/generate.py'
 if not args.ids_only and recipe_generator.is_file() and args.output.resolve()==(ROOT/'content').resolve() and args.source.resolve()==(ROOT.parent/'ntm-src').resolve():
  subprocess.run([sys.executable,str(recipe_generator)],check=True)
 hazard_generator=HERE.parent/'hazard-gen/extract.py'
 if not args.ids_only and hazard_generator.exists() and args.output.resolve()==(ROOT/'content').resolve():
  subprocess.run([sys.executable,str(hazard_generator)],check=True)
 creative_generator=HERE.parent/'creative-tabs/generate.py'
 if not args.ids_only and creative_generator.exists():
  # Stage the other generator's output; merge its language additions only after
  # content generation has finalized the item/block keys.
  with tempfile.TemporaryDirectory(prefix='ntm-creative-') as staging:
   stage=Path(staging)
   subprocess.run([sys.executable,str(creative_generator),'--mod',str(args.output),'--ntm-src',str(args.source),'--ids',str(args.output/'ids.json'),'--out',str(stage)],check=True)
   owned=[]
   for file in sorted((stage/'data').rglob('*.json')):
    rel=str(file.relative_to(stage));write_json(args.output/rel,json.loads(file.read_text()));owned.append(rel)
   additions=json.loads((stage/'lang-additions.json').read_text())
   for loc,keys in additions.items():
    path=args.output/f'assets/ntm/lang/{loc}.json';translations=json.loads(path.read_text());translations.update(keys);write_json(path,translations)
   previous=args.output/'.creative-tabs-files.json'
   if previous.exists():
    for rel in json.loads(previous.read_text()):
     if rel not in owned:(args.output/rel).unlink(missing_ok=True)
   write_json(previous,owned)
 if not args.ids_only:VanillaReferences().validate_content(args.output)
 print(json.dumps({'entries':len(rows),'blocks':sum(r['type']=='block' for r in rows),'items':sum(r['type']=='item' for r in rows),'generated_files':count,'ids':str(args.output/'ids.json')},ensure_ascii=False))

if __name__=='__main__':main()

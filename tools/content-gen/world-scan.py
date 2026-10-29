"""Offline MCA/NBT reader; vanilla padded block-state packing, no dependencies."""
import struct,zlib,gzip,json,sys,collections,hashlib
from pathlib import Path
class NBT:
 def __init__(self,b):self.b=b;self.p=0
 def read(self,n):r=self.b[self.p:self.p+n];self.p+=n;return r
 def num(self,f):return struct.unpack('>'+f,self.read(struct.calcsize('>'+f)))[0]
 def string(self):return self.read(self.num('H')).decode()
 def tag(self,t):
  if t in range(1,7):return self.num({1:'b',2:'h',3:'i',4:'q',5:'f',6:'d'}[t])
  if t==7:return self.read(self.num('i'))
  if t==8:return self.string()
  if t==9:
   ty=self.num('b');n=self.num('i');return [self.tag(ty) for _ in range(n)]
  if t==10:
   out={}
   while (ty:=self.num('b')):key=self.string();out[key]=self.tag(ty)
   return out
  if t in [11,12]:return [self.num('i' if t==11 else 'Q') for _ in range(self.num('i'))]
  raise ValueError(t)
 def root(self):ty=self.num('b');self.string();return self.tag(ty)
def chunks(world):
 for path in sorted((Path(world)/'region').glob('*.mca')):
  data=path.read_bytes()
  for i in range(1024):
   off=int.from_bytes(data[i*4:i*4+3],'big')*4096
   if not off:continue
   n=int.from_bytes(data[off:off+4],'big');ty=data[off+4];b=data[off+5:off+4+n]
   b=zlib.decompress(b) if ty==2 else gzip.decompress(b) if ty==1 else b
   yield NBT(b).root()
def section_states(sec):
 bs=sec.get('block_states',{});pal=bs.get('palette',['minecraft:air']);pal=[p if isinstance(p,str) else p.get('id',p.get('Name',p.get(''))) for p in pal]
 bits=max(4,(len(pal)-1).bit_length());per=64//bits;mask=(1<<bits)-1;data=bs.get('data')
 for i in range(4096):yield pal[(data[i//per]>>((i%per)*bits))&mask] if data else pal[0]
def ore_map(world,bounds=None):
 ores={};full=[]
 for c in chunks(world):
  cx,cz=c['xPos'],c['zPos']
  if bounds and not(bounds[0]<=cx<=bounds[1] and bounds[2]<=cz<=bounds[3]):continue
  full.append((cx,cz,c.get('Status',c.get('status'))))
  for sec in c.get('sections',[]):
   for i,name in enumerate(section_states(sec)):
    if name.startswith('ntm:ore_'):ores[(cx*16+i%16,sec['Y']*16+i//256,cz*16+(i//16)%16)]=name
 return ores,full
def blocks_at(world,positions):
 by=collections.defaultdict(list);out={}
 for p in positions:by[(p[0]//16,p[2]//16,p[1]//16)].append(p)
 for c in chunks(world):
  for sec in c.get('sections',[]):
   ps=by.get((c['xPos'],c['zPos'],sec['Y']))
   if ps:
    states=list(section_states(sec))
    for x,y,z in ps:out[(x,y,z)]=states[(y%16)*256+(z%16)*16+x%16]
 return out
if __name__=='__main__':
 ores,full=ore_map(sys.argv[1],(-4,4,-4,4));entries=sorted([list(p)+[n] for p,n in ores.items()]);print(json.dumps({'chunks':len(full),'statuses':dict(collections.Counter(s for x,z,s in full)),'counts':dict(collections.Counter(ores.values())),'sha256':hashlib.sha256(json.dumps(entries).encode()).hexdigest(),'ores':entries},indent=2))

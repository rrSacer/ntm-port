"""M1 worldgen bridge: structure starts invoke ordinary ore placed features (S6)."""

OVERWORLD={'biomes':'#minecraft:is_overworld','targets':['stone','deepslate']}
NETHER={'biomes':'#minecraft:is_nether','targets':['netherrack']}
# Gneiss veins replace stone_gneiss only; the stratum that creates it is not ported (see docs/ore-distribution.md).
GNEISS={'biomes':'#minecraft:is_overworld','targets':['ntm:stone_gneiss']}

# HbmWorldGen.generateNether and the gneiss calls in HbmWorldGen.generateSurface, with WorldConfig counts.
# nextInt(127) makes the Nether origin range inclusive 0..126; cobalt uses minHeight 100 + nextInt(26).
EXTRA=[
 dict(material='nether_uranium',block='ntm:ore_nether_uranium',attempts_per_chunk=8,size=6,min_y=0,max_y=126,salt=2167187,**NETHER),
 dict(material='nether_tungsten',block='ntm:ore_nether_tungsten',attempts_per_chunk=10,size=10,min_y=0,max_y=126,salt=2167204,**NETHER),
 dict(material='nether_sulfur',block='ntm:ore_nether_sulfur',attempts_per_chunk=26,size=12,min_y=0,max_y=126,salt=2167221,**NETHER),
 dict(material='nether_fire',block='ntm:ore_nether_fire',attempts_per_chunk=24,size=6,min_y=0,max_y=126,salt=2167238,**NETHER),
 dict(material='nether_cobalt',block='ntm:ore_nether_cobalt',attempts_per_chunk=2,size=6,min_y=100,max_y=125,salt=2167255,**NETHER),
 dict(material='gneiss_uranium',block='ntm:ore_gneiss_uranium',attempts_per_chunk=21,size=6,min_y=30,max_y=39,salt=2167272,**GNEISS),
 dict(material='gneiss_copper',block='ntm:ore_gneiss_copper',attempts_per_chunk=36,size=6,min_y=30,max_y=39,salt=2167289,**GNEISS),
 dict(material='gneiss_asbestos',block='ntm:ore_gneiss_asbestos',attempts_per_chunk=6,size=6,min_y=30,max_y=39,salt=2167306,**GNEISS),
 dict(material='gneiss_lithium',block='ntm:ore_gneiss_lithium',attempts_per_chunk=6,size=6,min_y=30,max_y=39,salt=2167323,**GNEISS),
]

def generate(emit, rows, distributions):
 ids={row['id'] for row in rows}
 for spec in list(distributions)+EXTRA:
  name=spec['material'];ore=spec['block'];salt=spec['salt'];biomes=spec.get('biomes',OVERWORLD['biomes'])
  if ore not in ids:raise ValueError(f'Worldgen block absent from catalog: {ore}')
  def put(kind,value):emit(f'data/ntm/worldgen/{kind}/m1_{name}.json',value)
  feature='ntm:m1_'+name
  put('structure_set',{'structures':[{'structure':feature,'weight':1}],'placement':{'type':'minecraft:random_spread','spacing':1,'separation':0,'salt':salt,'spread_type':'linear'}})
  put('structure',{'type':'minecraft:jigsaw','biomes':biomes,'step':'underground_ores','terrain_adaptation':'none','start_pool':feature,'size':1,'start_height':{'absolute':32},'use_expansion_hack':False,'max_distance_from_center':16})
  put('template_pool',{'fallback':'minecraft:empty','elements':[{'weight':1,'element':{'element_type':'minecraft:feature_pool_element','projection':'rigid','feature':feature}}]})
  put('placed_feature',{'feature':feature,'placement':[{'type':'minecraft:count','count':spec['attempts_per_chunk']},{'type':'minecraft:in_square'},{'type':'minecraft:offset','x':8,'y':0,'z':8},{'type':'minecraft:height_range','height':{'type':'minecraft:uniform','min_inclusive':{'absolute':spec['min_y']},'max_inclusive':{'absolute':spec['max_y']}}}]})
  put('configured_feature',{'type':'minecraft:ore','size':spec['size'],'discard_chance_on_air_exposure':0.0,'targets':[{'target':{'predicate_type':'minecraft:block_match','block':target if ':' in target else 'minecraft:'+target},'state':{'id':ore}} for target in spec.get('targets',OVERWORLD['targets'])]})

 if "ntm:ore_bedrock" in ids:
  import bedrock_worldgen
  bedrock_worldgen.generate(emit)

 if "ntm:ore_oil" in ids:
  import oil_worldgen
  oil_worldgen.generate(emit,rows)

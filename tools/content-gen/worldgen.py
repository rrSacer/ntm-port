"""M1 worldgen bridge: structure starts invoke ordinary ore placed features (S6)."""

def generate(emit, rows, distributions):
 ids={row['id'] for row in rows}
 for spec in distributions:
  name=spec['material'];ore=spec['block'];salt=spec['salt']
  if ore not in ids:raise ValueError(f'Worldgen block absent from catalog: {ore}')
  def put(kind,value):emit(f'data/ntm/worldgen/{kind}/m1_{name}.json',value)
  feature='ntm:m1_'+name
  put('structure_set',{'structures':[{'structure':feature,'weight':1}],'placement':{'type':'minecraft:random_spread','spacing':1,'separation':0,'salt':salt,'spread_type':'linear'}})
  put('structure',{'type':'minecraft:jigsaw','biomes':'#minecraft:is_overworld','step':'underground_ores','terrain_adaptation':'none','start_pool':feature,'size':1,'start_height':{'absolute':32},'use_expansion_hack':False,'max_distance_from_center':16})
  put('template_pool',{'fallback':'minecraft:empty','elements':[{'weight':1,'element':{'element_type':'minecraft:feature_pool_element','projection':'rigid','feature':feature}}]})
  put('placed_feature',{'feature':feature,'placement':[{'type':'minecraft:count','count':spec['attempts_per_chunk']},{'type':'minecraft:in_square'},{'type':'minecraft:offset','x':8,'y':0,'z':8},{'type':'minecraft:height_range','height':{'type':'minecraft:uniform','min_inclusive':{'absolute':spec['min_y']},'max_inclusive':{'absolute':spec['max_y']}}}]})
  put('configured_feature',{'type':'minecraft:ore','size':spec['size'],'discard_chance_on_air_exposure':0.0,'targets':[{'target':{'predicate_type':'minecraft:block_match','block':'minecraft:'+target},'state':{'id':ore}} for target in ['stone','deepslate']]})

 if "ntm:ore_bedrock" in ids:
  import bedrock_worldgen
  bedrock_worldgen.generate(emit)

 if "ntm:ore_oil" in ids:
  import oil_worldgen
  oil_worldgen.generate(emit,rows)

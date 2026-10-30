"""Petroleum datapack feature builders and the measured S6 placement bridge."""

HOT_DRY=['minecraft:desert','minecraft:badlands','minecraft:wooded_badlands','minecraft:eroded_badlands']


def geode(filling=4):
    stone={'type':'minecraft:simple','state':{'id':'minecraft:stone'}}
    return {'type':'minecraft:geode','blocks':{
        'filling_provider':{'type':'minecraft:simple','state':{'id':'ntm:ore_oil'}},
        'inner_layer_provider':stone,'alternate_inner_layer_provider':stone,
        'middle_layer_provider':stone,'outer_layer_provider':stone,
        'inner_placements':[{'id':'minecraft:stone'}],
        'cannot_replace':'#ntm:oil_protected','invalid_blocks':[]},
        'layers':{'filling':filling,'inner_layer':filling+.4,'middle_layer':filling+1.4,'outer_layer':filling+2.4},
        'crack':{'generate_crack_chance':0},'use_potential_placements_chance':0,
        'outer_wall_distance':{'type':'minecraft:uniform','min_inclusive':2,'max_inclusive':4},
        'distribution_points':{'type':'minecraft:uniform','min_inclusive':3,'max_inclusive':4},
        'point_offset':{'type':'minecraft:uniform','min_inclusive':1,'max_inclusive':2},
        'min_gen_offset':-16,'max_gen_offset':16,'noise_multiplier':.025,'invalid_blocks_threshold':4}


def ore():
    return {'type':'minecraft:ore','size':64,'discard_chance_on_air_exposure':0,
            'targets':[{'target':{'predicate_type':'minecraft:block_match','block':'minecraft:stone'},'state':{'id':'ntm:ore_oil'}}]}


def ore_patch(count):
    # 26.3 has no legacy random_patch feature. A selector's fallback can run a
    # placed feature repeatedly, with the same count and horizontal jitter.
    spread={'type':'minecraft:uniform','min_inclusive':-2,'max_inclusive':2}
    return {'type':'minecraft:random_selector','features':[],
            'default':{'feature':ore(),'placement':[{'type':'minecraft:count','count':count},
                {'type':'minecraft:offset','x':spread,'y':0,'z':spread}]}}


def disk(block,targets,radius,half_height):
    return {'type':'minecraft:disk','radius':radius,'half_height':half_height,
            'target':{'type':'minecraft:matching_blocks','blocks':targets},
            'state_provider':{'type':'minecraft:simple','state':{'id':block}}}


def bridge(emit,name,config,chance,low,high,salt,biomes='#minecraft:is_overworld',offset=(8,0,8)):
    ident='ntm:'+name
    def put(kind,value):emit(f'data/ntm/worldgen/{kind}/{name}.json',value)
    put('structure_set',{'structures':[{'structure':ident,'weight':1}],
        'placement':{'type':'minecraft:random_spread','spacing':1,'separation':0,'salt':salt,'spread_type':'linear'}})
    put('structure',{'type':'minecraft:jigsaw','biomes':biomes,'step':'underground_ores','terrain_adaptation':'none',
        'start_pool':ident,'size':1,'start_height':{'absolute':32},'use_expansion_hack':False,'max_distance_from_center':16})
    put('template_pool',{'fallback':'minecraft:empty','elements':[{'weight':1,'element':{
        'element_type':'minecraft:feature_pool_element','projection':'rigid','feature':ident}}]})
    height={'type':'minecraft:constant','value':{'absolute':low}} if low==high else {
        'type':'minecraft:uniform','min_inclusive':{'absolute':low},'max_inclusive':{'absolute':high}}
    put('placed_feature',{'feature':ident,'placement':[{'type':'minecraft:rarity_filter','chance':chance},
        {'type':'minecraft:in_square'},{'type':'minecraft:offset','x':offset[0],'y':offset[1],'z':offset[2]},
        {'type':'minecraft:height_range','height':height}]})
    put('configured_feature',config)


def protect_geode(emit,rows,registry_path=None):
    import json
    import os
    from pathlib import Path
    resources=Path(os.environ.get('HORIZON_RESOURCES','/home/sacer/Games/horizon/.local/horizon-client/26.3'))
    registry=json.loads((registry_path or resources/'registries.json').read_text())
    protected=set(registry['minecraft:block']['entries'])
    protected.update(r['id'] for r in rows if r['type']=='block')
    protected=sorted(protected-{'minecraft:stone'})
    # A complement tag keeps the geode's stone crust/filling from replacing
    # caves, water, other ores or NTM blocks. Split the generated registry list
    # into readable pieces; the standard tag resolver expands references.
    tags=[]
    for index,start in enumerate(range(0,len(protected),400)):
        tag='oil_protected_'+str(index);tags.append('#ntm:'+tag)
        emit('data/ntm/tags/block/'+tag+'.json',{'replace':False,'values':protected[start:start+400]})
    emit('data/ntm/tags/block/oil_protected.json',{'replace':False,'values':tags})


def generate(emit,rows):
    emit('data/ntm/tags/worldgen/biome/oil_hot_dry.json',{'replace':False,'values':HOT_DRY})
    # Baseline 1/100 plus an independent 1/49 in dry-hot biomes gives
    # 1/100 + 1/49 - 1/4900 = 0.030204 versus source floor(100/3)=33.
    for name,chance,biome,salt in [('oil_bubble',100,'#minecraft:is_overworld',884021),
                                  ('oil_bubble_hot_dry',49,'#ntm:oil_hot_dry',884023)]:
        bridge(emit,name,ore_patch(3),chance,15,39,salt,biome)
    bridge(emit,'oil_bedrock',disk('ntm:ore_bedrock_oil',
           ['minecraft:bedrock','minecraft:stone','minecraft:deepslate'],4,2),200,-62,-62,884029)
    # Oil sand is limited to sand substrates, as the Java source; the vanilla
    # disk bound (radius<=8) is a smaller, flattened approximation.
    sand=disk('ntm:ore_oil_sand',['minecraft:sand','minecraft:red_sand'],8,4)
    bridge(emit,'oil_sand',sand,200,60,67,884033,'#ntm:oil_hot_dry')
    # Source effective frequency is floor(200/3)=66. With independent starts,
    # extra rarity 98 gives p=1/200+1/98-1/19600 ~= 1/66 (0.01% relative).
    bridge(emit,'oil_sand_hot_dry',sand,98,60,67,884039,'#ntm:oil_hot_dry')

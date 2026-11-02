"""Current reactor identities and source fuel metadata (legacy pile excluded)."""
import re
from bulk_recipes import clean
from generate import lang, registration

BASE='src/main/java/com/hbm/'


def zirnox_fuels(source):
    path=BASE+'items/machine/ItemZirnoxRod.java'
    text=clean((source/path).read_text()).split('enum EnumZirnoxType {',1)[1].split(';',1)[0]
    values=re.findall(r'(\w+)\(([\d_]+),\s*([\d_]+)(?:,\s*(true|false))?\)',text)
    if len(values)!=11:raise ValueError('ZIRNOX fuel enum drift')
    return [{'member':member,'metadata':meta,'max_life':int(life.replace('_','')),
             'heat_per_decay':int(heat.replace('_','')),'breeding':breeding=='true'}
            for meta,(member,life,heat,breeding) in enumerate(values)]


def pile_fuels(source):
    path=BASE+'items/machine/ItemPileRodMK2.java'
    text=clean((source/path).read_text()).split('enum EnumPileRod {',1)[1].split(';',1)[0]
    values=re.findall(r'(\w+)\(([^)]+)\)',text)
    if len(values)!=9:raise ValueError('Pile MK2 fuel enum drift')
    result=[]
    for meta,(member,args) in enumerate(values):
        numbers=[float(x.strip().replace('_','').removesuffix('D')) for x in args.split(',')]
        if len(numbers)==1:source_flux=numbers[0];reaction=life=heat=0;turns_into=None
        elif len(numbers)==4:reaction,life,heat,target=numbers;source_flux=0;turns_into=int(target)
        else:raise ValueError('Pile fuel constructor drift: '+member)
        result.append({'member':member,'metadata':meta,'neutron_source':source_flux,'reaction_multiplier':reaction,
                       'life':life,'heat_per_neutron':heat,'turns_into_metadata':turns_into})
    return result


def fuel_rows(source):
    translations={loc:lang(source/f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    items=(source/(BASE+'items/ModItems.java')).read_text();rows=[]
    for var,enum,fuels,separator in [('pile_rod','EnumPileRod',pile_fuels(source),'.'),('rod_zirnox','EnumZirnoxType',zirnox_fuels(source),'_')]:
        name,texture=registration(items,var,'item')
        for f in fuels:
            suffix=f['member'].lower();key='item.'+name+separator+suffix+'.name'
            rows.append({'id':name+'__'+suffix,'source_variable':var,'type':'item','category':'component',
                         'metadata':f['metadata'],'subtype_kind':'enum','source_enum':enum+'.'+f['member'],
                         'max_stack_size':1 if var=='rod_zirnox' else 64,
                         'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}{separator}{suffix}.png',
                         'names':{loc:tr.get(key, {'thorium':'Haste de Tório Natural do Chicago Pile', 'thorium_fuel':'Haste de Combustível de Tório do Chicago Pile'}.get(suffix,translations['en_us'].get(key))) for loc,tr in translations.items()},
                         'behavior_gap':'Source fuel/transmutation table in reactors_simple.json; per-stack life/depletion NBT, dynamic radiation and reactor interaction require native logic. No vanilla max_damage substitution.'})
    return rows

STRUCTURAL_ROLES=('dummy','core','channel','fuel_in','fuel_out','air_in','air_out','control','edge')
DEVICE_ROLES=('loader','vent','control')
MACHINES=('reactor_zirnox','zirnox_destroyed','pile_device','pile_block')


def build(source):
    rows=fuel_rows(source)
    blocks=(source/(BASE+'blocks/ModBlocks.java')).read_text()
    translations={loc:lang(source/f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    root='src/main/resources/assets/hbm/textures/'
    name,texture=registration(blocks,'pile_brick','block')
    rows.append({'id':name,'source_variable':'pile_brick','type':'block','category':'decorative','template':'minecraft:stone',
                 'source_texture':root+'blocks/'+texture+'.png',
                 'surface_textures':{'side':root+'blocks/pile_brick_side.png','top':root+'blocks/pile_brick_top.png','bottom':root+'blocks/pile_brick_top.png'},
                 'names':{loc:tr['tile.pile_brick.name'] for loc,tr in translations.items()},
                 'behavior_gap':'Source has distinct north/south brick faces vs east/west; cube_bottom_top uses side on all four. Hand drill formation/flammability require native logic.'})
    # ExplosionNukeGeneric.waste(35) transforms grass and mycelium into
    # these source blocks; neither was in the original minimal fallout set.
    for var in ['waste_earth','waste_mycelium']:
        name,_=registration(blocks,var,'block')
        side='waste_grass_side' if var=='waste_earth' else 'waste_mycelium_side'
        top='waste_grass_top' if var=='waste_earth' else 'waste_mycelium_top'
        rows.append({'id':name,'source_variable':var,'type':'block','category':'fallout','template':'minecraft:dirt',
                     'source_texture':root+'blocks/'+side+'.png',
                     'surface_textures':{'side':root+'blocks/'+side+'.png','top':root+'blocks/'+top+'.png','bottom':root+'blocks/waste_earth_bottom.png'},
                     'block_properties':{'hardness':0.6,'explosion_resistance':0.6,'requires_tool':False,'sounds':'minecraft:grass_block','light':15 if var=='waste_mycelium' else 0},
                     'drop':'minecraft:dirt','names':{loc:tr['tile.'+name+'.name'] for loc,tr in translations.items()},
                     'behavior_gap':'Source mycelium spread is disabled by default; native ZIRNOX applies its radiation-on-walk rule. Vanilla dirt plant support differs from Cave-only source.'})
    for var in ['reactor_zirnox','zirnox_destroyed']:
        name,texture=registration(blocks,var,'block')
        rows.append({'id':name,'source_variable':var,'type':'block','category':'machine','template':'minecraft:beehive',
                     'source_texture':root+'blocks/'+texture+'.png',
                     'textures':{face:'blocks/'+texture for face in ['side','top','front']},
                     'block_properties':{'hardness':100.0 if var=='zirnox_destroyed' else 5.0,'explosion_resistance':480.0 if var=='zirnox_destroyed' else 60.0},
                     'names':{loc:tr.get('tile.'+name+'.name','ZIRNOX Destruído' if loc=='pt_br' else 'Destroyed ZIRNOX') for loc,tr in translations.items()},
                     **({'creative_hidden':True,'loot_pools':[{'rolls':1,'entries':[{'type':'minecraft:item','name':item,'modifier':[{'type':'minecraft:set_count','count':n}]}]} for item,n in [('ntm:concrete_smooth',6),('ntm:pipe__steel',4),('ntm:steel_grate',2),('ntm:debris_metal',6),('ntm:debris_graphite',2),('ntm:fallout',4)]]} if var=='zirnox_destroyed' else {}),
                     'behavior_gap':'Source multiblock/model, core/proxy conversion, interaction, fluids, failure and destroyed-reactor random salvage deferred to native logic; source casing fallback only.'})
    for meta,role in enumerate(DEVICE_ROLES):
        model='models/pile/pile_'+role
        rows.append({'id':'pile_device__'+role,'source_variable':'pile_device','type':'block','category':'machine',
                     'template':'minecraft:beehive','metadata':meta,'subtype_kind':'enum','source_block_metadata_base':meta*4,
                     'source_texture':root+model+'.png','textures':{face:model for face in ['side','top','front']},
                     'names':{loc:tr['tile.pile_device.'+role+'.name'] for loc,tr in translations.items()},
                     'behavior_gap':'Original TESR atlas projected on fallback cube; model and loader/vent/control interactions require native implementation. Source item meta differs from block meta (role*4+orientation).'} )
    pt=dict(zip(STRUCTURAL_ROLES,['Estrutura','Núcleo','Canal','Entrada de Combustível','Saída de Combustível','Entrada de Ar','Saída de Ar','Controle','Borda']))
    for meta,role in enumerate(STRUCTURAL_ROLES):
        rows.append({'id':'pile_block__'+role,'source_variable':'pile_block','type':'block','category':'machine',
                     'template':'minecraft:beehive','metadata':meta,'subtype_kind':'enum','creative_hidden':True,'loot_none':True,
                     'source_texture':root+'blocks/pile_block.png',
                     'textures':{'side':'blocks/pile_block','top':'blocks/pile_block_top','front':'blocks/pile_block'},
                     'names':{'en_us':'Chicago Pile ('+role.replace('_',' ').title()+')','pt_br':'Chicago Pile ('+pt[role]+')'},
                     'behavior_gap':'Internal source role split into identity; no recipe, hidden from creative. Native structure core links, channels, connected textures and reverting to pile_brick on dismantling are deferred. No item loot.'})
    return rows


def pile_tools(source):
    """The two original HAND_DRILL tools required to form/drill a pile."""
    items=(source/(BASE+'items/ModItems.java')).read_text()
    translations={loc:lang(source/f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    rows=[]
    for var in ('hand_drill','hand_drill_desh'):
        match=re.search(r'\b'+var+r'\s*=\s*new ItemTooling\(ToolType.HAND_DRILL,\s*(\d+)\)',items)
        if not match:raise ValueError('HAND_DRILL declaration drift: '+var)
        durability=int(match[1]);name,_=registration(items,var,'item')
        rows.append({'id':name,'source_variable':var,'source_class':'ItemTooling','type':'item','category':'tool',
                     'source_texture':f'src/main/resources/assets/hbm/textures/items/{name}.png',
                     'max_stack_size':1,'durability':{'max_damage':durability,'wear':durability>0},
                     **({'max_damage':durability} if durability else {}),
                     'names':{loc:tr['item.'+name+'.name'] for loc,tr in translations.items()},
                     'behavior_gap':'Native HAND_DRILL interaction on the MK2 pile only. Other IToolable interactions, crafting remainder and no-repair source behavior are not implemented.'})
    return rows

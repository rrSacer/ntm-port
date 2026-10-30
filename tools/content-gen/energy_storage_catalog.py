"""Original battery-pack metadata and active nonreactor network identities."""
import re
from bulk_recipes import clean
from generate import lang, registration

BASE='src/main/java/com/hbm/'
CABLES=('red_cable_classic','red_cable_paintable','red_cable_gauge','red_cable_box')
CONNECTORS=('red_connector','red_connector_super','red_pylon','red_pylon_steel',
            'red_pylon_medium_wood','red_pylon_medium_wood_transformer',
            'red_pylon_medium_steel','red_pylon_medium_steel_transformer','red_pylon_large')
CONTROLS=('cable_switch','cable_detector')


def packs(source):
    path=BASE+'items/machine/ItemBatteryPack.java';text=clean((source/path).read_text())
    body=text.split('enum EnumBatteryPack {',1)[1].split(';',1)[0]
    values=re.findall(r'(\w+)\s*\("([^"]+)",\s*([\d_]+)L,\s*(true|false|[\d *]+)\)',body)
    if len(values)!=12:raise ValueError('Battery pack enum drift')
    from bedrock_processing_common import integer
    result=[]
    for meta,(member,texture,rate,kind) in enumerate(values):
        discharge=int(rate.replace('_',''));capacitor=kind=='true'
        duration=(600 if capacitor else 18000) if kind in ['true','false'] else integer(kind)
        result.append({'metadata':meta,'member':member,'id':'ntm:battery_pack__'+member.lower(),
                       'capacitor':capacitor,'capacity_he':discharge*duration,
                       'charge_he_per_tick':discharge if capacitor else discharge*10,
                       'discharge_he_per_tick':discharge,'duration_ticks':duration,
                       'model_texture':'src/main/resources/assets/hbm/textures/models/machines/'+texture+'.png',
                       'source':path})
    return result


def pack_rows(source):
    items=(source/(BASE+'items/ModItems.java')).read_text()
    name,texture=registration(items,'battery_pack','item')
    translations={loc:lang(source/f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    return [{'id':p['id'].removeprefix('ntm:'),'source_variable':'battery_pack','type':'item','category':'component',
             'metadata':p['metadata'],'subtype_kind':'enum','source_enum':'EnumBatteryPack.'+p['member'],
             'max_stack_size':1,'source_texture':f'src/main/resources/assets/hbm/textures/items/{texture}.png',
             'names':{loc:tr['item.'+name+'.'+p['member'].lower()+'.name'] for loc,tr in translations.items()},
             'battery':{k:p[k] for k in ['capacity_he','charge_he_per_tick','discharge_he_per_tick']},
             'source_model_texture':p['model_texture'],
             'behavior_gap':'Registered source item icon fallback; source per-pack 3D renderer and charge NBT, charging/discharging, charged creative variants and energy tooltip require native item support.'} for p in packs(source)]


def build(source):
    blocks=(source/(BASE+'blocks/ModBlocks.java')).read_text()
    translations={loc:lang(source/f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    rows=pack_rows(source)
    icons={'red_cable_paintable':'red_cable_base','red_cable_gauge':'deco_red_copper',
           'cable_switch':'cable_switch_off','cable_detector':'cable_detector_off'}
    for variable in CABLES+CONNECTORS+CONTROLS:
        name,texture=registration(blocks,variable,'block');texture=texture or icons[variable]
        cable=variable in CABLES
        row={'id':name,'source_variable':variable,'type':'block','category':'cable' if cable else 'machine',
             'template':'minecraft:stone' if cable else 'minecraft:beehive',
             'network_role':'adjacent_cable' if cable else 'wire_endpoint' if variable in CONNECTORS else 'network_control',
             'names':{loc:tr['tile.'+name+'.name'] for loc,tr in translations.items()},
             'source_texture':f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
             'behavior_gap':'Original icon/casing cube fallback. Connectivity, painting/disguise, gauge, cable boxes, long-distance wire links, redstone switching/detection and custom geometry require native logic/visuals.'}
        if not cable:
            row['textures']={face:'blocks/'+texture for face in ['side','top','front']}
            if variable in CONTROLS:row['textures']['front_on']='blocks/'+variable+'_on'
            row['opaque_machine_textures']=True
        rows.append(row)
    return rows

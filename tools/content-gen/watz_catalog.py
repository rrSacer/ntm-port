"""Watz identities missing from the bulk import: reactor, pump, core structure, casing, pellets."""
import re
from bulk_recipes import clean
from generate import lang, registration

BASE='src/main/java/com/hbm/'
ASSETS='src/main/resources/assets/hbm/'
MACHINES=('watz','watz_pump','struct_watz_core')


def pellets(source):
    text=clean((source/(BASE+'items/machine/ItemWatzPellet.java')).read_text()).split('enum EnumWatzType {',1)[1].split(';\n\t\t\n',1)[0]
    pattern=re.compile(r'(\w+)\(\s*(0x[0-9A-Fa-f]+),\s*(0x[0-9A-Fa-f]+),\s*([\d_]+),\s*([\d_.]+)D?,\s*([\d_.]+)D,\s*(null|new [^,]+?\)(?:\.with\w+\([^)]*\))*),\s*(null|new [^,]+?\)(?:\.with\w+\([^)]*\))*),\s*(null|new [^;]+?\)(?:\.with\w+\([^)]*\))*)\)\s*,?',re.S)
    result=[]
    for meta,m in enumerate(pattern.finditer(text)):
        result.append({'member':m[1],'metadata':meta,'color_light':int(m[2],16),'color_dark':int(m[3],16),'passive':float(m[4].replace('_','')),
                       'heat_emission':float(m[5].replace('_','')),'mud_content_raw':float(m[6]),
                       'burn':None if m[7]=='null' else m[7].replace('new ',''),'heat_divisor':None if m[8]=='null' else m[8].replace('new ',''),
                       'absorb':None if m[9]=='null' else m[9].replace('new ','')})
    if [p['member'] for p in result]!=['SCHRABIDIUM','HES','MES','LES','HEN','MEU','MEP','LEAD','BORON','DU','NQD','NQR']:
        raise ValueError('Watz pellet enum drift')
    return result


def desaturate(color):
    # ItemWatzPellet.desaturate, with Java int truncation of each channel.
    r,g,b=(color>>16)&255,(color>>8)&255,color&255
    avg=(r+g+b)//3
    def step(c):
        c=int(c-(c-avg)*0.9)
        return int(c*0.75)
    return (step(r)<<16)|(step(g)<<8)|step(b)


def build(source):
    blocks=(source/(BASE+'blocks/ModBlocks.java')).read_text()
    translations={loc:lang(source/f'{ASSETS}lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    def names(key):return {loc:tr.get(key) or translations['en_us'][key] for loc,tr in translations.items()}
    root=ASSETS+'textures/'
    rows=[]
    for var in MACHINES:
        name,texture=registration(blocks,var,'block')
        rows.append({'id':name,'source_variable':var,'type':'block','category':'machine','template':'minecraft:beehive',
                     'source_texture':root+f'blocks/{texture}.png','textures':{face:'blocks/'+texture for face in ('side','top','front')},
                     'opaque_machine_textures':True,'names':names('tile.'+name+'.name'),
                     'behavior_gap':'Source '+{'watz':'multiblock reactor body with four side volumes, tanks, reaction and GUI','watz_pump':'vertical multiblock pump','struct_watz_core':'core component that the Watz drops when broken and that is crafted by hand'}[var]
                                    +'; native/visual work, steel or source cube fallback only.'})
    name,texture=registration(blocks,'watz_end','block')
    for meta,(suffix,tex,key) in enumerate([('','watz_casing','tile.watz_end.name'),('__bolted','watz_casing_bolted','tile.watz_end_bolted.name')]):
        rows.append({'id':name+suffix,'source_variable':'watz_end','type':'block','category':'decorative','template':'minecraft:stone',
                     'metadata':meta,'subtype_kind':'enum','source_texture':root+f'blocks/{tex}.png','names':names(key),
                     'behavior_gap':'Source BlockToolConversion: the screwdriver/bolt-driver conversions between the plain and riveted casing are native logic; both are plain cubes here.'})
    for form,depleted in (('watz_pellet',False),('watz_pellet_depleted',True)):
        for p in pellets(source):
            suffix=p['member'].lower()
            light,dark=(desaturate(p['color_light']),desaturate(p['color_dark'])) if depleted else (p['color_light'],p['color_dark'])
            rows.append({'id':f'{form}__{suffix}','source_variable':form,'type':'item','category':'component','metadata':p['metadata'],
                         'subtype_kind':'enum','source_enum':'EnumWatzType.'+p['member'],'max_stack_size':16,
                         'source_texture':ASSETS+'textures/items/watz_pellet.png','tint':{'light':f'{light:06X}','dark':f'{dark:06X}'},
                         'names':names(f'item.{form}.{suffix}.name'),
                         'behavior_gap':'Fuel table in machine-recipes/reactors_watz.json; per-stack depletion is NBT and the durability bar needs native logic. Icon recoloured from the source RGB remap (depleted = ItemWatzPellet.desaturate).'})
    return rows

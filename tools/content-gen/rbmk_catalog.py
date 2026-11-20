"""RBMK identities (columns, console, panels, debris, fuel rods and pellets) from the NTM source."""
import re
from generate import lang, registration

BASE='src/main/java/com/hbm/'
ASSETS='src/main/resources/assets/hbm/'
DEBRIS=('pribris','pribris_burning','pribris_radiating','pribris_digamma')


def block_variables(source):
    text=(source/(BASE+'blocks/ModBlocks.java')).read_text()
    names=re.findall(r'public static Block ((?:rbmk_|pribris)\w*);',text)
    if len(names)!=37:raise ValueError('RBMK block list drift: %d'%len(names))
    return names


def item_variables(source):
    text=(source/(BASE+'items/ModItems.java')).read_text()
    rods=re.findall(r'(?m)^\s*public static ItemRBMKRod (rbmk_fuel_\w+);',text)
    pellets=re.findall(r'(?m)^\s*public static ItemRBMKPellet (rbmk_pellet_\w+);',text)
    if (len(rods),len(pellets))!=(33,32):raise ValueError('RBMK fuel list drift')
    return rods,pellets


MACHINES=tuple(v for v in ('rbmk_rod','rbmk_rod_mod','rbmk_rod_reasim','rbmk_rod_reasim_mod','rbmk_control','rbmk_control_mod',
    'rbmk_control_auto','rbmk_control_reasim','rbmk_control_reasim_auto','rbmk_blank','rbmk_boiler','rbmk_reflector','rbmk_absorber',
    'rbmk_moderator','rbmk_outgasser','rbmk_storage','rbmk_cooler','rbmk_heater','rbmk_console','rbmk_crane_console',
    'rbmk_display_blank','rbmk_display','rbmk_key_pad','rbmk_gauge','rbmk_numitron','rbmk_graph','rbmk_lever','rbmk_indicator',
    'rbmk_terminal','rbmk_autoloader','rbmk_loader','rbmk_steam_inlet','rbmk_steam_outlet'))


# The keys ItemRBMKRod.addInformation resolves (I18nUtil.resolveKey): its traits, the DRX's, the neutron types and the
# cooling hint.
TOOLTIP_KEYS=re.compile(r'^(trait\.rbm[kx]\.[\w.]+|desc\.item\.wasteCooling|trait\.hot)$')


def tooltip_lang(source, languages):
    """The rods' tooltip keys in both languages, as the source has them (pt_br falls back to en_us, as the game's
    language fallback does)."""
    tr={loc:lang(source/f'{ASSETS}lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    for key,value in tr['en_us'].items():
        if TOOLTIP_KEYS.match(key):
            languages['en_us'][key]=value
            languages['pt_br'][key]=tr['pt_br'].get(key,value)


# The eighteen columns (RBMKBase): their strength is the base class's, unless ModBlocks sets another.
COLUMNS=MACHINES[:18]


def column_strength(source, var):
    """(hardness, explosion_resistance) of RBMK column `var`: `RBMKBase()`'s setHardness/setResistance, or the
    declaration's own in ModBlocks; the resistance as the engine reads it, Java's x 3 / 5 (`setResistance` keeps x 3,
    `getExplosionResistance` gives / 5), as the ZIRNOX's 100 -> 60."""
    base=(source/(BASE+'blocks/machine/rbmk/RBMKBase.java')).read_text()
    ctor=re.search(r'protected RBMKBase\(\)\s*\{(.*?)\}',base,re.S)
    if not ctor:raise ValueError('RBMKBase constructor drift')
    def value(text,setter):
        m=re.findall(setter+r'\(\s*([0-9.]+)F?\s*\)',text)
        return float(m[-1]) if m else None
    line=next(l for l in (source/(BASE+'blocks/ModBlocks.java')).read_text().splitlines() if re.match(r'\s*'+var+r'\s*=\s*new ',l))
    hardness=value(line,'setHardness') or value(ctor[1],'setHardness')
    resistance=value(line,'setResistance') or value(ctor[1],'setResistance')
    if hardness is None or resistance is None:raise ValueError('RBMK column strength drift: '+var)
    return hardness, resistance*3/5


def build(source):
    blocks=(source/(BASE+'blocks/ModBlocks.java')).read_text();items=(source/(BASE+'items/ModItems.java')).read_text()
    translations={loc:lang(source/f'{ASSETS}lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    # rbmk_fuel_test has no lang entry; its source fullName is the only label.
    fallback={'item.rbmk_fuel_test.name':'RBMK Fuel Rod (THE VOICES)'}
    def names(key):return {loc:tr.get(key) or translations['en_us'].get(key) or fallback[key] for loc,tr in translations.items()}
    variables=block_variables(source)
    if set(variables)-set(DEBRIS)!=set(MACHINES):raise ValueError('RBMK machine set drift')
    rows=[]
    for var in variables:
        name,texture=registration(blocks,var,'block')
        root=ASSETS+'textures/blocks/'
        def exists(path):return (source/(root+path+'.png')).is_file()
        if var in DEBRIS:
            rows.append({'id':name,'source_variable':var,'type':'block','category':'decorative','template':'minecraft:stone',
                         'source_texture':root+texture+'.png','names':names('tile.'+name+'.name'),
                         'behavior_gap':'Source debris: burning/radiating/digamma ticking, spread and radiation are native logic; plain cube here.'})
            continue
        side=texture+'_side' if exists(texture+'_side') else texture
        top=texture+'_top' if exists(texture+'_top') else side
        row={'id':name,'source_variable':var,'type':'block','category':'machine','template':'minecraft:beehive',
             'source_texture':root+side+'.png','names':names('tile.'+name+'.name'),
             'textures':{'side':'blocks/'+side,'top':'blocks/'+top,'front':'blocks/'+side},
             'behavior_gap':'Source BlockDummyable/TESR model, column height, lids, fluids, flux, heat, panels and interaction are native/visual work; casing cube fallback only.'}
        if not exists(side):raise ValueError('Missing RBMK texture: '+side)
        if var in COLUMNS:
            hardness,resistance=column_strength(source,var)
            row['block_properties']={'hardness':hardness,'explosion_resistance':resistance}
        rows.append(row)
    rods,pellets=item_variables(source)
    for var in ['rbmk_lid','rbmk_lid_glass','rbmk_tool']+pellets+rods:
        name,texture=registration(items,var,'item')
        kind='fuel' if var in rods else 'pellet' if var in pellets else 'tool'
        row={'id':name,'source_variable':var,'type':'item','category':'component','max_stack_size':1 if kind in('fuel','tool') else 64,
             'source_texture':f'{ASSETS}textures/items/{texture}.png','names':names('item.'+name+'.name'),
             'behavior_gap':{'fuel':'Source ItemRBMKRod table in reactors_rbmk.json; yield, xenon and heat are per-stack NBT, so dynamic radiation and the durability bar need native logic. No vanilla max_damage substitution.',
                             'pellet':'Pellet is a crafting/disassembly ingredient; enrichment and poison NBT belong to the fuel rod state.',
                             'tool':'Console linking positions are native item data.'}.get(kind,'Panel placement on a column belongs to native logic.')}
        rows.append(row)
    return rows

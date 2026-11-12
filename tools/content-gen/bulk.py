#!/usr/bin/env python3
"""Audit source declarations and produce a conservative, reproducible bulk catalog.

Only representable content is emitted. Every other declaration has an explicit
reason in bulk-coverage.json; custom code is never silently replaced by an icon.
"""
import argparse
import json
import re
from pathlib import Path
from generate import lang, write_json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

def clean(text):
    return re.sub(r'//[^\n]*|/\*.*?\*/', lambda m: '\n'*m[0].count('\n'), text, flags=re.S)

def declarations(text):
    text = clean(text)
    return [(m[1], m[2], m[0], text[:m.start()].count('\n')+1) for m in re.finditer(r'^\s*(\w+)\s*=\s*(?:\([A-Za-z0-9_$.]+\)\s*)?new\s+(\w+)\b[^;]*;', text, re.M)]

def split(text):
    """Split comma arguments outside Java strings and balanced containers."""
    out=[]; start=0; depth=0; quote=None; escape=False
    for i,c in enumerate(text):
        if quote:
            if escape: escape=False
            elif c=='\\': escape=True
            elif c==quote: quote=None
        elif c in '\"\'': quote=c
        elif c in '([{': depth+=1
        elif c in ')]}': depth-=1
        elif c==',' and depth==0: out.append(text[start:i].strip()); start=i+1
    if text[start:].strip(): out.append(text[start:].strip())
    return out

def enums(java):
    found={}
    for file in sorted(java.rglob('*.java')):
        text=clean(file.read_text())
        for m in re.finditer(r'\benum\s+(\w+)[^{]*\{',text):
            # Constants end at the first semicolon or closing brace. Their
            # constructors may have parentheses, but no method bodies here.
            end=re.search(r'[;}]',text[m.end():]); body=text[m.end():m.end()+end.start()] if end else ''
            values=[]
            for constant in split(body):
                match=re.match(r'([A-Z][A-Z_0-9]*)\b(?:\((.*?)\))?$',constant,re.S)
                if match: values.append((match[1],match[2]))
                else: break
            if values: found[m[1]]=values
    return found

MATERIAL_ALIAS={'MINGRADE':'red_copper','KNO':'niter','THORIUM':'thorium','DURA':'dura_steel','CMB':'combine_steel','MAGTUNG':'magnetized_tungsten','STAR':'starmetal','RGP':'pu_mix','RGA':'am_mix'}
MATERIAL_FORMS={'ingot_raw':('INGOT','ingot'),'nugget_raw':('NUGGET','nugget'),'plate_cast':('CASTPLATE','plate'),'plate_welded':('WELDEDPLATE','plate'),'wire_fine':('WIRE','wire'),'wire_dense':('DENSEWIRE','wire'),'bedrock_ore_fragment':('FRAGMENT','fragment'),'part_barrel_light':('LIGHTBARREL','component'),'part_barrel_heavy':('HEAVYBARREL','component'),'part_receiver_light':('LIGHTRECEIVER','component'),'part_receiver_heavy':('HEAVYRECEIVER','component'),'part_mechanism':('MECHANISM','component'),'part_stock':('STOCK','component'),'part_grip':('GRIP','component')}
MATERIAL_PREFIXES=('ingot_','nugget_','powder_','plate_','wire_','wiring_','billet_','crystal_','fragment_','scrap','dust','pellet_','gem_')
# Whitelisted classes only: behavior-bearing subclasses remain in the audit.
STATIC_ITEMS={'Item','ItemCustomLore'}
STATIC_BLOCKS={'BlockGeneric','BlockReinforcedBlock','BlockConcrete','BlockConcreteColored','BlockBrick','BlockDeco','BlockReinforcedGlass','BlockGlass','BlockNTMGlass','BlockPillar','BlockRotatablePillar','BlockBeaconable','BlockHazard','BlockHotHazard','BlockNoSpawn','BlockDragonProof','BlockFalling','BlockSand','BlockGravel'}
# Full-cube ore surfaces are useful content; hazards/processing remain explicit gaps.
ORE_CLASSES={'BlockGeneric','BlockOre','BlockOutgas','BlockNetherCoal','BlockSmolder','BlockDragonProof','BlockDepthOre','BlockDepth','BlockCluster'}

# Generic class -> form mapping for blocks the whitelist does not name. A block
# class with no shape/render override anywhere in its ancestry is a full cube;
# behavior hooks are kept as an explicit gap. Everything with another form
# (doors, panes, fluids, gases, containers, fire, detonators...) stays audited.
SHAPE_OVERRIDES=re.compile(r'\b(?:isOpaqueCube|renderAsNormalBlock|getRenderType|setBlockBounds|getCollisionBoundingBoxFromPool|getSelectedBoundingBoxFromPool|getBlocksMovement|getCollisionBoundingBoxFromPool)\b')
BEHAVIOR_HOOKS=('onEntityCollidedWithBlock','updateTick','onBlockActivated','randomDisplayTick','onNeighborBlockChange','onBlockAdded','getItemDropped','quantityDropped','getLightValue','isBeaconBase','onBlockPlacedBy','breakBlock','harvestBlock')
OTHER_FORMS=re.compile(r'Door|Rail|Conveyor|Pane|Fence|Ladder|Slab|Stairs|Fluid|Gas|Fire|Detonat|Flammable|Container|Liquid|Plant|Reeds|Vine|Mush(?!Huge)|Spike|Barbed|Layering|Struct|Glyph|Keyhole|Seal|Wand|Teleanchor|Machine|Bomb|RBMK|Watz|PWR|Pillar')
OTHER_FAMILIES=re.compile(r'^(cm_|fusion_|icf_|hadron_|spikes|wand_|det_|crystal_|rad_absorber|taint$|balefire|fire_digamma|digamma_matter|mud_block|acid_block|toxic_block|schrabidic_block|corium_block|volcanic_|rad_lava|ore_oil|ore_bedrock_oil|ore_gneiss_gas|ore_nether_smoldering|stone_keyhole|brick_forgotten_lock|brick_jungle_(glyph|circle)|watz_|glyphid_base|book_guide|conveyor|crane|press_preheater|machine_)')


def static_cube(cls,parents,classbody,name,texture_exists,tex):
    """Return spec updates for a full-cube block class, or None."""
    if OTHER_FORMS.search(' '.join(parents)) or OTHER_FAMILIES.search(name) or SHAPE_OVERRIDES.search(classbody):return None
    update={}
    if 'BlockFalling' in parents:update['template']='minecraft:gravel'
    hooks=[h for h in BEHAVIOR_HOOKS if re.search(r'\b'+h+r'\b',classbody)]
    if hooks:update['behavior_gap']='Source hooks not implemented by the static block ('+', '.join(hooks)+'); full-cube surface only'
    if tex:
        faces={face:tex+suffix for face,suffix in (('side','_side'),('top','_top'),('bottom','_bottom')) if texture_exists(tex+suffix)}
        if 'top' in faces or 'side' in faces:
            update['surface_textures']={'side':faces.get('side',tex),'top':faces.get('top',faces.get('side',tex)),'bottom':faces.get('bottom',faces.get('top',tex))}
    return update

# Generic item class -> form mapping. A consumable/kit/loose-material item class that
# keeps its behavior in use hooks (right click, eat, tooltip, hit, NBT state) is
# declared as an inert item with those hooks listed as a gap; classes that carry
# equipment, tool, render-pass or machine state stay audited.
ITEM_FORBIDDEN_BASES={'ItemArmor','ItemTool','ItemSword','ItemHoe','ItemPickaxe','ItemAxe','ItemSpade','ItemBucket','ItemRecord','ItemArmorMod','ItemBlock','ItemFuelRod','ItemNuclearWaste','ItemEnumMulti','ItemGenericPart','ItemBattery','ItemKeyPin','ItemCraftingDegradation','ItemFluidIdentifier'}
ITEM_RENDER_HOOKS=re.compile(r'\b(?:registerIcons|getIconFromDamage|getIconFromDamageForRenderPass|getColorFromItemStack|getSubItems|requiresMultipleRenderPasses|getRenderPasses|getItemStackDisplayName)\b')
ITEM_OTHER_FAMILIES=re.compile(r'Missile|Sat|Chip|Pile|Plate|Depleted|Armor|Gun|Weapon|Sword|Tool|Fusion|Chopper|Lock|Door|Mod|Hot$|Pipette|Infinite|Designator|Detonator|Grenade|Breeding|Rod|Dosimeter|Geiger|Detector|Scanner|Linker|RTTY|Remote|Sensor|Wand|Siphon|Rebar|TeleLink|Muffler|Diagnostic|MS$')
ITEM_OTHER_NAMES=re.compile(r'^(mp_|missile_|gun_|ammo_|sat_|designator|spawn_|padlock|door_|detonator|grenade_|pile_|plate_fuel|waste_|rod|fusion_|icf_|pa_|wand_|reactor_sensor|meltdown_tool|jetpack_|mysteryshovel|upgrade_|linker|siphon|rebar_placer|rtty_)')
ITEM_HOOKS=('onUpdate','onItemRightClick','onItemUse','onItemUseFirst','hitEntity','onEaten','getMaxItemUseDuration','getItemUseAction','onUsingTick','onPlayerStoppedUsing','itemInteractionForEntity','hasEffect','getContainerItem','addInformation','getDamage','showDurabilityBar')


def static_item(cls,chain,body,name):
    """Return spec updates for an inert consumable/kit/material item class, or None."""
    if ITEM_FORBIDDEN_BASES & set(chain) or ITEM_RENDER_HOOKS.search(body):return None
    if ITEM_OTHER_FAMILIES.search(cls) or ITEM_OTHER_NAMES.search(name):return None
    hooks=[h for h in ITEM_HOOKS if re.search(r'\b'+h+r'\b',body)]
    state=bool(re.search(r'\b(?:NBTTagCompound|stackTagCompound|setTagCompound)\b',body))
    update={'behavior_gap':'Inert item: source hooks not implemented ('+(', '.join(hooks) or 'none')+')'+('; per-stack NBT state' if state else '')}
    if re.search(r'\bsetMaxDamage\(',body):update['max_stack_size']=1
    return update


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source',type=Path,default=ROOT.parent/'ntm-src')
    p.add_argument('--stage',choices=['materials','components','decorative'],default='decorative')
    a=p.parse_args(); java=a.source/'src/main/java/com/hbm'; assets=a.source/'src/main/resources/assets/hbm'
    translations={loc:lang(assets/f'lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    existing=json.loads((HERE/'m1.json').read_text())['entries']; used={r['id'] for r in existing}
    # Rows already in bulk.json stay byte-identical on regeneration, and names
    # another catalog (m3.json, bombs.json) already declares are never claimed here.
    frozen={r['id']:r for r in json.loads((HERE/'bulk.json').read_text())['entries']} if (HERE/'bulk.json').exists() else {}
    claimed={r['source_variable'] for f in ('m3.json','bombs.json') if (HERE/f).exists() for r in json.loads((HERE/f).read_text())['entries']}
    source={'item':(java/'items/ModItems.java').read_text()+'\n'+(java/'items/ModItemsArmor.java').read_text(),'block':(java/'blocks/ModBlocks.java').read_text()}
    item_main_lines=len((java/'items/ModItems.java').read_text().splitlines())
    enum=enums(java); rows=[]; audit=[]; machines=[]
    mats=[]; od=(java/'inventory/OreDictManager.java').read_text()
    for line in clean((java/'inventory/material/Mats.java').read_text()).splitlines():
        m=re.search(r'(MAT_\w+)\s*=\s*(make\w*)\(\s*([^,]+),\s*(df\("[^"]+"\)|\w+)',line)
        if not m: continue
        expression=m[3].strip().replace('_ES','20000').replace('_AS','30').replace('_VS','0')
        if not re.fullmatch(r'[0-9 +]+',expression): continue
        meta=sum(int(x) for x in expression.split('+'))
        key=m[4]; df=re.search(r'\b'+re.escape(key)+r'\s*=\s*new DictFrame\("([^"]+)"',od)
        dictname=key[4:-2] if key.startswith('df(') else df[1] if df else key
        colors=re.findall(r'0x([0-9A-Fa-f]{6})',line)
        material=MATERIAL_ALIAS.get(m[1][4:],m[1][4:].lower())
        forms=re.search(r'\.setAutogen\(([^)]*)\)',line)
        mats.append({'constant':m[1],'metadata':meta,'material':material,'forms':set(split(forms[1])) if forms else set(),'colors':colors,'dict':dictname,'smeltable':m[2] in ('makeSmeltable','makeAdditive')})
    def add(spec):
        if spec['id'] in used: return
        spec=frozen.get(spec['id'],spec)
        if not (a.source/spec['source_texture']).is_file():
            audit.append({'source_variable':spec['source_variable'],'id':spec['id'],'reason':'missing source texture','texture':spec['source_texture']}); return
        used.add(spec['id']); rows.append(spec)
    classes={}
    for f in java.rglob('*.java'):
        text=clean(f.read_text()); match=re.search(r'\bclass\s+(\w+)\s+extends\s+(\w+)',text)
        if match: classes[match[1]]=(match[2],str(f.relative_to(a.source)),text)
    def ancestry(cls):
        seen=set()
        while cls in classes and cls not in seen:
            seen.add(cls); cls=classes[cls][0]
        return seen|{cls}
    block_decls={v:(c,stmt) for v,c,stmt,_ in declarations(source['block'])}
    def block_texture(variable):
        if variable not in block_decls:return None
        stmt=block_decls[variable][1]
        m=re.search(r'\.setBlockTextureName\(RefStrings.MODID\s*\+\s*":([^"]+)"\)',stmt)
        return m[1] if m else None
    for kind,text in source.items():
        sourcepath=f'src/main/java/com/hbm/{"items/ModItems" if kind=="item" else "blocks/ModBlocks"}.java'
        for var,cls,stmt,line in declarations(text):
            name=re.search(r'\.(?:setUnlocalizedName|setBlockName)\("([^"]+)"\)',stmt)
            if not name: continue
            name=name[1]
            if name in used: continue
            reg=rf'(?:GameRegistry\.register{kind.title()}\(\s*{var}\s*,|\bregister\(\s*{var}\s*\))'
            if not re.search(reg,text): audit.append({'source_variable':var,'class':cls,'reason':'not registered by supported source helper'});continue
            location=f'{sourcepath}:{line}'
            if kind=='item' and line>item_main_lines:location=f'src/main/java/com/hbm/items/ModItemsArmor.java:{line-item_main_lines-1}'
            spec={'id':name+'__item' if kind=='item' and name=='fallout' else name,'type':kind,'source_variable':var,'source_class':cls,'source_location':location}
            tex=re.search(r'\.(?:setTextureName|setBlockTextureName)\(RefStrings.MODID\s*\+\s*":([^"]+)"\)',stmt)
            tex=tex[1] if tex else None
            stage='materials' if name.startswith(MATERIAL_PREFIXES) or name in ('fluorite','niter','sulfur','lithium','lignite','cinnebar','oil_tar','coal_infernal') else 'components'
            spec['category']=name.split('_')[0] if stage=='materials' else 'component'
            if cls in ('ItemAutogen','ItemScraps'):
                shape,category=MATERIAL_FORMS.get(var,(None,'scrap'))
                for mat in mats:
                    if not (mat['smeltable'] if cls=='ItemScraps' else shape in mat['forms']): continue
                    sub=dict(spec,id=name+'__'+mat['material'],category=category,material=mat['material'],metadata=mat['metadata'],source_material=mat['constant'],subtype_kind='material')
                    override=re.search(r'\.aot\(Mats\.'+mat['constant']+r',\s*"([^"]+)"\)',stmt)
                    sub['source_texture']='src/main/resources/assets/hbm/textures/items/'+(override[1] if override else tex)+'.png'
                    if not override and mat['colors']:
                        # equal-color materials use getColorFromItemStack (multiplicative tint).
                        sub['tint']={'light':mat['colors'][0],'dark':mat['colors'][1] if len(mat['colors'])>1 else mat['colors'][0]} if len(mat['colors'])>1 and mat['colors'][0]!=mat['colors'][1] else {'multiply':mat['colors'][-1]}
                    key='hbmmat.'+mat['dict'].lower()
                    sub['names']={loc:translations[loc].get('item.'+name+'.name',name+' %s').replace('%s',translations[loc].get(key,mat['material'].replace('_',' ').title())) for loc in translations}
                    if a.stage=='materials' and category=='component': continue
                    add(sub)
                continue
            if a.stage!='materials' and kind=='item':
                tool=re.search(r'_(pickaxe|axe|shovel|hoe|sword)$',name)
                ordinary=tool and cls in ('ItemToolAbility','ItemSwordAbility','ModHoe','ModSword','ModPickaxe','ModAxe','ModSpade') and '.addAbility(' not in stmt
                armor=name.startswith(('steel_','titanium_','alloy_','cobalt_','starmetal_')) and name.rsplit('_',1)[-1] in ('helmet','plate','legs','boots') and cls in ('ArmorFSB','ModArmor')
                if ordinary or armor:
                    material=name.split('_')[0]; tier='iron' if material in ('steel','titanium','cobalt') else 'netherite' if material in ('schrabidium','euphemium','starmetal','desh','cmb','dineutronium') else 'diamond'
                    slot=tool[1] if ordinary else {'helmet':'helmet','plate':'chestplate','legs':'leggings','boots':'boots'}[name.rsplit('_',1)[-1]]
                    spec.update(category='tool' if ordinary else 'armor',template='minecraft:'+tier+'_'+slot,max_stack_size=1,equipment_gap={'source_stats':stmt.strip(),'differences':['durability','attack damage/speed or armor protection/toughness','enchantability','repair material','equipped armor appearance uses vanilla template'], 'reason':'content API 0.2 exposes template/max_stack_size/rarity only; common equipment adaptation authorized by coordinator'})
                    spec['source_texture']='src/main/resources/assets/hbm/textures/items/'+tex+'.png'
                    add(spec);continue
            if kind=='block':
                if cls=='BlockConcreteColored':
                    if a.stage!='decorative':continue
                    for meta,color in enumerate('white orange magenta light_blue yellow lime pink gray silver cyan purple blue brown green red black'.split()):
                        key='tile.'+name+'.'+color+'.name'
                        add(dict(spec,id=name+'__'+color,category='decorative',metadata=meta,subtype_kind='color',source_texture='src/main/resources/assets/hbm/textures/blocks/'+tex+'_'+color+'.png',names={loc:translations[loc].get(key,translations[loc].get('tile.'+name+'.name','Concrete')+' ('+color.replace('_',' ')+')') for loc in translations}))
                    continue
                if cls in ('BlockMeteorOre','BlockConcreteColoredExt','BlockOreBasalt','BlockResourceStone','BlockBiomeStone','BlockCoke','BlockCap','BlockLightstone','BlockNTMSand','BlockEnumMulti','BlockEnumMultiCT') and var not in claimed:
                    category='ore' if name.startswith('ore_') else 'resource_block' if name=='block_coke' else 'decorative'
                    if a.stage!='decorative' and category=='decorative':continue
                    body=classes[cls][2]
                    em=re.search(r'super\([^;]*?([A-Za-z0-9_.]+)\.class,\s*(true|false),\s*(true|false)\)',body)
                    em=em or re.search(r'new \w+\([^;]*?([A-Za-z0-9_.]+)\.class,\s*(true|false),\s*(true|false)\)',stmt)
                    if not em:audit.append({**spec,'reason':'unresolved block enum constructor'});continue
                    enumname=em[1].split('.')[-1];sep='_' if cls=='BlockOreBasalt' else '.'
                    for meta,(value,args) in enumerate(enum.get(enumname,[])):
                        suffix=value.lower();texture=(tex or name)+(sep+suffix if em[3]=='true' else '')
                        if em[3]=='true' and not (a.source/('src/main/resources/assets/hbm/textures/blocks/'+texture+'.png')).is_file():
                            # Some enum blocks name their faces with `_` instead of `.`.
                            alt=(tex or name)+('_' if sep=='.' else '.')+suffix
                            if (a.source/('src/main/resources/assets/hbm/textures/blocks/'+alt+'.png')).is_file():texture=alt
                        key='tile.'+name+(sep+suffix if em[2]=='true' else '')+'.name'
                        sub=dict(spec,id=name+'__'+suffix,category=category,metadata=meta,subtype_kind='enum',source_enum=enumname+'.'+value,source_texture='src/main/resources/assets/hbm/textures/blocks/'+texture+'.png',names={loc:translations[loc].get(key,(name+' '+suffix).replace('_',' ').title()) for loc in translations})
                        if cls in ('BlockCap','BlockLightstone','BlockNTMSand','BlockEnumMulti','BlockEnumMultiCT') and (a.source/('src/main/resources/assets/hbm/textures/blocks/'+texture+'_top.png')).is_file():
                            sub['surface_textures']={k:'src/main/resources/assets/hbm/textures/blocks/'+texture+end+'.png' for k,end in [('side',''),('top','_top'),('bottom','_top')]}
                        if cls=='BlockOreBasalt':
                            sub['surface_textures']={k:'src/main/resources/assets/hbm/textures/blocks/'+texture+end+'.png' for k,end in [('side',''),('top','_top'),('bottom','_top')]}
                            if args and args.startswith('ModItems.'):sub['drop']='ntm:'+args.split('.')[-1]
                        if category=='ore':sub.update(worldgen_status='catalog only; no worldgen translation',behavior_gap='source special generation/hazard/fortune behavior requires logic')
                        add(sub)
                    continue
                if cls=='BlockGenericStairs' or 'BlockGenericStairs' in ancestry(cls):
                    if a.stage!='decorative':continue
                    base=re.search(r'new \w+Stairs\((\w+)',stmt)
                    texture=block_texture(base[1]) if base else None
                    if texture:
                        add(dict(spec,category='decorative',template='minecraft:stone_stairs',source_texture='src/main/resources/assets/hbm/textures/blocks/'+texture+'.png'));continue
                if cls=='BlockMultiSlab':
                    if a.stage!='decorative':continue
                    constructor=re.search(r'new BlockMultiSlab\(([^)]*)\)',stmt)
                    if constructor:
                        bases=split(constructor[1])[2:]
                        for meta,base in enumerate(bases):
                            texture=block_texture(base)
                            if texture:add(dict(spec,id=name+'__'+base,category='decorative',metadata=meta,subtype_kind='slab',template='minecraft:stone_slab',source_texture='src/main/resources/assets/hbm/textures/blocks/'+texture+'.png',behavior_gap='source metadata split; vanilla slab waterlogging is a 26.3 adaptation',names={loc:translations[loc].get('tile.'+base+'.name',base.replace('_',' ').title())+(' Slab' if loc=='en_us' else ' (Laje)') for loc in translations}))
                        continue
                stage='materials' if name.startswith(('ore_','cluster_','block_')) and (name.startswith(('ore_','cluster_')) or cls in ('BlockGeneric','BlockBeaconable','BlockHazard','BlockHotHazard')) else 'decorative'
                spec['category']='ore' if name.startswith('ore_') else 'storage' if stage=='materials' else 'decorative'
                # bulk storage is not fed through the M1's curated nine-ingot rule.
                if spec['category']=='storage': spec['category']='resource_block'
                parents=ancestry(cls); classbody='\n'.join(classes[c][2] for c in parents if c in classes)
                ismachine='BlockContainer' in parents or bool(re.search(r'createNewTileEntity|createTileEntity|hasTileEntity',classbody))
                if ismachine:
                    tiles=sorted(set(re.findall(r'new (TileEntity\w+)',classbody)))
                    tilebody='\n'.join(classes[t][2] for t in tiles if t in classes)
                    machines.append({**spec,'class_file':classes.get(cls,('',None,''))[1],'tile_entities':tiles,'gui':sorted(set(re.findall(r'GUI_ENUM\.(\w+)|GUI_(\w+)',classbody+tilebody))),'energy':bool(re.search(r'IEnergy|power|Power|HE|Energy',tilebody)) if tiles else None,'fluid':bool(re.search(r'FluidTank|IFluid|FluidType',tilebody)) if tiles else None,'multiblock':bool(re.search(r'Dummy|Multiblock|MultiBlock|Dummyable',cls+classbody)),'status':'not ported; heuristic flags require machine agent verification'})
                    continue
                allowed=(cls in ORE_CLASSES and name.startswith(('ore_','cluster_'))) or cls in STATIC_BLOCKS
                if not allowed:
                    form=static_cube(cls,parents,classbody,name,lambda stem:(a.source/('src/main/resources/assets/hbm/textures/blocks/'+stem+'.png')).is_file(),tex) if cls in classes and var not in claimed else None
                    if form is None:
                        audit.append({**spec,'reason':'custom block behavior/model or unsupported block subclass'});continue
                    spec.update(form)
                    spec['source_form']='generic full-cube class (no shape override in ancestry)'
                if name.startswith(('ore_oil','ore_bedrock_oil','ore_gneiss_gas','ore_nether_smoldering')):
                    audit.append({**spec,'reason':'dynamic resource extraction/fire logic required'});continue
                if cls in ('BlockDepth','BlockDepthOre'):spec['behavior_gap']='Depth rock requires NTM mining tools; imported static content, custom harvesting not implemented'
                if cls=='BlockFalling':spec['template']='minecraft:gravel'
                if cls in ('BlockGlass','BlockNTMGlass','BlockReinforcedGlass'):spec['template']='minecraft:glass'
                if spec.get('surface_textures') and not next(iter(spec['surface_textures'].values())).startswith('src/'):
                    spec['surface_textures']={k:'src/main/resources/assets/hbm/textures/blocks/'+v+'.png' for k,v in spec['surface_textures'].items()}
                if cls in ('BlockPillar','BlockRotatablePillar'):
                    top=re.search(r'new '+cls+r'\(Material.\w+, RefStrings.MODID\s*\+\s*":([^"]+)"',stmt)
                    if top and tex:
                        spec['surface_textures']={k:'src/main/resources/assets/hbm/textures/blocks/'+v+'.png' for k,v in [('side',tex),('top',top[1]),('bottom',top[1])]}
                        if cls=='BlockRotatablePillar':spec['template']='minecraft:quartz_pillar'
                if cls in ('BlockHazard','BlockHotHazard'):spec['behavior_gap']='hazard/display effects and beacon base behavior require logic; static storage surface only'
                if name.startswith('ore_'):
                    spec['source_dimension']='minecraft:the_nether' if '_nether_' in name else 'minecraft:the_end' if name=='ore_tikite' or name.startswith('ore_end_') else 'overworld/special substrate (not inferred)'
                    spec['worldgen_status']='catalog only; no additional worldgen translated'
                    if cls not in ('BlockGeneric','BlockOre'): spec['behavior_gap']='outgassing/heat/radiation behavior not implemented by content package'
            elif cls in ('ItemEnumMulti','ItemGenericPart','ItemCircuit'):
                em=re.search(r'new ItemEnumMulti\((\w+(?:\.\w+)*)\.class,\s*(true|false),\s*(true|false)\)',stmt)
                enumname=em[1].split('.')[-1] if em else 'EnumCircuitType' if cls=='ItemCircuit' else 'EnumPartType'
                values=enum.get(enumname,[])
                if a.stage=='materials' and name not in ('coke','briquette','chunk_ore','ash','tar'): continue
                for meta,(value,args) in enumerate(values):
                    suffix=value.lower(); texture=(tex or name)+('.'+suffix if (em and em[3]=='true') or cls=='ItemCircuit' else '')
                    if cls=='ItemGenericPart': texture=json.loads(split(args)[0])
                    key='item.'+name+('.'+suffix if not em or em[2]=='true' else '')+'.name'
                    sub=dict(spec,id=name+'__'+suffix,metadata=meta,subtype_kind='enum',source_enum=enumname+'.'+value,source_texture='src/main/resources/assets/hbm/textures/items/'+texture+'.png',names={loc:translations[loc].get(key,(name+' '+suffix).replace('_',' ').title()) for loc in translations})
                    sub['category']='material' if name in ('coke','briquette','chunk_ore','ash','tar') else 'component'
                    add(sub)
                if not values:audit.append({**spec,'reason':'enum constants not resolved'})
                continue
            elif cls not in STATIC_ITEMS:
                body=classes.get(cls,('',None,''))[2]
                hooks=r'\b(?:onUpdate|onItemRightClick|onItemUse|onItemUseFirst|onEntitySwing|hitEntity|onBlockDestroyed|itemInteractionForEntity|onLeftClickEntity|onPlayerStoppedUsing|onEaten|onUsingTick|onArmorTick|getSubItems|getColorFromItemStack|registerIcons|getIconFromDamage|getIconFromDamageForRenderPass|requiresMultipleRenderPasses|setHasSubtypes|setUnlocalizedName|getUnlocalizedName|getItemStackDisplayName)\s*\('
                plain_consumer=classes.get(cls,('',None,''))[0]=='Item' and not re.search(hooks,body) and not re.search(r'\b(?:NBTTagCompound|setTagCompound|hasTagCompound|stackTagCompound)\b',body) and not any(word in cls for word in ('Tool','Weapon','Battery','InfiniteFluid','DemonCore'))
                if plain_consumer:
                    spec['source_configuration']=stmt.strip()
                    spec['behavior_gap']='Static component identity; machine-consumed configuration/durability/tooltips require consuming machine logic'
                    default_stack=re.search(r'setMaxStackSize\((\d+)\)',body)
                    if default_stack:spec['max_stack_size']=int(default_stack[1])
                    elif 'setMaxDamage(' in body:spec['max_stack_size']=1
                else:
                    chain=ancestry(cls)
                    form=static_item(cls,chain,'\n'.join(classes[c][2] for c in chain if c in classes),name) if var not in claimed else None
                    if form is None:
                        audit.append({**spec,'reason':'requires item logic/durability/equipment/fluid API; not plain content'});continue
                    spec.update(form);spec['source_form']='generic inert item class (use hooks only)'
                    default_stack=re.search(r'setMaxStackSize\((\d+)\)',classes.get(cls,('','',''))[2])
                    if default_stack and 'max_stack_size' not in form:spec['max_stack_size']=int(default_stack[1])
            if a.stage=='materials' and stage!='materials':continue
            if a.stage=='components' and stage=='decorative':continue
            if not tex:
                audit.append({**spec,'reason':'custom texture/model; no direct texture declaration'});continue
            if kind=='item' and re.search(r'\.setCreativeTab\(null\)',stmt):
                audit.append({**spec,'reason':'hidden/technical source item'});continue
            stack=re.search(r'\.setMaxStackSize\((\d+)\)',stmt)
            if stack:spec['max_stack_size']=int(stack[1])
            spec['source_texture']='src/main/resources/assets/hbm/textures/'+('items/' if kind=='item' else 'blocks/')+tex+'.png'
            add(spec)
    material_source=(java/'main/MainRegistry.java').read_text()+'\n'+source['item']
    definitions={m[1]:m[0].strip() for m in re.finditer(r'(?:public static )?(?:ToolMaterial|ArmorMaterial)\s+(\w+)\s*=\s*EnumHelper\.add(?:Tool|Armor)Material\([^;]+;',material_source)}
    for row in rows:
        if row.get('equipment_gap'):
            stmt=row['equipment_gap']['source_stats'];row['equipment_gap']['source_material_definitions']=[definition for key,definition in definitions.items() if re.search(r'\b'+key+r'\b',stmt)]
    from bulk_extra import extend
    extend(a.source,rows,audit,a.stage,classes,enum,add)
    write_json(ROOT/'content/equipment_backlog.json',{'schema':1,'entries':[r for r in audit if r.get('source_variable') not in ('detonator', 'mirror_tool') and any(x in r.get('source_class','') for x in ('Armor','Sword','Tool','Hoe','Gun','Weapon','Detonator','Pickaxe','Spade','Axe'))],'notes':'Custom abilities, hazmat/radiation protection, firearms and other equipment require logic. Authorized bomb detonator and mirror adjustment tool are registered separately.'})
    write_json(HERE/'bulk.json',{'schema':1,'stage':a.stage,'entries':sorted(rows,key=lambda x:x['id'])})
    bomb_variables={r['source_variable'] for r in json.loads((HERE/'bombs.json').read_text())['entries']} if (HERE/'bombs.json').exists() else set()
    machines=[r for r in machines if r['source_variable'] not in bomb_variables]
    write_json(ROOT/'content/machines_backlog.json',{'schema':1,'notes':'Static inspection, not functional machine support. GUI/energy/fluid/multiblock flags are evidence to verify, not guaranteed absence when false. Existing M1 declarations are excluded.','entries':machines})
    registered={kind:set(re.findall(r'GameRegistry\.register'+kind.title()+r'\(\s*(\w+)\s*,|\bregister\(\s*(\w+)\s*\)',text)) for kind,text in source.items()}
    accounted={kind:{r['source_variable'] for r in rows+existing+audit+machines if r.get('type')==kind} for kind in source}
    unresolved=[{'type':kind,'source_variable':(a or b),'reason':'Registered declaration not classified by scanner (cast/alias/dynamic expression); inspect source'} for kind,variables in registered.items() for a,b in sorted(variables) if (a or b) not in accounted[kind] and (a or b) not in bomb_variables]
    # A variable another full declaration (m3, bombs) covers is not a gap here; item_hooks only overrides base rows.
    elsewhere=[r for r in audit if r.get('source_variable') in claimed]
    audit=[r for r in audit if r.get('source_variable') not in claimed]
    write_json(HERE/'bulk-coverage.json',{'schema':1,'stage':a.stage,'imported':len(rows),'source_declarations':sum(len(declarations(t)) for t in source.values()),'categories':{cat:sum(r['category']==cat for r in rows) for cat in sorted({r['category'] for r in rows})},'gaps':audit,'claimed_elsewhere':len(elsewhere),'unresolved_registrations':unresolved})
    print(json.dumps({'stage':a.stage,'imported':len(rows),'machine_backlog':len(machines),'gaps':len(audit)},ensure_ascii=False))

if __name__=='__main__': main()

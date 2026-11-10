#!/usr/bin/env python3
"""Translate the supported literal Java recipe subset, with provenance and gaps.

No Java evaluation, guessed inputs or temporary crafting bridges. Ambiguous
expressions, dynamic loops and conditional recipes are reported, never emitted.
"""
import hashlib
import json
import re
from collections import defaultdict
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]

def clean(text):
    return re.sub(r'//[^\n]*|/\*.*?\*/',lambda m:'\n'*m[0].count('\n'),text,flags=re.S)

def split(text):
    out=[];start=0;depth=0;quote=None;escape=False
    for i,c in enumerate(text):
        if quote:
            if escape:escape=False
            elif c=='\\':escape=True
            elif c==quote:quote=None
        elif c in '\"\'':quote=c
        elif c in '([{':depth+=1
        elif c in ')]}':depth-=1
        elif c==',' and depth==0:out.append(text[start:i].strip());start=i+1
    if text[start:].strip():out.append(text[start:].strip())
    return out

def calls(text,pattern):
    for m in re.finditer(pattern+r'\s*\(',text):
        depth=1;quote=None;escape=False;end=m.end()
        while end<len(text) and depth:
            c=text[end]
            if quote:
                if escape:escape=False
                elif c=='\\':escape=True
                elif c==quote:quote=None
            elif c in '\"\'':quote=c
            elif c=='(':depth+=1
            elif c==')':depth-=1
            end+=1
        if depth==0:yield m.start(),text[m.end():end-1],m[0].split('(')[0].strip()

class Unsupported(ValueError):pass

# Renamed and metadata-bearing vanilla identities from 1.7.10.
RENAMES={'reeds':'sugar_cane','slime_ball':'slime_ball','brick_block':'bricks','stonebrick':'stone_bricks','melon_block':'melon','netherbrick':'nether_bricks','end_stone':'end_stone','lit_furnace':'furnace','golden_rail':'powered_rail','golden_apple':'golden_apple','dye':'ink_sac','snow_layer':'snow','hardened_clay':'terracotta','double_stone_slab':'smooth_stone','planks':'oak_planks','log':'oak_log','log2':'acacia_log','leaves':'oak_leaves','leaves2':'acacia_leaves','sapling':'oak_sapling','wool':'white_wool','stone_slab':'smooth_stone_slab','wooden_slab':'oak_slab','wooden_door':'oak_door','fence':'oak_fence','fence_gate':'oak_fence_gate','wooden_button':'oak_button','wooden_pressure_plate':'oak_pressure_plate','redstone_torch':'redstone_torch','quartz_block':'quartz_block','cooked_fished':'cooked_cod','fish':'cod','cooked_fish':'cooked_cod','speckled_melon':'glistering_melon_slice','experience_bottle':'experience_bottle','fireworks':'firework_rocket','firework_charge':'firework_star','waterlily':'lily_pad','water_bucket':'water_bucket','lava_bucket':'lava_bucket','skull':'skeleton_skull','noteblock':'note_block','iron_door':'iron_door'}
COLORS='white orange magenta light_blue yellow lime pink gray light_gray cyan purple blue brown green red black'.split()
DYES='ink_sac red_dye green_dye cocoa_beans lapis_lazuli purple_dye cyan_dye light_gray_dye gray_dye pink_dye lime_dye yellow_dye light_blue_dye magenta_dye orange_dye bone_meal'.split()

class Resolver:
    def __init__(self,src,rows):
        self.src=src;self.rows=rows;self.byvar=defaultdict(list);self.bymat={};self.byenum={};self.ore=defaultdict(set)
        self.vanilla=Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/data/minecraft')
        for r in rows:
            self.byvar[r['source_variable']].append(r)
            if 'source_material' in r:self.bymat[(r['source_variable'],r['source_material'])]=r
            if 'source_enum' in r:self.byenum[(r['source_variable'],r['source_enum'])]=r
        text=clean((src/'src/main/java/com/hbm/inventory/OreDictManager.java').read_text());self.odtext=text
        self.constants=dict(re.findall(r'\b(KEY_\w+)\s*=\s*"([^"]+)"',text))
        self.frames={}
        for m in re.finditer(r'\b(\w+)\s*=\s*new DictFrame\(([^;]*)\);',text):
            names=re.findall(r'"([^"]+)"',m[2]);self.frames[m[1]]=names[-1:] if '?' in m[2] else names
        # Exact source ore-dictionary registration chains (not name heuristics).
        for stmt in text.split(';'):
            match=re.search(r'^\s*(\w+)\s*\.',stmt)
            if not match or match[1] not in self.frames:continue
            for pos,args,method in calls(stmt,r'\.\w+'):
                shape=method[1:]
                if shape.startswith('haz'):shape=shape[3:].lower()
                for expression in split(args):
                    try:item,count=self.item(expression,unqualified=True)
                    except Unsupported:continue
                    for name in self.frames[match[1]]:self.ore[shape+name].add(item)
        # MaterialShapes generated entries register their own dictionary key.
        mats=clean((src/'src/main/java/com/hbm/inventory/material/Mats.java').read_text())
        shapes={'shell':'shell','ingot_raw':'ingot','nugget_raw':'nugget','wire_fine':'wireFine','wire_dense':'wireDense','plate_cast':'plateTriple','plate_welded':'plateSextuple','bedrock_ore_fragment':'bedrockorefragment','part_barrel_light':'barrelLight','part_barrel_heavy':'barrelHeavy','part_receiver_light':'receiverLight','part_receiver_heavy':'receiverHeavy','part_mechanism':'gunMechanism','part_stock':'stock','part_grip':'grip'}
        for r in rows:
            if 'source_material' not in r or r['source_variable'] not in shapes:continue
            m=re.search(r'\b'+r['source_material']+r'\s*=\s*make\w*\(\s*[^,]+,\s*(\w+)',mats)
            if m:
                for name in self.frames.get(m[1],[]):self.ore[shapes[r['source_variable']]+name].add(r['id'])
        vanilla_ores={'ingotIron':'iron_ingot','ingotGold':'gold_ingot','ingotCopper':'copper_ingot','ingotBrick':'brick','ingotBrickNether':'nether_brick','ingotNetherBrick':'nether_brick','nuggetIron':'iron_nugget','nuggetGold':'gold_nugget','dustRedstone':'redstone','dustGlowstone':'glowstone_dust','dustGunpowder':'gunpowder','gemDiamond':'diamond','gemEmerald':'emerald','gemQuartz':'quartz','gemLapis':'lapis_lazuli','blockIron':'iron_block','blockGold':'gold_block','blockDiamond':'diamond_block','blockEmerald':'emerald_block','blockGlass':'glass','blockGlassColorless':'glass','paneGlass':'glass_pane','paneGlassColorless':'glass_pane','sand':'sand','stone':'stone','cobblestone':'cobblestone','stickWood':'stick','slimeball':'slime_ball'}
        vanilla_ores['blockRedstone']='redstone_block'
        for ore,item in vanilla_ores.items():self.ore[ore].add('minecraft:'+item)
        for color in COLORS:self.ore['dye'+''.join(x.title() for x in color.split('_'))].add('minecraft:'+color+'_dye')
        # Forge's generic dye key includes all vanilla dye metadata and NTM's
        # explicit registrations (including wildcard enum families).
        self.ore['dye'].update('minecraft:'+name for name in DYES)
        for _, args, _ in calls(text, r'OreDictionary\.registerOre'):
            parts=split(args)
            if len(parts)!=2 or parts[0]!='"dye"':continue
            wildcard=re.fullmatch(r'new ItemStack\((\w+),\s*1,\s*OreDictionary\.WILDCARD_VALUE\)',parts[1])
            if wildcard:self.ore['dye'].update(r['id'] for r in self.byvar.get(wildcard[1],[]))
            else:
                try:self.ore['dye'].add(self.item(parts[1],unqualified=True)[0])
                except Unsupported:pass
        for ore,tag in [('logWood','logs'),('plankWood','planks'),('slabWood','wooden_slabs'),('treeLeaves','leaves'),('treeSapling','saplings')]:self.ore[ore].add('#minecraft:'+tag)
    def row(self,var,meta=None,kind=None):
        candidates=[r for r in self.byvar.get(var,[]) if kind is None or r['type']==kind or (kind=='item' and r.get('source_item_variable')==var)]
        if meta is not None:
            for r in candidates:
                if r.get('metadata')==meta:return r['id']
            raise Unsupported(f'missing subtype {var}:{meta}')
        plain=[r for r in candidates if 'metadata' not in r]
        if len(plain)==1:return plain[0]['id']
        # Java default metadata is 0, not the first material/enum declaration.
        for r in candidates:
            if r.get('metadata')==0:return r['id']
        raise Unsupported('unregistered/ambiguous source item '+var)
    def metadata(self,expr,var):
        expr=expr.strip()
        if expr.isdigit():return int(expr)
        m=re.fullmatch(r'(?:Mats\.)?(MAT_\w+)\.id',expr)
        if m:
            r=self.bymat.get((var,m[1]))
            if r:return r['metadata']
        m=re.fullmatch(r'(\w+\.\w+)\.ordinal\(\)',expr)
        if m:
            r=self.byenum.get((var,m[1]))
            if r:return r['metadata']
        raise Unsupported('dynamic/unsupported metadata '+expr)
    def item(self,expr,unqualified=False):
        expr=expr.strip()
        m=re.fullmatch(r'Item\.getItemFromBlock\((.*)\)',expr,re.S)
        if m:return self.item(m[1])
        m=re.fullmatch(r'new (?:ItemStack|ComparableStack)\((.*)\)',expr,re.S)
        if m:
            parts=split(m[1]);count=int(parts[1]) if len(parts)>1 and parts[1].isdigit() else 1 if len(parts)==1 else None
            if count is None:raise Unsupported('dynamic count '+expr)
            if len(parts)>2:
                var=re.search(r'(\w+)$',parts[0])[1]
                meta=self.metadata(parts[2],var)
                if parts[0].startswith(('ModItems.','ModBlocks.')):return self.row(var,meta,'item' if parts[0].startswith('ModItems.') else 'block'),count
                return self.vanilla_item(parts[0],meta),count
            item,_=self.item(parts[0],unqualified);return item,count
        m=re.fullmatch(r'(?:DictFrame\.)?fromOne\((.*)\)',expr,re.S)
        if m:
            parts=split(m[1]);var=parts[0].split('.')[-1];r=self.byenum.get((var,parts[1]))
            if not r:raise Unsupported('missing enum '+expr)
            return r['id'],int(parts[2]) if len(parts)>2 and parts[2].isdigit() else 1
        m=re.fullmatch(r'(?:Mats\.)?(MAT_\w+)\.make\(ModItems\.(\w+)(?:,\s*(\d+))?\)',expr)
        if m:
            r=self.bymat.get((m[2],m[1]))
            if not r:raise Unsupported('missing material '+expr)
            return r['id'],int(m[3] or 1)
        m=re.fullmatch(r'Mod(Items|Blocks)\.(\w+)',expr)
        if m:return self.row(m[2],kind='item' if m[1]=='Items' else 'block'),1
        if expr.startswith(('Items.','Blocks.')):return self.vanilla_item(expr),1
        if unqualified and re.fullmatch(r'\w+',expr):return self.row(expr),1
        raise Unsupported('unsupported expression '+expr[:150])
    def vanilla_item(self,expr,meta=0):
        name=expr.split('.')[-1]
        variants={'coal':['coal','charcoal'],'dye':DYES,'wool':[c+'_wool' for c in COLORS],'stained_hardened_clay':[c+'_terracotta' for c in COLORS],'stained_glass':[c+'_stained_glass' for c in COLORS],'planks':[c+'_planks' for c in ('oak','spruce','birch','jungle','acacia','dark_oak')],'log':[c+'_log' for c in ('oak','spruce','birch','jungle')],'log2':[c+'_log' for c in ('acacia','dark_oak')],'stone':['stone','granite','polished_granite','diorite','polished_diorite','andesite','polished_andesite'],'sand':['sand','red_sand'],'stonebrick':['stone_bricks','mossy_stone_bricks','cracked_stone_bricks','chiseled_stone_bricks']}
        if name in variants:
            try:name=variants[name][meta]
            except IndexError:raise Unsupported('unsupported vanilla metadata '+expr+':'+str(meta))
        elif meta:raise Unsupported('unsupported vanilla metadata '+expr+':'+str(meta))
        name=RENAMES.get(name,name)
        # Item assets include block items too; prevent stale 1.7 ids entering recipes.
        if not (self.vanilla.parent.parent/'assets/minecraft/items'/ (name+'.json')).exists():
            raise Unsupported('missing vanilla 26.3 item '+name)
        return 'minecraft:'+name
    def ingredient(self,expr,emit):
        expr=expr.strip();ore=None
        if expr in self.constants:ore=self.constants[expr]
        elif expr.startswith('"') and expr.endswith('"'):ore=json.loads(expr)
        m=re.fullmatch(r'(?:OreDictManager\.)?(\w+)\.(\w+)\(\)',expr)
        if m:
            names=self.frames.get(m[1],[])
            shape={'plateCast':'plateTriple','plateWelded':'plateSextuple'}.get(m[2],m[2])
            values=set().union(*(self.ore.get(shape+n,set()) for n in names))
            ore=shape+(names[0] if names else m[1])
        elif ore:values=self.ore.get(ore,set())
        else:return self.item(expr)[0]
        if not values:raise Unsupported('unresolved ore dictionary '+expr)
        slug=re.sub('[^a-z0-9_/.-]','_',ore.lower())
        emit('data/ntm/tags/item/bulk/'+slug+'.json',{'replace':False,'values':sorted(values)})
        return '#ntm:bulk/'+slug

def annotate(src,rows):
    """Exact DSL outputs + plainly labelled mention index for other patterns."""
    outputs=defaultdict(list);mentions=defaultdict(list)
    root=src/'src/main/java/com/hbm'; resolver=Resolver(src,rows)
    for file in sorted((root/'inventory/recipes').rglob('*.java')):
        text=clean(file.read_text());machine=re.sub(r'Recipes(?:NT)?$','',file.stem)
        machine=re.sub(r'(?<!^)(?=[A-Z])','_',machine).lower()
        for pos,args,_ in calls(text,r'\.outputItems'):
            for expression in split(args):
                try:item,_=resolver.item(expression)
                except Unsupported:continue
                outputs[item].append({'machine':machine,'source':str(file.relative_to(src))+':'+str(text[:pos].count('\n')+1)})
        legacy={'ShredderRecipes':r'(?:ShredderRecipes\.)?setRecipe','BlastFurnaceRecipes':r'addRecipe','PressRecipes':r'makeRecipe','CrystallizerRecipes':r'addRecipe','CentrifugeRecipes':r'addRecipe'}
        if file.stem in legacy:
            for pos,args,_ in calls(text,r'\b'+legacy[file.stem]):
                parts=split(args)
                if len(parts)<2:continue
                try:item,_=resolver.item(parts[-1])
                except Unsupported:continue
                outputs[item].append({'machine':machine,'source':str(file.relative_to(src))+':'+str(text[:pos].count('\n')+1)})
        for match in re.finditer(r'Mod(?:Items|Blocks)\.(\w+)',text):
            mentions[match[1]].append({'machine':machine,'source':str(file.relative_to(src))+':'+str(text[:match.start()].count('\n')+1)})
    generic={'wire_fine':'press/wiremill','wire_dense':'anvil','plate_cast':'foundry','plate_welded':'welding','bedrock_ore_fragment':'bedrock_mining/centrifuge','scraps':'foundry_recycling','part_barrel_light':'foundry','part_barrel_heavy':'foundry','part_receiver_light':'foundry','part_receiver_heavy':'foundry','part_mechanism':'foundry','part_stock':'foundry','part_grip':'foundry'}
    for r in rows:
        r['source_recipe_machine']=list({x['machine']:x for x in outputs.get(r['id'],[])+r.get('source_recipe_machine',[])}.values())
        if not r['source_recipe_machine'] and r['source_variable'] in generic:r['source_recipe_machine']=[{'machine':generic[r['source_variable']],'source':'NTM MaterialShapes/recipe autogen; material-specific recipe requires machine port'}]
        if not r['source_recipe_machine']:
            refs=mentions.get(r['source_variable'],[])
            r['source_recipe_references']=list({x['machine']:x for x in refs}.values())
        r['recipe_resolution']='indexed exact machine output where supported; references may be inputs; see bulk recipe coverage for unresolved expressions'
    return resolver

def generate(src,out,rows,emit):
    base_resolver=Resolver(src,rows);resolver=base_resolver;pile_resolver=None;provenance=[];gaps=[];existing=set()
    remainders={r['id']:r['crafting_remainder'] for r in rows if r.get('crafting_remainder')}
    # M1 retains ownership of its recipes. Avoid duplicate shapes/results.
    for f in (out/'data/ntm/recipe').glob('*.json'):
        if not f.name.startswith('bulk__'):
            recipe=json.loads(f.read_text());existing.add(json.dumps(recipe,sort_keys=True))
    def add(recipe,location):
        # Content declarations cannot yet attach the NTM crafting remainder
        # to these items. Do not publish a recipe that loses its iron casing.
        inputs=json.dumps({k:v for k,v in recipe.items() if k!='result'})
        for ident,remainder in remainders.items():
            if ident in inputs:raise Unsupported('native crafting remainder required: '+ident+' returns '+remainder)
        signature=json.dumps(recipe,sort_keys=True)
        if signature in existing:return
        existing.add(signature); name='bulk__'+hashlib.sha256((location+signature).encode()).hexdigest()[:18]
        emit('data/ntm/recipe/'+name+'.json',recipe)
        provenance.append({'id':'ntm:'+name,'source':location,'type':recipe['type'],'result':recipe['result']})
    java=src/'src/main/java/com/hbm'
    # Only known vanilla recipe entrypoints; nested condition/loop recipes are skipped.
    files=[java/'main/CraftingManager.java']+sorted((java/'crafting').glob('*.java'))
    for file in files:
        text=clean(file.read_text());scope=[];guarded=[];last=0;quote=None;escaped=False;pending_else=False
        for i,c in enumerate(text):
            if quote:
                if escaped:escaped=False
                elif c=='\\':escaped=True
                elif c==quote:quote=None
                continue
            if c in '\"\'':quote=c;continue
            if c=='{':
                header=text[last:i]
                # enable528 defaults to false: `if(!enable528)` always runs and its
                # `else` (ReaSim/528 variants) never does. Any other guard stays unsupported.
                default_on=bool(re.fullmatch(r'\s*if\s*\(\s*!\s*GeneralConfig\.enable528\s*\)\s*',header))
                default_off=pending_else and bool(re.fullmatch(r'\s*else\s*',header))
                pending_else=False
                scope.append((i,default_off or (not default_on and (bool(re.search(r'\b(?:if|for|while|switch)\s*\(',header)) or bool(scope and scope[-1][1]))),default_on));last=i+1
            elif c=='}':
                if scope:
                    start,guard,default_on=scope.pop()
                    pending_else=default_on
                    if guard:guarded.append((start,i))
                last=i+1
            elif c==';':last=i+1
        def supported(pos):return not any(lo<pos<hi for lo,hi in guarded)
        for pos,args,callee in calls(text,r'\b(?:addRecipeAuto|addShapelessAuto|addZIRNOXRod|addRBMKRod|addPellet|add1To9Pair|addMineralSet|GameRegistry\.addSmelting|GameRegistry\.addRecipe|GameRegistry\.addShapelessRecipe)'):
            location=str(file.relative_to(src))+':'+str(text[:pos].count('\n')+1)
            resolver=base_resolver
            # These newly imported tools need DictGroup ANY_PLASTIC. Reuse
            # the machine resolver's existing exact group expansion while
            # keeping the previous vanilla catalog's conservative profile.
            # The radiation claddings (onda 1) need DictGroup ANY_RUBBER the same way.
            if (file.name=='ToolRecipes.java' and re.match(r'new ItemStack\(ModItems\.hand_drill(?:_desh)?\)',args.strip())) or (file.name=='ConsumableRecipes.java' and re.match(r'new ItemStack\(ModItems\.cladding_rubber, 1\)',args.strip())):
                if pile_resolver is None:
                    from machine_recipes import MachineResolver
                    pile_resolver=MachineResolver(src,rows)
                resolver=pile_resolver
            try:
                parts=split(args)
                if callee=='addZIRNOXRod':
                    if file.name!='RodRecipes.java' or len(parts)!=2 or not parts[1].startswith('EnumZirnoxType.'):
                        raise Unsupported('unsupported ZIRNOX helper call')
                    # Both original overloads fill an empty casing with two
                    # billets. Keep this source helper separate from a generic
                    # invented recipe; fail if either overload changes.
                    for ingredient in ['billet', 'mat.billet()']:
                        expected='new Object[] { ModItems.rod_zirnox_empty, '+ingredient+', '+ingredient+' }'
                        if expected not in text:raise Unsupported('ZIRNOX helper body drift')
                    ingredient=parts[0]+'.billet()' if parts[0] in resolver.frames else parts[0]
                    parts=['new ItemStack(ModItems.rod_zirnox, 1, '+parts[1]+'.ordinal())',
                           'new Object[] { ModItems.rod_zirnox_empty, '+ingredient+', '+ingredient+' }']
                    callee='addShapelessAuto'
                if callee=='addPellet':
                    if file.name!='RodRecipes.java' or len(parts)!=2 or not parts[1].startswith('EnumWatzType.'):
                        raise Unsupported('unsupported Watz pellet helper call')
                    # Both original overloads are the same ring: four ingots around a graphite ingot.
                    for ingredient in ['mat.ingot()','item']:
                        expected='new Object[] { " I ", "IGI", " I ", \'I\', '+ingredient+', \'G\', GRAPHITE.ingot() }'
                        if expected not in text:raise Unsupported('Watz pellet helper body drift')
                    ingredient=parts[0]+'.ingot()' if parts[0] in resolver.frames else parts[0]
                    parts=['new ItemStack(ModItems.watz_pellet, 1, '+parts[1]+'.ordinal())',
                           'new Object[] { " I ", "IGI", " I ", \'I\', '+ingredient+', \'G\', GRAPHITE.ingot() }']
                    callee='addRecipeAuto'
                if callee=='addRBMKRod':
                    if file.name!='RodRecipes.java' or len(parts)!=2 or not parts[1].startswith('ModItems.rbmk_fuel_'):
                        raise Unsupported('unsupported RBMK helper call')
                    # Both original overloads are shapeless: empty rod plus eight billets.
                    for ingredient in ['mat.billet()','billet']:
                        expected='new Object[] { ModItems.rbmk_fuel_empty, '+', '.join([ingredient]*8)+' }'
                        if expected not in text:raise Unsupported('RBMK helper body drift')
                    ingredient=parts[0]+'.billet()' if parts[0] in resolver.frames else parts[0]
                    parts=['new ItemStack('+parts[1]+', 1)','new Object[] { ModItems.rbmk_fuel_empty, '+', '.join([ingredient]*8)+' }']
                    callee='addShapelessAuto'
                if callee in ('GameRegistry.addRecipe','GameRegistry.addShapelessRecipe'):
                    wrapper=re.fullmatch(r'new (ShapedOreRecipe|ShapelessOreRecipe)\((.*)\)',parts[0],re.S)
                    if wrapper:
                        parts=split(wrapper[2]);callee='addShapelessAuto' if wrapper[1]=='ShapelessOreRecipe' else 'addRecipeAuto'
                    else:callee='addShapelessAuto' if callee=='GameRegistry.addShapelessRecipe' else 'addRecipeAuto'
                if not supported(pos):raise Unsupported('conditional/loop source recipe not translated')
                if callee=='add1To9Pair' or callee=='addMineralSet':
                    pairs=[parts] if len(parts)==2 else [parts[:2],parts[1:]]
                    for pair in pairs:
                        # addMineralSet order is nugget, ingot, block; add1To9Pair big,small.
                        big,small=pair if callee=='add1To9Pair' else pair[::-1]
                        big,big_count=resolver.item(big);small,small_count=resolver.item(small)
                        if big_count!=1 or (small_count not in (1,9)):raise Unsupported('noncanonical pair counts')
                        add({'type':'minecraft:crafting_shaped','category':'misc','pattern':['###']*3,'key':{'#':small},'result':{'id':big,'count':1}},location)
                        add({'type':'minecraft:crafting_shapeless','category':'misc','ingredients':[big],'result':{'id':small,'count':9}},location)
                elif callee=='GameRegistry.addSmelting':
                    ingredient,_=resolver.item(parts[0]);result,count=resolver.item(parts[1]);xp=float(parts[2].rstrip('Ff'))
                    for kind,ticks in [('smelting',200),('blasting',100)]:
                        add({'type':'minecraft:'+kind,'category':'misc','ingredient':ingredient,'result':{'id':result,'count':count},'experience':xp,'cookingtime':ticks},location+('; blasting extension' if kind=='blasting' else ''))
                else:
                    # This item's wildcard output exists solely for NEI; its
                    # type is stored in NBT, not a metadata-split identity.
                    output=parts[0]
                    if output=='new ItemStack(ModItems.fluid_identifier_multi, 1, OreDictionary.WILDCARD_VALUE)':
                        output='new ItemStack(ModItems.fluid_identifier_multi, 1)'
                    result,count=resolver.item(output);array=re.fullmatch(r'new Object\[\]\s*\{(.*)\}',parts[1],re.S)
                    if not array:raise Unsupported('unsupported recipe ingredient array')
                    ingredients=split(array[1]);recipe={'category':'misc','result':{'id':result,'count':count}}
                    if callee=='addRecipeAuto':
                        pattern=[]
                        while ingredients and ingredients[0].startswith('"'):pattern.append(json.loads(ingredients.pop(0)))
                        if not pattern or len(pattern)>3 or max(map(len,pattern))>3:raise Unsupported('non-vanilla crafting dimensions')
                        if len(ingredients)%2:raise Unsupported('odd shaped key list')
                        key={}
                        for symbol,expression in zip(ingredients[::2],ingredients[1::2]):
                            if not re.fullmatch("'[^']'",symbol):raise Unsupported('dynamic key '+symbol)
                            key[symbol[1]]=resolver.ingredient(expression,emit)
                        if {c for row in pattern for c in row if c!=' '}!=set(key):raise Unsupported('shaped key mismatch')
                        recipe.update(type='minecraft:crafting_shaped',pattern=pattern,key=key)
                    else:
                        if not 1<=len(ingredients)<=9:raise Unsupported('non-vanilla shapeless size')
                        recipe.update(type='minecraft:crafting_shapeless',ingredients=[resolver.ingredient(x,emit) for x in ingredients])
                    add(recipe,location)
            except (Unsupported,ValueError,IndexError,KeyError) as exc:
                gaps.append({'source':location,'callee':callee,'reason':str(exc),'expression':args[:600]})
    emit('bulk-recipe-coverage.json',{'schema':1,'recipes':provenance,'generated_count':len(provenance),'gaps':gaps,'notes':'Only unconditional supported literal recipes. Machine-only recipes are indexed in ids.json. No new temporary recipe bridges. Blasting is an explicit 26.3 extension.'})
    return provenance

"""Explicit render-only extensions to the declaration scanner.

No custom interaction is inherited: machine consumption, hazard effects,
connected textures and NBT inventories remain documented logic gaps.
"""
import json
import re
from pathlib import Path

def extend(src,rows,audit,stage,classes,enums,add):
    from bulk import declarations,split
    from generate import lang
    java=src/'src/main/java/com/hbm';assets=src/'src/main/resources/assets/hbm'
    langs={loc:lang(assets/f'lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    resolved=set()
    allowed_items={'ItemStamp','ItemCatalyst','ItemBedrockOre','ItemByproduct','ItemBedrockOreNew','ItemAlexandrite'}
    allowed_blocks={'BlockNTMGlassCT','BlockGenericTooltip','BlockDecoCT','BlockSellafieldOre'}
    for kind in ('item','block'):
        folder='items' if kind=='item' else 'blocks';file=java/f'{folder}/Mod{folder.title()}.java'
        for var,cls,stmt,line in declarations(file.read_text()):
            if cls not in (allowed_items if kind=='item' else allowed_blocks):continue
            if kind=='block' and stage!='decorative':continue
            if kind=='item' and stage=='materials' and cls not in ('ItemBedrockOre','ItemByproduct','ItemBedrockOreNew'):continue
            name=re.search(r'\.(?:setUnlocalizedName|setBlockName)\("([^"]+)"\)',stmt)
            if not name:continue
            name=name[1];texture=re.search(r'\.(?:setTextureName|setBlockTextureName)\(RefStrings.MODID\s*\+\s*":([^"]+)"',stmt)
            texture=texture[1] if texture else None
            spec={'id':name,'source_variable':var,'source_class':cls,'source_location':str(file.relative_to(src))+':'+str(line),'type':kind,'category':'component' if kind=='item' else 'decorative'}
            if cls=='ItemBedrockOreNew':
                tint_constants={'none':'FFFFFF','roasted':'CFCFCF','arc':'C3A2A2','washed':'DBE2CB'}
                for g,(grade,args) in enumerate(enums['BedrockOreGrade']):
                    args=split(args);grade_tint=tint_constants.get(args[0],args[0].removeprefix('0x'));prefix=json.loads(args[1]);traits=args[2:]
                    for t,(typename,args2) in enumerate(enums['BedrockOreType']):
                        params=split(args2);suffix=json.loads(params[2]);sub=dict(spec,id=name+'__'+grade.lower()+'_'+suffix,metadata=(g<<4)|t,subtype_kind='bedrock',source_grade='BedrockOreGrade.'+grade,source_ore_type='BedrockOreType.'+typename,category='processed_ore',source_texture=f'src/main/resources/assets/hbm/textures/items/bedrock_ore_{prefix}_{suffix}.png',tint={'light':params[0].removeprefix('0x'),'dark':params[1].removeprefix('0x'),'multiply':grade_tint},names={loc:langs[loc].get(f'item.bedrock_ore.grade.{grade.lower()}.name',grade.replace('_',' ').title()+' %s').replace('%s',langs[loc].get(f'item.bedrock_ore.type.{suffix}.name',suffix.title())) for loc in langs},source_recipe_machine=[{'machine':'bedrock_processing','source':str(classes[cls][1])}],behavior_gap='Static grade/type identity; processing and NBT ore concentration require machine logic')
                        sub['overlay_textures']=[{'source':f'src/main/resources/assets/hbm/textures/items/bedrock_ore_overlay.{trait.lower()}.png'} for trait in traits]
                        add(sub)
                resolved.add(var);continue
            if cls in ('ItemBedrockOre','ItemByproduct'):
                enumname='EnumBedrockOre' if cls=='ItemBedrockOre' else 'EnumByproduct'
                for meta,(value,args) in enumerate(enums[enumname]):
                    params=split(args);color=params[1] if cls=='ItemBedrockOre' else params[0]
                    key='item.'+name+('.'+value.lower() if cls=='ItemByproduct' else '')+'.name'
                    names={loc:langs[loc].get(key,name+' %s') for loc in langs}
                    if cls=='ItemBedrockOre':
                        ore=json.loads(params[0]).lower();names={loc:text.replace('%s',langs[loc].get('item.ore.'+ore,ore.title())) for loc,text in names.items()}
                    sub=dict(spec,id=name+'__'+value.lower(),metadata=meta,subtype_kind='enum',source_enum=enumname+'.'+value,source_texture=f'src/main/resources/assets/hbm/textures/items/{texture}.png',category='processed_ore',names=names)
                    if cls=='ItemByproduct':sub['tint']={'multiply':color.removeprefix('0x')}
                    else:sub['overlay_textures']=[{'source':'src/main/resources/assets/hbm/textures/items/ore_overlay.png','tint':{'multiply':color.removeprefix('0x')}}]
                    add(sub)
                resolved.add(var);continue
            if cls=='ItemAlexandrite':spec.update(category='gem',texture_crop=[0,0,16,16],behavior_gap='Static first texture frame; source light-sensitive color animation not implemented')
            if cls=='BlockNTMGlassCT':
                tex=re.search(r'new BlockNTMGlassCT\(\d+,\s*RefStrings.MODID\s*\+\s*":([^"]+)"',stmt)
                texture=tex[1] if tex else texture
                spec.update(template='minecraft:glass',behavior_gap='Connected texture seams are rendered as isolated source glass faces; no CT adjacency stitching')
                spec['loot_none']=not bool(re.search(r'Material\.\w+,\s*true\)',stmt))
            if not texture:continue
            spec['source_texture']=f'src/main/resources/assets/hbm/textures/{"items" if kind=="item" else "blocks"}/{texture}.png'
            if cls=='BlockDecoCT':spec['behavior_gap']='Isolated source tile; connected texture adjacency stitching not implemented'
            if cls=='BlockSellafieldOre':
                spec.update(category='ore',background_texture='src/main/resources/assets/hbm/textures/blocks/sellafield_slaked.png',behavior_gap='Static surface; Sellafield biome tint, spread and hazards require logic',worldgen_status='not generated; NTM special region missing')
                if name.endswith('diamond'):spec['drop']='minecraft:diamond'
                if name.endswith('emerald'):spec['drop']='minecraft:emerald'
                if name.endswith('radgem'):spec['drop']='ntm:gem_rad'
            if cls in ('ItemStamp','ItemCatalyst'):
                spec['max_stack_size']=1;spec['behavior_gap']='Machine-dependent durability/catalyst stats require machine logic; static item identity only'
                spec['source_configuration']=stmt.strip()
            add(spec);resolved.add(var)
    # The audit describes gaps left after all scanner extensions.
    audit[:]=[x for x in audit if x.get('source_variable') not in resolved]

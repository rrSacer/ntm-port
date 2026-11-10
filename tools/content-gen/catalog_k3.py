"""K3 source identities: inert doors/locks and remaining machine casings.

Uses the bulk declaration/class scanner. This is an explicit batch boundary,
not an inference that a missing machine has vanilla behavior.
"""
import json
import re
from pathlib import Path
from bulk import clean, declarations
from generate import lang

HERE=Path(__file__).resolve().parent
BASE='src/main/java/com/hbm/'


def build(source):
    spec=json.loads((HERE/'catalog-k3.json').read_text())
    assets=source/'src/main/resources/assets/hbm'
    texts={kind:(source/(BASE+file)).read_text() for kind,file in [('block','blocks/ModBlocks.java'),('item','items/ModItems.java')]}
    decls={kind:{v:(c,s,line) for v,c,s,line in declarations(text)} for kind,text in texts.items()}
    classes={}
    for path in (source/BASE).rglob('*.java'):
        body=clean(path.read_text());m=re.search(r'\bclass\s+(\w+)\s+extends\s+(\w+)',body)
        if m:classes[m[1]]=(m[2],body,str(path.relative_to(source)))
    translations={loc:lang(assets/f'lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
    def chain(cls):
        result=[]
        while cls in classes and cls not in result:
            result.append(cls);cls=classes[cls][0]
        return result+[cls]
    rows=[]
    for kind,variables in spec.items():
        for var in variables:
            cls,stmt,line=decls[kind][var]
            name=re.search(r'\.(?:setBlockName|setUnlocalizedName)\("([^"]+)"\)',stmt)[1]
            # ModBlocks and ModItems both register these four original names.
            ident=name
            ancestry=chain(cls);body='\n'.join(classes[c][1] for c in ancestry if c in classes)
            tile=kind=='block' and ('BlockContainer' in ancestry or bool(re.search(r'createNewTileEntity|createTileEntity|hasTileEntity',body)))
            tex=re.search(r'\.(?:setBlockTextureName|setTextureName)\(RefStrings.MODID\s*\+\s*":([^"]+)"\)',stmt)
            texture=(('blocks/' if kind=='block' else 'items/')+tex[1]) if tex else None
            candidates=[]
            if texture:candidates += [texture,texture+'_side',texture+'_front',texture+'_lower',texture+'_top']
            # Only source-declared icon strings; arbitrary filename similarity
            # must never choose an unrelated machine's texture.
            candidates+=['blocks/'+m for m in re.findall(r'registerIcon\(RefStrings.MODID\s*\+\s*":([^"]+)"\)',body)]
            texture=next((c for c in candidates if (assets/('textures/'+c+'.png')).is_file()),None)
            fallback=texture is None
            if fallback:
                if kind=='item':raise ValueError('No source item texture: '+var)
                texture='blocks/block_steel'
            names={loc:tr.get(('tile.' if kind=='block' else 'item.')+name+'.name',name.replace('_',' ').title()) for loc,tr in translations.items()}
            row={'id':ident,'type':kind,'source_variable':var,'source_class':cls,
                 'source_location':BASE+('blocks/ModBlocks.java' if kind=='block' else 'items/ModItems.java')+':'+str(line),
                 'source_class_file':classes.get(cls,('',None,None))[2],
                 'source_configuration':stmt.strip(),'names':names,
                 'category':'machine' if tile else 'decorative' if kind=='block' else 'component',
                 'source_texture':'src/main/resources/assets/hbm/textures/'+texture+'.png',
                 'behavior_gap':'Inert source identity only. No processing, inventory, item use, locking/key pairing, opening, redstone, linked multiblock, moving collision, model animation, hazards or technical state is implemented. Source casing/icon projected on one static block/item.',
                 'source_has_tile':tile,'source_ancestry':ancestry}
            if fallback:row['behavior_gap']+=' No resolvable block icon in source declaration/class; source block_steel fallback, faithful atlas/model deferred.'
            if tile:
                row.update(template='minecraft:beehive',textures={face:texture for face in ['side','top','front']},opaque_machine_textures=True)
                row['source_tiles']=sorted(set(re.findall(r'new (TileEntity\w+)',body)))
            elif kind=='block':row['template']='minecraft:stone'
            else:
                stack=re.search(r'\.setMaxStackSize\((\d+)\)',stmt) or re.search(r'(?:setMaxStackSize\(|maxStackSize\s*=\s*)(\d+)',body)
                row['max_stack_size']=int(stack[1]) if stack else 64
                keys=sorted(set(re.findall(r'(?:get|set)(?:String|Integer|Long|Boolean|Double|Float|IntArray|Tag)\("([^"]+)"',body)))
                if keys:row['source_nbt_keys']=keys
                row['behavior_gap']+=' Per-stack source NBT and durability/use hooks are deferred; no minecraft:max_damage is inferred.'
            if '.setCreativeTab(null)' in stmt.replace(' ',''):row['creative_hidden']=True
            if cls=='BlockModDoor':
                row.update(template='minecraft:oak_door',source_item_variable=var,
                           door_textures={'bottom':'src/main/resources/assets/hbm/textures/blocks/'+name+'_lower.png',
                                          'top':'src/main/resources/assets/hbm/textures/blocks/'+name+'_upper.png'},
                           item_texture='src/main/resources/assets/hbm/textures/items/'+name+'.png')
                row['behavior_gap']='Source manual/redstone door mapped to vanilla oak_door placement, halves, hinge and open states. NTM resistance, sounds and source metadata/NBT transform differ; no tile entity or invented lock.'
            # MachineElectricFurnace.getItemDropped gives the unlit block; MachineBrickFurnace has no
            # getItemDropped (only the client's pick-block getItem), so the lit brick furnace drops itself.
            if name=='machine_electric_furnace_on':row['drop']='ntm:machine_electric_furnace_off'
            rows.append(row)
    return rows

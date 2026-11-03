"""Source RTG decay/config and radgen fuel contracts; no generic crafter."""
import re
import struct
from bulk_recipes import clean
from bedrock_processing_common import BASE, integer, write
from radioisotope_catalog import waste_classes


def f32(value):
    return struct.unpack('f',struct.pack('f',value))[0]


def export(source, output, rows):
    items_path=BASE+'items/ModItems.java';items=clean((source/items_path).read_text())
    pellets=[]
    pattern=r'(pellet_rtg\w*)\s*=\s*new ItemRTGPellet\((.*?)\)\.setDecays\(DepletedRTGMaterial\.(\w+),\s*\(long\)\s*\(RTGUtil.getLifespan\(([\d.]+)F,\s*HalfLifeType\.(\w+),\s*false\)\s*\*\s*1\.5\)\)'
    known={r['id'] for r in rows}
    for m in re.finditer(pattern,items):
        var,heat,depleted,life,unit=m.groups()
        if heat.isdigit():on=off=int(heat)
        else:
            choice=re.fullmatch(r'VersatileConfig.rtgDecay\(\) \? (\d+) : (\d+)',heat)
            if not choice:raise ValueError('RTG heat expression drift: '+heat)
            on,off=map(int,choice.groups())
        factor={'LONG':48000*100*100,'MEDIUM':48000*100,'SHORT':48000}[unit]
        # Java evaluates float multiplication, converts to long, then applies
        # the double 1.5 multiplier and converts to long once more.
        ticks=int(int(f32(f32(factor)*f32(float(life))))*1.5)
        pellets.append({'item':'ntm:'+var,'heat_tu':{'decay_enabled':on,'decay_disabled':off},
                        'half_life':float(life),'half_life_unit':unit,'lifespan_ticks':ticks,
                        'decay_item':'ntm:pellet_rtg_depleted__'+depleted.lower(),
                        'source':items_path+':'+str(items[:m.start()].count('\n')+1)})
    if len(pellets)!=10:raise ValueError('RTG pellet contract drift')
    rtg_path=BASE+'tileentity/machine/TileEntityMachineRTG.java';rtg=clean((source/rtg_path).read_text())
    rad_path=BASE+'tileentity/machine/TileEntityMachineRadGen.java';rad=clean((source/rad_path).read_text())
    if not re.search(r'power \+= heat \* 5;',rtg):raise ValueError('RTG conversion drift')
    cap=re.search(r'heatMax = VersatileConfig.rtgDecay\(\) \? (\d+) : (\d+)',rtg)
    slots=int(re.search(r'super\((\d+)\)',rtg)[1])
    machine_config=clean((source/(BASE+'config/MachineConfig.java')).read_text())
    defaults={key:re.search(r'boolean '+key+r' = (true|false)',machine_config)[1]=='true' for key in ['doRTGsDecay','scaleRTGPower']}
    fuels=[]
    # Expand only the four explicit WasteClass loops; no fuel is inferred
    # from its display name or radiation rate.
    pattern=r'new ComparableStack\(ModItems\.(nuclear_waste_\w+), 1, i\),\s*new Triplet<Integer, Integer, ItemStack>\(([^,]+),\s*([^,]+),\s*new ItemStack\(ModItems\.(\w+), 1, i\)\)'
    for m in re.finditer(pattern,rad):
        var,power,duration,depleted=m.groups();family='short' if '_short' in var else 'long'
        for meta,(member,_,_,_) in enumerate(waste_classes(source,family)):
            suffix='__'+member.lower()
            fuels.append({'input':{'item':'ntm:'+var+suffix,'count':1},'output':{'item':'ntm:'+depleted+suffix,'count':1},
                          'power_he_per_tick':integer(power),'duration_ticks':integer(duration),'metadata':meta,
                          'source':rad_path+':'+str(rad[:m.start()].count('\n')+1)})
    pattern=r'new ComparableStack\(ModItems\.(scrap_nuclear|gem_rad)\),\s*new Triplet<Integer, Integer, ItemStack>\(([^,]+),\s*([^,]+),\s*(null|new ItemStack\(Items.diamond\))\)'
    for m in re.finditer(pattern,rad):
        var,power,duration,result=m.groups()
        fuels.append({'input':{'item':'ntm:'+var,'count':1},'output':None if result=='null' else {'item':'minecraft:diamond','count':1},
                      'power_he_per_tick':integer(power),'duration_ticks':integer(duration),
                      'source':rad_path+':'+str(rad[:m.start()].count('\n')+1)})
    if len(fuels)!=28:raise ValueError('Radgen fuel count drift: '+str(len(fuels)))
    for fuel in fuels:
        for entry in [fuel['input'],fuel['output']]:
            if entry and entry['item'].startswith('ntm:') and entry['item'] not in known:raise ValueError('Missing '+entry['item'])
    for pellet in pellets:
        if pellet['item'] not in known or pellet['decay_item'] not in known:raise ValueError('Missing RTG identity')
    return write(output,'radioisotopes',{'recipes':[], 'skipped':[], 'rtg_pellets':pellets,'radgen_fuels':fuels,
        'machine_config':{
            'ntm:machine_rtg_grey':{'source':rtg_path,'slots':slots,'power_max_he':int(re.search(r'powerMax = (\d+)',rtg)[1]),
                'heat_max_tu':{'decay_enabled':int(cap[1]),'decay_disabled':int(cap[2])},'he_per_tu':5,
                'tick_order':'Provide stored power first, sum per-pellet heat (before its decay), clamp heat to heatMax, add heat*5 HE, clamp stored power to powerMax.',
                'decay':'Per stack PELLET_DEPLETION. Uninitialized getLifespan initializes full lifespan; with an existing NBT compound missing the field it reads 0. If decay enabled and remaining<=0 replace with decay item; otherwise decrement by one. Heat was sampled before replacement.',
                'scaled_power':'When scaleRTGPower is enabled: ceil(baseHeat * remaining/maxLifespan), cast to Java short; otherwise baseHeat.',
                'automation':'All 15 slots exposed on every face; insertion delegates to TileEntityMachineBase (false), extraction denied. Manual vanilla slots accept any item.'},
            'ntm:machine_radgen':{'source':rad_path,'input_slots':12,'output_slots':12,
                'power_max_he':int(re.search(r'maxPower = (\d+)',rad)[1]),
                'tick_order':'Provide stored power first. Start eligible idle queues, consume exactly one input and store a singular copy without NBT. Then each active queue produces its HE, increments progress and completes at progress>=duration, inserting its residue. Clamp the total power to maxPower.',
                'output_capacity':'Checked when starting only: empty output, no residue, or matching item+metadata with enough stack capacity. No recheck on completion if automation has changed the output.',
                'is_on':'Any nonempty processing queue, even while storage is full; full storage does not pause/stop fuel processing.',
                'queues':'Twelve independent persisted queues (progress, maxProgress, production and processing stack).',
                'ports':'One source power port three blocks behind the core, determined by source orientation.'}},
        'config':{'source':BASE+'config/VersatileConfig.java','decay':'enable528 || MachineConfig.doRTGsDecay',
                  'scale_power':'enable528 || MachineConfig.scaleRTGPower','defaults':defaults,
                  'lifespan':'RTGUtil.getLifespan uses 48000 ticks/day, 100 days/year here, float32 multiplication then long conversion; setDecays multiplies the long by 1.5 and converts back to long. External config overrides not applied.'},
        'selection':'Source algorithm/fuel snapshot, no synthetic generic crafting duration or radiation-to-HE conversion.',
        'runtime_gaps':['Conditional radgen insertion awaits the shared engine inventory-rule hook (wave 0); pure rules and transactional runtime implemented.'],
        'coverage':{'machines':2,'rtg_pellets':len(pellets),'radgen_fuels':len(fuels),'skipped':0}})

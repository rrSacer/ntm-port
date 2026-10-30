"""Solar heat collection and Stirling conversion: source algorithms, not crafter recipes."""
import re
from bedrock_processing_common import BASE, write


def export(source, output, rows):
    def tile(name):
        path = BASE + 'tileentity/machine/TileEntity' + name + '.java'
        return path, (source / path).read_text()
    def loc(path, text, needle):
        return path + ':' + str(text[:text.index(needle)].count('\n') + 1)
    def match(text, pattern):
        result = re.search(pattern, text)
        if not result: raise ValueError('Solar/Stirling source drift: ' + pattern)
        return result
    def constant(text, name):
        value = match(text, r'\b' + name + r' = ([\d.]+)D?;')[1]
        return float(value) if '.' in value else int(value)

    path, text = tile('SolarBoiler')
    divisor = int(match(text, r'int process = heat / (\d+)')[1])
    ratio = int(match(text, r'steam.getFill\(\) \+ process \* (\d+)')[1])
    boiler = {
        'source': path,
        'tank_capacities_mb': {f.lower(): int(match(text, r'new FluidTank\(Fluids\.' + f + r', ([\d_]+)')[1].replace('_', '')) for f in ['WATER', 'STEAM']},
        'heat_per_water_mb': divisor, 'steam_per_water_mb': ratio,
        'operation': 'process=heat/heat_per_water_mb (Java int division); display=process BEFORE capacity limits; process=max(0,min(process,waterFill,(steamCapacity-steamFill)/steam_per_water_mb)); consume process water and produce process*steam_per_water_mb steam. Reset heat=0 every server tick, including unused heat and remainder.',
        'ports': [{'offset':[0,0,0], 'side':'down', 'fluid':'water', 'direction':'input'}, {'offset':[0,2,0], 'side':'up', 'fluid':'steam', 'direction':'output'}],
        'tick_order': 'Mirrors add heat during their own server tick; the boiler processes its current accumulator then resets it. Preserve tick order rather than inventing stored thermal capacity.',
        'sources': {'conversion':loc(path,text,'int process ='), 'reset':loc(path,text,'heat = 0;')},
    }
    path, text = tile('SolarMirror')
    toolpath = BASE + 'items/tool/ItemMirrorTool.java'; tool = (source / toolpath).read_text()
    mirror = {
        'source':path, 'sky_light_subtraction':int(match(text, r'skylightSubtracted - (\d+)')[1]),
        'target_boiler_offset':[0,-1,0],
        'operation':'If targetY<mirrorY, turn off and return. sun=savedSkyLightAtMirror-skylightSubtracted-sky_light_subtraction. If sun<=0 or the block above the mirror cannot see sky, turn off and return; otherwise isOn=true. Only if the tile at (targetX,targetY-1,targetZ) is a solar boiler, add sun to its heat accumulator. isOn can be true without a valid boiler.',
        'targeting': {'tool_source':toolpath, 'target':'Tool stores boiler core position with Y+1, then sets the mirror target.',
                      'max_distance_blocks':int(match(tool,r'lengthVector\(\) <= (\d+)')[1]),
                      'angle_condition':'dx*dx+dz*dz <= dy*dy; measured from mirror to stored target, inclusive.',
                      'validation':'Distance and angle are checked by ItemMirrorTool when linking, not by the mirror on each tick; the mirror only checks targetY>=mirrorY and solar visibility.'},
        'sources':{'solar':loc(path,text,'int sun ='), 'target':loc(path,text,'getTileEntity(tX, tY - 1, tZ)'), 'link':loc(toolpath,tool,'boolean withinReach')},
        'runtime_gaps':['Mirror tool is equipment backlog; linking, sky-light/weather sampling and beam rendering require runtime support.']}
    path, text = tile('Stirling')
    diffusion = constant(text,'diffusion'); efficiency = constant(text,'efficiency')
    # maxHeat() uses literals 300/1500, not configurable maxHeatNormal/maxHeatSteel.
    limits = match(text,r'maxHeat\(\) \{\s*return this.getBlockType\(\) == ModBlocks.machine_stirling \? (\d+) : (\d+)')
    configs = {'ntm:machine_solar_boiler':boiler, 'ntm:solar_mirror':mirror}
    for name, gear, creative in [('machine_stirling',0,False),('machine_stirling_steel',1,False),('machine_stirling_creative',2,True)]:
        configs['ntm:'+name] = {
            'source':path, 'diffusion':diffusion, 'efficiency':1.0 if creative else efficiency,
            'creative':creative, 'generates_without_heat':False,
            'max_heat_tu_per_tick':int(limits[1] if gear==0 else limits[2]),
            'gear_metadata':gear, 'repair_item':None if creative else 'ntm:gear_large__'+('iron' if gear==0 else 'steel'),
            'heat_input':'Only IHeatSource immediately below. heatSrc=trunc(sourceHeat*diffusion); if positive, consume exactly heatSrc and add to heat. Otherwise heat=max(heat-max(heat/1000,1),0).',
            'tick_order':'Update ports and provide previous powerBuffer first. If hasCog: reset powerBuffer=0, pull heat, powerBuffer=trunc(heat*efficiency), process overspeed. If no cog: reset overspeed/warnCooldown, decrement positive residual powerBuffer by1. Reset heat=0 at tick end.',
            'buffer':'Per-tick output, not accumulated storage; getMaxPower()=powerBuffer and getProviderSpeed()=hasCog ? powerBuffer : 0.',
            'overspeed':{'enabled':not creative, 'limit_ticks':constant(text,'overspeedLimit'),
                         'condition':'heat>maxHeat; consecutive ticks increment overspeed, otherwise reset0; failure when overspeed>limit (strictly greater).',
                         'warning_after_ticks':int(match(text,r'overspeed > (\d+) && warnCooldown')[1]),
                         'warning_cooldown_ticks':int(match(text,r'warnCooldown = (\d+);\s*worldObj.playSoundEffect')[1]),
                         'failure':'Set hasCog=false, explosion strength5 without fire or block damage, eject EntityCog; current powerBuffer remains but provider speed becomes0.'},
            'source_config_gap':'maxHeatNormal/maxHeatSteel are configurable fields but maxHeat() uses hardcoded 300/1500; preserve the effective literals. External config overrides are not applied in this snapshot.',
            'sources':{'heat':loc(path,text,'protected void tryPullHeat()'), 'conversion':loc(path,text,'this.powerBuffer = (long)'), 'max_heat':loc(path,text,'public int maxHeat()'), 'overspeed':loc(path,text,'if(overspeed > overspeedLimit)')},
            'runtime_gaps':['Cog loss/repair, missing-cog block-item metadata1, explosion/projectile, thermal/power networks, ports, animation and multiblock require runtime support.', 'Creative gear metadata2 has no declared item; creative bypasses overspeed and has no source crafting recipe.'] if creative else ['Cog loss/repair, missing-cog block-item metadata1, explosion/projectile, thermal/power networks, ports, animation and multiblock require runtime support.']}
    return write(output,'solar_stirling',{'machine_config':configs, 'recipes':[], 'skipped':[],
        'selection':'Algorithmic solar and thermal generators; no invented generic duration, fixed power or infinite creative energy.',
        'requires_runtime_fields':['machine_config'],
        'coverage':{'machines':len(configs),'skipped':0},
        'runtime_gaps':['Content/algorithm snapshot only. Solar targeting tool and native machine behavior are separate work; fallback cubes do not implement the source multiblocks.']})

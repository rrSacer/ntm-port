#!/usr/bin/env python3
"""Oil drill defaults and algorithmic extraction contracts, never flat recipes."""
import re
from bulk_recipes import clean, Unsupported
from bedrock_processing_common import BASE, write


def export(source,out,rows):
    basepath=BASE+'tileentity/machine/oil/TileEntityOilDrillBase.java'
    base=clean((source/basepath).read_text());machines={}
    fields={'maxPower':'energy_capacity_he','consumption':'power_he_per_tick','delay':'delay_ticks',
            'oilPerDepsoit':'oil_per_deposit_mb','gasPerDepositMin':'gas_min_mb','gasPerDepositMax':'gas_max_mb',
            'drainChance':'depletion_chance','solutionRequired':'fracksol_required_mb',
            'oilPerBedrockDepsoit':'bedrock_oil_mb','gasPerBedrockDepositMin':'bedrock_gas_min_mb',
            'gasPerBedrockDepositMax':'bedrock_gas_max_mb','destructionRange':'surface_destruction_range'}
    for var,tile in [('machine_well','TileEntityMachineOilWell'),('machine_pumpjack','TileEntityMachinePumpjack'),('machine_fracking_tower','TileEntityMachineFrackingTower')]:
        path=BASE+'tileentity/machine/oil/'+tile+'.java';text=clean((source/path).read_text());cfg={'sources':{},'source_class':path}
        for match in re.finditer(r'static (int|long|double) (\w+) = ([\d_.]+)[LD]?;',text):
            if match[2] not in fields:continue
            key=fields[match[2]];cfg[key]=float(match[3]) if match[1]=='double' else int(match[3].replace('_',''))
            cfg['sources'][key]=path+':'+str(text[:match.start()].count('\n')+1)
        required=set(list(fields.values())[:7])
        if not required<=set(cfg):raise Unsupported('oil drill config drift '+tile+':'+str(required-set(cfg)))
        origin=text if 'public int getDrillDepth()' in text else base
        originpath=path if origin is text else basepath
        m=re.search(r'public int getDrillDepth\(\)\s*\{\s*return (\d+);',origin)
        if m is None:raise Unsupported('oil drill depth drift')
        cfg['legacy_drill_depth_y']=int(m[1]);cfg['sources']['legacy_drill_depth_y']=originpath+':'+str(origin[:m.start()].count('\n')+1)
        cfg['port_drill_depth_y']=-64 if var=='machine_fracking_tower' else cfg['legacy_drill_depth_y']
        cfg['depth_adaptation']='Fracker follows the 26.3 overworld floor at -64 to reach adapted bedrock oil; ordinary drill depth remains source default 5.'
        cfg['outputs']=['oil','gas'];cfg['gas_distribution']='uniform inclusive integer range'
        cfg['deposit_blocks']=['ntm:ore_oil','ntm:ore_oil_empty']+(['ntm:ore_bedrock_oil'] if var=='machine_fracking_tower' else [])
        cfg['depleted_block']='ntm:ore_oil_empty'
        cfg['tanks']=[{'fluid':name,'capacity_mb':64000} for name in ['oil','gas']+(['fracksol'] if var=='machine_fracking_tower' else [])]
        cfg['tank_source']=path+':'+str(text[:text.index('tanks = new FluidTank')].count('\n')+1) if var=='machine_fracking_tower' else basepath+':'+str(base[:base.index('tanks = new FluidTank')].count('\n')+1)
        cfg['base_methods']={}
        for method in ['updateEntity','getPowerReqEff','getDelayEff','tryDrill','trySuck','suckRec']:
            m=re.search(r'public (?:void|int|boolean) '+method+r'\(',base)
            cfg['base_methods'][method]=basepath+':'+str(base[:m.start()].count('\n')+1)
        cfg['extraction_method']=path+':'+str(text[:text.index('public void onSuck(')].count('\n')+1)
        machines['ntm:'+var]=cfg
    return write(out,'oil_extraction',{'machine_config':machines,
        'selection':'algorithmic vertical drill and six-neighbor depleted-deposit search, not a player recipe list',
        'recipes':[],'skipped':[],'coverage':{'machines':3,'flat_recipes':0,'skipped':0},
        'rules':{'energy':'charge power_he_per_tick on every eligible server tick, including ticks not divisible by delay; eligibility requires both output tanks not full',
                 'drilling':'scan down from machine y-1, skip oil_pipe; the first other block is pumped or drilled; drilling requires explosion resistance <1000 and places a no-drop oil_pipe',
                 'deposit_search':'accept oil/full or empty at drill tip; recurse through empty deposits using shuffled six directions, with visited set and depth limit 64; stop at first full deposit',
                 'production':'each successful normal suck produces oil_per_deposit_mb and uniform gas, clamped to capacity; deplete full deposit with depletion_chance',
                 'fracking':'requires fracksol_required_mb; bedrock deposit uses bedrock rates without depletion; consume solution after production; surface OilSpot call is documentary, outside content worldgen',
                 'upgrades':'source getPowerReqEff/getDelayEff and AFTERBURN remain algorithmic runtime rules; this table is unupgraded defaults, not a substitute implementation'},
        'notes':['No deterministic oil-per-block recipe is invented from expected probabilistic depletion yields.',
                 'port_drill_depth_y is an explicit 26.3 height adaptation; legacy value and source are retained.',
                 'Transformer construction is in vanilla crafting; energy conversion behavior is a separate logic gap.']})

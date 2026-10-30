"""Battery pack arithmetic, native-ready network identities and source rules."""
import re
from bedrock_processing_common import BASE, write
from bulk_recipes import clean
from energy_storage_catalog import packs, CABLES, CONNECTORS, CONTROLS


def export(source, output, rows):
    endpoints={}
    for variable in CONNECTORS:
        cls=('ConnectorSuper' if variable=='red_connector_super' else 'Connector' if variable=='red_connector' else
             'PylonLarge' if variable=='red_pylon_large' else 'PylonMedium' if '_medium_' in variable else 'Pylon')
        path=BASE+'tileentity/network/TileEntity'+cls+'.java';text=clean((source/path).read_text())
        length=int(re.search(r'getMaxWireLength\(\)\s*\{\s*return (\d+)',text)[1])
        parent=clean((source/(BASE+'tileentity/network/TileEntityConnector.java')).read_text()) if cls=='ConnectorSuper' else text
        group=re.search(r'return ConnectionType\.(\w+)',parent)[1]
        endpoints['ntm:'+variable]={'source':path,'max_wire_length_blocks':length,'connection_type':group,
                                    'transformer':'_transformer' in variable}
    storage_path=BASE+'tileentity/machine/storage/';base=clean((source/(storage_path+'TileEntityBatteryBase.java')).read_text())
    modes={name:int(value) for name,value in re.findall(r'int mode_(\w+) = (\d+)',base)}
    if modes!={'input':0,'buffer':1,'output':2,'none':3}:raise ValueError('Battery mode drift')
    battery_packs=packs(source)
    known={r['id'] for r in rows}
    if any(p['id'] not in known for p in battery_packs):raise ValueError('Missing battery pack')
    redd_path=storage_path+'TileEntityBatteryREDD.java';redd=clean((source/redd_path).read_text())
    divisor=int(re.search(r'getMaxPower\(\) \{ return Long.MAX_VALUE / (\d+)L',redd)[1])
    net_path='src/main/java/api/hbm/energymk2/PowerNetMK2.java';net=clean((source/net_path).read_text())
    priorities=clean((source/'src/main/java/api/hbm/energymk2/IEnergyReceiverMK2.java').read_text()).split('enum ConnectionPriority {',1)[1].split('}',1)[0]
    priorities=[name.strip() for name in priorities.split(',')]
    return write(output,'energy_storage',{'recipes':[], 'skipped':[], 'battery_packs':battery_packs,
        'machine_config':{
            'ntm:machine_battery_socket':{'source':storage_path+'TileEntityBatterySocket.java','slots':1,
                'storage':'No internal energy store. Slot0 must implement IBatteryItem; getPower/getMaxPower and rates delegate to that item. Empty or invalid slot returns zero.',
                'power_item_field':'charge (source long NBT); pack getCharge initializes missing data to zero. charge/set/discharge setters do not clamp by themselves; callers apply limits.',
                'rates':'provider = dischargeRate in output/buffer modes; receiver = chargeRate in input/buffer modes; otherwise zero.',
                'special_item_gap':'Source battery_sc has fluctuation and discharge-beam behavior; battery_sc/cube_power/potato accessories are outside this batch.',
                'history':'Average of previous and current power, compared to oldest of 20 samples; source uses long arithmetic.'},
            'ntm:machine_battery_redd':{'source':redd_path,'slots':2,'storage':'Unbounded BigInteger energy, persisted as signed byte array. No invented fixed storage capacity.',
                'network_window_he':(2**63-1)//divisor,'provision_window_he':((2**63-1)//divisor)//2,
                'network_window_note':'getMaxPower is Long.MAX_VALUE/100 for connection speed; getPower reports min(actual, network_window/2). setPower is a no-op; transferPower adds and returns0, usePower subtracts.',
                'tick_order':'Base battery networking first; then charge from slot0 and charge item in slot1. Delta compares a BigInteger average against oldest of 20 samples.',
                'construction_gap':'Original PlasmaForgeRecipes.java ass.fensusan; plasma forge not ported. No temporary crafting/assembly replacement.'}},
        'battery_base':{'source':storage_path+'TileEntityBatteryBase.java','modes':modes,
            'default_low':int(re.search(r'redLow = (\d+)',base)[1]),'default_high':int(re.search(r'redHigh = (\d+)',base)[1]),
            'default_priority':'LOW','redstone':'Any indirectly-powered port selects redHigh; otherwise redLow. Cached mode is reused by rate queries.',
            'network_membership':'input=receiver only, output=provider only, buffer=both, none=neither. Invalid priority (null, ordinal0 or4) resets LOW.',
            'comparator':'Clamp(round(power/max(maxPower,1)*15),0,15), using double arithmetic.',
            'buttons':'low/high advance modulo4; priority advances through LOW..HIGH and wraps.'},
        'network':{'cables':['ntm:'+name for name in CABLES],'wire_endpoints':endpoints,'controls':['ntm:'+name for name in CONTROLS],
            'distribution':{'source':net_path,'priorities':priorities,
                'supply':'For valid providers: min(getPower(),getProviderSpeed()), include positive entries only.',
                'demand':'For valid receivers: min(getMaxPower()-getPower(),getReceiverSpeed()), include positive entries, grouped by priority.',
                'delivery':'Start toTransfer=min(totalSupply,totalDemand). Traverse priority groups from highest ordinal down; allocate trunc(clamp(toTransfer * demand_i/groupDemand,0,demand_i)); account for transferPower leftovers.',
                'source_quirk':'After EACH priority group the source subtracts cumulative energyUsed from toTransfer (not merely that group\'s delta). Preserve this literal behavior in a faithful port.',
                'provider_debit':'Proportional to each provider supply, Java double weight and long truncation. Remaining rounding surplus is debited from random providers, bounded by iterationsLeft.',
                'rounding_iterations':int(re.search(r'int iterationsLeft = (\d+)',net)[1]),
                'tracker':'Sum provider power before minus nonnegative provider power after; no synthetic cable loss or HE/t rating added.'},
            'link_source':BASE+'tileentity/network/TileEntityPylonBase.java',
            'link_validation':'Same ConnectionType, distinct endpoints, Euclidean distance between connection points <= min(endpoint length limits). SINGLE/TRIPLE/QUAD describe compatibility, not a maximum link count.',
            'connections':'Persisted endpoint coordinate lists plus color. Medium pylons connect to adjacent devices only for transformer variants; connector uses its oriented back face; ordinary pylon excludes down.',
            'switch':{'source':BASE+'blocks/network/CableSwitch.java','operation':'Non-sneaking right click toggles metadata0/1. TileEntityCableSwitch joins only in state1 and destroys its node when switched off.'},
            'detector':{'source':BASE+'blocks/network/CableDetector.java','operation':'An input controlled by neighboring redstone: indirectly powered means metadata1, otherwise0. Uses the same switch tile. It does not measure energy throughput.'},
            'scope':'Four cable identities are simple blocks without block entities. Connectors, pylons and controls reserve beehive/facing/honey state for future native logic; all are Inert now.'},
        'legacy_gaps':[{'id':'ntm:cable_diode','source':BASE+'blocks/ModBlocks.java','reason':'Deprecated and creativeTab null; not imported as an active network device.'}],
        'runtime_gaps':['No native battery/network logic here. Item charge/tooltip and per-pack 3D renderer, wire placement, paint/disguise, gauge, cable boxes, collisions, full source orientations/multiblocks and menus remain separate work.',
                        'Four-way horizontal facing cannot encode every original six-face connector orientation; native geometry/orientation mapping remains to implement.'],
        'coverage':{'battery_packs':len(battery_packs),'cables':len(CABLES),'wire_endpoints':len(endpoints),'controls':len(CONTROLS),'storage_blocks':2,'skipped':0}})

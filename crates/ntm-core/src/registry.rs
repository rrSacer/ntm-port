//! Which blocks the logic knows, and how their states carry the machine's
//! look. Mirrors `content/ids.json` (`machines`, `logic`, `internal_blocks`);
//! a test checks the two agree.

use crate::machine::Kind;

/// How a block's state shows a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockSpec {
    pub id: &'static str,
    pub kind: Kind,
    /// Horizontal facing, set by the template on placement; kept as placed.
    pub facing: Option<&'static str>,
    /// A numeric property 0..=max: 0 idle, 1..=max the machine's level.
    pub level: Option<(&'static str, u8)>,
    /// Its HE ports, as the NTM's `getPorts` (on one block).
    pub ports: Ports,
}

/// The NTM's power port shapes, brought to one block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ports {
    /// No power.
    None,
    /// `TilePortShapes.around`: one port open on all six faces (electric
    /// furnace, electric press): it joins everything around it.
    AllRound,
    /// The battery's `solderer` shape: a separate port on each horizontal
    /// side, so energy never crosses the battery.
    Sides,
    /// The wood burner: one port at its back (`dir.getOpposite()`).
    Back,
}

/// A horizontal facing as an offset.
pub fn facing_dir(facing: &str) -> Option<crate::Pos> {
    match facing {
        "north" => Some([0, 0, -1]),
        "south" => Some([0, 0, 1]),
        "east" => Some([1, 0, 0]),
        "west" => Some([-1, 0, 0]),
        _ => None,
    }
}

/// The ports of a machine of `spec` facing `facing`.
pub fn ports(spec: &BlockSpec, facing: Option<&str>) -> Vec<crate::energy::Port> {
    use crate::energy::DIRS;
    match spec.ports {
        Ports::None => Vec::new(),
        Ports::AllRound => vec![DIRS.to_vec()],
        Ports::Sides => DIRS
            .iter()
            .filter(|d| d[1] == 0)
            .map(|d| vec![*d])
            .collect(),
        Ports::Back => match facing.and_then(facing_dir) {
            Some([x, y, z]) => vec![vec![[-x, -y, -z]]],
            None => vec![DIRS.to_vec()],
        },
    }
}

const HONEY: Option<(&str, u8)> = Some(("honey_level", 5));

/// The property a bomb block's template (`minecraft:redstone_lamp`) sets
/// while redstone powers it.
pub const BOMB_POWERED: &str = "lit";

const fn bomb(id: &'static str) -> BlockSpec {
    BlockSpec {
        id,
        kind: Kind::Bomb,
        facing: None,
        level: None,
        ports: Ports::None,
    }
}

/// A nuke (`nuke_*`): a block entity's template (beehive, C2 of the
/// migration), so a facing and the honey level like the machines; no `lit`.
const fn nuke(id: &'static str) -> BlockSpec {
    BlockSpec {
        id,
        kind: Kind::Bomb,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    }
}

pub fn is_bomb(id: &str) -> bool {
    spec(id).is_some_and(|s| s.kind == Kind::Bomb)
}

pub const BLOCKS: &[BlockSpec] = &[
    // K3 tile identities: registered without processing or invented inventories.
    BlockSpec {
        id: "ntm:seal_hatch",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:vault_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:blast_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:sliding_blast_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:fire_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:transition_seal",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:silo_hatch",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:silo_hatch_large",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:secure_access_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:large_vehicle_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:qe_containment",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:qe_sliding_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:round_airlock_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:sliding_seal_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:water_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:cargo_door",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:event_tester",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:obj_tester",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:deco_emitter",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:part_emitter",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:filing_cabinet",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_ashpit",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:furnace_iron",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:furnace_steel",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:furnace_combination",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_sawmill",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_strand_caster",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_fel",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_silex",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_rotary_furnace",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_furnace_brick_off",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_furnace_brick_on",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_industrial_generator",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_cyclotron",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_exposure_chamber",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_electric_furnace_on",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_microwave",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_teleporter",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_radiolysis",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_drain",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:fan",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_keyforge",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:tesla",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:chimney_brick",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:chimney_industrial",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_autosaw",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_thresher",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_annihilator",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_rockmill",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_mining_laser",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_assembly_factory",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_precass",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_arc_welder",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_soldering_station",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_chemical_factory",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_supercomputer",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_orbus",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_lpw2",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_ammo_press",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_tower_small",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_tower_large",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_deuterium_extractor",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_intake",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_compressor",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_compressor_compact",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_autocrafter",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_funnel",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_siren",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_forcefield",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:decon",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_battery",
        kind: Kind::Battery,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::Sides,
    },
    BlockSpec {
        id: "ntm:machine_wood_burner",
        kind: Kind::WoodBurner,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::Back,
    },
    BlockSpec {
        id: "ntm:machine_electric_furnace_off",
        kind: Kind::ElectricFurnace,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_press",
        kind: Kind::BurnerPress,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_epress",
        kind: Kind::ElectricPress,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // Recipe machines (`crate::crafter`, docs/maquinas.md).
    BlockSpec {
        id: "ntm:machine_assembly_machine",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_chemical_plant",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The PUREX (`crate::crafter::Station::Purex`): ports on its whole
    // bottom ring, as the proxies of its multiblock.
    // The bedrock chain's stage 3 (`crate::crafter`): the pyrolysis oven and
    // the electrolyser's fluid line.
    BlockSpec {
        id: "ntm:machine_pyrooven",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_electrolyser",
        kind: Kind::Electrolyser,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The crystallizer (`crate::crafter::Station::Crystallizer`), the
    // bedrock chain's stage 2.
    BlockSpec {
        id: "ntm:machine_crystallizer",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_purex",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_centrifuge",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_gascent",
        kind: Kind::GasCent,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The blast furnace (`crate::blastfurnace`): no energy; fluid ports
    // (hot air in, flue gas out) all round the core.
    BlockSpec {
        id: "ntm:machine_blast_furnace",
        kind: Kind::BlastFurnace,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // The foundry (`crate::foundry`, docs/maquinas.md): the firebox heats
    // the crucible above it; channels, tank, outlet, slagtap, mold and
    // basin come in the next stages.
    BlockSpec {
        id: "ntm:heater_firebox",
        kind: Kind::Firebox,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_crucible",
        kind: Kind::Crucible,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:foundry_channel",
        kind: Kind::Channel,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:foundry_tank",
        kind: Kind::FoundryTank,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:foundry_outlet",
        kind: Kind::Outlet,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:foundry_slagtap",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:foundry_mold",
        kind: Kind::Mold,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:foundry_basin",
        kind: Kind::Basin,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Anvils (`crate::anvil`): the tier is the block's.
    BlockSpec {
        id: "ntm:anvil_iron",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_lead",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_steel",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_desh",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_ferrouranium",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_saturnite",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_bismuth_bronze",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_arsenic_bronze",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_schrabidate",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_dnt",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_osmiridium",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:anvil_murky",
        kind: Kind::Anvil,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // The bedrock chain, stage 1 (`crate::excavator`, `crate::bedrock`).
    BlockSpec {
        id: "ntm:machine_excavator",
        kind: Kind::Excavator,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_ore_slopper",
        kind: Kind::OreSlopper,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The bedrock chain's stage 4 (`crate::arc_furnace`).
    BlockSpec {
        id: "ntm:machine_arc_furnace_large",
        kind: Kind::ArcFurnace,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The oil chain's batch A (9c504cd): coal to fluids and back
    // (`crafter::Station::Liquefactor`, `Station::Solidifier`).
    BlockSpec {
        id: "ntm:machine_liquefactor",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_solidifier",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The oil chain's batch B (6ce8965): the refinery, the fractioning
    // tower (`crate::refinery`, `crate::fraction`), the mixer (a crafter
    // station).
    BlockSpec {
        id: "ntm:machine_refinery",
        kind: Kind::Refinery,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_fraction_tower",
        kind: Kind::FractionTower,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_mixer",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The oil chain's batch C (0f061ca): boilers, cracking, reforming,
    // hydrotreating, vacuum distillation, the coker (`crate::boiler`,
    // `crate::processing`, `crate::coker`).
    BlockSpec {
        id: "ntm:machine_boiler",
        kind: Kind::Boiler,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_industrial_boiler",
        kind: Kind::Boiler,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_catalytic_cracker",
        kind: Kind::Processor,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_catalytic_reformer",
        kind: Kind::Processor,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_hydrotreater",
        kind: Kind::Processor,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_vacuum_distill",
        kind: Kind::Processor,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_coker",
        kind: Kind::Coker,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The oil chain's batch D (795e8b7): the drills (`crate::oil_drill`);
    // the transformer, inert (no logic of its own in the NTM's chain).
    BlockSpec {
        id: "ntm:machine_well",
        kind: Kind::Drill,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_pumpjack",
        kind: Kind::Drill,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_fracking_tower",
        kind: Kind::Drill,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_transformer",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // The oil chain's batch E (1675788): the Hephaestus heat exchanger and
    // the flare stack. Inert until their stage is approved.
    BlockSpec {
        id: "ntm:machine_hephaestus",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_flare",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Energy batch 1 (`crate::heater`): the heaters but the firebox. The
    // oven is a firebox with its own fuels; the electric heater's power
    // comes in at its plug (container_bridge's MULTIBLOCKS).
    BlockSpec {
        id: "ntm:heater_oven",
        kind: Kind::Firebox,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:heater_oilburner",
        kind: Kind::Heater,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:heater_electric",
        kind: Kind::Heater,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:heater_heatex",
        kind: Kind::Heater,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Energy batch 6 (3c5a6573): the battery socket and REDD, the
    // connectors and pylons (long wires), the switch and the detector.
    // Inert until their stage is approved.
    BlockSpec {
        id: "ntm:machine_battery_socket",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_battery_redd",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_connector",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_connector_super",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_pylon",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_pylon_steel",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_pylon_medium_wood",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_pylon_medium_wood_transformer",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_pylon_medium_steel",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_pylon_medium_steel_transformer",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:red_pylon_large",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:cable_switch",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:cable_detector",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Reactors R4 (c1e30d25): the Watz, its pump and its core structure.
    // Inert until their stage is approved.
    BlockSpec {
        id: "ntm:watz",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:watz_pump",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:struct_watz_core",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Reactors R3 (6fb2e04f): the PWR's controller and its casing.
    // Inert until their stage is approved.
    BlockSpec {
        id: "ntm:pwr_controller",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pwr_block__internal",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pwr_block__port",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Reactors R2 (2b74fe22): the RBMK's columns, consoles and panels.
    // Inert until their stage is approved.
    BlockSpec {
        id: "ntm:rbmk_rod",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_rod_mod",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_rod_reasim",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_rod_reasim_mod",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_control",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_control_mod",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_control_auto",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_control_reasim",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_control_reasim_auto",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_blank",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_boiler",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_reflector",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_absorber",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_moderator",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_outgasser",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_storage",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_cooler",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_heater",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_console",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_crane_console",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_display_blank",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_display",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_key_pad",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_gauge",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_numitron",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_graph",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_lever",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_indicator",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_terminal",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_autoloader",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_loader",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_steam_inlet",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:rbmk_steam_outlet",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Reactors R1 (7e023241): the ZIRNOX and the graphite pile's blocks.
    // Inert until their stage is approved.
    BlockSpec {
        id: "ntm:machine_zirnox",
        kind: Kind::Zirnox,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:zirnox_destroyed",
        kind: Kind::Zirnox,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_device__loader",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_device__vent",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_device__control",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__dummy",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__core",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__channel",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__fuel_in",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__fuel_out",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__air_in",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__air_out",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__control",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:pile_block__edge",
        kind: Kind::Pile,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Energy batch 5 (db859b71): the RTG and the radiation generator.
    // Inert until their stage is approved.
    BlockSpec {
        id: "ntm:machine_rtg_grey",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_radgen",
        kind: Kind::Inert,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Energy batch 4 (fa0862a): the solar boiler, its mirrors and the
    // Stirling engines (`crate::solar_stirling`).
    BlockSpec {
        id: "ntm:machine_solar_boiler",
        kind: Kind::SolarStirling,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:solar_mirror",
        kind: Kind::SolarStirling,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_stirling",
        kind: Kind::SolarStirling,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::Sides,
    },
    BlockSpec {
        id: "ntm:machine_stirling_steel",
        kind: Kind::SolarStirling,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::Sides,
    },
    BlockSpec {
        id: "ntm:machine_stirling_creative",
        kind: Kind::SolarStirling,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::Sides,
    },
    // Energy batch 3 (3c93924): the combustion generators
    // (`crate::combustion`); the multiblocks' power and fluid come through
    // their proxies (container_bridge).
    BlockSpec {
        id: "ntm:machine_diesel",
        kind: Kind::Combustion,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_combustion_engine",
        kind: Kind::Combustion,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_turbofan",
        kind: Kind::Combustion,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_turbinegas",
        kind: Kind::Combustion,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // Energy batch 2 (`crate::steam`): steam engines, turbines and
    // condensers. Their power and fluid come through their proxies
    // (container_bridge).
    BlockSpec {
        id: "ntm:machine_steam_engine",
        kind: Kind::Steam,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_industrial_turbine",
        kind: Kind::Steam,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_chungus",
        kind: Kind::Steam,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_condenser",
        kind: Kind::Steam,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_condenser_powered",
        kind: Kind::Steam,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The shredder (`crate::shredder`): `TilePortShapes.around`, energy only.
    BlockSpec {
        id: "ntm:machine_shredder",
        kind: Kind::Shredder,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // Fluids (`crate::fluidbox`, docs/fluidos.md): the duct is a "cable" of
    // one fluid type, the barrel and the tank store it.
    BlockSpec {
        id: "ntm:fluid_duct_neo",
        kind: Kind::Duct,
        facing: None,
        level: None,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:barrel_steel",
        kind: Kind::FluidStore,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_fluidtank",
        kind: Kind::FluidStore,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Bombs (`crate::bomb`, docs/bombas.md). The charges: redstone-lamp
    // template, so vanilla redstone sets `lit` (`BOMB_POWERED`) and the
    // server reads it. The nukes: beehive (a block entity), facing and
    // honey level; their redstone is read through the native host.
    bomb("ntm:tnt_ntm"),
    bomb("ntm:dynamite"),
    bomb("ntm:semtex"),
    bomb("ntm:c4"),
    nuke("ntm:nuke_boy"),
    nuke("ntm:nuke_gadget"),
    nuke("ntm:nuke_man"),
    nuke("ntm:nuke_mike"),
    nuke("ntm:nuke_tsar"),
    nuke("ntm:nuke_n2"),
    nuke("ntm:nuke_fstbmb"),
    // The spikes' state block (composter template): a test battery while the
    // content pack isn't installed.
    BlockSpec {
        id: "ntm:teste",
        kind: Kind::Battery,
        facing: None,
        level: Some(("level", 8)),
        ports: Ports::Sides,
    },
];

/// HE conductors (`BlockCable`): no state, the server reads the
/// neighbours. The red cable and energy batch 6's classic, paintable, gauge
/// and box, all plain conductors (the gauge's reading and the box's look
/// aside).
pub const CABLES: &[&str] = &[
    "ntm:red_cable",
    "ntm:red_cable_classic",
    "ntm:red_cable_paintable",
    "ntm:red_cable_gauge",
    "ntm:red_cable_box",
];

/// The internal block the client writes to answer the server
/// (`token`): a composter-template block, `level` 0..=8.
pub const TOKEN: &str = "ntm:logic_token";

/// A machine's fluid ports (the NTM's `TilePortShapes`, on one block): every
/// machine with tanks has one port open all round; ducts are the cables.
pub fn fluid_ports(spec: &BlockSpec, facing: Option<&str>) -> Vec<crate::energy::Port> {
    if spec.kind == Kind::Zirnox { return vec![vec![]]; }
    if spec.id == "ntm:pile_device__vent" { return facing.and_then(facing_dir).map(|d| vec![vec![d]]).unwrap_or_default(); }
    if spec.id == "ntm:machine_solar_boiler" { return vec![vec![[0,-1,0]]]; }
    match spec.kind {
        Kind::FluidStore
        | Kind::Crafter
        | Kind::GasCent
        | Kind::BlastFurnace
        | Kind::Excavator
        | Kind::OreSlopper
        | Kind::Electrolyser
        | Kind::Refinery
        | Kind::FractionTower
        | Kind::Boiler
        | Kind::Coker
        | Kind::Processor
        | Kind::Drill
        | Kind::Steam
        | Kind::Combustion => vec![crate::energy::DIRS.to_vec()],
        // The heaters' ports are on their sides (`getPorts`): the boiler on
        // top never pours into them.
        Kind::Heater => vec![
            crate::energy::DIRS
                .iter()
                .filter(|d| d[1] == 0)
                .copied()
                .collect(),
        ],
        _ => Vec::new(),
    }
}

pub fn is_duct(id: &str) -> bool {
    spec(id).is_some_and(|s| s.kind == Kind::Duct)
}

pub fn spec(id: &str) -> Option<&'static BlockSpec> {
    BLOCKS.iter().find(|b| b.id == id)
}

pub fn is_cable(id: &str) -> bool {
    CABLES.contains(&id)
}

/// Every machine block id, for the client's "did the player use a
/// machine" check.
pub fn machine_ids() -> impl Iterator<Item = &'static str> {
    BLOCKS.iter().map(|b| b.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn content(path: &str) -> Option<Value> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/");
        let text = std::fs::read_to_string(format!("{root}{path}")).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The table above against the content pack, when the pack is there.
    #[test]
    fn agrees_with_content_ids() {
        let Some(ids) = content("ids.json") else {
            eprintln!("content/ids.json missing: skipped");
            return;
        };
        let machines: Vec<&str> = ids["machines"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for m in &machines {
            let s = spec(m).unwrap_or_else(|| panic!("{m} is in ids.json, not in BLOCKS"));
            let entry = ids["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"] == *m)
                .unwrap_or_else(|| panic!("{m} has no entry"));
            let logic = &entry["logic"];
            if logic.is_object() {
                assert_eq!(logic["facing"].as_str(), s.facing, "{m} facing");
                let level = logic["level"]["prop"]
                    .as_str()
                    .map(|p| (p, logic["level"]["max"].as_u64().unwrap() as u8));
                assert_eq!(level, s.level, "{m} level");
            }
        }
        let ducts: Vec<&str> = ids["ducts"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for b in BLOCKS.iter().filter(|b| b.kind == Kind::Duct) {
            assert!(ducts.contains(&b.id), "{} not in ids.json ducts", b.id);
        }
        for b in BLOCKS
            .iter()
            .filter(|b| b.id != "ntm:teste" && !matches!(b.kind, Kind::Bomb | Kind::Duct))
        {
            assert!(
                machines.contains(&b.id),
                "{} not in ids.json machines",
                b.id
            );
        }
        // Bombs: `bombs`, the charges redstone lamps whose `lit` the logic
        // reads, the nukes beehives with no `lit`; every component their
        // screens ask for.
        let bombs: Vec<&str> = ids["bombs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let entry = |id: &str| {
            ids["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"] == id)
                .cloned()
                .unwrap_or_else(|| panic!("{id} has no entry"))
        };
        for b in BLOCKS.iter().filter(|b| b.kind == Kind::Bomb) {
            assert!(bombs.contains(&b.id), "{} not in ids.json bombs", b.id);
            let e = entry(b.id);
            if b.id.starts_with("ntm:nuke_") {
                assert_eq!(e["template"], "minecraft:beehive", "{}", b.id);
                assert!(e["logic"]["powered"].is_null(), "{}", b.id);
            } else {
                assert_eq!(e["template"], "minecraft:redstone_lamp", "{}", b.id);
                assert_eq!(e["logic"]["powered"], BOMB_POWERED, "{}", b.id);
            }
            for s in crate::bomb::bomb_type(b.id).unwrap().slots {
                assert_eq!(entry(s.item)["type"], "item", "{}", s.item);
            }
        }
        for b in &bombs {
            assert!(is_bomb(b), "{b} in ids.json bombs, not in BLOCKS");
        }
        assert_eq!(entry(crate::bomb::DETONATOR)["type"], "item");
        let internal = ids["internal_blocks"].as_array().unwrap();
        assert!(internal.iter().any(|b| b["id"] == TOKEN));
        let token = content("data/ntm/horizon/block/logic_token.json").unwrap();
        assert_eq!(token["template"], "minecraft:composter");
    }
}

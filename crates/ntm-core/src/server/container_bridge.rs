//! Machines whose items live in their block entity's slots (docs/nativo.md).
//! The slots are the truth: each tick, before the machines run, what
//! players moved in the slots since the last write is applied to the
//! machine; after the machines ran, the machine's items are written back,
//! with its menu's data.
use super::{Key, Server, ServerHost};
use crate::{
    Pos,
    crafter::Crafter,
    machine::{Data, Machine},
};
use std::collections::BTreeMap;

/// A slot's stack as the engine reports it: the item, how many and its
/// `minecraft:damage` (a blade's wear).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stack {
    pub id: String,
    pub count: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub damage: u32,
    /// The item's `minecraft:custom_data` as SNBT (`{ntm_ore:{...}}`): items
    /// with different data don't pile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl Stack {
    pub fn new(id: &str, count: u32) -> Self {
        Self {
            id: id.into(),
            count,
            damage: 0,
            custom: None,
        }
    }
}

/// The engine's slots of a machine.
pub type Slots = Vec<Option<Stack>>;

/// A machine that keeps its items as the engine's slots (new machines: no
/// transactions, nothing to reconcile). Before the machines run its slots
/// become the engine's; after, the slots it changed are written back.
pub trait Slotted {
    /// Its slots, in the order of its `horizon_machine/<block>.json`.
    fn slots_mut(&mut self) -> &mut Slots;
    /// The values its screen shows (`horizon_screen/<block>.json`).
    fn screen_data(&self) -> Vec<i32>;
    /// A button of its menu (the return value is the old screen's: true
    /// when the button opened a list there).
    fn button(&mut self, _id: u32) -> bool {
        false
    }
    /// A machine from before the slots (its items in its own fields, moved
    /// by transactions on the old screen, which an engine without slots
    /// still uses) mirrors them: `sync_in` reads the slots into the fields
    /// after the engine's are copied in, `sync_out` the fields into the
    /// slots before they are written back.
    fn sync_in(&mut self) {}
    fn sync_out(&mut self) {}
    /// Such a machine, seen with empty engine slots in a new session, keeps
    /// what its fields hold (the world's slots come from before them: a
    /// machine filled by transactions), as the assembler and the bombs do.
    fn migrates(&self) -> bool {
        false
    }
    /// What it holds outside its slots (molten metal as scraps), dropped
    /// when it is broken: the engine spills the slots, not this.
    fn held_contents(&self) -> Vec<(String, u32)> {
        Vec::new()
    }
    /// Its state outside the slots, saved with them in the engine's block
    /// entity (after the screen's values): the world's save then has the
    /// slots and the state from the same moment, so a crash between the
    /// world's save and the mod's can't have an item both in a slot and
    /// melted (or in neither).
    fn save_state(&self) -> Vec<i32> {
        Vec::new()
    }
    /// The state back from the block entity, at the first sight of it in a
    /// session.
    fn load_state(&mut self, _v: &[i32]) {}
}

/// What the machine in `data` holds outside the engine's slots.
pub(super) fn held_contents(data: &Data) -> Vec<(String, u32)> {
    let mut d = data.clone();
    slotted(&mut d).map_or_else(Vec::new, |n| n.held_contents())
}

/// The machines that implement [`Slotted`] (`Layout::Native`).
fn slotted(data: &mut Data) -> Option<&mut dyn Slotted> {
    match data {
        Data::Shredder(s) => Some(s),
        Data::BlastFurnace(b) => Some(b),
        Data::Firebox(f) => Some(f),
        Data::Crucible(c) => Some(c),
        Data::Mold(c) | Data::Basin(c) => Some(c),
        Data::Channel(c) => Some(c),
        Data::FoundryTank(c) => Some(c),
        Data::Outlet(c) => Some(c),
        Data::Anvil(a) => Some(a),
        Data::BurnerPress(p) => Some(p),
        Data::ElectricPress(p) => Some(p),
        Data::GasCent(g) => Some(g),
        Data::Excavator(e) => Some(e),
        Data::OreSlopper(o) => Some(o),
        Data::ArcFurnace(f) => Some(f),
        Data::Electrolyser(e) => Some(&mut **e),
        Data::Refinery(r) => Some(&mut **r),
        Data::Coker(c) => Some(&mut **c),
        Data::Processor(p) => Some(&mut **p),
        Data::Drill(d) => Some(&mut **d),
        Data::Heater(h) => h.slotted(),
        Data::Battery(b) => Some(b),
        Data::WoodBurner(w) => Some(w),
        Data::FluidStore(f) => Some(f),
        Data::ElectricFurnace(f) => Some(f),
        Data::Steam(s) => s.slotted(),
        Data::Combustion(c) => Some(c.slotted()),
        Data::Pile(p) if matches!(&**p, crate::pile::Pile::Loader(_)) => Some(&mut **p),
        Data::SolarStirling(s) => Some(&mut **s),
        Data::Radioisotope(r) => Some(&mut **r),
        Data::Zirnox(z) if !z.destroyed => Some(&mut **z),
        _ => None,
    }
}

/// The blocks whose machines use the engine's slots, and their layout:
/// (block, first input slot, inputs, output slot).
pub const SLOTTED: &[(&str, Layout)] = &[
    ("ntm:machine_rtg_grey", Layout::Native),
    ("ntm:machine_radgen", Layout::Native),
    ("ntm:machine_zirnox", Layout::Native),
    ("ntm:pile_device__loader", Layout::Native),
    ("ntm:machine_solar_boiler", Layout::Native),
    ("ntm:solar_mirror", Layout::Native),
    ("ntm:machine_stirling", Layout::Native),
    ("ntm:machine_stirling_steel", Layout::Native),
    ("ntm:machine_stirling_creative", Layout::Native),
    ("ntm:machine_shredder", Layout::Native),
    ("ntm:machine_blast_furnace", Layout::Native),
    ("ntm:heater_firebox", Layout::Native),
    ("ntm:heater_oven", Layout::Native),
    // The machines of the old screen only, with native screens now.
    ("ntm:machine_battery", Layout::Native),
    ("ntm:teste", Layout::Native),
    ("ntm:machine_wood_burner", Layout::Native),
    ("ntm:machine_electric_furnace_off", Layout::Native),
    ("ntm:barrel_steel", Layout::Native),
    ("ntm:machine_fluidtank", Layout::Native),
    ("ntm:heater_oilburner", Layout::Native),
    ("ntm:heater_heatex", Layout::Native),
    // Energy batch 3 (`crate::combustion`): the generators' slots are their
    // containers, upgrades, pistons and identifiers.
    ("ntm:machine_diesel", Layout::Native),
    ("ntm:machine_combustion_engine", Layout::Native),
    ("ntm:machine_turbofan", Layout::Native),
    ("ntm:machine_turbinegas", Layout::Native),
    // Energy batch 2: no slots, the models' values.
    ("ntm:machine_steam_engine", Layout::Native),
    ("ntm:machine_industrial_turbine", Layout::Native),
    ("ntm:machine_chungus", Layout::Native),
    ("ntm:machine_condenser_powered", Layout::Native),
    ("ntm:machine_crucible", Layout::Native),
    ("ntm:foundry_mold", Layout::Native),
    ("ntm:foundry_basin", Layout::Native),
    ("ntm:foundry_channel", Layout::Native),
    ("ntm:foundry_tank", Layout::Native),
    ("ntm:foundry_outlet", Layout::Native),
    ("ntm:machine_press", Layout::Native),
    ("ntm:machine_epress", Layout::Native),
    ("ntm:machine_gascent", Layout::Native),
    ("ntm:machine_excavator", Layout::Native),
    ("ntm:machine_ore_slopper", Layout::Native),
    ("ntm:anvil_iron", Layout::Native),
    ("ntm:anvil_lead", Layout::Native),
    ("ntm:anvil_steel", Layout::Native),
    ("ntm:anvil_desh", Layout::Native),
    ("ntm:anvil_ferrouranium", Layout::Native),
    ("ntm:anvil_saturnite", Layout::Native),
    ("ntm:anvil_bismuth_bronze", Layout::Native),
    ("ntm:anvil_arsenic_bronze", Layout::Native),
    ("ntm:anvil_schrabidate", Layout::Native),
    ("ntm:anvil_dnt", Layout::Native),
    ("ntm:anvil_osmiridium", Layout::Native),
    ("ntm:anvil_murky", Layout::Native),
    (
        "ntm:machine_assembly_machine",
        Layout::Crafter {
            inputs: 4..16,
            outputs: 16..17,
        },
    ),
    // HBM's other slots (battery, upgrades, fluid containers) have no logic
    // in the port: the screens leave them out.
    (
        "ntm:machine_chemical_plant",
        Layout::Crafter {
            inputs: 0..3,
            outputs: 3..6,
        },
    ),
    (
        "ntm:machine_centrifuge",
        Layout::Crafter {
            inputs: 0..1,
            outputs: 1..5,
        },
    ),
    // HBM's own indices (ContainerMachinePUREX): 0-3 are slots without logic.
    (
        "ntm:machine_purex",
        Layout::Crafter {
            inputs: 4..7,
            outputs: 7..13,
        },
    ),
    // ContainerPyroOven: 1 the input, 2 the output (0 battery, 3 the
    // identifier, 4–5 upgrades).
    (
        "ntm:machine_pyrooven",
        Layout::Crafter {
            inputs: 1..2,
            outputs: 2..3,
        },
    ),
    // ContainerLiquefactor: 0 the input, 1 the battery, 2–3 upgrades (no
    // item out).
    (
        "ntm:machine_liquefactor",
        Layout::Crafter {
            inputs: 0..1,
            outputs: 1..1,
        },
    ),
    // ContainerSolidifier: 0 the output, 1 the battery, 2–3 upgrades, 4 the
    // identifier (the tank takes the first fluid in instead).
    (
        "ntm:machine_solidifier",
        Layout::Crafter {
            inputs: 0..0,
            outputs: 0..1,
        },
    ),
    // The oil chain's batch C with screens: GUIMachineCoker (0 the
    // identifier, 1 the coke), the reformer and hydrotreater (11: 0 the
    // battery, 10 the catalyst), the vacuum distiller (12: 0 the battery).
    ("ntm:machine_coker", Layout::Native),
    // The oil drills: 0 the battery, 1–4 containers, 5–7 upgrades.
    ("ntm:machine_well", Layout::Native),
    ("ntm:machine_pumpjack", Layout::Native),
    ("ntm:machine_fracking_tower", Layout::Native),
    ("ntm:machine_catalytic_reformer", Layout::Native),
    ("ntm:machine_hydrotreater", Layout::Native),
    ("ntm:machine_vacuum_distill", Layout::Native),
    // ContainerMachineRefinery: 13 slots (`crate::refinery`).
    ("ntm:machine_refinery", Layout::Native),
    // ContainerMachineMixer: 0 the battery, 1 the item in, 2 the identifier
    // (no effect: the recipe is picked), 3–4 upgrades.
    (
        "ntm:machine_mixer",
        Layout::Crafter {
            inputs: 1..2,
            outputs: 2..2,
        },
    ),
    // ContainerElectrolyser: both lines on its 21 slots (`crate::electrolyser`).
    ("ntm:machine_electrolyser", Layout::Native),
    // ContainerMachineArcFurnaceLarge: 30 slots (`crate::arc_furnace`).
    ("ntm:machine_arc_furnace_large", Layout::Native),
    // ContainerCrystallizer: 0 the input, 2 the output (1 battery, 3–4 fluid
    // containers, 5–6 upgrades, 7 the identifier: no logic in the port).
    (
        "ntm:machine_crystallizer",
        Layout::Crafter {
            inputs: 0..1,
            outputs: 2..3,
        },
    ),
    ("ntm:nuke_boy", Layout::Bomb),
    ("ntm:nuke_gadget", Layout::Bomb),
    ("ntm:nuke_man", Layout::Bomb),
    ("ntm:nuke_mike", Layout::Bomb),
    ("ntm:nuke_tsar", Layout::Bomb),
    ("ntm:nuke_n2", Layout::Bomb),
    ("ntm:nuke_fstbmb", Layout::Bomb),
];

#[derive(Clone, Debug, PartialEq)]
pub enum Layout {
    /// HBM's processing machines: input and output slots, each a view of
    /// the machine's piles.
    Crafter {
        inputs: std::ops::Range<usize>,
        outputs: std::ops::Range<usize>,
    },
    /// A bomb: one slot per part, in the bomb type's order.
    Bomb,
    /// A machine that keeps its items as slots ([`Slotted`]).
    Native,
}

/// Multiblocks (tools/visual-gen/visual.json): the volume around the core
/// in the placer's frame (x right, z away from the player), as the engine
/// places it.
pub const MULTIBLOCKS: &[(&str, [i32; 3], [i32; 3])] = &[
    ("ntm:machine_assembly_machine", [-1, 0, -1], [1, 0, 1]),
    ("ntm:machine_chemical_plant", [-1, 0, -1], [1, 0, 1]),
    ("ntm:machine_purex", [-2, 0, -2], [2, 0, 2]),
    // MachineExcavator.getAllPorts: on the layer above its core.
    ("ntm:machine_excavator", [-3, 1, -3], [3, 1, 3]),
    // TileEntityMachineOreSlopper.getPorts: on its bottom layer.
    ("ntm:machine_ore_slopper", [-1, 0, -3], [1, 0, 3]),
    // MachineCrystallizer.fillSpace: the makeExtra ports on its bottom layer.
    ("ntm:machine_crystallizer", [-1, 0, -1], [1, 0, 1]),
    // MachineLiquefactor and MachineSolidifier: their second layer. HBM's
    // ports are its four sides and the top; a box can't leave the corners out.
    ("ntm:machine_liquefactor", [-1, 1, -1], [1, 1, 1]),
    ("ntm:machine_solidifier", [-1, 1, -1], [1, 1, 1]),
    // MachinePyroOven: its bottom layer (HBM's ports are a column of it on
    // the placer's right and one on top).
    ("ntm:machine_pyrooven", [-2, 0, -3], [2, 0, 3]),
    // MachineElectrolyser and MachineArcFurnaceLarge: their bottom layer.
    ("ntm:machine_electrolyser", [-1, 0, -5], [3, 0, 5]),
    ("ntm:machine_arc_furnace_large", [-2, 0, -2], [2, 0, 2]),
    // MachineRefinery: its bottom layer (the makeExtra corners).
    ("ntm:machine_refinery", [-1, 0, -1], [1, 0, 1]),
    // Oil batch C's electric machines: their bottom layer.
    ("ntm:machine_catalytic_reformer", [-2, 0, -1], [2, 0, 1]),
    ("ntm:machine_hydrotreater", [-1, 0, -1], [1, 0, 1]),
    ("ntm:machine_vacuum_distill", [-1, 0, -1], [1, 0, 1]),
    // MachineGasFlare: its bottom layer.
    ("ntm:machine_flare", [-1, 0, -1], [1, 0, 1]),
    // MachinePumpjack: its bottom layer.
    ("ntm:machine_pumpjack", [0, 0, -1], [6, 0, 1]),
    // HeaterElectric.getAllPorts: its plug, two cells towards the placer
    // (offset 2), and the cell between it and the core.
    ("ntm:heater_electric", [0, 0, -2], [0, 0, -1]),
];

/// Exact outward port cells, in the order of registry::ports. These do not
/// become cables: the graph keeps each cell's faces and machine owner.
pub fn energy_port_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<Pos> {
    if block == crate::radioisotopes::RADGEN {
        return facing.and_then(crate::registry::facing_dir).map(|d|vec![[core[0]-d[0]*3,core[1],core[2]-d[2]*3]]).unwrap_or_default();
    }
    if matches!(block, "ntm:machine_stirling" | "ntm:machine_stirling_steel" | "ntm:machine_stirling_creative") {
        crate::energy::DIRS.iter().filter(|d| d[1] == 0).map(|d| [core[0]+d[0], core[1], core[2]+d[2]]).collect()
    } else { Vec::new() }
}

/// The cells of the multiblock of `block` at `core` that pass energy to
/// it (HBM's `makeExtra` proxies): its bottom layer but the core.
pub fn proxy_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<Pos> {
    let Some(f) = facing.and_then(crate::registry::facing_dir) else {
        return Vec::new();
    };
    if let Some((_, cells)) = ENERGY_CHAINS.iter().find(|(b, _)| *b == block) {
        return cells.iter().map(|&l| world_cell(core, f, l)).collect();
    }
    let Some((_, min, max)) = MULTIBLOCKS.iter().find(|(b, ..)| *b == block) else {
        return Vec::new();
    };
    // The placer looked against the facing: z runs that way, x to their right.
    let mut out = Vec::new();
    for z in min[2]..=max[2] {
        for x in min[0]..=max[0] {
            if (x, z) != (0, 0) {
                out.push(world_cell(core, f, [x, min[1], z]));
            }
        }
    }
    out
}

/// Multiblocks' fluid proxies (HBM's `getAllPorts` and `makeExtra`): the
/// cells, in the placer's frame (x right, y up, z away from the player),
/// that join the core's fluid networks as ducts. A chain of them reaches the
/// core: the blast furnace's ports up its front reach it down its column.
pub const FLUID_PROXIES: &[(&str, &[[i32; 3]])] = &[
    ("ntm:machine_solar_boiler", &[[0,2,0]]),
    // MachineExcavator.getAllPorts: (±1, 1, 3) and (±3, 1, 0). The whole
    // layer above the core holds them and reaches the core through the cell
    // over it.
    (
        "ntm:machine_excavator",
        &[
            [-3, 1, -3],
            [-2, 1, -3],
            [-1, 1, -3],
            [0, 1, -3],
            [1, 1, -3],
            [2, 1, -3],
            [3, 1, -3],
            [-3, 1, -2],
            [-2, 1, -2],
            [-1, 1, -2],
            [0, 1, -2],
            [1, 1, -2],
            [2, 1, -2],
            [3, 1, -2],
            [-3, 1, -1],
            [-2, 1, -1],
            [-1, 1, -1],
            [0, 1, -1],
            [1, 1, -1],
            [2, 1, -1],
            [3, 1, -1],
            [-3, 1, 0],
            [-2, 1, 0],
            [-1, 1, 0],
            [0, 1, 0],
            [1, 1, 0],
            [2, 1, 0],
            [3, 1, 0],
            [-3, 1, 1],
            [-2, 1, 1],
            [-1, 1, 1],
            [0, 1, 1],
            [1, 1, 1],
            [2, 1, 1],
            [3, 1, 1],
            [-3, 1, 2],
            [-2, 1, 2],
            [-1, 1, 2],
            [0, 1, 2],
            [1, 1, 2],
            [2, 1, 2],
            [3, 1, 2],
            [-3, 1, 3],
            [-2, 1, 3],
            [-1, 1, 3],
            [0, 1, 3],
            [1, 1, 3],
            [2, 1, 3],
            [3, 1, 3],
        ],
    ),
    // TileEntityMachineOreSlopper.getPorts: eight cells of its bottom layer,
    // which reaches the core.
    (
        "ntm:machine_ore_slopper",
        &[
            [-1, 0, -3],
            [0, 0, -3],
            [1, 0, -3],
            [-1, 0, -2],
            [0, 0, -2],
            [1, 0, -2],
            [-1, 0, -1],
            [0, 0, -1],
            [1, 0, -1],
            [-1, 0, 0],
            [1, 0, 0],
            [-1, 0, 1],
            [0, 0, 1],
            [1, 0, 1],
            [-1, 0, 2],
            [0, 0, 2],
            [1, 0, 2],
            [-1, 0, 3],
            [0, 0, 3],
            [1, 0, 3],
        ],
    ),
    // TilePortShapes.purex: the edge of its bottom layer.
    (
        "ntm:machine_purex",
        &[
            [-2, 0, -2],
            [-2, 0, -1],
            [-2, 0, 0],
            [-2, 0, 1],
            [-2, 0, 2],
            [2, 0, -2],
            [2, 0, -1],
            [2, 0, 0],
            [2, 0, 1],
            [2, 0, 2],
            [-1, 0, -2],
            [0, 0, -2],
            [1, 0, -2],
            [-1, 0, 2],
            [0, 0, 2],
            [1, 0, 2],
        ],
    ),
    // MachineCrystallizer.getAllPorts: the four corners of its bottom layer,
    // which reach the core through the cells between them.
    (
        "ntm:machine_crystallizer",
        &[[-1, 0, -1], [-1, 0, 0], [-1, 0, 1], [0, 0, -1], [0, 0, 1], [1, 0, -1], [1, 0, 0], [1, 0, 1]],
    ),
    // getAllPorts of MachineLiquefactor and MachineSolidifier: the top, and
    // the four sides of the second layer, each reaching the core down the
    // centre column.
    (
        "ntm:machine_liquefactor",
        &[[0, 1, 0], [0, 2, 0], [0, 3, 0], [1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1]],
    ),
    (
        "ntm:machine_solidifier",
        &[[0, 1, 0], [0, 2, 0], [0, 3, 0], [1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1]],
    ),
    // MachinePyroOven.getAllPorts, (-x, y, -z) here: rotatePorts turns HBM's
    // x to the placer's left and z to them. (-2, 0, -2..2) is the column on
    // their right, reaching the core through (1, 0, 0); (1, 2, 0) is on top,
    // down through (-1, 1, 0) and (-1, 0, 0).
    (
        "ntm:machine_pyrooven",
        &[
            [2, 0, -2],
            [2, 0, -1],
            [2, 0, 0],
            [2, 0, 1],
            [2, 0, 2],
            [1, 0, 0],
            [-1, 2, 0],
            [-1, 1, 0],
            [-1, 0, 0],
        ],
    ),
    // MachineElectrolyser.getAllPorts, (-x, y, -z) here: three at each end,
    // five down its left edge (x -1) and five down its right one (x 3).
    // The left edge's row and the right one's reach the core, the right
    // through (2, 0, 0) and (1, 0, 0).
    (
        "ntm:machine_electrolyser",
        &[
            [-1, 0, -5],
            [-1, 0, -4],
            [-1, 0, -3],
            [-1, 0, -2],
            [-1, 0, -1],
            [-1, 0, 0],
            [-1, 0, 1],
            [-1, 0, 2],
            [-1, 0, 3],
            [-1, 0, 4],
            [-1, 0, 5],
            [0, 0, -5],
            [1, 0, -5],
            [0, 0, 5],
            [1, 0, 5],
            [3, 0, -4],
            [3, 0, -3],
            [3, 0, -2],
            [3, 0, -1],
            [3, 0, 0],
            [3, 0, 1],
            [3, 0, 2],
            [3, 0, 3],
            [3, 0, 4],
            [2, 0, 0],
            [1, 0, 0],
        ],
    ),
    // MachineRefinery.getAllPorts: the four corners of its bottom layer,
    // which reach the core through the cells between them.
    (
        "ntm:machine_refinery",
        &[[-1, 0, -1], [-1, 0, 0], [-1, 0, 1], [0, 0, -1], [0, 0, 1], [1, 0, -1], [1, 0, 0], [1, 0, 1]],
    ),
    // MachineFractionTower.getAllPorts: the middle of each side, next to the core.
    ("ntm:machine_fraction_tower", &[[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]),
    // Oil batch C. MachineCatalyticCracker.getAllPorts, at (-x, 0, -z) here,
    // are eight cells on the edge of its bottom layer; that layer carries them
    // to the core.
    (
        "ntm:machine_catalytic_cracker",
        &[
            [-2, 0, -3],
            [-1, 0, -3],
            [0, 0, -3],
            [1, 0, -3],
            [2, 0, -3],
            [3, 0, -3],
            [-2, 0, -2],
            [-1, 0, -2],
            [0, 0, -2],
            [1, 0, -2],
            [2, 0, -2],
            [3, 0, -2],
            [-2, 0, -1],
            [-1, 0, -1],
            [0, 0, -1],
            [1, 0, -1],
            [2, 0, -1],
            [3, 0, -1],
            [-2, 0, 0],
            [-1, 0, 0],
            [1, 0, 0],
            [2, 0, 0],
            [3, 0, 0],
            [-2, 0, 1],
            [-1, 0, 1],
            [0, 0, 1],
            [1, 0, 1],
            [2, 0, 1],
            [3, 0, 1],
            [-2, 0, 2],
            [-1, 0, 2],
            [0, 0, 2],
            [1, 0, 2],
            [2, 0, 2],
            [3, 0, 2],
            [-2, 0, 3],
            [-1, 0, 3],
            [0, 0, 3],
            [1, 0, 3],
            [2, 0, 3],
            [3, 0, 3],
        ],
    ),
    // MachineCatalyticReformer.getAllPorts: on its bottom layer, which
    // carries them to the core.
    (
        "ntm:machine_catalytic_reformer",
        &[[-2, 0, -1], [-1, 0, -1], [0, 0, -1], [1, 0, -1], [2, 0, -1], [-2, 0, 0], [-1, 0, 0], [1, 0, 0], [2, 0, 0], [-2, 0, 1], [-1, 0, 1], [0, 0, 1], [1, 0, 1], [2, 0, 1]],
    ),
    // The hydrotreater's, vacuum distiller's and coker's makeExtra corners,
    // chained to the core by their bottom ring.
    ("ntm:machine_hydrotreater", &[[-1, 0, -1], [-1, 0, 0], [-1, 0, 1], [0, 0, -1], [0, 0, 1], [1, 0, -1], [1, 0, 0], [1, 0, 1]]),
    ("ntm:machine_vacuum_distill", &[[-1, 0, -1], [-1, 0, 0], [-1, 0, 1], [0, 0, -1], [0, 0, 1], [1, 0, -1], [1, 0, 0], [1, 0, 1]]),
    ("ntm:machine_coker", &[[-1, 0, -1], [-1, 0, 0], [-1, 0, 1], [0, 0, -1], [0, 0, 1], [1, 0, -1], [1, 0, 0], [1, 0, 1]]),
    // MachineHeatBoiler.getAllPorts: its two sides, and the top down the
    // centre column; the industrial boiler's four sides and its top.
    (
        "ntm:machine_boiler",
        &[[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, 2, 0], [0, 3, 0]],
    ),
    (
        "ntm:machine_industrial_boiler",
        &[[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, 2, 0], [0, 3, 0], [0, 4, 0]],
    ),
    // MachineGasFlare.getAllPorts: the middle of each side, next to the core.
    ("ntm:machine_flare", &[[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]),
    // MachineHephaestus.getAllPorts: the middle of each side at the bottom
    // and at the top (y 11), joined by the centre column.
    (
        "ntm:machine_hephaestus",
        &[
            [1, 0, 0],
            [-1, 0, 0],
            [0, 0, 1],
            [0, 0, -1],
            [0, 1, 0], [0, 2, 0], [0, 3, 0], [0, 4, 0], [0, 5, 0], [0, 6, 0],
            [0, 7, 0], [0, 8, 0], [0, 9, 0], [0, 10, 0], [0, 11, 0],
            [1, 11, 0],
            [-1, 11, 0],
            [0, 11, 1],
            [0, 11, -1],
        ],
    ),
    // MachinePumpjack.getAllPorts, at (-x, 0, -z) here: two each side of its
    // base, reaching the core along it. The oil well's and fracking tower's
    // ports are their core.
    (
        "ntm:machine_pumpjack",
        &[[2, 0, -1], [4, 0, -1], [2, 0, 1], [4, 0, 1], [1, 0, 0], [2, 0, 0], [3, 0, 0], [4, 0, 0]],
    ),
    // MachineChemicalPlant.getAllPorts: its bottom ring.
    (
        "ntm:machine_chemical_plant",
        &[[-1, 0, -1], [-1, 0, 0], [-1, 0, 1], [0, 0, -1], [0, 0, 1], [1, 0, -1], [1, 0, 0], [1, 0, 1]],
    ),
    (
    "ntm:machine_blast_furnace",
    &[
        [1, 0, 0],
        [-1, 0, 0],
        [0, 0, 1],
        [0, 0, -1],
        [0, 1, 0],
        [0, 2, 0],
        [0, 3, 0],
        [0, 4, 0],
        [0, 5, 0],
        [0, 6, 0],
        [0, 3, 1],
        [0, 3, -1],
        [0, 5, 1],
        [0, 5, -1],
    ],
    ),
    // HeaterOilburner.getAllPorts (`TilePortShapes.flare`): the four sides
    // of its bottom layer.
    ("ntm:heater_oilburner", &[[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]),
    // TileEntityHeaterHeatex.getPorts: its four corners (the NTM's connect
    // front and back only).
    ("ntm:heater_heatex", &[[1, 0, 1], [-1, 0, 1], [1, 0, -1], [-1, 0, -1]]),
    // Energy batch 2's ports (HBM's `dir` is towards the placer, -z here;
    // `rot`, its turn up, is -x). TileEntitySteamEngine: three on its left,
    // a layer up.
    ("ntm:machine_steam_engine", &[[-1, 1, -1], [-1, 1, 0], [-1, 1, 1]]),
    // TileEntityMachineIndustrialTurbine.getFluidPorts.
    ("ntm:machine_industrial_turbine", &[[-1, 0, -3], [1, 0, -3], [-1, 0, 1], [1, 0, 1], [0, 2, -3], [0, 2, 1]]),
    // TileEntityChungus.getFluidPorts.
    ("ntm:machine_chungus", &[[0, 2, -4], [-2, 0, 0], [2, 0, 0]]),
    // TilePortShapes.condenser: six a layer up.
    ("ntm:machine_condenser_powered", &[[-3, 1, 0], [3, 1, 0], [1, 1, -1], [-1, 1, -1], [1, 1, 1], [-1, 1, 1]]),
    // Energy batch 3. TileEntityMachineCombustionEngine.getPorts: both sides
    // of its core and of the row behind it.
    ("ntm:machine_combustion_engine", &[[-1, 0, 0], [1, 0, 0], [-1, 0, 1], [1, 0, 1]]),
    // TileEntityMachineTurbofan.getPorts: a connector on each side, and the
    // one next to it towards the intake.
    ("ntm:machine_turbofan", &[[0, 0, -1], [1, 0, -1], [0, 0, 1], [1, 0, 1]]),
    // TileEntityMachineTurbineGas.getPorts: two each side of its middle
    // (the corners of the row before and after the core, and four out),
    // and the end port on the layer above.
    (
        "ntm:machine_turbinegas",
        &[[-1, 0, 0], [-1, 0, -1], [-1, 0, 1], [1, 0, 0], [2, 0, 0], [3, 0, 0], [4, 0, 0], [4, 0, -1], [4, 0, 1], [5, 0, 0], [5, 1, 0]],
    ),
];

/// Multiblocks' power ports that MULTIBLOCKS' one-layer box can't hold:
/// each a chain of cells from the one next to the core to the port, in the
/// placer's frame, as energy proxies.
pub const ENERGY_CHAINS: &[(&str, &[[i32; 3]])] = &[
    // TileEntitySteamEngine.getPorts: power and fluid at the same three.
    ("ntm:machine_steam_engine", &[[0, 1, 0], [-1, 1, 0], [-1, 1, -1], [-1, 1, 1]]),
    // getPowerPorts: at the back, a layer up.
    ("ntm:machine_industrial_turbine", &[[0, 1, 0], [0, 1, 1], [0, 1, 2], [0, 1, 3]]),
    // getPowerPorts: ten back, on its bottom layer.
    (
        "ntm:machine_chungus",
        &[[0, 0, 1], [0, 0, 2], [0, 0, 3], [0, 0, 4], [0, 0, 5], [0, 0, 6], [0, 0, 7], [0, 0, 8], [0, 0, 9], [0, 0, 10]],
    ),
    // The combustion engine's and the turbofan's power ports are their fluid
    // ports; the gas turbine's (`getPowerPorts`) is four out on the layer above.
    ("ntm:machine_combustion_engine", &[[-1, 0, 0], [1, 0, 0], [-1, 0, 1], [1, 0, 1]]),
    ("ntm:machine_turbofan", &[[0, 0, -1], [1, 0, -1], [0, 0, 1], [1, 0, 1]]),
    ("ntm:machine_turbinegas", &[[0, 1, 0], [-1, 1, 0], [-2, 1, 0], [-3, 1, 0], [-4, 1, 0]]),
    // TilePortShapes.condenser: the same six as its fluid.
    (
        "ntm:machine_condenser_powered",
        &[[0, 1, 0], [-1, 1, 0], [-2, 1, 0], [-3, 1, 0], [1, 1, 0], [2, 1, 0], [3, 1, 0], [1, 1, -1], [-1, 1, -1], [1, 1, 1], [-1, 1, 1]],
    ),
];

/// Cell `l` (placer's frame) of the multiblock with its core at `core`.
pub fn world_cell(core: Pos, facing: [i32; 3], l: [i32; 3]) -> Pos {
    let fwd = [-facing[0], -facing[2]];
    let right = [-fwd[1], fwd[0]];
    [
        core[0] + right[0] * l[0] + fwd[0] * l[2],
        core[1] + l[1],
        core[2] + right[1] * l[0] + fwd[1] * l[2],
    ]
}

/// The cells that carry the fluids of the multiblock of `block` at `core`.
pub fn fluid_proxy_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<Pos> {
    if block == crate::zirnox::BLOCK { return crate::zirnox::Zirnox::port_cells(core,facing).into_iter().map(|(p,_)|p).collect(); }
    let Some((_, cells)) = FLUID_PROXIES.iter().find(|(b, _)| *b == block) else {
        return Vec::new();
    };
    let Some(f) = facing.and_then(crate::registry::facing_dir) else {
        return Vec::new();
    };
    cells.iter().map(|&l| world_cell(core, f, l)).collect()
}

pub fn layout(block: &str) -> Option<&'static Layout> {
    SLOTTED.iter().find(|(b, _)| *b == block).map(|(_, l)| l)
}

/// What the bridge last wrote to (or read from) each machine's slots.
#[derive(Clone, Debug, Default)]
pub struct Bridge {
    pub(super) last: BTreeMap<Key, Slots>,
    pub(super) data: BTreeMap<Key, Vec<i32>>,

}

fn totals<'a>(slots: impl Iterator<Item = &'a Option<Stack>>) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for s in slots.flatten() {
        *out.entry(s.id.clone()).or_insert(0) += i64::from(s.count);
    }
    out
}

fn add(list: &mut Vec<(String, u32)>, id: &str, d: i64) {
    match list.iter_mut().find(|(i, _)| i == id) {
        Some(e) => e.1 = (i64::from(e.1) + d).max(0) as u32,
        None if d > 0 => list.push((id.into(), d as u32)),
        None => {}
    }
    list.retain(|e| e.1 > 0);
}

/// What players did to the slots between `last` and `now`, applied to `list`.
fn apply_delta(list: &mut Vec<(String, u32)>, last: &BTreeMap<String, i64>, now: &BTreeMap<String, i64>) {
    let ids: std::collections::BTreeSet<&String> = last.keys().chain(now.keys()).collect();
    for id in ids {
        let d = now.get(id).copied().unwrap_or(0) - last.get(id).copied().unwrap_or(0);
        if d != 0 {
            add(list, id, d);
        }
    }
}

/// `want` laid over the slots `range` of `slots`: each slot keeps its item
/// while there is some of it left, the rest fills the empty slots.
pub(crate) fn lay_out(slots: &mut Slots, range: std::ops::Range<usize>, want: &[(String, u32)], max: u32) {
    let mut left: Vec<(String, u32)> = want.to_vec();
    for s in range.clone() {
        if let Some(stack) = &slots[s] {
            let have = left.iter_mut().find(|(i, _)| *i == stack.id).map_or(0, |e| {
                let t = e.1.min(max);
                e.1 -= t;
                t
            });
            slots[s] = (have > 0).then(|| Stack {
                count: have,
                ..stack.clone()
            });
        }
    }
    for s in range {
        if slots[s].is_some() {
            continue;
        }
        let Some(e) = left.iter_mut().find(|e| e.1 > 0) else {
            break;
        };
        let t = e.1.min(max);
        e.1 -= t;
        slots[s] = Some(Stack::new(&e.0, t));
    }
}

/// The values a bomb's screen shows (tools/visual-gen/visual.json): the
/// stages it could set off (Little Boy's lamp; Ivy Mike's and the Tsar's
/// two, as `isReady` and `isFilled`), and its parts in place (the N² bar).
fn bomb_data(b: &crate::bomb::Bomb) -> Vec<i32> {
    let Some(t) = b.kind() else { return vec![0, 0] };
    let stages = t.stages.iter().filter(|s| (0..s.slots).all(|i| b.slot_ok(i))).count();
    let parts = (0..t.slots.len()).filter(|&i| b.slot_ok(i)).count();
    vec![stages as i32, parts as i32]
}

/// The values the assembly machine's screen shows (tools/visual-gen/visual.json).
/// The index of a crafter's menu data with its recipe + 1 (0: none), in
/// its station's table (`crate::native::recipes`).
pub const CRAFTER_RECIPE: usize = 30;

pub(crate) fn crafter_data(c: &Crafter) -> Vec<i32> {
    let clamp = |v: u64| v.min(i32::MAX as u64) as i32;
    let recipe = c.current();
    let progress = recipe.map_or(0, |r| (u64::from(c.progress) * 1000 / u64::from(r.duration.max(1))) as i32);
    let tank = |t: Option<&crate::fluid::Tank>| match t {
        Some(t) => [
            t.mb as i32,
            t.cap as i32,
            crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32),
        ],
        None => [0, 0, 0],
    };
    let [ia, ic, icol] = tank(c.tin.first());
    let [oa, oc, ocol] = tank(c.tout.first());
    let led = match recipe {
        Some(_) if c.progress > 0 => 2,
        Some(_) => 1,
        None => 0,
    };
    let power_ok = recipe.is_some_and(|r| c.power >= r.power) as i32;
    vec![
        clamp(c.power),
        clamp(c.max_power()),
        progress,
        1000,
        ia,
        ic,
        icol,
        oa,
        oc,
        ocol,
        led,
        power_ok,
    ]
    .into_iter()
    // Then every tank, three in and three out (the chemical plant's).
    .chain((0..3).flat_map(|i| tank(c.tin.get(i))))
    .chain((0..3).flat_map(|i| tank(c.tout.get(i))))
    .chain(std::iter::once(recipe.and_then(|r| c.station.recipes().iter().position(|x| x.name == r.name)).map_or(0, |i| i as i32 + 1)))
    .collect()
}

impl Server {
    /// Before the machines run: what players moved in the slots.
    pub(super) fn slots_in(&mut self, host: &mut impl ServerHost) {
        let presses = host.machine_buttons();
        let Some(r) = self.ready() else { return };
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| layout(&m.block).is_some() && r.loaded.get(*k) != Some(&false))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            let Some(mut now) = host.machine_slots(&key.0, key.1) else {
                continue;
            };
            // Item data as the engine writes it back, canonical: compared
            // and piled as the same data the port wrote.
            for s in now.iter_mut().flatten() {
                if let Some(c) = s.custom.as_deref().and_then(crate::snbt::canonical) {
                    s.custom = Some(c);
                }
            }
            let Some(m) = r.world.machines.get_mut(&key) else { continue };
            let Some(layout) = layout(&m.block) else { continue };
            let first = !r.bridge.last.contains_key(&key);
            let observed = now.clone();
            if r.restored.remove(&key) {
                // The model came from the journal and the block entity is older: the model's slots are the truth, and
                // the next step writes them over the block entity's.
                r.bridge.last.insert(key, observed);
                continue;
            }
            let last = r.bridge.last.get(&key).cloned().unwrap_or_else(|| now.clone());
            let empty = now.iter().all(Option::is_none);
            match (&mut m.data, layout) {
                (Data::Crafter(c), Layout::Crafter { inputs, outputs }) => {
                    // Its battery slot isn't one of the machine's: the
                    // creative battery there keeps it full.
                    if let Some(i) = c.station.battery_slot() {
                        c.power = crate::machine::charge_from_slot(&now, i, c.power, c.max_power());
                    }
                    if first && !empty {
                        // A new session: the slots, saved with the world, are
                        // the truth; a machine whose slots are empty keeps what
                        // it had from transactions, written to them next.
                        c.items = totals(now[inputs.clone()].iter())
                            .into_iter()
                            .map(|(i, n)| (i, n as u32))
                            .collect();
                        // Outputs of other items than the slots' stay.
                        let shown = totals(now[outputs.clone()].iter());
                        c.out.retain(|(i, _)| !shown.contains_key(i));
                        c.out.extend(shown.into_iter().map(|(i, n)| (i, n as u32)));
                    } else {
                        apply_delta(&mut c.items, &totals(last[inputs.clone()].iter()), &totals(now[inputs.clone()].iter()));
                        apply_delta(&mut c.out, &totals(last[outputs.clone()].iter()), &totals(now[outputs.clone()].iter()));
                    }
                    c.pick_by_items();
                }
                (Data::Bomb(b), Layout::Bomb) => {
                    if !(first && empty) {
                        // The slots are the truth; on a new session with empty
                        // slots, the parts it had from transactions go to them.
                        for (i, s) in b.slots.iter_mut().enumerate() {
                            *s = now.get(i).cloned().flatten().map(|s| s.id);
                        }
                    }
                }
                (data, Layout::Native) => {
                    let Some(n) = slotted(data) else { continue };
                    if first
                        && let Some(saved) = host.machine_data(&key.0, key.1)
                    {
                        // The world's save: its state with its slots.
                        let at = n.screen_data().len();
                        if saved.len() > at {
                            n.load_state(&saved[at..]);
                        }
                    }
                    if !(first && empty && n.migrates()) {
                        *n.slots_mut() = now.clone();
                        n.sync_in();
                    }
                }
                _ => continue,
            }
            r.bridge.last.insert(key, observed);
        }
        for (_, dim, pos, button) in presses {
            self.machine_button(&(dim, pos), button);
        }
    }

    /// Derive slot writes only inside the speculative local step. `sync_out`
    /// may create items, so network persistence must never call this helper.
    pub(super) fn machine_output(m: &mut Machine, last: &Slots) -> (Slots, Vec<i32>) {
        let Some(layout) = layout(&m.block) else { return (vec![None; last.len()], Vec::new()) };
        let mut want = last.clone();
        let data = match (&mut m.data, layout) {
                (Data::Crafter(c), Layout::Crafter { inputs, outputs }) => {
                    lay_out(&mut want, inputs.clone(), &c.items, crate::crafter::STACK);
                    // The outputs over their slots; what they can't show stays
                    // in the machine (its old screen gives it).
                    lay_out(&mut want, outputs.clone(), &c.out, crate::crafter::STACK);
                    crafter_data(c)
                }
                (Data::Bomb(b), Layout::Bomb) => {
                    for (i, s) in b.slots.iter().enumerate() {
                        if let Some(cell) = want.get_mut(i) {
                            // A part keeps the damage its slot gave it.
                            let damage = cell.as_ref().filter(|c| Some(&c.id) == s.as_ref()).map_or(0, |c| c.damage);
                            *cell = s.clone().map(|id| Stack { id, count: 1, damage, custom: None });
                        }
                    }
                    bomb_data(b)
                }
                (data, Layout::Native) => {
                    let Some(n) = slotted(data) else { return (vec![None; last.len()], Vec::new()) };
                    n.sync_out();
                    want = n.slots_mut().clone();
                    let mut v = n.screen_data();
                    v.extend(n.save_state());
                    v
                }
                _ => return (last.clone(), Vec::new()),
            };
        (want, data)
    }

    pub(super) fn machine_screen(m: &mut Machine) -> Vec<i32> {
        match &mut m.data {
            Data::Crafter(c) => crafter_data(c),
            Data::Bomb(b) => bomb_data(b),
            d => slotted(d).map_or_else(Vec::new, |n| {
                let mut data = n.screen_data();
                data.extend(n.save_state());
                data
            }),
        }
    }

    /// The machine at `key` went away: true when its items were the
    /// engine's slots, which the engine drops itself.
    pub(super) fn slots_gone(&mut self, key: &Key) -> bool {
        let Some(r) = self.ready() else { return false };
        r.bridge.data.remove(key);
        r.bridge.last.remove(key).is_some()
    }

    /// A menu button: the machine's own, or a recipe picked from the
    /// list (`crate::native::recipes`).
    pub(super) fn machine_button(&mut self, key: &Key, button: u32) {
        let Some(r) = self.ready() else { return };
        let Some(m) = r.world.machines.get_mut(key) else { return };
        if let Some(n) = slotted(&mut m.data) {
            n.button(button);
            return;
        }
        if let Data::Crafter(c) = &mut m.data
            && let Some(pick) = crate::native::recipe_button(button)
            && crate::native::recipes(&m.block).is_some()
        {
            match pick.and_then(|i| c.station.recipes().get(i)) {
                Some(r) => c.set_recipe(r),
                None if pick.is_none() => {
                    c.recipe = None;
                    c.progress = 0;
                }
                None => {}
            }
        }
    }
}

/// The slots a bomb's parts take (one each), for tests.
pub fn bomb_slots(parts: &[Option<&str>]) -> Slots {
    parts.iter().map(|p| p.map(|id| Stack::new(id, 1))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_keeps_slots_and_fills_holes() {
        let a = |n| Some(Stack::new("a", n));
        let mut slots: Slots = vec![None, Some(Stack { damage: 7, ..Stack::new("a", 5) }), None];
        lay_out(&mut slots, 0..3, &[("a".into(), 3), ("b".into(), 2)], 64);
        assert_eq!(
            slots,
            vec![Some(Stack::new("b", 2)), Some(Stack { damage: 7, ..Stack::new("a", 3) }), None]
        );
        let mut slots: Slots = vec![a(5), None];
        lay_out(&mut slots, 0..2, &[("a".into(), 100)], 64);
        assert_eq!(slots, vec![a(64), a(36)]);
    }

    #[test]
    fn proxies_turn_with_the_facing() {
        // Facing north, the placer looked south: their right is west.
        assert_eq!(world_cell([0, 64, 0], [0, 0, -1], [1, 0, 0]), [-1, 64, 0]);
        assert_eq!(world_cell([0, 64, 0], [0, 0, -1], [0, 3, 1]), [0, 67, 1]);
        let cells = fluid_proxy_cells("ntm:machine_blast_furnace", [0, 64, 0], Some("east"));
        assert_eq!(cells.len(), 14);
        assert!(cells.contains(&[0, 70, 0]));
        assert!(proxy_cells("ntm:machine_assembly_machine", [0, 64, 0], Some("north")).len() == 8);
        // The crystallizer's corners chain to its core through the ring.
        let ring = fluid_proxy_cells("ntm:machine_crystallizer", [0, 64, 0], Some("west"));
        assert_eq!(ring.len(), 8);
        assert!(ring.contains(&[1, 64, 1]) && ring.contains(&[0, 64, 1]));
        assert_eq!(proxy_cells("ntm:machine_crystallizer", [0, 64, 0], Some("west")).len(), 8);
        // The liquefactor's top port reaches its core down the centre column.
        let column = fluid_proxy_cells("ntm:machine_liquefactor", [0, 64, 0], Some("north"));
        assert!([65, 66, 67].iter().all(|&y| column.contains(&[0, y, 0])));
        assert_eq!(proxy_cells("ntm:machine_solidifier", [0, 64, 0], Some("south")).len(), 8);
        // Facing north, the placer looked south: the oven's port column on
        // their right is west of the core, its top port east.
        let oven = fluid_proxy_cells("ntm:machine_pyrooven", [0, 64, 0], Some("north"));
        assert!((-2..=2).all(|z| oven.contains(&[-2, 64, z])));
        assert!(oven.contains(&[1, 66, 0]));
        // The electrolyser's right-hand ports are three cells from its core.
        let cell = fluid_proxy_cells("ntm:machine_electrolyser", [0, 64, 0], Some("north"));
        assert!(cell.contains(&[-3, 64, 4]) && cell.contains(&[1, 64, -5]));
    }

    #[test]
    fn a_bomb_screen_counts_stages_and_parts() {
        let mut b = crate::bomb::Bomb::new("ntm:nuke_mike");
        assert_eq!(bomb_data(&b), [0, 0]);
        for i in 0..4 {
            b.slots[i] = Some("ntm:explosive_lenses".into());
        }
        // A part in the wrong slot counts for nothing.
        b.slots[6] = Some("ntm:explosive_lenses".into());
        assert_eq!(bomb_data(&b), [0, 4]);
        b.slots[4] = Some("ntm:man_core".into());
        assert_eq!(bomb_data(&b), [1, 5]);
        for (i, id) in [(5, "ntm:mike_core"), (6, "ntm:mike_deut"), (7, "ntm:mike_cooling_unit")] {
            b.slots[i] = Some(id.into());
        }
        assert_eq!(bomb_data(&b), [2, 8]);
    }

    #[test]
    fn a_delta_moves_only_what_players_moved() {
        let mut list = vec![("a".to_string(), 10)];
        let last = totals([Some(Stack::new("a", 4))].iter());
        let now = totals([Some(Stack::new("a", 1)), Some(Stack::new("b", 2))].iter());
        apply_delta(&mut list, &last, &now);
        assert_eq!(list, vec![("a".to_string(), 7), ("b".to_string(), 2)]);
    }
}

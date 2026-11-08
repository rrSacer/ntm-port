//! Pressurized water reactor: fuels, structure scan, controller tick, coolants and moderators.
//!
//! Sources (HBM pin 4f5924e): `ItemPWRFuel`, `TileEntityPWRController`, `MachinePWRController`,
//! `BlockPWR`, `FT_PWRModerator`, and the PWR traits registered in Fluids.java.
//!
//! **Constructor quirk kept on purpose.** `EnumPWRFuel` has a constructor that takes a yield, but
//! it never assigns it: every fuel, including `BFB_AM_MIX` and `BFB_PU241` (declared with
//! 250 000 000), keeps the field initializer 1 000 000 000. [`PwrFuel::declared_yield`] records
//! the ignored declaration; [`PwrFuel::yield_`] is the value the reactor really uses.
use super::function::Function;
use super::heatable::heatable;
use std::collections::HashMap;

/// `EnumPWRFuel`, in source order (the item metadata).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PwrFuel {
    Meu, Heu233, Heu235, Men, Hen237, Mox, Mep, Hep239, Hep241, Mea, Hea242, Hes326, Hes327, BfbAmMix, BfbPu241,
}

impl PwrFuel {
    pub const ALL: [PwrFuel; 15] = [
        PwrFuel::Meu, PwrFuel::Heu233, PwrFuel::Heu235, PwrFuel::Men, PwrFuel::Hen237, PwrFuel::Mox, PwrFuel::Mep,
        PwrFuel::Hep239, PwrFuel::Hep241, PwrFuel::Mea, PwrFuel::Hea242, PwrFuel::Hes326, PwrFuel::Hes327,
        PwrFuel::BfbAmMix, PwrFuel::BfbPu241,
    ];

    /// `EnumUtil.grabEnumSafely`
    pub fn from_meta(meta: i32) -> PwrFuel {
        Self::ALL[(meta % 15).unsigned_abs() as usize]
    }

    /// Heat per flux, in TU.
    pub fn heat_emission(self) -> f64 {
        use PwrFuel::*;
        match self {
            Meu => 5.0,
            Heu233 | Heu235 | Men | Hen237 | Mox | Mep | Mea => 7.5,
            Hep239 | Hep241 | Hea242 => 10.0,
            Hes326 | Hes327 => 12.5,
            BfbAmMix | BfbPu241 => 2.5,
        }
    }

    pub fn function(self) -> Function {
        use PwrFuel::*;
        let log = |level: f64| Function::logarithmic(level).with_div(2_500.0);
        match self {
            Meu => log(20.0 * 30.0),
            Heu233 => Function::sqrt(25.0),
            Heu235 => Function::sqrt(22.5),
            Men => log(22.5 * 30.0),
            Hen237 => Function::sqrt(27.5),
            Mox => log(20.0 * 30.0),
            Mep => log(22.5 * 30.0),
            Hep239 => Function::sqrt(22.5),
            Hep241 => Function::sqrt(25.0),
            Mea => log(25.0 * 30.0),
            Hea242 => Function::sqrt(25.0),
            Hes326 => Function::sqrt(27.5),
            Hes327 => Function::sqrt(30.0),
            BfbAmMix | BfbPu241 => Function::sqrt(15.0),
        }
    }

    /// The yield written in the enum declaration, which the Java constructor drops.
    pub fn declared_yield(self) -> Option<f64> {
        matches!(self, PwrFuel::BfbAmMix | PwrFuel::BfbPu241).then_some(250_000_000.0)
    }

    /// The yield the reactor uses: always the field initializer.
    pub fn yield_(self) -> f64 {
        1_000_000_000.0
    }
}

/// Counts and capacities written by `TileEntityPWRController.setup`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PwrSetup {
    pub rod_count: i32,
    pub connections: i32,
    pub connections_controlled: i32,
    pub heatex_count: i32,
    pub channel_count: i32,
    pub heatsink_count: i32,
    pub source_count: i32,
    pub core_heat_capacity: i64,
    /// Fuel rod positions (order unspecified; the Java list follows hash-map order).
    pub rods: Vec<Pos3>,
    /// Port block positions (order unspecified).
    pub ports: Vec<Pos3>,
}

pub const CORE_HEAT_CAPACITY_BASE: i64 = 10_000_000;
pub const HULL_HEAT_CAPACITY_BASE: i64 = 10_000_000;
pub const MAX_STRUCTURE_SIZE: usize = 4096;
pub const TANK_CAPACITY: i32 = 128_000;

pub type Pos3 = (i32, i32, i32);

/// Blocks the scan distinguishes. `Air` stands for any block that is not part of the PWR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PwrBlock {
    Air,
    Fuel,
    Control,
    Channel,
    Heatex,
    Heatsink,
    NeutronSource,
    Casing,
    Reflector,
    Port,
    Controller,
}

impl PwrBlock {
    fn is_valid_core(self) -> bool {
        matches!(self, PwrBlock::Fuel | PwrBlock::Control | PwrBlock::Channel | PwrBlock::Heatex | PwrBlock::Heatsink | PwrBlock::NeutronSource)
    }
    fn is_valid_casing(self) -> bool {
        matches!(self, PwrBlock::Casing | PwrBlock::Reflector | PwrBlock::Port)
    }
}

/// Offsets of `ForgeDirection.VALID_DIRECTIONS` (DOWN, UP, NORTH, SOUTH, WEST, EAST).
const DIRS: [Pos3; 6] = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];

fn add(p: Pos3, d: Pos3, n: i32) -> Pos3 {
    (p.0 + d.0 * n, p.1 + d.1 * n, p.2 + d.2 * n)
}

/// `TileEntityPWRController.setup`
pub fn setup(parts: &HashMap<Pos3, PwrBlock>, rods: &[Pos3]) -> PwrSetup {
    let mut s = PwrSetup::default();
    let (mut doubles, mut controlled_doubles) = (0i32, 0i32);
    for (pos, block) in parts {
        match block {
            PwrBlock::Fuel => s.rod_count += 1,
            PwrBlock::Heatex => s.heatex_count += 1,
            PwrBlock::Channel => s.channel_count += 1,
            PwrBlock::Heatsink => s.heatsink_count += 1,
            PwrBlock::NeutronSource => s.source_count += 1,
            PwrBlock::Port => s.ports.push(*pos),
            _ => {}
        }
    }
    for &fuel in rods {
        s.rods.push(fuel);
        for dir in DIRS {
            let mut controlled = false;
            for i in 1..16 {
                match parts.get(&add(fuel, dir, i)) {
                    None | Some(PwrBlock::Casing) => break,
                    Some(PwrBlock::Control) => controlled = true,
                    Some(PwrBlock::Fuel) => {
                        if controlled { controlled_doubles += 1 } else { doubles += 1 }
                        break;
                    }
                    Some(PwrBlock::Reflector) => {
                        if controlled { controlled_doubles += 2 } else { doubles += 2 }
                        break;
                    }
                    Some(_) => {}
                }
            }
        }
    }
    s.connections = doubles / 2;
    s.connections_controlled = controlled_doubles / 2;
    s.heatsink_count = s.heatsink_count.min(80);
    s.core_heat_capacity = CORE_HEAT_CAPACITY_BASE + s.heatsink_count as i64 * (CORE_HEAT_CAPACITY_BASE / 20);
    s
}

/// Outcome of `MachinePWRController.assemble`.
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    /// The messages the player would be sent, in order (one marker particle each).
    pub errors: Vec<&'static str>,
    pub assembled: bool,
    /// Present only when the scan succeeded.
    pub setup: Option<PwrSetup>,
    /// Every position the scan claimed (the blocks that become `pwr_block`), controller included.
    pub parts: HashMap<Pos3, PwrBlock>,
}

/// `MachinePWRController.assemble` + `floodFill`. `block_at` reads the world; `controller_meta`
/// is the controller's facing metadata (2..5 for horizontal facings; 0, 1 and anything else give
/// a zero horizontal offset, so the scan starts on the controller itself and finds nothing).
pub fn assemble(block_at: &dyn Fn(Pos3) -> PwrBlock, controller: Pos3, controller_meta: i32) -> Assembly {
    let mut assembly: HashMap<Pos3, PwrBlock> = HashMap::new();
    let mut rods: HashMap<Pos3, PwrBlock> = HashMap::new();
    let mut has_source = false;
    let mut errors: Vec<&'static str> = Vec::new();
    let mut errored = false;
    assembly.insert(controller, PwrBlock::Controller);

    // getOrientation(meta).getOpposite() horizontal offsets
    let (ox, oz) = match controller_meta {
        2 => (0, 1),
        3 => (0, -1),
        4 => (1, 0),
        5 => (-1, 0),
        _ => (0, 0),
    };
    let start = (controller.0 + ox, controller.1, controller.2 + oz);

    // Recursive depth-first flood fill, order preserved with an explicit stack of (position, next child).
    const ORDER: [Pos3; 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
    let mut stack: Vec<(Pos3, usize)> = Vec::new();
    let mut visit = |pos: Pos3, stack: &mut Vec<(Pos3, usize)>, assembly: &mut HashMap<Pos3, PwrBlock>| {
        if assembly.contains_key(&pos) {
            return;
        }
        if assembly.len() >= MAX_STRUCTURE_SIZE {
            errored = true;
            errors.push("Max size exceeded");
            return;
        }
        let block = block_at(pos);
        if block.is_valid_casing() {
            assembly.insert(pos, block);
            return;
        }
        if block.is_valid_core() {
            assembly.insert(pos, block);
            if block == PwrBlock::Fuel {
                rods.insert(pos, block);
            }
            if block == PwrBlock::NeutronSource {
                has_source = true;
            }
            stack.push((pos, 0));
            return;
        }
        errors.push("Non-reactor block");
        errored = true;
    };
    visit(start, &mut stack, &mut assembly);
    while let Some(top) = stack.last_mut() {
        if top.1 == ORDER.len() {
            stack.pop();
            continue;
        }
        let child = add(top.0, ORDER[top.1], 1);
        top.1 += 1;
        visit(child, &mut stack, &mut assembly);
    }

    if rods.is_empty() {
        errors.push("Fuel rods required");
        errored = true;
    }
    if !has_source {
        errors.push("Neutron sources required");
        errored = true;
    }
    let mut rod_list: Vec<Pos3> = rods.keys().copied().collect();
    rod_list.sort_unstable();
    let setup = (!errored).then(|| setup(&assembly, &rod_list));
    Assembly { errors, assembled: !errored, setup, parts: assembly }
}

/// What a `pwr_block` does when it dissolves (`BlockPWR.breakBlock`): it turns back into the block it
/// replaced and tells the controller it is no longer assembled.
pub fn internal_block_dissolves(world_time: i64, has_original_block: bool, core_assembled: Option<bool>, core_chunk_loaded: bool) -> bool {
    if world_time % 20 != 0 || !has_original_block {
        return false;
    }
    match core_assembled {
        Some(assembled) => !assembled,
        None => core_chunk_loaded,
    }
}

/// A fluid the controller can carry in its cold tank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fluid(pub &'static str);
pub const NONE: Fluid = Fluid("none");

/// `FT_PWRModerator` multiplier of a fluid (1.0 when it has none).
pub fn moderator_multiplier(fluid: &str) -> f64 {
    heatable(fluid).and_then(|h| h.moderator).unwrap_or(1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuelStack {
    pub hot: bool,
    pub meta: i32,
    pub count: i32,
}

pub const MAX_STACK: i32 = 64;

/// What the tick asks the world to do. Block changes, the explosion and the satellite ping are not
/// ported; the caller applies them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PwrEvents {
    /// Core heat went above its capacity: the controller block breaks, every rod becomes a
    /// corium block (meta 5) and a 15-power fiery explosion happens at `center`.
    pub meltdown: Option<Meltdown>,
    /// `SatelliteRayScan.reportEvent(INFO_NUCLEAR, 200)`, every 100 ticks while fuel burns.
    pub satellite_nuclear_ping: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Meltdown {
    pub rods: Vec<Pos3>,
    /// Mean of the rod block centres (NaN when there are no rods, as in the source).
    pub center: (f64, f64, f64),
    pub explosion_power: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PwrController {
    pub core_heat: i64,
    pub hull_heat: i64,
    pub flux: f64,
    pub rod_level: f64,
    pub rod_target: f64,
    pub type_loaded: i32,
    pub amount_loaded: i32,
    pub progress: f64,
    pub process_time: f64,
    pub rod_count: i32,
    pub connections: i32,
    pub connections_controlled: i32,
    pub heatex_count: i32,
    pub heatsink_count: i32,
    pub channel_count: i32,
    pub source_count: i32,
    pub core_heat_capacity: i64,
    pub unload_delay: i32,
    pub assembled: bool,
    pub cold_type: Fluid,
    pub cold_fill: i32,
    pub hot_type: Fluid,
    pub hot_fill: i32,
    /// Input slot (fresh fuel) and output slot (spent fuel).
    pub slot_in: Option<FuelStack>,
    pub slot_out: Option<FuelStack>,
    pub rods: Vec<Pos3>,
}

impl Default for PwrController {
    fn default() -> Self {
        PwrController {
            core_heat: 0, hull_heat: 0, flux: 0.0, rod_level: 100.0, rod_target: 100.0, type_loaded: 0, amount_loaded: 0,
            progress: 0.0, process_time: 0.0, rod_count: 0, connections: 0, connections_controlled: 0, heatex_count: 0,
            heatsink_count: 0, channel_count: 0, source_count: 0, core_heat_capacity: CORE_HEAT_CAPACITY_BASE,
            unload_delay: 0, assembled: false, cold_type: Fluid("coolant"), cold_fill: 0, hot_type: Fluid("coolant_hot"),
            hot_fill: 0, slot_in: None, slot_out: None, rods: Vec::new(),
        }
    }
}

/// `1 - e^(-x/d)`
pub fn get_x_over_e(x: f64, d: f64) -> f64 {
    1.0 - std::f64::consts::E.powf(-x / d)
}

/// `TileEntityPWRController.connectinFunc`
pub fn connectin_func(connections: f64) -> f64 {
    connections / 10.0 * (1.0 - get_x_over_e(connections, 300.0)) + connections / 150.0 * get_x_over_e(connections, 300.0)
}

impl PwrController {
    pub fn apply_setup(&mut self, s: &PwrSetup) {
        self.rod_count = s.rod_count;
        self.connections = s.connections;
        self.connections_controlled = s.connections_controlled;
        self.heatex_count = s.heatex_count;
        self.channel_count = s.channel_count;
        self.heatsink_count = s.heatsink_count;
        self.source_count = s.source_count;
        self.core_heat_capacity = s.core_heat_capacity;
        self.rods = s.rods.clone();
    }

    pub fn total_process_multiplier(&self) -> f64 {
        let total = self.connections as f64 + self.connections_controlled as f64 * (1.0 - (self.rod_level / 100.0));
        connectin_func(total)
    }

    fn rod_count_for_coolant(&self) -> i32 {
        self.rod_count + (self.heatsink_count as f64 / 4.0).ceil() as i32
    }

    /// `setupTanks`: a fluid without a PWR heating efficiency empties both tanks.
    fn setup_tanks(&mut self) {
        let step = heatable(self.cold_type.0).filter(|h| h.eff_pwr > 0.0);
        let (cold, hot) = match step {
            Some(h) => (self.cold_type, Fluid(h.hot)),
            None => (NONE, NONE),
        };
        if self.cold_type != cold {
            self.cold_type = cold;
            self.cold_fill = 0;
        }
        if self.hot_type != hot {
            self.hot_type = hot;
            self.hot_fill = 0;
        }
    }

    fn update_coolant(&mut self) {
        let Some(h) = heatable(self.cold_type.0).filter(|h| h.eff_pwr > 0.0) else { return };
        let mut cooling_eff = self.channel_count as f64 / self.rod_count_for_coolant() as f64 * 0.1;
        if cooling_eff > 1.0 {
            cooling_eff = 1.0;
        }
        let heat_to_use = self.hull_heat.min((self.hull_heat as f64 * cooling_eff * h.eff_pwr) as i64).min(2_000_000_000) as i32;
        let cool_cycles = self.cold_fill / h.amount_req;
        let hot_cycles = (TANK_CAPACITY - self.hot_fill) / h.amount_produced;
        let heat_cycles = heat_to_use / h.heat_req;
        let cycles = cool_cycles.min(hot_cycles.min(heat_cycles));
        self.hull_heat -= h.heat_req.wrapping_mul(cycles) as i64;
        self.cold_fill = self.cold_fill.wrapping_sub(h.amount_req.wrapping_mul(cycles));
        self.hot_fill = self.hot_fill.wrapping_add(h.amount_produced.wrapping_mul(cycles));
    }

    fn meltdown(&self) -> Meltdown {
        let (mut x, mut y, mut z) = (0.0f64, 0.0f64, 0.0f64);
        for p in &self.rods {
            x += p.0 as f64 + 0.5;
            y += p.1 as f64 + 0.5;
            z += p.2 as f64 + 0.5;
        }
        let n = self.rods.len() as f64;
        Meltdown { rods: self.rods.clone(), center: (x / n, y / n, z / n), explosion_power: 15.0 }
    }

    /// One server tick (`TileEntityPWRController.updateEntity`, server side). `chunks_loaded` is
    /// "the controller's chunk and the four chunks two away diagonally all exist"; `world_time` is
    /// `getTotalWorldTime`. The fluid-identifier slot that normally picks the cold fluid while no
    /// fuel is loaded is not modelled: set `cold_type` directly.
    pub fn tick(&mut self, chunks_loaded: bool, world_time: i64) -> PwrEvents {
        let mut ev = PwrEvents::default();
        self.setup_tanks();
        if self.unload_delay > 0 {
            self.unload_delay -= 1;
        }
        if !chunks_loaded {
            self.unload_delay = 60;
        }
        if !self.assembled {
            return ev; // destroyAllPorts
        }
        if self.unload_delay > 0 {
            self.hull_heat = 0;
            self.core_heat = 0;
            return ev;
        }

        let fuel_in = self.slot_in;
        if let Some(f) = fuel_in.filter(|f| !f.hot) {
            if self.type_loaded == -1 || self.amount_loaded <= 0 {
                self.type_loaded = f.meta;
                self.amount_loaded += 1;
                self.take_input();
            } else if f.meta == self.type_loaded && self.amount_loaded < self.rod_count {
                self.amount_loaded += 1;
                self.take_input();
            }
        }

        let diff = self.rod_level - self.rod_target;
        if diff < 1.0 && diff > -1.0 {
            self.rod_level = self.rod_target;
        }
        if self.rod_target > self.rod_level {
            self.rod_level += 1.0;
        }
        if self.rod_target < self.rod_level {
            self.rod_level -= 1.0;
        }

        let multiplier = moderator_multiplier(self.cold_type.0);
        let mut new_flux = self.source_count.wrapping_mul(20);

        if self.type_loaded != -1 && self.amount_loaded > 0 {
            let fuel = PwrFuel::from_meta(self.type_loaded);
            let used_rods = self.total_process_multiplier();
            let flux_per_rod = self.flux / self.rod_count as f64;
            let output_per_rod = fuel.function().effonix(flux_per_rod);
            let total_output = output_per_rod * self.amount_loaded as f64 * used_rods;
            let mut total_heat_output = total_output * fuel.heat_emission();
            if self.cold_fill > 0 {
                total_heat_output *= multiplier;
            }
            self.core_heat = (self.core_heat as f64 + total_heat_output) as i64;
            new_flux = (new_flux as f64 + total_output) as i32;

            self.process_time = fuel.yield_() as i32 as f64;
            self.progress += total_output;
            if self.progress >= self.process_time {
                self.progress -= self.process_time;
                match &mut self.slot_out {
                    None => self.slot_out = Some(FuelStack { hot: true, meta: self.type_loaded, count: 1 }),
                    Some(s) if s.hot && s.meta == self.type_loaded && s.count < MAX_STACK => s.count += 1,
                    Some(_) => {}
                }
                self.amount_loaded -= 1;
            }
            if world_time % 100 == 0 {
                ev.satellite_nuclear_ping = true;
            }
        }
        if self.amount_loaded <= 0 {
            self.type_loaded = -1;
        }
        if self.amount_loaded > self.rod_count {
            self.amount_loaded = self.rod_count;
        }

        // core cooling
        let approach = get_x_over_e(self.heatex_count as f64 * 5.0 / self.rod_count_for_coolant() as f64, 2.0) / 2.0;
        let average = (self.core_heat + self.hull_heat) / 2;
        self.core_heat = (self.core_heat as f64 - (self.core_heat - average) as f64 * approach) as i64;
        self.hull_heat = (self.hull_heat as f64 - (self.hull_heat - average) as f64 * approach) as i64;

        self.update_coolant();

        self.core_heat = (self.core_heat as f64 * 0.999) as i64;
        self.hull_heat = (self.hull_heat as f64 * 0.999) as i64;

        self.flux = new_flux as f64;
        if self.cold_fill > 0 {
            self.flux *= multiplier;
        }
        if self.core_heat > self.core_heat_capacity {
            ev.meltdown = Some(self.meltdown());
        }
        ev
    }

    fn take_input(&mut self) {
        if let Some(s) = &mut self.slot_in {
            s.count -= 1;
            if s.count <= 0 {
                self.slot_in = None;
            }
        }
    }

    /// `receiveControl("control")`: the GUI slider, clamped to 0..=100.
    pub fn set_rod_target(&mut self, value: i32) {
        self.rod_target = value.clamp(0, 100) as f64;
    }

    /// OpenComputers `setLevel`
    pub fn set_rod_target_f(&mut self, value: f64) {
        self.rod_target = value.clamp(0.0, 100.0);
    }

    /// ROR `setrods`: `percent` is inverted into the target.
    pub fn ror_set_rods(&mut self, percent: i32) {
        self.rod_target = (100 - percent.clamp(0, 100)) as f64;
    }

    /// ROR `jettison`: drops the loaded fuel without producing spent fuel.
    pub fn ror_jettison(&mut self) {
        self.type_loaded = -1;
        self.amount_loaded = 0;
        self.progress = 0.0;
    }

    /// ROR `depletion` percentage.
    pub fn ror_depletion(&self) -> i32 {
        (self.progress * 100.0 / self.process_time) as i32
    }
}

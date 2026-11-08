//! The columns of one reactor layer and the step that runs them.
//!
//! One `step` is one server tick: first every neutron stream queued by the
//! previous tick is resolved, in queue order (`NeutronHandler.onServerTick`,
//! START phase); then every column updates, in registration order
//! (`updateEntity`). Rods queue the streams of the next tick while they
//! update. The Java node cache is an optimization the port does not need: the
//! lookups are live (the one visible difference is a stale cache entry for a
//! column removed less than 20 ticks ago, see `docs/rbmk-core.md`).
//!
//! Chunk access, world blocks, debris, meltdown consequences and saving are
//! the engine's: the step reports them (`rad`, `meltdowns`) instead of doing
//! them.

use super::dials::Dials;
use super::jmath;
use super::jrandom::JavaRandom;
use super::rods::{NType, RodStack, RodType};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

/// `TileEntityRBMKControl.speed`.
pub const CONTROL_SPEED: f64 = 0.00277;
/// `TileEntityRBMKControl.consumption` / `maxPower`.
pub const CONTROL_CONSUMPTION: i64 = 5_000;
pub const CONTROL_MAX_POWER: i64 = CONTROL_CONSUMPTION * 10;
/// `TileEntityRBMKBase.maxWater` / `maxSteam`.
pub const MAX_WATER: i32 = 16_000;
pub const MAX_STEAM: i32 = 16_000;
/// `TileEntityRBMKBase.maxHeat`.
pub const MAX_HEAT: f64 = 1500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rod,
    RodReaSim,
    Control,
    ControlReaSim,
    ControlAuto,
    ControlReaSimAuto,
    Moderator,
    Reflector,
    Absorber,
    Blank,
}

/// `RBMKNeutronHandler.RBMKType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RbmkType {
    Rod,
    Moderator,
    ControlRod,
    Reflector,
    Absorber,
    Other,
}

impl Kind {
    pub fn rbmk_type(self) -> RbmkType {
        match self {
            Kind::Rod | Kind::RodReaSim => RbmkType::Rod,
            Kind::Control | Kind::ControlReaSim | Kind::ControlAuto | Kind::ControlReaSimAuto => RbmkType::ControlRod,
            Kind::Moderator => RbmkType::Moderator,
            Kind::Reflector => RbmkType::Reflector,
            Kind::Absorber => RbmkType::Absorber,
            Kind::Blank => RbmkType::Other,
        }
    }

    fn is_rod(self) -> bool {
        matches!(self, Kind::Rod | Kind::RodReaSim)
    }

    fn is_control(self) -> bool {
        self.rbmk_type() == RbmkType::ControlRod
    }

    /// `TileEntityRBMKControl.isPowered`.
    pub fn is_powered(self) -> bool {
        matches!(self, Kind::ControlReaSim | Kind::ControlReaSimAuto)
    }

    /// `RBMKBase.isOpaqueCube` is true; `RBMKRod` overrides it to false.
    fn opaque(self) -> bool {
        !self.is_rod()
    }
}

/// The fuel slot and the flux buffers of a rod column.
#[derive(Clone, Debug, Default)]
pub struct RodCol {
    pub fuel: Option<(&'static RodType, RodStack)>,
    pub flux_quantity: f64,
    pub flux_fast_ratio: f64,
    pub last_flux_quantity: f64,
    pub last_flux_ratio: f64,
    pub has_rod: bool,
}

/// `RBMKFunction` of the automatic control rod.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoFunction {
    Linear,
    QuadUp,
    QuadDown,
}

#[derive(Clone, Debug, Default)]
pub struct ControlCol {
    pub level: f64,
    pub last_level: f64,
    pub target_level: f64,
    /// Manual rods only.
    pub starting_level: f64,
    pub has_power: bool,
    pub power: i64,
    /// Automatic rods only.
    pub function: Option<AutoFunction>,
    pub level_lower: f64,
    pub level_upper: f64,
    pub heat_lower: f64,
    pub heat_upper: f64,
}

#[derive(Clone, Debug)]
pub enum Data {
    Passive,
    Rod(RodCol),
    Control(ControlCol),
}

#[derive(Clone, Debug)]
pub struct Column {
    pub kind: Kind,
    pub x: i32,
    pub z: i32,
    pub heat: f64,
    pub reasim_water: i32,
    pub reasim_steam: i32,
    pub crane_indicator: i32,
    /// `hasLid()` (control rods always have one).
    pub lid: bool,
    /// `isModerated()` of the block (graphite-topped rods and control rods).
    pub moderated: bool,
    pub data: Data,
}

impl Column {
    pub fn new(kind: Kind, x: i32, z: i32) -> Column {
        let data = match kind {
            Kind::Rod | Kind::RodReaSim => Data::Rod(RodCol::default()),
            k if k.is_control() => Data::Control(ControlCol {
                function: k.auto_function(),
                ..ControlCol::default()
            }),
            _ => Data::Passive,
        };
        Column {
            kind,
            x,
            z,
            heat: 0.0,
            reasim_water: 0,
            reasim_steam: 0,
            crane_indicator: 0,
            lid: true,
            moderated: false,
            data,
        }
    }

    pub fn rod(&self) -> Option<&RodCol> {
        match &self.data {
            Data::Rod(r) => Some(r),
            _ => None,
        }
    }

    pub fn rod_mut(&mut self) -> Option<&mut RodCol> {
        match &mut self.data {
            Data::Rod(r) => Some(r),
            _ => None,
        }
    }

    pub fn control(&self) -> Option<&ControlCol> {
        match &self.data {
            Data::Control(c) => Some(c),
            _ => None,
        }
    }

    pub fn control_mut(&mut self) -> Option<&mut ControlCol> {
        match &mut self.data {
            Data::Control(c) => Some(c),
            _ => None,
        }
    }

    /// `TileEntityRBMKControl.getMult` (`ControlManual` adds the surge).
    pub fn control_mult(&self, dials: &Dials) -> f64 {
        let c = self.control().expect("control column");
        if matches!(self.kind, Kind::Control | Kind::ControlReaSim) {
            let mut surge = 0.0;
            if c.target_level < c.starting_level && (c.level - c.target_level).abs() > 0.01 {
                surge = jmath::sin(jmath::pow(1.0 - c.level, 15.0) * PI)
                    * (c.starting_level - c.target_level)
                    * dials.surge_mod;
            }
            c.level + surge
        } else {
            c.level
        }
    }

    /// `setTarget`: a manual rod remembers where it started from.
    pub fn set_target(&mut self, target: f64) {
        let manual = matches!(self.kind, Kind::Control | Kind::ControlReaSim);
        let c = self.control_mut().expect("control column");
        c.target_level = target;
        if manual {
            c.starting_level = c.level;
        }
    }
}

impl Kind {
    fn auto_function(self) -> Option<AutoFunction> {
        match self {
            Kind::ControlAuto | Kind::ControlReaSimAuto => Some(AutoFunction::Linear),
            _ => None,
        }
    }
}

/// One queued neutron stream.
#[derive(Clone, Debug)]
struct Stream {
    origin: usize,
    vx: f64,
    vz: f64,
    flux_quantity: f64,
    flux_ratio: f64,
}

/// A radiation increment at a block position (`ChunkRadiationManager`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadEvent {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub amount: f32,
}

pub struct Reactor {
    pub dials: Dials,
    pub rng: JavaRandom,
    /// Y of the layer (the base block of every column).
    pub y: i32,
    pub columns: Vec<Column>,
    index: HashMap<(i32, i32), usize>,
    /// Opaque world blocks above the layer: `(x, z, height index)`.
    pub obstacles: HashSet<(i32, i32, i32)>,
    streams: Vec<Stream>,
    /// Radiation produced during the last step.
    pub rad: Vec<RadEvent>,
    /// Columns whose heat passed the limit during the last step with meltdowns
    /// enabled; the engine runs the meltdown.
    pub meltdowns: Vec<usize>,
}

fn java_floor(v: f64) -> i32 {
    v.floor() as i32
}

/// `0.05F` as the double Java widens it to.
fn rad_scale() -> f64 {
    f64::from(0.05_f32)
}

impl Reactor {
    pub fn new(dials: Dials, rng: JavaRandom, y: i32) -> Reactor {
        Reactor {
            dials,
            rng,
            y,
            columns: Vec::new(),
            index: HashMap::new(),
            obstacles: HashSet::new(),
            streams: Vec::new(),
            rad: Vec::new(),
            meltdowns: Vec::new(),
        }
    }

    /// Registers a column; the registration order is the update order (D1).
    pub fn add(&mut self, column: Column) -> usize {
        let i = self.columns.len();
        self.index.insert((column.x, column.z), i);
        self.columns.push(column);
        i
    }

    pub fn at(&self, x: i32, z: i32) -> Option<usize> {
        self.index.get(&(x, z)).copied()
    }

    /// Number of streams waiting for the next step.
    pub fn pending_streams(&self) -> usize {
        self.streams.len()
    }

    fn height(&self) -> i32 {
        self.dials.column_height + 1
    }

    /// `getHits`: opaque blocks in the column of blocks at `(x, z)`.
    fn hits(&self, x: i32, z: i32) -> i32 {
        let col = self.at(x, z).map(|i| self.columns[i].kind.opaque());
        (0..self.height())
            .filter(|&h| self.obstacles.contains(&(x, z, h)) || col == Some(true))
            .count() as i32
    }

    fn add_rad(&mut self, x: i32, z: i32, amount: f32) {
        self.rad.push(RadEvent { x, y: self.y, z, amount });
    }

    /// One server tick.
    pub fn step(&mut self) {
        self.rad.clear();
        self.meltdowns.clear();
        let streams = std::mem::take(&mut self.streams);
        for mut s in streams {
            self.run_stream(&mut s);
        }
        for i in 0..self.columns.len() {
            self.update(i);
        }
    }

    // ---- neutron streams

    fn irradiate_hits(&mut self, flux: f64, x: i32, z: i32, hits: i32) {
        let amount = (flux * rad_scale() * (1.0 - f64::from(hits) / f64::from(self.height()))) as f32;
        self.add_rad(x, z, amount);
    }

    fn run_stream(&mut self, s: &mut Stream) {
        if s.flux_quantity == 0.0 {
            return;
        }
        let (ox, oz) = (self.columns[s.origin].x, self.columns[s.origin].z);
        let height = self.height();
        let range = self.dials.flux_range;
        let mut moderated_count = 0;
        let moderate = |s: &mut Stream, eff: f64| s.flux_ratio *= 1.0 - eff;

        for i in 1..=range {
            let fi = f64::from(i);
            let (px, pz) = (ox + java_floor(0.5 + s.vx * fi), oz + java_floor(0.5 + s.vz * fi));
            if s.flux_quantity == 0.0 {
                return;
            }
            let Some(t) = self.at(px, pz) else {
                let hits = self.hits(px, pz);
                if hits == height {
                    return;
                } else if hits > 0 {
                    // Java passes the stream's origin cell here, not the cell that was hit
                    self.irradiate_hits(s.flux_quantity, ox, oz, hits);
                    s.flux_quantity *= 1.0 - f64::from(hits) / f64::from(height);
                } else {
                    self.irradiate_hits(s.flux_quantity, ox, oz, 0);
                }
                continue;
            };
            let kind = self.columns[t].kind;
            let ty = kind.rbmk_type();
            if ty == RbmkType::Other {
                continue;
            }
            if !self.columns[t].lid {
                self.add_rad(px, pz, (s.flux_quantity * rad_scale()) as f32);
            }
            if ty == RbmkType::Moderator || self.columns[t].moderated {
                moderated_count += 1;
                moderate(s, self.dials.moderator_efficiency);
            }
            if ty == RbmkType::Rod {
                // IRBMKFluxReceiver
                if self.columns[t].rod().is_some_and(|r| r.has_rod) {
                    self.receive_flux(t, s);
                    return;
                }
            } else if ty == RbmkType::ControlRod {
                if self.columns[t].control().unwrap().level > 0.0 {
                    s.flux_quantity *= self.columns[t].control_mult(&self.dials);
                    continue;
                }
                return;
            } else if ty == RbmkType::Reflector {
                if self.columns[s.origin].moderated {
                    moderated_count += 1;
                }
                if s.flux_ratio > 0.0 && moderated_count > 0 {
                    for _ in 0..moderated_count {
                        moderate(s, self.dials.moderator_efficiency);
                    }
                }
                if self.dials.reflector_efficiency != 1.0 {
                    s.flux_quantity *= self.dials.reflector_efficiency;
                    continue;
                }
                self.receive_flux(s.origin, s);
                return;
            } else if ty == RbmkType::Absorber {
                self.columns[t].heat += self.dials.absorber_heat_conversion * s.flux_quantity;
                if self.dials.absorber_efficiency == 1.0 {
                    return;
                }
                s.flux_quantity *= self.dials.absorber_efficiency;
            }
        }

        // The last cell of the path decides what happens to what is left.
        let (lx, lz) = (ox + java_floor(0.5 + s.vx * f64::from(range)), oz + java_floor(0.5 + s.vz * f64::from(range)));
        let Some(last) = self.at(lx, lz) else {
            let (ax, az) = (java_floor(f64::from(ox) + s.vx), java_floor(f64::from(oz) + s.vz));
            let hits = self.hits(ax, az);
            self.irradiate_hits(s.flux_quantity, ax, az, hits);
            return;
        };
        if self.columns[last].kind.rbmk_type() == RbmkType::ControlRod {
            let mult = self.columns[last].control_mult(&self.dials);
            if mult > 0.0 {
                s.flux_quantity *= mult;
                // Java then irradiates the cell behind the rod only when the
                // origin has no cached node; the origin of a stream is a
                // column, so it has one and nothing more happens.
            }
        }
    }

    /// `TileEntityRBMKRod.receiveFlux`.
    fn receive_flux(&mut self, t: usize, s: &Stream) {
        let r = self.columns[t].rod_mut().expect("flux receiver is a rod");
        let fast_flux = r.flux_quantity * r.flux_fast_ratio;
        let fast_in = s.flux_quantity * s.flux_ratio;
        r.flux_quantity += s.flux_quantity;
        r.flux_fast_ratio = (fast_flux + fast_in) / r.flux_quantity;
    }

    // ---- column updates

    fn update(&mut self, i: usize) {
        match self.columns[i].kind {
            Kind::Rod | Kind::RodReaSim => self.update_rod(i),
            k if k.is_control() => self.update_control(i),
            _ => self.base_update(i),
        }
    }

    fn update_control(&mut self, i: usize) {
        let kind = self.columns[i].kind;
        if matches!(kind, Kind::ControlAuto | Kind::ControlReaSimAuto) {
            let heat = self.columns[i].heat;
            let c = self.columns[i].control().unwrap();
            let lower = c.heat_lower.min(c.heat_upper);
            let upper = c.heat_lower.max(c.heat_upper);
            let faux = if heat < lower {
                c.level_lower
            } else if heat > upper {
                c.level_upper
            } else {
                match c.function {
                    Some(AutoFunction::Linear) => {
                        (heat - c.heat_lower) * ((c.level_upper - c.level_lower) / (c.heat_upper - c.heat_lower)) + c.level_lower
                    }
                    Some(AutoFunction::QuadUp) => {
                        jmath::pow((heat - c.heat_lower) / (c.heat_upper - c.heat_lower), 2.0) * (c.level_upper - c.level_lower)
                            + c.level_lower
                    }
                    Some(AutoFunction::QuadDown) => {
                        jmath::pow((heat - c.heat_upper) / (c.heat_lower - c.heat_upper), 2.0) * (c.level_lower - c.level_upper)
                            + c.level_upper
                    }
                    None => 0.0,
                }
            };
            let mut target = faux * 0.01;
            // MathHelper.clamp_double lets NaN through
            if target < 0.0 {
                target = 0.0;
            } else if target > 1.0 {
                target = 1.0;
            }
            self.columns[i].control_mut().unwrap().target_level = target;
        }

        let speed = CONTROL_SPEED * self.dials.control_speed;
        let powered = kind.is_powered();
        let c = self.columns[i].control_mut().unwrap();
        c.has_power = true;
        if powered && c.power < CONTROL_CONSUMPTION {
            c.has_power = false;
        }
        c.last_level = c.level;
        if c.has_power {
            if c.level < c.target_level {
                c.level += speed;
                if c.level > c.target_level {
                    c.level = c.target_level;
                }
            }
            if c.level > c.target_level {
                c.level -= speed;
                if c.level < c.target_level {
                    c.level = c.target_level;
                }
            }
            if powered && c.level != c.last_level {
                c.power -= CONTROL_CONSUMPTION;
            }
        }
        self.base_update(i);
    }

    /// `IEnergyReceiverMK2.transferPower` of a powered control rod: returns what
    /// did not fit.
    pub fn transfer_power(&mut self, i: usize, power: i64) -> i64 {
        let kind = self.columns[i].kind;
        let max = if kind.is_powered() { CONTROL_MAX_POWER } else { 0 };
        let c = self.columns[i].control_mut().expect("control column");
        if power + c.power <= max {
            c.power += power;
            return 0;
        }
        let capacity = max - c.power;
        c.power = max;
        power - capacity
    }

    fn flux_from_type(r: &RodCol, n: NType) -> f64 {
        let fast = r.flux_quantity * r.flux_fast_ratio;
        let slow = r.flux_quantity * (1.0 - r.flux_fast_ratio);
        match n {
            NType::Slow => slow + fast * 0.5,
            NType::Fast => fast + slow * 0.3,
            NType::Any => r.flux_quantity,
        }
    }

    fn update_rod(&mut self, i: usize) {
        let dials = self.dials;
        let (x, z, lid) = (self.columns[i].x, self.columns[i].z, self.columns[i].lid);
        let fuel = self.columns[i].rod_mut().unwrap().fuel.take();
        let Some((rod, mut stack)) = fuel else {
            let r = self.columns[i].rod_mut().unwrap();
            r.last_flux_ratio = 0.0;
            r.last_flux_quantity = 0.0;
            r.flux_quantity = 0.0;
            r.flux_fast_ratio = 0.0;
            r.has_rod = false;
            self.base_update(i);
            return;
        };
        let r = self.columns[i].rod().unwrap();
        let ratio_out = if rod.r_type == NType::Slow { 0.0 } else { 1.0 };
        let flux_in = Self::flux_from_type(r, rod.n_type);
        let flux_q = r.flux_quantity;
        let flux_out = stack.burn(rod, &dials, flux_in);
        stack.update_heat(rod, &dials, 1.0);
        let heat = self.columns[i].heat;
        self.columns[i].heat += stack.provide_heat(rod, &dials, heat, 1.0);
        self.columns[i].rod_mut().unwrap().fuel = Some((rod, stack));
        if !lid {
            self.add_rad(x, z, (flux_q * rad_scale()) as f32);
        }
        self.base_update(i);
        if self.columns[i].heat > MAX_HEAT {
            if !dials.meltdowns_disabled {
                self.meltdowns.push(i);
            }
            let r = self.columns[i].rod_mut().unwrap();
            r.last_flux_ratio = 0.0;
            r.last_flux_quantity = 0.0;
            r.flux_quantity = 0.0;
            return;
        }
        if self.columns[i].heat > 10_000.0 {
            self.columns[i].heat = 10_000.0;
        }
        let r = self.columns[i].rod_mut().unwrap();
        r.last_flux_quantity = r.flux_quantity;
        r.last_flux_ratio = r.flux_fast_ratio;
        r.flux_quantity = 0.0;
        r.flux_fast_ratio = 0.0;
        self.spread_flux(i, flux_out, ratio_out);
        self.columns[i].rod_mut().unwrap().has_rod = true;
    }

    /// `spreadFlux`: four cardinal streams, or eight ReaSim ones.
    fn spread_flux(&mut self, i: usize, flux: f64, ratio: f64) {
        if flux == 0.0 {
            return;
        }
        if self.columns[i].kind == Kind::RodReaSim {
            let (mut vx, mut vz) = (1.0_f64, 0.0_f64);
            let rot = |vx: &mut f64, vz: &mut f64, deg: f64| {
                let a = deg / 180.0 * PI;
                let (c, s) = (jmath::cos(a), jmath::sin(a));
                let (nx, nz) = (*vx * c + *vz * s, *vz * c - *vx * s);
                *vx = nx;
                *vz = nz;
            };
            rot(&mut vx, &mut vz, f64::from(self.rng.next_int(4)) * 9.0);
            for _ in 0..8 {
                self.streams.push(Stream { origin: i, vx, vz, flux_quantity: flux * 0.75, flux_ratio: ratio });
                rot(&mut vx, &mut vz, 45.0);
            }
        } else {
            for (vx, vz) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
                self.streams.push(Stream { origin: i, vx, vz, flux_quantity: flux, flux_ratio: ratio });
            }
        }
    }

    // ---- heat

    /// `TileEntityRBMKBase.updateEntity`.
    fn base_update(&mut self, i: usize) {
        if self.columns[i].crane_indicator > 0 {
            self.columns[i].crane_indicator -= 1;
        }
        self.move_heat(i);
        if self.dials.reasim_boilers {
            self.boil_water(i);
        }
    }

    fn boil_water(&mut self, i: usize) {
        let c = &mut self.columns[i];
        if c.heat < 100.0 {
            return;
        }
        let consumption = self.dials.boiler_heat_consumption;
        let available_heat = (c.heat - 100.0) / consumption;
        let available_water = f64::from(c.reasim_water);
        let available_space = f64::from(MAX_STEAM - c.reasim_steam);
        // BobMathUtil.min starts at Double.MAX_VALUE and skips NaN
        let mut min = f64::MAX;
        for v in [available_heat, available_water, available_space] {
            if v < min {
                min = v;
            }
        }
        let speed = self.dials.reasim_boiler_speed.clamp(0.0, 1.0);
        let processed = (min * speed).floor() as i32;
        if processed <= 0 {
            return;
        }
        c.reasim_water -= processed;
        c.reasim_steam += processed;
        c.heat -= f64::from(processed) * consumption;
    }

    /// `moveHeat`: averages heat (and ReaSim water/steam) with the up to four
    /// neighbours, then cools passively.
    fn move_heat(&mut self, i: usize) {
        let reasim = self.dials.reasim_boilers;
        let (x, z) = (self.columns[i].x, self.columns[i].z);
        let mut rec = vec![i];
        let mut heat_tot = self.columns[i].heat;
        let mut water_tot = self.columns[i].reasim_water;
        let mut steam_tot = self.columns[i].reasim_steam;
        // NORTH, EAST, SOUTH, WEST
        for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            if let Some(n) = self.at(x + dx, z + dz) {
                rec.push(n);
                heat_tot += self.columns[n].heat;
                if reasim {
                    water_tot += self.columns[n].reasim_water;
                    steam_tot += self.columns[n].reasim_steam;
                }
            }
        }
        let members = rec.len() as i32;
        let step = self.dials.column_heat_flow;
        if members > 1 {
            let target = heat_tot / f64::from(members);
            let (t_water, r_water) = (water_tot / members, water_tot % members);
            let (t_steam, r_steam) = (steam_tot / members, steam_tot % members);
            for &m in &rec {
                let c = &mut self.columns[m];
                let delta = target - c.heat;
                c.heat += delta * step;
                if reasim {
                    c.reasim_water = t_water;
                    c.reasim_steam = t_steam;
                }
            }
            if reasim {
                self.columns[i].reasim_water += r_water;
                self.columns[i].reasim_steam += r_steam;
            }
        }
        // coolPassively(members - 1)
        let neighbors = members - 1;
        let (min, max) = (self.dials.passive_cooling_inner, self.dials.passive_cooling);
        let cooling = min + (max - min) * (f64::from(4 - neighbors.clamp(0, 4)) / 4.0);
        let c = &mut self.columns[i];
        c.heat -= cooling;
        if c.heat < 20.0 {
            c.heat = 20.0;
        }
    }
}


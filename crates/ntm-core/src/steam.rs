//! Energy batch 2 (docs/maquinas.md, "Energia, lote 2"): steam into power
//! and spent steam back into water. `tileentity/machine/TileEntitySteamEngine.java`,
//! `TileEntityTurbineBase.java` with `TileEntityMachineIndustrialTurbine.java`
//! and `TileEntityChungus.java`, `TileEntityCondenser.java` and
//! `TileEntityCondenserPowered.java`; their numbers in the content agent's
//! `steam.json`, the steams' `FT_Coolable` in `fluid_traits.json`
//! (`crate::heater::tables`).
//!
//! The generators offer this tick's power to the network and lose what it
//! doesn't take (the NTM's `powerBuffer`, reset each tick). The turbines
//! keep the NTM's int: operations times heat multiply as a signed 32-bit
//! int before the efficiency, so the Leviathan's dense steams can wrap;
//! a negative output offers nothing here.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    heater::{Coolable, tables},
    machine::{Effect, Logic, fill_level},
    machine::Held,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

fn turbine_coolable(fluid: &str) -> Option<&'static Coolable> {
    tables().coolable.get(fluid).filter(|c| c.turbine > 0.0)
}

/// The tank's fluid as the turbine's ints see it: `ops · heat` wrapped to
/// 32 bits, times the efficiency, cast to a long.
fn generation(ops: u32, c: &Coolable, eff: f64) -> i64 {
    (f64::from((ops as i32).wrapping_mul(c.heat as i32)) * eff) as i64
}

pub mod actions {
    pub const LEVER: u32 = 1;
}

// ------------------------------------------------------------ steam engine

pub const ENGINE_EFFICIENCY: f64 = 0.85;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SteamEngine {
    /// None: its machine file only carries the model's values.
    #[serde(skip)]
    pub slots: Slots,
    pub input: Tank,
    pub output: Tank,
    #[serde(skip)]
    pub buffer: u64,
    /// The Java engine saves acceleration, while its rotor starts anew on load.
    #[serde(default)]
    pub acceleration:f32,
    #[serde(skip)]
    pub rotor:f32,
    #[serde(skip)]
    pub sound_hit:bool,
}

impl Default for SteamEngine {
    fn default() -> Self {
        Self { slots: Slots::new(), input: Tank::typed("steam", 2_000), output: Tank::typed("spentsteam", 20), buffer: 0,acceleration:0.,rotor:0.,sound_hit:false }
    }
}

impl SteamEngine {
    /// Steam in, spent steam out, as many operations as both allow:
    /// `ops · heat · 0.85` HE this tick.
    pub fn step(&mut self) -> bool {
        self.buffer = 0;
        self.sound_hit=false;
        self.input.set_type("steam");
        self.output.set_type("spentsteam");
        let Some(c) = turbine_coolable("steam") else { return false };
        let ops = (self.input.mb / c.in_mb).min(self.output.free() / c.out_mb);
        self.input.drain(ops * c.in_mb);
        self.output.fill("spentsteam", ops * c.out_mb);
        self.buffer = (f64::from(ops * c.heat) * c.turbine * ENGINE_EFFICIENCY) as u64;
        self.acceleration=(self.acceleration+if ops>0{0.1}else{-0.1}).clamp(0.,40.);
        self.rotor+=self.acceleration;
        if self.rotor>=360.{self.rotor-=360.;self.sound_hit=true;}
        ops > 0
    }
}

// --------------------------------------------------------------- turbines

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    /// `TileEntityMachineIndustrialTurbine`: a fifth of its input a tick,
    /// its power through a flywheel; the lever resizes its tanks.
    Industrial,
    /// `TileEntityChungus`, the Leviathan: all of its input, at 85 %.
    Chungus,
}

impl Shape {
    pub fn of(block: &str) -> Option<Self> {
        match block {
            "ntm:machine_industrial_turbine" => Some(Self::Industrial),
            "ntm:machine_chungus" => Some(Self::Chungus),
            _ => None,
        }
    }
    fn efficiency(self) -> f64 {
        if self == Self::Industrial { 1.0 } else { 0.85 }
    }
    fn consumption(self) -> f64 {
        if self == Self::Industrial { 0.2 } else { 1.0 }
    }
    fn caps(self) -> (u32, u32) {
        if self == Self::Industrial { (750_000, 3_000_000) } else { (1_000_000_000, 1_000_000_000) }
    }
    pub fn title(self) -> &'static str {
        if self == Self::Industrial { "Turbina industrial" } else { "Turbina Leviatã" }
    }
}

/// `FLYWHEEL_MAX_ENERGY`: the industrial turbine's flywheel mass.
pub const FLYWHEEL: f64 = 0.5e8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Turbine {
    #[serde(skip)]
    pub slots: Slots,
    pub shape: Shape,
    pub input: Tank,
    pub output: Tank,
    pub flywheel: i64,
    pub max_power: i64,
    #[serde(skip)]
    pub buffer: i64,
    #[serde(skip)]
    pub on: bool,
}

impl Turbine {
    pub fn new(shape: Shape) -> Self {
        let (i, o) = shape.caps();
        Self { slots: Slots::new(), shape, input: Tank::typed("steam", i), output: Tank::typed("spentsteam", o), flywheel: 0, max_power: 0, buffer: 0, on: false }
    }

    /// `updateEntity`: the operations a fifth (or all) of the input and the
    /// room out allow, their power to the flywheel (industrial) or out.
    pub fn step(&mut self) -> bool {
        let was = self.on;
        self.buffer = 0;
        self.on = false;
        let eff = self.shape.efficiency();
        let valid = match turbine_coolable(&self.input.fluid) {
            Some(c) if c.turbine * eff > 0.0 => {
                let eff = c.turbine * eff;
                self.output.set_type(&c.out);
                let take = (f64::from(self.input.mb) * self.shape.consumption()).ceil().min(f64::from(self.input.mb));
                let ops = ((take / f64::from(c.in_mb)) as u32).min(self.output.free() / c.out_mb);
                if ops > 0 {
                    self.input.drain(ops * c.in_mb);
                    let out = c.out.clone();
                    self.output.fill(&out, ops * c.out_mb);
                    self.generate(generation(ops, c, eff), c, eff);
                }
                self.on = ops > 0;
                true
            }
            _ => false,
        };
        if self.shape == Shape::Industrial {
            self.flywheel_tick();
        }
        if !valid {
            self.output.set_type("");
        }
        self.on || was
    }

    /// `generatePower`: the Leviathan's straight out; the industrial
    /// turbine's into its flywheel, its top output read from a full tick.
    fn generate(&mut self, power: i64, c: &Coolable, eff: f64) {
        if self.shape == Shape::Chungus {
            self.buffer += power;
            return;
        }
        let max_ops = (f64::from(self.input.cap) * self.shape.consumption() / f64::from(c.in_mb)).ceil() as u32;
        self.max_power = generation(max_ops, c, eff);
        self.flywheel += power;
    }

    /// `onServerTick`: the spin is the flywheel over its mass; it gives
    /// `max(spin, 5 %)` of the top output, at most what it holds.
    fn flywheel_tick(&mut self) {
        let spin = self.flywheel as f64 / FLYWHEEL;
        let target = ((spin.max(0.05) * self.max_power as f64) as i64).min(self.flywheel);
        self.flywheel -= target;
        self.buffer = target;
    }

    /// Whether cell `rel` of the multiblock (the placer's frame: x right,
    /// y up, z away) is the lever (`MachineIndustrialTurbine` and
    /// `MachineChungus.onBlockActivated`): the industrial turbine's three
    /// towards the placer a layer up, the Leviathan's two on its right.
    pub fn lever_cell(&self, rel: [i32; 3]) -> bool {
        match self.shape {
            Shape::Industrial => rel == [0, 1, -3],
            Shape::Chungus => rel[0] == 2 && (rel[2] == -1 || rel[2] == -2) && (0..2).contains(&rel[1]),
        }
    }

    /// `onLeverPull`: steam → hot → superhot → ultrahot → steam, the
    /// industrial turbine's tanks a tenth each step and a thousand times
    /// back (the content past a smaller tank is lost; a new type empties it).
    pub fn lever(&mut self) {
        let resize = self.shape == Shape::Industrial;
        let (input, output, scale): (&str, &str, Option<(u32, u32)>) = match self.input.fluid.as_str() {
            "steam" => ("hotsteam", "steam", Some((1, 10))),
            "hotsteam" => ("superhotsteam", "hotsteam", Some((1, 10))),
            "superhotsteam" => ("ultrahotsteam", "superhotsteam", Some((1, 10))),
            "ultrahotsteam" => ("steam", "spentsteam", Some((1000, 1))),
            _ => ("steam", "spentsteam", None),
        };
        self.input.set_type(input);
        self.output.set_type(output);
        if let (true, Some((mul, div))) = (resize, scale) {
            for t in [&mut self.input, &mut self.output] {
                t.cap = t.cap * mul / div;
                t.mb = t.mb.min(t.cap);
            }
        }
    }
}

// ------------------------------------------------------------- condensers

pub const CONDENSER_POWER: u64 = 10_000_000;
/// HE a mB in the powered condenser.
pub const CONDENSER_HE: u64 = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Condenser {
    #[serde(skip)]
    pub slots: Slots,
    pub powered: bool,
    pub input: Tank,
    pub output: Tank,
    pub power: u64,
    #[serde(skip)]
    pub on: bool,
}

impl Condenser {
    pub fn new(powered: bool) -> Self {
        let cap = if powered { 1_000_000 } else { 100 };
        Self { slots: Slots::new(), powered, input: Tank::typed("spentsteam", cap), output: Tank::typed("water", cap), power: 0, on: false }
    }

    /// As much spent steam as there is room for into water; the powered one
    /// only with `10 HE` a mB (5 % short is let through), then paid.
    pub fn step(&mut self) -> bool {
        let was = self.on;
        let convert = self.input.mb.min(self.output.free());
        let cost = u64::from(convert) * CONDENSER_HE;
        self.on = false;
        if !self.powered || self.power as f64 >= cost as f64 * 0.95 {
            self.input.drain(convert);
            self.output.fill("water", convert);
            if self.powered {
                self.power = self.power.saturating_sub(cost);
            }
            self.on = convert > 0;
        }
        self.on || was
    }
}

// ------------------------------------------------------- model values

/// The steam's step for the models (0 steam .. 3 ultrahot).
fn steam_index(fluid: &str) -> i32 {
    ["steam", "hotsteam", "superhotsteam", "ultrahotsteam"].iter().position(|f| *f == fluid).unwrap_or(0) as i32
}

/// No slots: their machine files carry the values the models animate by.
/// Engine [0] turning; turbines [0] turning, [1] the steam (0..=3);
/// powered condenser [0] working.
impl Slotted for SteamEngine {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![i32::from(self.buffer > 0)]
    }
}

impl Slotted for Turbine {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        let turning = self.on || (self.shape == Shape::Industrial && self.flywheel > 0);
        vec![i32::from(turning), steam_index(&self.input.fluid)]
    }
}

impl Slotted for Condenser {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![i32::from(self.on)]
    }
}

// ------------------------------------------------------------------- all

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Steam {
    Engine(SteamEngine),
    Turbine(Turbine),
    Condenser(Condenser),
}

impl Steam {
    pub fn of(block: &str) -> Option<Self> {
        match block {
            "ntm:machine_steam_engine" => Some(Self::Engine(SteamEngine::default())),
            "ntm:machine_condenser" => Some(Self::Condenser(Condenser::new(false))),
            "ntm:machine_condenser_powered" => Some(Self::Condenser(Condenser::new(true))),
            b => Shape::of(b).map(|s| Self::Turbine(Turbine::new(s))),
        }
    }

    fn tanks(&self) -> [&Tank; 2] {
        match self {
            Self::Engine(m) => [&m.input, &m.output],
            Self::Turbine(m) => [&m.input, &m.output],
            Self::Condenser(m) => [&m.input, &m.output],
        }
    }

    fn tanks_mut(&mut self) -> [&mut Tank; 2] {
        match self {
            Self::Engine(m) => [&mut m.input, &mut m.output],
            Self::Turbine(m) => [&mut m.input, &mut m.output],
            Self::Condenser(m) => [&mut m.input, &mut m.output],
        }
    }

    /// The ones with a machine file (the plain condenser has none).
    pub fn slotted(&mut self) -> Option<&mut dyn Slotted> {
        match self {
            Self::Engine(m) => Some(m),
            Self::Turbine(m) => Some(m),
            Self::Condenser(m) if m.powered => Some(m),
            Self::Condenser(_) => None,
        }
    }

    /// HE out this tick (generators).
    fn out(&self) -> u64 {
        match self {
            Self::Engine(m) => m.buffer,
            Self::Turbine(m) => m.buffer.max(0) as u64,
            Self::Condenser(_) => 0,
        }
    }
}

/// The generators offer their power; the powered condenser takes it.
impl EnergyNode for Steam {
    fn role(&self) -> Role {
        if matches!(self, Self::Condenser(_)) { Role::Consumer } else { Role::Provider }
    }
    fn offer(&self) -> u64 {
        self.out()
    }
    fn demand(&self) -> u64 {
        match self {
            Self::Condenser(c) if c.powered => CONDENSER_POWER.saturating_sub(c.power),
            _ => 0,
        }
    }
    fn give(&mut self, he: u64) {
        if let Self::Condenser(c) = self {
            c.power += he;
        }
    }
    fn take(&mut self, he: u64) {
        match self {
            Self::Engine(m) => m.buffer = m.buffer.saturating_sub(he),
            Self::Turbine(m) => m.buffer = (m.buffer - he.min(i64::MAX as u64) as i64).max(0),
            Self::Condenser(_) => {}
        }
    }
}

/// The input takes its type only (set by the lever, or fixed); the output
/// gives.
impl FluidNode for Steam {
    fn fluid_offer(&self, f: &str) -> u64 {
        let [_, out] = self.tanks();
        if out.fluid == f { u64::from(out.mb) } else { 0 }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        let [input, _] = self.tanks();
        if input.fluid == f { u64::from(input.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        self.tanks().into_iter().filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        let [input, _] = self.tanks_mut();
        input.fill(f, mb.min(u64::from(u32::MAX)) as u32);
    }
    fn fluid_take(&mut self, f: &str, mb: u64) {
        let [_, out] = self.tanks_mut();
        if out.fluid == f {
            out.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

impl Logic for Steam {
    fn tick(&mut self) -> bool {
        match self {
            Self::Engine(m) => m.step(),
            Self::Turbine(m) => m.step(),
            Self::Condenser(m) => m.step(),
        }
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        match self {
            Self::Condenser(c) if !c.powered => None,
            _ => Some(self),
        }
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        match self {
            Self::Engine(m) => fill_level(m.buffer, 3_400, max),
            Self::Turbine(m) if m.on => fill_level(m.buffer.max(1) as u64, m.max_power.max(m.buffer).max(1) as u64, max),
            Self::Turbine(_) => 0,
            Self::Condenser(c) => if c.on { max } else { 0 },
        }
    }
    fn view(&self) -> ViewModel {
        let [input, output] = self.tanks();
        let vm = match self {
            Self::Engine(m) => ViewModel::new("Motor a vapor").text(format!("{} HE/t", m.buffer), view::GREY),
            Self::Turbine(m) => {
                let vm = ViewModel::new(m.shape.title()).text(format!("{} HE/t", m.buffer.max(0)), view::GREY);
                let vm = if m.shape == Shape::Industrial {
                    vm.text(format!("Volante: {} HE, giro {:.0} %", m.flywheel, m.flywheel as f64 / FLYWHEEL * 100.0), view::GREY)
                } else {
                    vm
                };
                vm.button(actions::LEVER, "Alavanca: trocar o vapor")
            }
            Self::Condenser(c) if c.powered => ViewModel::new("Condensador a energia").bar("Energia", c.power, CONDENSER_POWER, "HE"),
            Self::Condenser(_) => ViewModel::new("Condensador"),
        };
        vm.text(input.line(), view::GREY).text(output.line(), view::GREY)
    }
    fn action(&mut self, action: u32, _held: Option<&Held>, _fx: &mut Vec<Effect>) {
        if let (Self::Turbine(t), actions::LEVER) = (self, action) {
            t.lever();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100 mB of steam into 1 of spent steam and 200 · 0.85 HE; its 20 mB
    /// output caps it at 20 operations a tick.
    #[test]
    fn the_steam_engine_turns_steam_into_power_its_output_allows() {
        let mut e = Steam::Engine(SteamEngine::default());
        e.fluid_give("steam", 2_000);
        e.tick();
        assert_eq!(e.offer(), 3_400);
        e.take(1_000);
        assert_eq!((e.offer(), e.fluid_offer("spentsteam"), e.fluid_offer("steam")), (2_400, 20, 0));
        e.tick();
        assert_eq!(e.offer(), 0, "saída cheia: nada");
    }

    /// The industrial turbine: a fifth of its steam a tick into the
    /// flywheel, which gives at least 5 % of its top output.
    #[test]
    fn the_industrial_turbine_spins_up_its_flywheel() {
        let mut t = Turbine::new(Shape::Industrial);
        t.input.fill("steam", 100_000);
        t.step();
        // ceil(100 000 · 0.2) / 100 = 200 operations, 40 000 HE.
        assert_eq!((t.input.mb, t.output.mb), (80_000, 200));
        // Top: ceil(750 000 · 0.2 / 100) = 1 500 operations, 300 000 HE.
        assert_eq!(t.max_power, 300_000);
        assert_eq!((t.buffer, t.flywheel), (15_000, 25_000));
        let mut out = 0;
        for _ in 0..200 {
            t.input.fill("steam", 20_000);
            t.step();
            out = t.buffer;
        }
        assert!(out > 15_000 && t.flywheel > 0, "o volante gira: {out} HE/t, {}", t.flywheel);
    }

    /// The Leviathan takes all its steam at 85 %, straight out; the dense
    /// steams wrap its int.
    #[test]
    fn the_leviathan_burns_all_its_steam_and_keeps_the_int_wrap() {
        let mut t = Turbine::new(Shape::Chungus);
        t.input.fill("steam", 10_000);
        t.step();
        assert_eq!((t.input.mb, t.output.mb, t.buffer), (0, 100, 17_000));
        let c = turbine_coolable("ultrahotsteam").expect("ultrahotsteam");
        let ops = (i32::MAX as u32 / c.heat) + 1;
        assert!(generation(ops, c, 0.85) < 0, "o int do NTM vira");
    }

    /// The lever: steam → hot (the industrial tanks a tenth) → … → steam.
    #[test]
    fn the_lever_cycles_the_steams_and_resizes_the_industrial_tanks() {
        let mut t = Turbine::new(Shape::Industrial);
        t.lever();
        assert_eq!((t.input.fluid.as_str(), t.output.fluid.as_str(), t.input.cap, t.output.cap), ("hotsteam", "steam", 75_000, 300_000));
        t.lever();
        t.lever();
        t.lever();
        assert_eq!((t.input.fluid.as_str(), t.input.cap, t.output.cap), ("steam", 750_000, 3_000_000));
        let mut c = Turbine::new(Shape::Chungus);
        c.lever();
        assert_eq!((c.input.fluid.as_str(), c.input.cap), ("hotsteam", 1_000_000_000));
        assert_eq!(c.screen_data(), vec![0, 1], "o modelo: parada, vapor quente");
    }

    /// The passive condenser turns all it can; the powered one only with
    /// 10 HE a mB.
    #[test]
    fn condensers_turn_spent_steam_into_water() {
        let mut c = Condenser::new(false);
        c.input.fill("spentsteam", 100);
        c.step();
        assert_eq!((c.input.mb, c.output.mb), (0, 100));
        let mut p = Steam::Condenser(Condenser::new(true));
        let Steam::Condenser(pc) = &mut p else { unreachable!() };
        pc.input.fill("spentsteam", 1_000);
        p.tick();
        let Steam::Condenser(pc) = &mut p else { unreachable!() };
        assert_eq!(pc.output.mb, 0, "sem energia, nada");
        p.give(9_500);
        p.tick();
        let Steam::Condenser(pc) = &p else { unreachable!() };
        assert_eq!((pc.output.mb, pc.power), (1_000, 0), "5 % de tolerância");
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;
    #[test]
    fn a_steam_engine_hits_once_per_rotor_revolution_and_saves_only_acceleration() {
        let mut e=SteamEngine::default();
        for tick in 1..=85 {
            e.input.mb=1000;e.output.mb=0;e.step();
            assert_eq!(e.sound_hit,tick==85,"Java acceleration ramps by .1 and the first turn finishes at tick 85");
        }
        let loaded:SteamEngine=serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(loaded.acceleration,e.acceleration);
        assert_eq!(loaded.rotor,0.);
        assert!(!loaded.sound_hit);
    }
}

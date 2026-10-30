//! The oil chain's batch C processors (docs/maquinas.md): the catalytic
//! cracker, the catalytic reformer, the hydrotreater and the vacuum
//! distiller (`tileentity/machine/oil/TileEntityMachine{CatalyticCracker,
//! CatalyticReformer,Hydrotreater,VacuumDistill}.java`). One machine with
//! four shapes: the recipe by the input tank's fluid (the first with a
//! recipe; the NTM types it with an identifier), a second input fixed by the
//! recipe (the cracker's steam, the hydrotreater's hydrogen), its outputs
//! typed by the recipe; every `every` ticks up to `ops` operations of 100 mB,
//! each paying its HE at once and wanting the catalytic converter where the
//! NTM does (never worn).
//!
//! Not here: the pressure the hydrotreater's hydrogen (1) and the vacuum
//! distiller's oil (2) need (nothing in the port compresses a fluid yet),
//! the containers' slots.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct Recipe {
    pub name: String,
    pub fin: Vec<(String, u32)>,
    pub fout: Vec<(String, u32)>,
    #[serde(default = "one")]
    pub every: u64,
    #[serde(default = "one32")]
    pub ops: u32,
    #[serde(default)]
    pub he_op: u64,
    #[serde(default)]
    pub catalyst: Option<String>,
}

fn one() -> u64 {
    1
}

fn one32() -> u32 {
    1
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Cracker,
    Reformer,
    Hydrotreater,
    Vacuum,
}

/// (input caps, output caps, max power, slots, battery slot, catalyst slot).
struct Spec {
    inputs: &'static [u32],
    outputs: &'static [u32],
    max_power: u64,
    slots: usize,
    battery: Option<usize>,
    catalyst: Option<usize>,
}

impl Shape {
    pub fn of(block: &str) -> Option<Self> {
        Some(match block {
            "ntm:machine_catalytic_cracker" => Shape::Cracker,
            "ntm:machine_catalytic_reformer" => Shape::Reformer,
            "ntm:machine_hydrotreater" => Shape::Hydrotreater,
            "ntm:machine_vacuum_distill" => Shape::Vacuum,
            _ => return None,
        })
    }

    pub fn recipes(self) -> &'static [Recipe] {
        static C: OnceLock<Table> = OnceLock::new();
        static R: OnceLock<Table> = OnceLock::new();
        static H: OnceLock<Table> = OnceLock::new();
        static V: OnceLock<Table> = OnceLock::new();
        let parse = |s: &str| serde_json::from_str::<Table>(s).expect("processing table");
        &match self {
            Shape::Cracker => C.get_or_init(|| parse(include_str!("../data/machines/cracking.json"))),
            Shape::Reformer => R.get_or_init(|| parse(include_str!("../data/machines/reforming.json"))),
            Shape::Hydrotreater => H.get_or_init(|| parse(include_str!("../data/machines/hydrotreating.json"))),
            Shape::Vacuum => V.get_or_init(|| parse(include_str!("../data/machines/vacuum_refinery.json"))),
        }
        .recipes
    }

    fn spec(self) -> Spec {
        match self {
            // No power and no slots; the spent steam's tank is 800 mB.
            Shape::Cracker => Spec { inputs: &[4_000, 8_000], outputs: &[4_000, 4_000, 800], max_power: 0, slots: 0, battery: None, catalyst: None },
            Shape::Reformer => Spec { inputs: &[64_000], outputs: &[24_000, 24_000, 24_000], max_power: 1_000_000, slots: 11, battery: Some(0), catalyst: Some(10) },
            Shape::Hydrotreater => Spec { inputs: &[64_000, 64_000], outputs: &[24_000, 24_000], max_power: 1_000_000, slots: 11, battery: Some(0), catalyst: Some(10) },
            Shape::Vacuum => Spec { inputs: &[64_000], outputs: &[24_000, 24_000, 24_000, 24_000], max_power: 1_000_000, slots: 12, battery: Some(0), catalyst: None },
        }
    }

    fn title(self) -> &'static str {
        match self {
            Shape::Cracker => "Craqueador catalítico",
            Shape::Reformer => "Reformador catalítico",
            Shape::Hydrotreater => "Hidrotratador",
            Shape::Vacuum => "Destilador a vácuo",
        }
    }

    pub fn recipe(self, fluid: &str) -> Option<&'static Recipe> {
        self.recipes().iter().find(|r| r.fin.first().is_some_and(|f| f.0 == fluid))
    }
}

pub const CATALYST: &str = "ntm:catalytic_converter";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Processor {
    pub shape: Shape,
    #[serde(skip)]
    pub slots: Slots,
    pub power: u64,
    pub inputs: Vec<Tank>,
    pub outputs: Vec<Tank>,
    /// Ticks run, for the interval.
    #[serde(default)]
    pub ticks: u64,
    #[serde(skip)]
    pub on: bool,
}

impl Processor {
    pub fn new(shape: Shape) -> Self {
        let s = shape.spec();
        Self {
            shape,
            slots: vec![None; s.slots],
            power: 0,
            inputs: s.inputs.iter().map(|&c| Tank::new(c)).collect(),
            outputs: s.outputs.iter().map(|&c| Tank::new(c)).collect(),
            ticks: 0,
            on: false,
        }
    }

    fn recipe(&self) -> Option<&'static Recipe> {
        self.shape.recipe(&self.inputs[0].fluid)
    }

    /// The tanks typed by the recipe (`setupTanks`); without one, the
    /// outputs lose their types.
    fn setup(&mut self) {
        match self.recipe() {
            Some(r) => {
                for (t, f) in self.inputs.iter_mut().zip(&r.fin).skip(1) {
                    t.set_type(&f.0);
                }
                for (t, f) in self.outputs.iter_mut().zip(&r.fout) {
                    t.set_type(&f.0);
                }
            }
            None => {
                for t in &mut self.outputs {
                    t.set_type("");
                }
            }
        }
    }

    fn has_catalyst(&self) -> bool {
        self.shape.spec().catalyst.is_none_or(|i| self.slots.get(i).and_then(|s| s.as_ref()).is_some_and(|s| s.id == CATALYST))
    }

    /// One operation if everything is there: true when it ran.
    fn operate(&mut self, r: &Recipe) -> bool {
        if self.power < r.he_op || !self.has_catalyst() {
            return false;
        }
        if self.inputs.iter().zip(&r.fin).any(|(t, f)| t.mb < f.1) {
            return false;
        }
        if self.outputs.iter().zip(&r.fout).any(|(t, f)| t.mb + f.1 > t.cap) {
            return false;
        }
        for (t, f) in self.inputs.iter_mut().zip(&r.fin) {
            t.drain(f.1);
        }
        for (t, f) in self.outputs.iter_mut().zip(&r.fout) {
            t.fill(&f.0, f.1);
        }
        self.power -= r.he_op;
        true
    }

    fn tank_values(t: &Tank) -> [i32; 3] {
        [t.mb as i32, t.cap as i32, crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)]
    }

    /// The screen's values: [0]/[1] power, [2] the working lamp, [3] the
    /// catalyst in, then each tank (amount, capacity, colour), the inputs
    /// first.
    pub fn screen_data(&self) -> Vec<i32> {
        let s = self.shape.spec();
        let mut v = vec![self.power.min(i32::MAX as u64) as i32, s.max_power as i32, i32::from(self.on), i32::from(self.has_catalyst())];
        for t in self.inputs.iter().chain(&self.outputs) {
            v.extend(Self::tank_values(t));
        }
        v
    }
}

impl EnergyNode for Processor {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        self.shape.spec().max_power.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

/// The first input takes a fluid with a recipe (the first while empty);
/// the second its recipe's; the outputs give.
impl FluidNode for Processor {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.outputs.iter().filter(|t| t.fluid == f).map(|t| u64::from(t.mb)).sum()
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        let first = &self.inputs[0];
        if (first.fluid == f || first.mb == 0) && self.shape.recipe(f).is_some() {
            return u64::from(first.free());
        }
        self.inputs.iter().skip(1).filter(|t| t.fluid == f).map(|t| u64::from(t.free())).sum()
    }
    fn fluid_types(&self) -> Vec<String> {
        self.inputs.iter().chain(&self.outputs).filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn accepts_any(&self) -> bool {
        self.inputs[0].mb == 0
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        let mb = mb.min(u64::from(u32::MAX)) as u32;
        if let Some(t) = self.inputs.iter_mut().skip(1).find(|t| t.fluid == f) {
            t.fill(f, mb);
            return;
        }
        let first = &mut self.inputs[0];
        if first.mb == 0 {
            first.set_type(f);
        }
        first.fill(f, mb);
    }
    fn fluid_take(&mut self, f: &str, mut mb: u64) {
        for t in self.outputs.iter_mut().filter(|t| t.fluid == f) {
            let n = mb.min(u64::from(t.mb)) as u32;
            t.drain(n);
            mb -= u64::from(n);
        }
    }
}

impl Logic for Processor {
    fn tick(&mut self) -> bool {
        let s = self.shape.spec();
        let before = self.power;
        if let Some(b) = s.battery {
            self.power = crate::machine::charge_from_slot(&self.slots, b, self.power, s.max_power);
        }
        self.ticks += 1;
        self.setup();
        self.on = false;
        if let Some(r) = self.recipe()
            && self.ticks.is_multiple_of(r.every)
        {
            for _ in 0..r.ops {
                if !self.operate(r) {
                    break;
                }
                self.on = true;
            }
        }
        self.on || before != self.power
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        if self.shape.spec().max_power > 0 { Some(self) } else { None }
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.on { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        let mut vm = ViewModel::new(self.shape.title());
        if self.shape.spec().max_power > 0 {
            vm = vm.bar("Energia", self.power, self.shape.spec().max_power, "HE");
        }
        for t in self.inputs.iter().chain(&self.outputs) {
            vm = vm.text(t.line(), view::GREY);
        }
        vm
    }
}

impl Slotted for Processor {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Processor::screen_data(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::Stack;

    /// Crude oil and steam: two operations every 5 ticks, 100 mB and 200 of
    /// steam each, into cracked oil, petroleum gas and 2 of spent steam.
    #[test]
    fn the_cracker_runs_twice_every_five_ticks_on_steam() {
        let mut p = Processor::new(Shape::Cracker);
        p.fluid_give("oil", 1_000);
        assert_eq!(p.fluid_demand("steam"), 0, "o vapor só depois da receita");
        p.tick();
        assert_eq!(p.inputs[1].fluid, "steam");
        p.fluid_give("steam", 8_000);
        for _ in 0..9 {
            p.tick();
        }
        // Ticks 5 and 10: 4 operations.
        assert_eq!((p.inputs[0].mb, p.inputs[1].mb), (600, 7_200));
        let out: Vec<_> = p.outputs.iter().map(|t| (t.fluid.as_str(), t.mb)).collect();
        assert_eq!(out, vec![("crackoil", 320), ("petroleum", 80), ("spentsteam", 8)]);
    }

    /// Heating oil: 20 000 HE an operation, the catalytic converter in slot
    /// 10 (kept).
    #[test]
    fn the_reformer_wants_its_catalyst_and_pays_per_operation() {
        let mut p = Processor::new(Shape::Reformer);
        p.fluid_give("heatingoil", 1_000);
        p.power = 50_000;
        p.tick();
        assert_eq!(p.inputs[0].mb, 1_000, "sem catalisador");
        p.slots[10] = Some(Stack::new(CATALYST, 1));
        p.tick();
        p.tick();
        p.tick();
        assert_eq!((p.inputs[0].mb, p.power), (800, 10_000), "duas operações, depois falta energia");
        assert_eq!(p.outputs[2].fluid, "hydrogen");
        assert_eq!(p.slots[10], Some(Stack::new(CATALYST, 1)));
    }

    /// Crude oil and hydrogen every 2 ticks; the vacuum distiller's four
    /// outputs.
    #[test]
    fn hydrotreater_and_vacuum_distiller() {
        let mut h = Processor::new(Shape::Hydrotreater);
        h.slots[10] = Some(Stack::new(CATALYST, 1));
        h.power = 1_000_000;
        h.fluid_give("oil", 1_000);
        h.tick();
        h.fluid_give("hydrogen", 100);
        for _ in 0..3 {
            h.tick();
        }
        assert_eq!((h.inputs[0].mb, h.inputs[1].mb, h.outputs[0].fluid.as_str(), h.outputs[0].mb), (800, 90, "oil_ds", 180));
        let mut v = Processor::new(Shape::Vacuum);
        v.power = 1_000_000;
        v.fluid_give("oil", 1_000);
        v.tick();
        let out: Vec<_> = v.outputs.iter().map(|t| (t.fluid.as_str(), t.mb)).collect();
        assert_eq!(out, vec![("heavyoil_vacuum", 40), ("reformate", 25), ("lightoil_vacuum", 20), ("sourgas", 15)]);
    }
}

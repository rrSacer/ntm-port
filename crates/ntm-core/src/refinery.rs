//! The oil refinery, oil batch B (docs/maquinas.md):
//! `tileentity/machine/oil/TileEntityMachineRefinery.java`, its recipes
//! `RefineryRecipes` (4: hot oil and its cracked and desulfurized kinds).
//!
//! Every tick with 5 HE and 100 mB in: the four fractions out, and every
//! 10th time the recipe's solid (sulfur, tar) to slot 11. Its input tank
//! takes the first fluid with a recipe (the NTM types it with the
//! identifier in slot 12); the four outputs take the recipe's fluids.
//! Not here: the explosion and the fire (`IOverpressurable`, nothing in the
//! port overpressures a machine yet), the containers of slots 1–10.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub mod slot {
    pub const BATTERY: usize = 0;
    pub const SOLID: usize = 11;
    pub const COUNT: usize = 13;
}

pub const MAX_POWER: u64 = 1_000;
pub const HE_PER_TICK: u64 = 5;
pub const INPUT_MB: u32 = 100;
pub const INPUT_CAP: u32 = 64_000;
pub const OUTPUT_CAP: u32 = 24_000;
/// `maxSulfur`: refinings a solid.
pub const SOLID_EVERY: u32 = 10;

#[derive(Deserialize)]
pub struct Recipe {
    pub name: String,
    pub fin: Vec<(String, u32)>,
    pub fout: Vec<(String, u32)>,
    /// (item, count, every): the solid.
    #[serde(default)]
    pub periodic: Vec<(String, u32, u32)>,
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn recipes() -> &'static [Recipe] {
    static T: OnceLock<Table> = OnceLock::new();
    &T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/refinery.json")).expect("refinery.json")).recipes
}

pub fn recipe(fluid: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|r| r.fin.first().is_some_and(|f| f.0 == fluid))
}

fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Refinery {
    /// The engine's slots, laid over on the first tick.
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    pub power: u64,
    pub input: Tank,
    pub outputs: [Tank; 4],
    /// Refinings toward the next solid (`sulfur`).
    pub sulfur: u32,
    /// Refined this tick (`isOn`).
    #[serde(skip)]
    pub on: bool,
}

impl Default for Refinery {
    fn default() -> Self {
        Self {
            slots: empty_slots(),
            power: 0,
            input: Tank::new(INPUT_CAP),
            outputs: std::array::from_fn(|_| Tank::new(OUTPUT_CAP)),
            sulfur: 0,
            on: false,
        }
    }
}

impl Refinery {
    /// `refine`: the outputs typed by the recipe, then one refining if it
    /// can. True when something changed.
    pub fn refine(&mut self) -> bool {
        self.on = false;
        let Some(r) = recipe(&self.input.fluid) else {
            let typed = self.outputs.iter().any(|t| !t.fluid.is_empty());
            for t in &mut self.outputs {
                t.set_type("");
            }
            return typed;
        };
        for (t, f) in self.outputs.iter_mut().zip(&r.fout) {
            t.set_type(&f.0);
        }
        if self.power < HE_PER_TICK || self.input.mb < INPUT_MB {
            return false;
        }
        if self.outputs.iter().zip(&r.fout).any(|(t, f)| t.mb + f.1 > t.cap) {
            return false;
        }
        self.on = true;
        self.input.drain(INPUT_MB);
        for (t, f) in self.outputs.iter_mut().zip(&r.fout) {
            t.fill(&f.0, f.1);
        }
        self.sulfur += 1;
        if self.sulfur >= SOLID_EVERY {
            self.sulfur -= SOLID_EVERY;
            if let Some((item, n, _)) = r.periodic.first() {
                match &mut self.slots[slot::SOLID] {
                    None => self.slots[slot::SOLID] = Some(Stack::new(item, *n)),
                    // Another item, or no room: the solid is lost, as in the NTM.
                    Some(s) if s.id == *item && s.count + n <= 64 => s.count += n,
                    Some(_) => {}
                }
            }
        }
        self.power -= HE_PER_TICK;
        true
    }

    fn tank_values(t: &Tank) -> [i32; 3] {
        [t.mb as i32, t.cap as i32, crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)]
    }

    /// The screen's values (agreed with the visual agent): [0]/[1] power,
    /// [2] the working lamp, [4..6] the input, [7..18] the four outputs,
    /// [19]/[20] exploded and tilted (0: neither happens in the port yet).
    pub fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![self.power as i32, MAX_POWER as i32, i32::from(self.on), 0];
        v.extend(Self::tank_values(&self.input));
        for t in &self.outputs {
            v.extend(Self::tank_values(t));
        }
        v.extend([0, 0]);
        v
    }
}

impl EnergyNode for Refinery {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        MAX_POWER.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

/// The input takes a fluid with a recipe (the first while empty); the four
/// outputs give.
impl FluidNode for Refinery {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.outputs.iter().filter(|t| t.fluid == f).map(|t| u64::from(t.mb)).sum()
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        let empty = self.input.mb == 0;
        if (self.input.fluid == f || empty) && recipe(f).is_some() { u64::from(self.input.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        std::iter::once(&self.input).chain(&self.outputs).filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn accepts_any(&self) -> bool {
        self.input.mb == 0
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.input.mb == 0 {
            self.input.set_type(f);
        }
        self.input.fill(f, mb.min(u64::from(u32::MAX)) as u32);
    }
    fn fluid_take(&mut self, f: &str, mut mb: u64) {
        for t in self.outputs.iter_mut().filter(|t| t.fluid == f) {
            let n = mb.min(u64::from(t.mb)) as u32;
            t.drain(n);
            mb -= u64::from(n);
        }
    }
}

impl Logic for Refinery {
    fn tick(&mut self) -> bool {
        let before = self.power;
        self.power = crate::machine::charge_from_slot(&self.slots, slot::BATTERY, self.power, MAX_POWER);
        self.refine() || before != self.power
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.on { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        let mut vm = ViewModel::new("Refinaria").bar("Energia", self.power, MAX_POWER, "HE");
        for t in std::iter::once(&self.input).chain(&self.outputs) {
            vm = vm.text(t.line(), view::GREY);
        }
        vm
    }
}

impl Slotted for Refinery {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Refinery::screen_data(self)
    }
    fn save_state(&self) -> Vec<i32> {
        vec![self.sulfur as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [sulfur, ..] = v {
            self.sulfur = (*sulfur).clamp(0, SOLID_EVERY as i32 - 1) as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100 mB of hot oil a tick at 5 HE into its four fractions; a sulfur
    /// every 10.
    #[test]
    fn hot_oil_refines_into_four_fractions_and_sulfur() {
        let mut r = Refinery::default();
        r.slots[slot::BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        assert_eq!(r.fluid_demand("water"), 0, "sem receita");
        r.fluid_give("hotoil", 2_000);
        for _ in 0..25 {
            r.tick();
        }
        let got: Vec<_> = r.outputs.iter().map(|t| (t.fluid.as_str(), t.mb)).collect();
        assert_eq!(got, vec![("heavyoil", 1_000), ("naphtha", 500), ("lightoil", 300), ("petroleum", 200)]);
        assert_eq!((r.input.mb, r.sulfur), (0, 0));
        assert_eq!(r.slots[slot::SOLID], Some(Stack::new("ntm:sulfur", 2)));
        assert_eq!(r.fluid_offer("naphtha"), 500);
        r.fluid_take("naphtha", 200);
        assert_eq!(r.outputs[1].mb, 300);
    }

    /// A full output stops it; without power nothing runs.
    #[test]
    fn a_full_output_or_no_power_stops_it() {
        let mut r = Refinery::default();
        r.fluid_give("hotoil", 1_000);
        assert!(!r.refine(), "sem energia");
        r.power = MAX_POWER;
        r.outputs[3].set_type("petroleum");
        r.outputs[3].mb = OUTPUT_CAP - 5;
        assert!(!r.refine());
        assert_eq!(r.input.mb, 1_000);
    }
}

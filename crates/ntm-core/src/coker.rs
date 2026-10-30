//! The coker of the oil chain's batch C (docs/maquinas.md):
//! `tileentity/machine/oil/TileEntityMachineCoker.java`, its recipes
//! `CokerRecipes` (33).
//!
//! Heated from below like the boilers (a quarter of the difference a tick,
//! up to 100 000 TU): each tick a hundredth of its heat goes into the
//! progress, and every 20 000 one coking, the recipe's fluid into its coke
//! (slot 1) and byproduct. Its input takes the first fluid with a recipe
//! (the NTM types it with the identifier in slot 0).

use crate::{
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub mod slot {
    pub const OUTPUT: usize = 1;
    pub const COUNT: usize = 2;
}

pub const MAX_HEAT: u32 = 100_000;
pub const PROCESS: u32 = 20_000;
pub const DIFFUSION: f64 = 0.25;

#[derive(Deserialize)]
pub struct Recipe {
    pub fin: Vec<(String, u32)>,
    pub out: Vec<(String, u32, f32)>,
    pub fout: Vec<(String, u32)>,
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn recipes() -> &'static [Recipe] {
    static T: OnceLock<Table> = OnceLock::new();
    &T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/coker.json")).expect("coker.json")).recipes
}

pub fn recipe(fluid: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|r| r.fin.first().is_some_and(|f| f.0 == fluid))
}

fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Coker {
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    pub heat: u32,
    pub progress: u32,
    pub input: Tank,
    pub output: Tank,
    #[serde(skip)]
    pub on: bool,
}

impl Default for Coker {
    fn default() -> Self {
        Self { slots: empty_slots(), heat: 0, progress: 0, input: Tank::new(16_000), output: Tank::new(8_000), on: false }
    }
}

impl Coker {
    /// `tryPullHeat`: full, it takes none; else a quarter of the
    /// difference; nothing to take, it cools by a thousandth.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        if self.heat >= MAX_HEAT {
            return 0;
        }
        if let Some(s) = source {
            if s == self.heat {
                return 0;
            }
            if s > self.heat {
                let diff = (f64::from(s - self.heat) * DIFFUSION).ceil() as u32;
                self.heat = (self.heat + diff).min(MAX_HEAT);
                return diff;
            }
        }
        self.heat = self.heat.saturating_sub((self.heat / 1000).max(1));
        0
    }

    /// `canProcess`: the fluid, room for the byproduct, the coke piling.
    fn can_process(&mut self, r: &Recipe) -> bool {
        if let Some((f, _)) = r.fout.first() {
            self.output.set_type(f);
        }
        if self.input.mb < r.fin[0].1 {
            return false;
        }
        if r.fout.first().is_some_and(|(_, n)| self.output.mb + n > self.output.cap) {
            return false;
        }
        match (r.out.first(), &self.slots[slot::OUTPUT]) {
            (Some((id, n, _)), Some(s)) => s.id == *id && s.count + n <= 64,
            _ => true,
        }
    }

    fn step(&mut self) -> bool {
        self.on = false;
        let Some(r) = recipe(&self.input.fluid) else { return false };
        if !self.can_process(r) {
            return false;
        }
        let burn = self.heat / 100;
        if burn == 0 {
            return false;
        }
        self.on = true;
        self.progress += burn;
        self.heat -= burn;
        if self.progress >= PROCESS {
            self.progress -= PROCESS;
            if let Some((id, n, _)) = r.out.first() {
                match &mut self.slots[slot::OUTPUT] {
                    Some(s) => s.count += n,
                    None => self.slots[slot::OUTPUT] = Some(Stack::new(id, *n)),
                }
            }
            if let Some((f, n)) = r.fout.first() {
                self.output.fill(f, *n);
            }
            self.input.drain(r.fin[0].1);
        }
        true
    }

    fn tank_values(t: &Tank) -> [i32; 3] {
        [t.mb as i32, t.cap as i32, crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)]
    }

    /// The screen's values: [0]/[1] heat, [2]/[3] progress, [4] working,
    /// [5..7] the input, [8..10] the byproduct.
    pub fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![self.heat as i32, MAX_HEAT as i32, self.progress as i32, PROCESS as i32, i32::from(self.on)];
        v.extend(Self::tank_values(&self.input));
        v.extend(Self::tank_values(&self.output));
        v
    }
}

impl FluidNode for Coker {
    fn fluid_offer(&self, f: &str) -> u64 {
        if self.output.fluid == f { u64::from(self.output.mb) } else { 0 }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if (self.input.fluid == f || self.input.mb == 0) && recipe(f).is_some() { u64::from(self.input.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        [&self.input, &self.output].into_iter().filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
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
    fn fluid_take(&mut self, f: &str, mb: u64) {
        if self.output.fluid == f {
            self.output.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

impl Logic for Coker {
    /// The heat is pulled by the server first (it reads the block under).
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.on { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Coqueificadora")
            .bar("Calor", u64::from(self.heat), u64::from(MAX_HEAT), "TU")
            .bar("Progresso", u64::from(self.progress), u64::from(PROCESS), "")
            .text(self.input.line(), view::GREY)
            .text(self.output.line(), view::GREY)
    }
}

impl Slotted for Coker {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Coker::screen_data(self)
    }
    fn save_state(&self) -> Vec<i32> {
        vec![self.progress as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [p, ..] = v {
            self.progress = (*p).clamp(0, PROCESS as i32) as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Heavy oil, 11 000 mB a coke: the progress is a hundredth of the
    /// heat a tick, 20 000 a coking.
    #[test]
    fn heat_cokes_heavy_oil_into_petroleum_coke() {
        let mut c = Coker::default();
        c.fluid_give("heavyoil", 12_000);
        c.heat = MAX_HEAT;
        let mut t = 0;
        while c.slots[slot::OUTPUT].is_none() && t < 1_000 {
            c.tick();
            t += 1;
        }
        assert_eq!(c.slots[slot::OUTPUT], Some(Stack::new("ntm:coke__petroleum", 1)));
        assert_eq!((c.input.mb, c.output.fluid.as_str(), c.output.mb), (1_000, "oil_coker", 1_100));
        // 1 000 then a bit less each tick: about 22 ticks of a full buffer.
        assert!((20..=25).contains(&t), "{t}");
        c.heat = MAX_HEAT;
        assert_eq!(c.pull_heat(Some(200_000)), 0, "cheio");
    }
}

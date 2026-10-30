//! The fractioning tower, oil batch B (docs/maquinas.md):
//! `tileentity/machine/oil/TileEntityMachineFractionTower.java`, its
//! recipes `FractionRecipes` (19).
//!
//! Every 10 ticks, 100 mB of its input into two fractions; no power. Its
//! input takes the first fluid with a recipe (the NTM types it with the
//! identifier); the two outputs take the recipe's fluids. Towers stack, one
//! 3 blocks above another: each tick the lower one pushes its input up and
//! pulls the fractions down (`stack`, from `server/oil_server.rs`).

use crate::{
    fluid::{FluidNode, Tank},
    machine::Logic,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const CAP: u32 = 4_000;
pub const INPUT_MB: u32 = 100;
/// Ticks between fractionings (`getTotalWorldTime() % 10`).
pub const EVERY: u64 = 10;
/// The tower above, this many blocks up.
pub const STACK_DY: i32 = 3;

#[derive(Deserialize)]
pub struct Recipe {
    pub fin: Vec<(String, u32)>,
    pub fout: Vec<(String, u32)>,
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn recipes() -> &'static [Recipe] {
    static T: OnceLock<Table> = OnceLock::new();
    &T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/fraction.json")).expect("fraction.json")).recipes
}

pub fn recipe(fluid: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|r| r.fin.first().is_some_and(|f| f.0 == fluid))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FractionTower {
    /// The input and its two fractions.
    pub tanks: [Tank; 3],
    /// Ticks run, for the fractioning every 10.
    #[serde(default)]
    pub ticks: u64,
}

impl Default for FractionTower {
    fn default() -> Self {
        Self { tanks: std::array::from_fn(|_| Tank::new(CAP)), ticks: 0 }
    }
}

impl FractionTower {
    /// `setupTanks`: the fractions typed by the input's recipe; without
    /// one, the tower empties of types (and of what they held).
    fn setup(&mut self) {
        match recipe(&self.tanks[0].fluid) {
            Some(r) => {
                for (t, f) in self.tanks[1..].iter_mut().zip(&r.fout) {
                    t.set_type(&f.0);
                }
            }
            None => {
                for t in &mut self.tanks {
                    t.set_type("");
                }
            }
        }
    }

    /// `fractionate`: 100 mB into the two fractions, room in both.
    fn fractionate(&mut self) -> bool {
        let Some(r) = recipe(&self.tanks[0].fluid) else { return false };
        let (l, rr) = (r.fout[0].1, r.fout[1].1);
        if self.tanks[0].mb < INPUT_MB || self.tanks[1].mb + l > CAP || self.tanks[2].mb + rr > CAP {
            return false;
        }
        self.tanks[0].drain(INPUT_MB);
        let (a, b) = (r.fout[0].0.clone(), r.fout[1].0.clone());
        self.tanks[1].fill(&a, l);
        self.tanks[2].fill(&b, rr);
        true
    }

    /// The lower tower's exchange with the one above: types made equal,
    /// the input up, the fractions down.
    pub fn stack(lower: &mut FractionTower, upper: &mut FractionTower) -> bool {
        for i in 0..3 {
            let f = lower.tanks[i].fluid.clone();
            upper.tanks[i].set_type(&f);
        }
        let oil = lower.tanks[0].mb.min(upper.tanks[0].free());
        let left = upper.tanks[1].mb.min(lower.tanks[1].free());
        let right = upper.tanks[2].mb.min(lower.tanks[2].free());
        lower.tanks[0].mb -= oil;
        lower.tanks[1].mb += left;
        lower.tanks[2].mb += right;
        upper.tanks[0].mb += oil;
        upper.tanks[1].mb -= left;
        upper.tanks[2].mb -= right;
        oil + left + right > 0
    }
}

/// The input takes a fluid with a recipe (the first while empty); the two
/// fractions give.
impl FluidNode for FractionTower {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.tanks[1..].iter().filter(|t| t.fluid == f).map(|t| u64::from(t.mb)).sum()
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        let t = &self.tanks[0];
        if (t.fluid == f || t.mb == 0) && recipe(f).is_some() { u64::from(t.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        self.tanks.iter().filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn accepts_any(&self) -> bool {
        self.tanks[0].mb == 0
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.tanks[0].mb == 0 {
            self.tanks[0].set_type(f);
        }
        self.tanks[0].fill(f, mb.min(u64::from(u32::MAX)) as u32);
    }
    fn fluid_take(&mut self, f: &str, mut mb: u64) {
        for t in self.tanks[1..].iter_mut().filter(|t| t.fluid == f) {
            let n = mb.min(u64::from(t.mb)) as u32;
            t.drain(n);
            mb -= u64::from(n);
        }
    }
}

impl Logic for FractionTower {
    fn tick(&mut self) -> bool {
        self.ticks += 1;
        self.setup();
        self.ticks.is_multiple_of(EVERY) && self.fractionate()
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        let mut vm = ViewModel::new("Torre de fração");
        for t in &self.tanks {
            vm = vm.text(t.line(), view::GREY);
        }
        vm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Heavy oil: 100 mB every 10 ticks into 30 of bitumen and 70 of smear.
    #[test]
    fn heavy_oil_splits_into_bitumen_and_smear() {
        let mut t = FractionTower::default();
        assert_eq!(t.fluid_demand("water"), 0);
        t.fluid_give("heavyoil", 1_000);
        for _ in 0..30 {
            t.tick();
        }
        assert_eq!((t.tanks[0].mb, t.tanks[1].fluid.as_str(), t.tanks[1].mb, t.tanks[2].fluid.as_str(), t.tanks[2].mb), (700, "bitumen", 90, "smear", 210));
    }

    /// Stacked: the input goes up, the fractions come down.
    #[test]
    fn a_stacked_tower_takes_the_input_up_and_gives_the_fractions_down() {
        let (mut low, mut high) = (FractionTower::default(), FractionTower::default());
        low.fluid_give("heavyoil", 3_000);
        low.tick();
        high.tanks[1].set_type("bitumen");
        high.tanks[1].mb = 300;
        assert!(FractionTower::stack(&mut low, &mut high));
        assert_eq!((low.tanks[0].mb, high.tanks[0].mb, high.tanks[0].fluid.as_str()), (0, 3_000, "heavyoil"));
        assert_eq!((low.tanks[1].mb, high.tanks[1].mb), (300, 0));
    }
}

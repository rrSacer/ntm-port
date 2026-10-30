//! The boilers of the oil chain's batch C (docs/maquinas.md):
//! `tileentity/machine/TileEntityHeatBoiler.java` and
//! `TileEntityHeatBoilerIndustrial.java`, their conversions the first
//! heating step of each fluid the boiler heats (`FT_Heatable`, efficiency 1:
//! water into steam, the crude oils into their hot kinds, air into a blast).
//!
//! The heat comes from the source under it (`server/oil_server.rs`: a
//! firebox), a share of the difference a tick; without it, it cools. Every
//! tick as many conversions as the input, the room out and the heat allow.
//! Its input takes the first fluid it heats (the NTM types it with an
//! identifier). Not here: the small boiler's explosion when its output is
//! full (it just stops), the sky's heat after a nuclear winter's fires.

use crate::{
    fluid::{FluidNode, Tank},
    machine::Logic,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct Step {
    pub fin: Vec<(String, u32)>,
    pub fout: Vec<(String, u32)>,
    pub heat: u32,
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Step>,
}

pub fn steps() -> &'static [Step] {
    static T: OnceLock<Table> = OnceLock::new();
    &T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/boiler.json")).expect("boiler.json")).recipes
}

pub fn step(fluid: &str) -> Option<&'static Step> {
    steps().iter().find(|s| s.fin.first().is_some_and(|f| f.0 == fluid))
}

pub const DIFFUSION: f64 = 0.1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Boiler {
    pub industrial: bool,
    pub heat: u32,
    pub input: Tank,
    pub output: Tank,
    #[serde(skip)]
    pub on: bool,
}

impl Boiler {
    pub fn new(industrial: bool) -> Self {
        let cap = if industrial { 64_000 } else { 16_000 };
        Self { industrial, heat: 0, input: Tank::new(cap), output: Tank::new(cap), on: false }
    }

    pub fn max_heat(&self) -> u32 {
        if self.industrial { 12_800_000 } else { 3_200_000 }
    }

    /// `tryPullHeat`: from `source` (the heat stored under it), a share of
    /// the difference; nothing to take, it cools by a thousandth. Returns
    /// the heat taken.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        if let Some(s) = source {
            if s == self.heat {
                return 0;
            }
            if s > self.heat {
                let diff = (f64::from(s - self.heat) * DIFFUSION).ceil() as u32;
                let diff = diff.min(self.max_heat() - self.heat);
                self.heat += diff;
                return diff;
            }
        }
        self.heat = self.heat.saturating_sub((self.heat / 1000).max(1));
        0
    }

    /// `setupTanks`: the output typed by the input's step, sized by its
    /// ratio (16 000 mB of water make room for 1 600 000 of steam).
    fn setup(&mut self) {
        match step(&self.input.fluid) {
            Some(s) => {
                let (req, prod) = (s.fin[0].1, s.fout[0].1);
                self.output.set_type(&s.fout[0].0);
                self.output.cap = self.input.cap / req * prod;
            }
            None => {
                self.input.set_type("");
                self.output.set_type("");
            }
        }
    }

    /// `tryConvert`: as many conversions as input, room and heat allow.
    fn convert(&mut self) -> bool {
        let Some(s) = step(&self.input.fluid) else { return false };
        let (req, prod, heat) = (s.fin[0].1, s.fout[0].1, s.heat.max(1));
        let ops = (self.input.mb / req).min((self.output.cap - self.output.mb) / prod).min(self.heat / heat);
        if ops == 0 {
            return false;
        }
        self.input.drain(req * ops);
        let f = s.fout[0].0.clone();
        self.output.fill(&f, prod * ops);
        self.heat -= heat * ops;
        true
    }
}

/// The input takes a fluid it heats (the first while empty); the output gives.
impl FluidNode for Boiler {
    fn fluid_offer(&self, f: &str) -> u64 {
        if self.output.fluid == f { u64::from(self.output.mb) } else { 0 }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if (self.input.fluid == f || self.input.mb == 0) && step(f).is_some() { u64::from(self.input.free()) } else { 0 }
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

impl Logic for Boiler {
    /// The heat is pulled by the server first (it reads the block under).
    fn tick(&mut self) -> bool {
        self.setup();
        self.on = self.convert();
        self.on
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.on { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new(if self.industrial { "Caldeira industrial" } else { "Caldeira" })
            .bar("Calor", u64::from(self.heat), u64::from(self.max_heat()), "TU")
            .text(self.input.line(), view::GREY)
            .text(self.output.line(), view::GREY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Water into steam: 200 TU and 1 mB a time into 100 mB.
    #[test]
    fn water_boils_into_steam_by_the_heat_it_has() {
        let mut b = Boiler::new(false);
        assert_eq!(b.fluid_demand("lava"), 0);
        b.fluid_give("water", 1_000);
        b.heat = 2_100;
        b.tick();
        assert_eq!((b.input.mb, b.output.fluid.as_str(), b.output.mb, b.heat), (990, "steam", 1_000, 100));
        assert_eq!(b.output.cap, 1_600_000);
    }

    /// The heat: a tenth of the difference from the source, rounded up;
    /// without one, a thousandth lost (at least 1).
    #[test]
    fn heat_comes_a_tenth_at_a_time_and_goes_without_a_source() {
        let mut b = Boiler::new(false);
        assert_eq!(b.pull_heat(Some(100_000)), 10_000);
        assert_eq!(b.pull_heat(Some(100_000)), 9_000);
        b.pull_heat(None);
        assert_eq!(b.heat, 19_000 - 19);
    }
}

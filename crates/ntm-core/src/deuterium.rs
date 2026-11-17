//! Deuterium extractor (`ntm:machine_deuterium_extractor`,
//! `MachineDeuteriumExtractor.java`, `TileEntityDeuteriumExtractor.java`).
//!
//! Single-block machine with no GUI (`ILookOverlay` only) and `around` ports
//! for power, water input (`1 000 mB`), and heavy water output (`100 mB`).
//! Each tick with `power >= 500`, `water >= 100`, and room in the output tank,
//! it converts up to `2 mB` of heavy water (`50 mB` water per `1 mB` heavy
//! water, rounded before multiplication) and drains `500 HE`.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_deuterium_extractor";

/// `TileEntityDeuteriumExtractor.getMaxPower`: `10_000` HE.
pub const MAX_POWER: u64 = 10_000;
/// `getMaxPower() / 20`: `500` HE consumed per active tick.
pub const STEP_POWER: u64 = MAX_POWER / 20;
/// `tanks[0] = new FluidTank(Fluids.WATER, 1000)`.
pub const WATER_CAP: u32 = 1_000;
/// `hasEnoughWater()`: `tanks[0].getFill() >= 100`.
pub const WATER_MIN: u32 = 100;
/// `tanks[1] = new FluidTank(Fluids.HEAVYWATER, 100)`.
pub const HEAVYWATER_CAP: u32 = 100;
/// `50 mB` of water per `1 mB` of heavy water.
pub const WATER_PER_HEAVYWATER: u32 = 50;

pub const IN_FLUID: &str = "water";
pub const OUT_FLUID: &str = "heavywater";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeuteriumExtractor {
    pub power: u64,
    pub input: Tank,
    pub output: Tank,
    #[serde(skip)]
    pub on: bool,
}

impl Default for DeuteriumExtractor {
    fn default() -> Self {
        Self {
            power: 0,
            input: Tank::typed(IN_FLUID, WATER_CAP),
            output: Tank::typed(OUT_FLUID, HEAVYWATER_CAP),
            on: false,
        }
    }
}

impl DeuteriumExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_power(&self) -> bool {
        self.power >= STEP_POWER
    }

    pub fn has_enough_water(&self) -> bool {
        self.input.mb >= WATER_MIN
    }

    /// `TileEntityDeuteriumExtractor.updateEntity` (`TileEntityDeuteriumExtractor.java:40-48`).
    pub fn step(&mut self) -> bool {
        let was_power = self.power;
        let was_in = self.input.mb;
        let was_out = self.output.mb;
        let was_on = self.on;
        self.on = self.has_power() && self.has_enough_water() && self.output.free() > 0;
        if self.on {
            let convert = (self.input.mb.min(self.output.cap) / WATER_PER_HEAVYWATER)
                .min(self.output.free());
            self.input.drain(convert * WATER_PER_HEAVYWATER);
            self.output.fill(OUT_FLUID, convert);
            self.power -= STEP_POWER;
        }
        self.power != was_power
            || self.input.mb != was_in
            || self.output.mb != was_out
            || self.on != was_on
    }
}

impl Logic for DeuteriumExtractor {
    fn tick(&mut self) -> bool {
        self.step()
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
        ViewModel::new("Extrator de Deutério")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .text(self.input.line(), view::GREY)
            .text(self.output.line(), view::GREY)
    }
}

impl EnergyNode for DeuteriumExtractor {
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
        self.power = (self.power + he).min(MAX_POWER);
    }

    fn take(&mut self, _he: u64) {}
}

impl FluidNode for DeuteriumExtractor {
    fn fluid_offer(&self, fluid: &str) -> u64 {
        if fluid == OUT_FLUID && self.output.fluid == OUT_FLUID {
            u64::from(self.output.mb)
        } else {
            0
        }
    }

    fn fluid_demand(&self, fluid: &str) -> u64 {
        if fluid == IN_FLUID && self.input.fluid == IN_FLUID {
            u64::from(self.input.free())
        } else {
            0
        }
    }

    fn fluid_types(&self) -> Vec<String> {
        [&self.input, &self.output]
            .into_iter()
            .filter(|t| !t.fluid.is_empty())
            .map(|t| t.fluid.clone())
            .collect()
    }

    fn fluid_give(&mut self, fluid: &str, mb: u64) {
        if fluid == IN_FLUID {
            self.input.fill(fluid, mb.min(u64::from(u32::MAX)) as u32);
        }
    }

    fn fluid_take(&mut self, fluid: &str, mb: u64) {
        if fluid == OUT_FLUID && self.output.fluid == OUT_FLUID {
            self.output.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_constants_match_java() {
        let ex = DeuteriumExtractor::new();
        assert_eq!(MAX_POWER, 10_000);
        assert_eq!(STEP_POWER, 500);
        assert_eq!(WATER_CAP, 1_000);
        assert_eq!(WATER_MIN, 100);
        assert_eq!(HEAVYWATER_CAP, 100);
        assert_eq!(ex.input.fluid, "water");
        assert_eq!(ex.input.cap, 1_000);
        assert_eq!(ex.output.fluid, "heavywater");
        assert_eq!(ex.output.cap, 100);
        assert_eq!(ex.demand(), 10_000);
        assert_eq!(ex.offer(), 0);
        assert_eq!(ex.fluid_demand("water"), 1_000);
        assert_eq!(ex.fluid_demand("heavywater"), 0);
        assert_eq!(ex.fluid_offer("heavywater"), 0);
        assert_eq!(ex.fluid_types(), vec!["water".to_string(), "heavywater".to_string()]);
        assert_eq!(ex.level(5), 0);
    }

    #[test]
    fn threshold_gates_and_exact_50_to_1_conversion() {
        let mut ex = DeuteriumExtractor::new();
        // Below 500 HE: does not convert even with full water.
        ex.give(499);
        ex.fluid_give("water", 1_000);
        assert!(!ex.step());
        assert_eq!((ex.power, ex.input.mb, ex.output.mb, ex.on), (499, 1_000, 0, false));

        // 500 HE + 99 mB water (< 100 min): does NOT run and does NOT spend power.
        ex.give(1);
        ex.input.mb = 99;
        assert!(!ex.step());
        assert_eq!((ex.power, ex.input.mb, ex.output.mb, ex.on), (500, 99, 0, false));

        // 500 HE + 100 mB water -> 2 mB heavywater, 0 water, 0 power.
        ex.fluid_give("water", 1);
        assert!(ex.step());
        assert_eq!((ex.power, ex.input.mb, ex.output.mb, ex.on), (0, 0, 2, true));
        assert_eq!(ex.level(5), 5);

        // Next tick without power turns off and marks changed=true.
        assert!(ex.step());
        assert!(!ex.on);
        assert_eq!(ex.level(5), 0);
        assert!(!ex.step());
    }

    #[test]
    fn output_cap_and_partial_room_respect_java_rounding() {
        let mut ex = DeuteriumExtractor::new();
        ex.give(2_000);
        ex.fluid_give("water", 149);
        ex.output.mb = 99; // 1 mB free room
        assert!(ex.step());
        // convert = min(min(100, 149)/50 = 2, free = 1) = 1 -> drains 50 mB water, fills 1 mB, drains 500 HE.
        assert_eq!((ex.power, ex.input.mb, ex.output.mb, ex.on), (1_500, 99, 100, true));

        // Output tank full (free == 0): must NOT consume power or water even if water >= 100.
        ex.fluid_give("water", 100);
        assert!(ex.step()); // only `on` transitions true -> false
        assert_eq!((ex.power, ex.input.mb, ex.output.mb, ex.on), (1_500, 199, 100, false));
        assert!(!ex.step());
        assert_eq!((ex.power, ex.input.mb, ex.output.mb), (1_500, 199, 100));
    }

    #[test]
    fn rejects_wrong_fluids_and_serde_roundtrips_without_transient_on() {
        let mut ex = DeuteriumExtractor::new();
        ex.fluid_give("steam", 500);
        assert_eq!(ex.input.mb, 0);
        ex.fluid_give("water", 250);
        ex.give(1_250);
        assert!(ex.step());
        assert!(ex.on);
        assert_eq!(ex.fluid_offer("heavywater"), 2);
        ex.fluid_take("water", 1);
        assert_eq!(ex.fluid_offer("heavywater"), 2);

        let back: DeuteriumExtractor = serde_json::from_str(&serde_json::to_string(&ex).unwrap()).unwrap();
        assert_eq!(back.power, 750);
        assert_eq!(back.input.mb, 150);
        assert_eq!(back.output.mb, 2);
        assert!(!back.on);
    }
}

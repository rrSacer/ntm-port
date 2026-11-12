//! The firebox (`tileentity/machine/TileEntityHeaterFirebox.java`,
//! `TileEntityFireboxBase.java:73-124`): burns solid fuel into heat for
//! the machine above it (a crucible). Two fuel slots (the engine's), up to
//! 100 000 TU; it burns while below the maximum and, idle, loses
//! `max(heat/1000, 1)` a tick. The fuel it lights leaves its ash in the ashpit right under it (slice 6,
//! `crate::ashpit`). Left out: smoke and soot.
//!
//! The heating oven (`TileEntityHeaterOven.java`, energy batch 1) is the
//! same base with its own fuels (`crate::heater`: the burn module's mods and
//! a time mult of 0.125 already in), a cap of 500 000 TU, and it pulls the
//! heat of the source under it at half (`tryPullHeat`).

use super::{HeatSource, Tables};
use crate::{
    machine::{Logic, fill_level},
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const SLOTS: usize = 2;
pub const MAX_HEAT: u32 = 100_000;
pub const OVEN_MAX_HEAT: u32 = 500_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Firebox {
    pub slots: Slots,
    pub heat: u32,
    pub burn: u32,
    pub max_burn: u32,
    pub burn_heat: u32,
    pub on: bool,
    /// The heating oven.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub oven: bool,
    /// The ash of the fuel lit this tick, for the ashpit under it (`TileEntityFireboxBase.java:85-91`): its kind
    /// (`getAshFromFuel`) and its `baseTime`, the burn module's time before the oven's `getTimeMult`. The firebox's
    /// and the oven's modules are the same and the firebox's mult is 1: its table's time.
    #[serde(skip)]
    pub ash_out: Option<(crate::fuel::Ash, u32)>,
}

impl Default for Firebox {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], heat: 0, burn: 0, max_burn: 0, burn_heat: 0, on: false, oven: false, ash_out: None }
    }
}

impl Firebox {
    pub fn oven() -> Self {
        Self { oven: true, ..Default::default() }
    }

    pub fn max_heat(&self) -> u32 {
        if self.oven { OVEN_MAX_HEAT } else { MAX_HEAT }
    }

    /// The oven's `tryPullHeat`: what fits under its cap, all taken from the
    /// source, half of it kept. Returns the heat taken.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        let Some(s) = source.filter(|_| self.oven) else { return 0 };
        let pull = s.min(self.max_heat().saturating_sub(self.heat));
        self.heat = (f64::from(self.heat) + f64::from(pull) * 0.5) as u32;
        pull
    }

    /// One tick; true when its items changed.
    pub fn step(&mut self, t: &Tables) -> bool {
        let mut moved = false;
        self.on = false;
        let (fuels, max) = (if self.oven { &crate::heater::tables().oven_fuels } else { &t.firebox_fuels }, self.max_heat());
        if self.burn == 0 {
            for i in 0..SLOTS {
                let Some(s) = &mut self.slots[i] else { continue };
                let Some(f) = fuels.get(&s.id).filter(|f| f.ticks > 0) else { continue };
                self.ash_out = Some((crate::fuel::ash(&s.id), t.firebox_fuels.get(&s.id).map_or(f.ticks, |base| base.ticks)));
                self.burn = f.ticks;
                self.max_burn = f.ticks;
                self.burn_heat = f.heat;
                s.count -= 1;
                if s.count == 0 {
                    self.slots[i] = f.container.as_deref().map(|c| Stack::new(c, 1));
                }
                self.on = true;
                moved = true;
                break;
            }
        } else {
            if self.heat < max {
                self.burn -= 1;
            }
            self.on = true;
        }
        if self.on {
            self.heat = (self.heat + self.burn_heat).min(max);
        } else {
            self.heat = self.heat.saturating_sub((self.heat / 1000).max(1));
            self.burn_heat = 0;
        }
        moved
    }

    /// The screen's values: heat, its maximum, burn left, burn of the
    /// last fuel, TU a tick, burning.
    pub fn screen_data(&self) -> Vec<i32> {
        vec![self.heat as i32, self.max_heat() as i32, self.burn as i32, self.max_burn as i32, self.burn_heat as i32, i32::from(self.on)]
    }
}

impl Logic for Firebox {
    fn tick(&mut self) -> bool {
        let was = self.on;
        self.step(Tables::ntm()) || was != self.on
    }

    fn level(&self, max: u8) -> u8 {
        if self.on { fill_level(u64::from(self.heat).max(1), u64::from(self.max_heat()), max) } else { 0 }
    }

    /// No item screen here: the fuel goes in through the engine's slots.
    fn view(&self) -> ViewModel {
        ViewModel::new(if self.oven { "Forno de aquecimento" } else { "Fornalha" })
            .bar("Calor", u64::from(self.heat), u64::from(self.max_heat()), "TU")
            .text(format!("{} TU/t, {} s de queima", self.burn_heat, self.burn / 20), view::GREY)
            .text("O combustível entra pela tela com slots (motor com o patch ntm-visual).", view::GREY)
    }

    fn contents(&self) -> Vec<(String, u32)> {
        self.slots.iter().flatten().map(|s| (s.id.clone(), s.count)).collect()
    }
}

impl Slotted for Firebox {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// Its heat and the fuel burning (which came out of a slot).
    fn save_state(&self) -> Vec<i32> {
        vec![self.heat as i32, self.burn as i32, self.max_burn as i32, self.burn_heat as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [heat, burn, max, bh, ..] = v {
            self.heat = (*heat).max(0) as u32;
            self.burn = (*burn).max(0) as u32;
            self.max_burn = (*max).max(0) as u32;
            self.burn_heat = (*bh).max(0) as u32;
        }
    }
    fn screen_data(&self) -> Vec<i32> {
        Firebox::screen_data(self)
    }
}

impl HeatSource for Firebox {
    fn heat_stored(&self) -> u32 {
        self.heat
    }
    fn use_heat(&mut self, heat: u32) {
        self.heat = self.heat.saturating_sub(heat);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundry::testing::tables;

    #[test]
    fn coal_burns_into_heat_up_to_the_maximum() {
        let t = tables();
        let mut f = Firebox::default();
        f.slots[0] = Some(Stack::new("minecraft:coal", 2));
        assert!(f.step(&t));
        assert_eq!((f.heat, f.burn, f.slots[0].as_ref().map(|s| s.count)), (200, 2000, Some(1)));
        for _ in 0..1000 {
            f.step(&t);
        }
        // 200 TU a tick: full after 500 ticks; then the burn time waits.
        assert_eq!(f.heat, MAX_HEAT);
        let left = f.burn;
        f.step(&t);
        assert_eq!(f.burn, left, "cheio: não gasta tempo de queima");
        f.use_heat(50_000);
        f.step(&t);
        assert_eq!(f.burn, left - 1);
    }

    /// The oven: coal 250 ticks at 1 000 TU (the burn module's), a cap of
    /// 500 000, and half of what it pulls from under it.
    #[test]
    fn the_heating_oven_burns_its_own_fuels_and_pulls_half() {
        let t = tables();
        let mut o = Firebox::oven();
        o.slots[0] = Some(Stack::new("minecraft:coal", 1));
        o.step(&t);
        assert_eq!((o.heat, o.burn, o.burn_heat), (1_000, 250, 1_000));
        assert_eq!(o.pull_heat(Some(10_000)), 10_000);
        assert_eq!(o.heat, 6_000);
        o.heat = OVEN_MAX_HEAT - 100;
        assert_eq!(o.pull_heat(Some(10_000)), 100, "só o que cabe");
        assert_eq!(Firebox::default().pull_heat(Some(10_000)), 0, "a firebox não puxa");
    }

    #[test]
    fn a_lava_bucket_leaves_its_bucket_and_idle_heat_fades() {
        let t = tables();
        let mut f = Firebox::default();
        f.slots[1] = Some(Stack::new("minecraft:lava_bucket", 1));
        f.step(&t);
        assert_eq!(f.slots[1], Some(Stack::new("minecraft:bucket", 1)));
        let mut f = Firebox { heat: 10_000, ..Default::default() };
        f.slots[0] = Some(Stack::new("minecraft:dirt", 1));
        f.step(&t);
        assert_eq!(f.heat, 9_990, "sem combustível: perde heat/1000");
        assert_eq!(f.slots[0], Some(Stack::new("minecraft:dirt", 1)));
    }
}

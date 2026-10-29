//! The firebox (`tileentity/machine/TileEntityHeaterFirebox.java`,
//! `TileEntityFireboxBase.java:73-124`): burns solid fuel into heat for
//! the machine above it (a crucible). Two fuel slots (the engine's), up to
//! 100 000 TU; it burns while below the maximum and, idle, loses
//! `max(heat/1000, 1)` a tick. Left out: the ash pit, smoke and soot.

use super::{HeatSource, Tables};
use crate::server::container_bridge::{Slots, Stack};
use serde::{Deserialize, Serialize};

pub const SLOTS: usize = 2;
pub const MAX_HEAT: u32 = 100_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Firebox {
    pub slots: Slots,
    pub heat: u32,
    pub burn: u32,
    pub max_burn: u32,
    pub burn_heat: u32,
    pub on: bool,
}

impl Default for Firebox {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], heat: 0, burn: 0, max_burn: 0, burn_heat: 0, on: false }
    }
}

impl Firebox {
    /// One tick; true when its items changed.
    pub fn step(&mut self, t: &Tables) -> bool {
        let mut moved = false;
        self.on = false;
        if self.burn == 0 {
            for i in 0..SLOTS {
                let Some(s) = &mut self.slots[i] else { continue };
                let Some(f) = t.firebox_fuels.get(&s.id).filter(|f| f.ticks > 0) else { continue };
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
            if self.heat < MAX_HEAT {
                self.burn -= 1;
            }
            self.on = true;
        }
        if self.on {
            self.heat = (self.heat + self.burn_heat).min(MAX_HEAT);
        } else {
            self.heat = self.heat.saturating_sub((self.heat / 1000).max(1));
            self.burn_heat = 0;
        }
        moved
    }

    /// The screen's values: heat, its maximum, burn left, burn of the
    /// last fuel, TU a tick, burning.
    pub fn screen_data(&self) -> Vec<i32> {
        vec![self.heat as i32, MAX_HEAT as i32, self.burn as i32, self.max_burn as i32, self.burn_heat as i32, i32::from(self.on)]
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

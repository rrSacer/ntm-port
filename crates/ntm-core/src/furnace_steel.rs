//! The steel furnace (`furnace_steel`), from
//! `tileentity/machine/TileEntityFurnaceSteel.java`: three lines that smelt
//! on the heat of the block under it, with the ore, log and tar bonus of
//! `addBonus`. Its items are the engine's slots: 0..3 the inputs, 3..6 the
//! outputs.

use crate::{
    furnaces::{STACK, fits, key, pull_heat},
    machine::{Logic, fill_level},
    recipes,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const INPUTS: std::ops::Range<usize> = 0..3;
pub const OUTPUTS: std::ops::Range<usize> = 3..6;
pub const SLOTS: usize = 6;
/// `processTime`: 40 000 TU, a vanilla furnace's 200 ticks at 200 HU/t.
pub const PROCESS_TIME: u32 = 40_000;
pub const MAX_HEAT: u32 = 100_000;
/// `diffusion`.
pub const DIFFUSION: f64 = 0.05;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Steel {
    pub slots: Slots,
    pub progress: [u32; 3],
    pub bonus: [u32; 3],
    pub heat: u32,
    pub was_on: bool,
    /// `lastItems`: what each input was at the end of the last tick; a change
    /// resets the line.
    #[serde(skip)]
    pub last: [Option<(String, u32)>; 3],
}

impl Default for Steel {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            progress: [0; 3],
            bonus: [0; 3],
            heat: 0,
            was_on: false,
            last: [None, None, None],
        }
    }
}

/// `addBonus` (lines 191-197): the item's OreDict names read as an ore (25),
/// a log (50) or tar (50).
pub fn bonus_of(item: &str) -> u32 {
    let names = crate::oredict::names(item);
    if names.iter().any(|n| n.starts_with("ore")) {
        25
    } else if names.iter().any(|n| n.starts_with("log") || n == "anyTar") {
        50
    } else {
        0
    }
}

impl Steel {
    /// `tryPullHeat` (lines 199-218): `source` is the heat stored under it.
    /// Full, it takes none; a hotter source gives a twentieth of the
    /// difference, rounded up; otherwise it cools by a thousandth. Returns
    /// the heat taken from the source.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        pull_heat(&mut self.heat, MAX_HEAT, DIFFUSION, source)
    }

    /// `canSmelt(index)` (lines 220-229).
    pub fn can_smelt(&self, i: usize) -> Option<&'static recipes::Smelt> {
        if self.heat < MAX_HEAT / 3 {
            return None;
        }
        fits(&self.slots[i], &self.slots[i + 3])
    }

    /// One `updateEntity` (lines 49-80), after `tryPullHeat`.
    pub fn step(&mut self) -> bool {
        let before = (self.heat, self.progress, self.bonus, self.was_on);
        let mut moved = false;
        self.was_on = false;
        let burn = (i64::from(self.heat) - i64::from(MAX_HEAT / 3)) / 10;
        for i in 0..3 {
            if self.slots[i].is_none()
                || self.last[i].is_none()
                || key(&self.slots[i]) != self.last[i]
            {
                self.progress[i] = 0;
                self.bonus[i] = 0;
            }
            if self.can_smelt(i).is_some() {
                self.progress[i] += burn as u32;
                self.heat = (i64::from(self.heat) - burn).max(0) as u32;
                self.was_on = true;
            }
            self.last[i] = key(&self.slots[i]);
            if self.progress[i] >= PROCESS_TIME {
                let Some(input) = self.slots[i].as_ref().map(|s| s.id.clone()) else {
                    continue;
                };
                let Some(r) = recipes::furnace().smelt(&input) else {
                    continue;
                };
                let (out, n) = (r.output.clone(), r.count);
                match &mut self.slots[i + 3] {
                    Some(o) => o.count += n,
                    o => *o = Some(Stack::new(&out, n)),
                }
                self.bonus[i] += bonus_of(&input);
                while self.bonus[i] >= 100 {
                    if let Some(o) = &mut self.slots[i + 3] {
                        o.count = STACK.min(o.count + n);
                    }
                    self.bonus[i] -= 100;
                }
                if let Some(s) = &mut self.slots[i] {
                    s.count -= 1;
                    if s.count == 0 {
                        self.slots[i] = None;
                    }
                }
                self.progress[i] = 0;
                moved = true;
            }
        }
        moved || before != (self.heat, self.progress, self.bonus, self.was_on)
    }

    /// The screen: heat, its maximum, the three progresses and bonuses, and
    /// whether it works (`GUIFurnaceSteel`).
    pub fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![self.heat as i32, MAX_HEAT as i32];
        v.extend(self.progress.map(|p| p as i32));
        v.extend(self.bonus.map(|b| b as i32));
        v.push(i32::from(self.was_on));
        v
    }
}

impl Slotted for Steel {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Steel::screen_data(self)
    }
    /// The heat, the progress and the bonus ride with the slots: a crash
    /// cannot leave an input smelted while its progress is still saved.
    fn save_state(&self) -> Vec<i32> {
        let mut v = vec![self.heat as i32];
        v.extend(self.progress.map(|p| p as i32));
        v.extend(self.bonus.map(|b| b as i32));
        v
    }
    /// `lastItems` is what the slots hold now: the engine saves both together.
    fn load_state(&mut self, v: &[i32]) {
        if let [h, p0, p1, p2, b0, b1, b2, ..] = v {
            self.heat = (*h).max(0) as u32;
            self.progress = [p0, p1, p2].map(|x| (*x).max(0) as u32);
            self.bonus = [b0, b1, b2].map(|x| (*x).max(0) as u32);
        }
        for i in 0..3 {
            self.last[i] = key(&self.slots[i]);
        }
    }
}

impl Logic for Steel {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn level(&self, max: u8) -> u8 {
        if self.was_on {
            fill_level(u64::from(self.heat), u64::from(MAX_HEAT), max).max(1)
        } else {
            0
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Forno de aço")
            .bar("Calor", u64::from(self.heat), u64::from(MAX_HEAT), "TU")
            .text("Os itens entram pela tela com slots.", view::GREY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hot(heat: u32, input: &str, n: u32) -> Steel {
        let mut f = Steel {
            heat,
            ..Default::default()
        };
        f.slots[0] = Some(Stack::new(input, n));
        f
    }

    #[test]
    fn it_pulls_a_twentieth_of_the_difference_rounded_up() {
        let mut f = Steel::default();
        assert_eq!(f.pull_heat(Some(1000)), 50);
        assert_eq!(f.heat, 50);
        assert_eq!(f.pull_heat(Some(1000)), 48);
        assert_eq!(f.heat, 98);
        // A source at the same heat gives none and the furnace keeps its own.
        let mut f = Steel {
            heat: 500,
            ..Default::default()
        };
        assert_eq!((f.pull_heat(Some(500)), f.heat), (0, 500));
        // No source, or a colder one: it cools by a thousandth, at least 1.
        assert_eq!((f.pull_heat(None), f.heat), (0, 499));
        assert_eq!((f.pull_heat(Some(10)), f.heat), (0, 498));
        // Full, it neither takes nor cools.
        let mut f = Steel {
            heat: MAX_HEAT,
            ..Default::default()
        };
        assert_eq!((f.pull_heat(Some(MAX_HEAT)), f.heat), (0, MAX_HEAT));
    }

    #[test]
    fn below_a_third_of_the_heat_nothing_smelts() {
        let mut f = hot(MAX_HEAT / 3 - 1, "minecraft:raw_iron", 4);
        f.step();
        assert_eq!((f.progress, f.was_on), ([0; 3], false));
    }

    #[test]
    fn at_full_heat_a_line_smelts_in_nine_ticks_cooling_the_furnace() {
        let mut f = hot(MAX_HEAT, "minecraft:raw_iron", 4);
        // The burn is a tenth of the heat over a third: 6 666, then 6 000...
        // (the heat falls as it is spent): 40 000 TU take 9 ticks.
        for _ in 0..8 {
            f.step();
        }
        assert_eq!(f.slots[3], None);
        f.step();
        assert_eq!(f.slots[3], Some(Stack::new("minecraft:iron_ingot", 1)));
        assert_eq!(f.slots[0], Some(Stack::new("minecraft:raw_iron", 3)));
        assert_eq!(f.heat, 59_163);
    }

    #[test]
    fn the_three_lines_draw_from_the_same_heat() {
        let mut f = hot(MAX_HEAT, "minecraft:raw_iron", 4);
        f.slots[1] = Some(Stack::new("minecraft:raw_iron", 4));
        f.slots[2] = Some(Stack::new("minecraft:raw_iron", 4));
        f.step();
        // The burn is read once; each line then spends it.
        assert_eq!(f.heat, MAX_HEAT - 3 * 6666);
        assert_eq!(f.progress, [6666; 3]);
    }

    #[test]
    fn a_changed_input_resets_its_line() {
        let mut f = hot(MAX_HEAT, "minecraft:raw_iron", 4);
        f.step();
        assert!(f.progress[0] > 0);
        f.slots[0] = Some(Stack::new("minecraft:raw_gold", 4));
        f.step();
        // The heat fell to 93 334: a burn of 6 000, the new item alone.
        assert_eq!(f.progress[0], 6000);
        let before = f.progress[0];
        f.slots[0].as_mut().unwrap().count = 2;
        f.step();
        assert!(
            f.progress[0] > before,
            "the count is not the item: it keeps going"
        );
    }

    #[test]
    fn ores_logs_and_tar_give_a_bonus_that_pays_out_every_hundred() {
        assert_eq!(bonus_of("minecraft:iron_ore"), 25);
        assert_eq!(bonus_of("minecraft:oak_log"), 50);
        assert_eq!(bonus_of("ntm:oil_tar__crude"), 50);
        assert_eq!(bonus_of("minecraft:raw_iron"), 0);
        let mut f = hot(MAX_HEAT, "minecraft:iron_ore", 20);
        f.bonus[0] = 75;
        f.last[0] = Some(("minecraft:iron_ore".into(), 0));
        f.progress[0] = PROCESS_TIME;
        f.step();
        // 75 + 25 = 100: one extra ingot with the first.
        assert_eq!(f.slots[3], Some(Stack::new("minecraft:iron_ingot", 2)));
        assert_eq!(f.bonus[0], 0);
    }

    #[test]
    fn the_extra_ingot_is_capped_by_the_stack() {
        let mut f = hot(MAX_HEAT, "minecraft:iron_ore", 20);
        f.slots[3] = Some(Stack::new("minecraft:iron_ingot", 63));
        f.bonus[0] = 75;
        f.last[0] = Some(("minecraft:iron_ore".into(), 0));
        f.progress[0] = PROCESS_TIME;
        f.step();
        assert_eq!(f.slots[3], Some(Stack::new("minecraft:iron_ingot", 64)));
    }

    #[test]
    fn the_state_survives_a_save_and_the_line_keeps_going() {
        let mut f = hot(MAX_HEAT, "minecraft:raw_iron", 4);
        f.step();
        f.bonus = [10, 20, 30];
        let saved = Slotted::save_state(&f);
        let mut g = Steel {
            slots: f.slots.clone(),
            ..Default::default()
        };
        Slotted::load_state(&mut g, &saved);
        assert_eq!((g.heat, g.progress, g.bonus), (f.heat, f.progress, f.bonus));
        let before = g.progress[0];
        g.step();
        assert!(g.progress[0] > before, "perdeu o fio: {:?}", g.progress);
    }

    #[test]
    fn it_glows_while_it_works() {
        let mut f = hot(MAX_HEAT, "minecraft:raw_iron", 1);
        assert_eq!(f.level(5), 0);
        f.step();
        assert!(f.level(5) > 0);
    }
}

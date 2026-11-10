//! The sawmill (`machine_sawmill`), from `tileentity/machine/TileEntitySawmill.java`:
//! on the heat of the block under it, a log becomes planks, a plank sticks, a
//! stick sawdust, a sapling a stick, with sawdust now and then as a second
//! product. Its three slots (0 the input, 1 the output, 2 the sawdust) are the
//! engine's; hoppers reach them through the four port cells of its multiblock.
//!
//! Left out, named in `docs/onda-2-sublote-1.md`: the click on the block that
//! puts one item in, takes the outputs and puts a blade back (`use_block`
//! hook), the damage to the living things in the blade's box and the
//! overspeed's consequences (the warning sound, the blast of strength 5 and the
//! blade flying off as `EntitySawblade`): they are world effects and entities.
//! Meanwhile the blade never leaves: a lost one could not be put back.

use crate::{
    machine::{Logic, fill_level},
    oredict,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const INPUT: usize = 0;
pub const OUTPUT: usize = 1;
pub const SAWDUST: usize = 2;
pub const SLOTS: usize = 3;
/// `processingTime`.
pub const TIME: u32 = 600;
/// `diffusion`: a tenth of the heat stored under it, a tick.
pub const DIFFUSION: f64 = 0.1;
pub const SAWDUST_ITEM: &str = "ntm:powder_sawdust";

/// The planks of a log, as the crafting table gives them (4 a log).
fn planks_of(log: &str) -> Option<String> {
    let path = log.strip_prefix("minecraft:")?;
    let path = path.strip_prefix("stripped_").unwrap_or(path);
    let wood = ["_log", "_wood", "_stem", "_hyphae"]
        .iter()
        .find_map(|s| path.strip_suffix(s))?;
    Some(format!("minecraft:{wood}_planks"))
}

/// `getOutput` (lines 190-224): what `item` becomes, with the chance of
/// sawdust beside it (the stick's 10 %, the rest 50 %, the sawdust none).
pub fn output(item: &str) -> Option<(Stack, f32)> {
    if oredict::has(item, "stickWood") {
        return Some((Stack::new(SAWDUST_ITEM, 1), 0.0));
    }
    if oredict::has(item, "logWood") {
        // 4 planks become 6: `stackSize * 6 / 4`.
        return planks_of(item).map(|p| (Stack::new(&p, 6), 0.5));
    }
    if oredict::has(item, "plankWood") {
        return Some((Stack::new("minecraft:stick", 6), 0.1));
    }
    if oredict::has(item, "treeSapling") {
        return Some((Stack::new("minecraft:stick", 1), 0.1));
    }
    None
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sawmill {
    pub slots: Slots,
    /// The heat it took this tick (`heat`: back to zero at the end of each).
    pub heat: u32,
    pub progress: u32,
    pub has_blade: bool,
    /// Ticks with more than 300 heat (`overspeed`), counted for the day the
    /// consequences exist.
    pub overspeed: u32,
    /// Sawdust rolls so far: the dice of the chance output.
    pub done: u64,
}

impl Default for Sawmill {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            heat: 0,
            progress: 0,
            has_blade: true,
            overspeed: 0,
            done: 0,
        }
    }
}

impl Sawmill {
    /// `tryPullHeat` (lines 235-250): a tenth of what the source stores, which
    /// loses it; the heat is this tick's alone. Returns what the source gave.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        let taken = source.map_or(0, |s| (f64::from(s) * DIFFUSION) as u32);
        self.heat = taken;
        taken
    }

    /// One `updateEntity` (lines 63-137), after `tryPullHeat`.
    pub fn step(&mut self) -> bool {
        let before = (self.progress, self.overspeed, self.slots.clone());
        if self.has_blade {
            if self.heat >= 100 {
                let result = self.slots[INPUT].as_ref().and_then(|s| output(&s.id));
                if let Some((product, sawdust)) = result {
                    self.progress += self.heat / 10;
                    if self.progress >= TIME {
                        self.progress = 0;
                        self.slots[INPUT] = None;
                        self.done += 1;
                        let roll = crate::explosion::crater::rand01(
                            self.done as i32,
                            4,
                            (self.done >> 31) as i32,
                            91,
                        );
                        self.slots[OUTPUT] = Some(product);
                        if sawdust > 0.0 && roll < sawdust {
                            self.slots[SAWDUST] = Some(Stack::new(SAWDUST_ITEM, 1));
                        }
                    }
                } else {
                    self.progress = 0;
                }
            } else {
                self.progress = 0;
            }
            self.overspeed = if self.heat > 300 {
                self.overspeed + 1
            } else {
                0
            };
        } else {
            self.overspeed = 0;
        }
        self.heat = 0;
        before != (self.progress, self.overspeed, self.slots.clone())
    }
}

impl Slotted for Sawmill {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![self.progress as i32, TIME as i32, i32::from(self.has_blade)]
    }
    /// The progress, the blade and the dice ride with the slots.
    fn save_state(&self) -> Vec<i32> {
        vec![
            self.progress as i32,
            i32::from(self.has_blade),
            self.overspeed as i32,
            self.done as i32,
        ]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [p, b, o, d, ..] = v {
            self.progress = (*p).max(0) as u32;
            self.has_blade = *b != 0;
            self.overspeed = (*o).max(0) as u32;
            self.done = (*d).max(0) as u64;
        }
    }
}

impl Logic for Sawmill {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn level(&self, max: u8) -> u8 {
        if self.progress > 0 {
            fill_level(u64::from(self.progress), u64::from(TIME), max).max(1)
        } else {
            0
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Serra")
            .bar("Progresso", u64::from(self.progress), u64::from(TIME), "")
            .text("Sem tela: os itens entram por funis.", view::GREY)
    }
}

/// `isItemValidForSlot` and `canExtractItem` (lines 190-199): one item at a
/// time, only into an empty machine and only what has an output; the outputs
/// leave, the input does not.
pub fn allows(slots: &[Option<Stack>], slot: usize, item: &Stack, insert: bool) -> bool {
    if !insert {
        return slot > 0;
    }
    slot == INPUT
        && slots.iter().all(Option::is_none)
        && item.count == 1
        && output(&item.id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(item: &str) -> Sawmill {
        let mut s = Sawmill::default();
        s.slots[INPUT] = Some(Stack::new(item, 1));
        s
    }

    #[test]
    fn the_four_recipes() {
        assert_eq!(
            output("minecraft:oak_log"),
            Some((Stack::new("minecraft:oak_planks", 6), 0.5))
        );
        assert_eq!(
            output("minecraft:stripped_birch_log"),
            Some((Stack::new("minecraft:birch_planks", 6), 0.5))
        );
        assert_eq!(
            output("minecraft:oak_planks"),
            Some((Stack::new("minecraft:stick", 6), 0.1))
        );
        assert_eq!(
            output("minecraft:stick"),
            Some((Stack::new(SAWDUST_ITEM, 1), 0.0))
        );
        assert_eq!(
            output("minecraft:oak_sapling"),
            Some((Stack::new("minecraft:stick", 1), 0.1))
        );
        assert_eq!(output("minecraft:stone"), None);
    }

    #[test]
    fn it_needs_100_heat_and_adds_a_tenth_a_tick() {
        let mut s = with("minecraft:oak_log");
        s.heat = 99;
        s.step();
        assert_eq!(s.progress, 0);
        // 200 heat: 20 a tick, 30 ticks for 600.
        for _ in 0..29 {
            s.heat = 200;
            s.step();
        }
        assert_eq!((s.progress, s.slots[OUTPUT].clone()), (580, None));
        s.heat = 200;
        s.step();
        assert_eq!(s.slots[OUTPUT], Some(Stack::new("minecraft:oak_planks", 6)));
        assert_eq!(s.slots[INPUT], None);
        assert_eq!(s.progress, 0);
    }

    #[test]
    fn the_heat_is_this_ticks_alone() {
        let mut s = Sawmill::default();
        assert_eq!(s.pull_heat(Some(5_000)), 500);
        assert_eq!(s.heat, 500);
        s.step();
        assert_eq!(s.heat, 0);
        assert_eq!(s.pull_heat(None), 0);
        assert_eq!(s.pull_heat(Some(9)), 0, "a tenth of 9 is less than one");
    }

    #[test]
    fn the_sawdust_comes_by_chance_and_never_from_a_stick() {
        let mut got = 0;
        for i in 0..200 {
            let mut s = with("minecraft:oak_log");
            s.done = i;
            s.heat = 6_000;
            s.step();
            got += i32::from(s.slots[SAWDUST].is_some());
        }
        assert!((60..140).contains(&got), "{got} of 200 at 50 %");
        for i in 0..50 {
            let mut s = with("minecraft:stick");
            s.done = i;
            s.heat = 6_000;
            s.step();
            assert_eq!(s.slots[OUTPUT], Some(Stack::new(SAWDUST_ITEM, 1)));
            assert!(s.slots[SAWDUST].is_none(), "o graveto já é serragem");
        }
    }

    #[test]
    fn the_overspeed_is_counted_and_reset() {
        let mut s = with("minecraft:oak_log");
        for _ in 0..70 {
            s.heat = 400;
            s.step();
        }
        assert_eq!(s.overspeed, 70);
        s.heat = 300;
        s.step();
        assert_eq!(s.overspeed, 0, "300 is not above 300");
        assert!(
            s.has_blade,
            "a lâmina não sai: a explosão e a lâmina voando são lacunas nominais"
        );
    }

    #[test]
    fn without_a_blade_nothing_saws() {
        let mut s = with("minecraft:oak_log");
        s.has_blade = false;
        s.heat = 1_000;
        s.step();
        assert_eq!(s.progress, 0);
    }

    #[test]
    fn it_takes_one_item_into_an_empty_machine_and_gives_the_outputs() {
        let empty: Slots = vec![None; SLOTS];
        let log = Stack::new("minecraft:oak_log", 1);
        assert!(allows(&empty, INPUT, &log, true));
        assert!(
            !allows(&empty, INPUT, &Stack::new("minecraft:oak_log", 2), true),
            "só um por vez"
        );
        assert!(!allows(
            &empty,
            INPUT,
            &Stack::new("minecraft:stone", 1),
            true
        ));
        assert!(!allows(&empty, OUTPUT, &log, true));
        let mut busy = empty.clone();
        busy[OUTPUT] = Some(Stack::new("minecraft:stick", 1));
        assert!(
            !allows(&busy, INPUT, &log, true),
            "só com os três slots vazios"
        );
        assert!(
            allows(&busy, OUTPUT, &log, false)
                && allows(&busy, SAWDUST, &log, false)
                && !allows(&busy, INPUT, &log, false)
        );
    }

    #[test]
    fn the_state_survives_a_save() {
        let s = Sawmill {
            progress: 321,
            has_blade: true,
            overspeed: 12,
            done: 7,
            ..Default::default()
        };
        let mut g = Sawmill::default();
        Slotted::load_state(&mut g, &Slotted::save_state(&s));
        assert_eq!((g.progress, g.overspeed, g.done), (321, 12, 7));
    }
}

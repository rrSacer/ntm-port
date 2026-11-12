//! The brick furnace (`machine_furnace_brick_off` and `_on`), from
//! `tileentity/machine/TileEntityFurnaceBrick.java`: a fuel furnace with a
//! fuel slot, smelting the vanilla recipes (`FurnaceRecipes`) and leaving
//! ash. The block swaps between its two ids while it burns
//! (`MachineBrickFurnace.updateBlockState`); the port keeps either id and
//! shows the burning as the block's level.
//!
//! Slots (`ContainerFurnaceBrick`): 0 the input, 1 the fuel, 2 the output
//! (`SlotSmelting`), 3 the ash (`SlotCraftingOutput`). Every tick (lines
//! 65-125): a fuel burns while there is something to smelt, the progress
//! grows by the input's burn speed (`burnSpeed`, 1 for the rest) up to 200,
//! and each 2 000 burn ticks of a kind of fuel (`ashLevel*`) leave one ash
//! in slot 3 when it has room.
//!
//! Left out: the ash is a plain item (the ashpit and the soot of the
//! firebox are the survival wave's), and the output stacks to 64 where the
//! source says `getMaxStackSize` of the item.

use crate::{
    fuel::{self, Ash},
    machine::{Logic, fill_level},
    recipes,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const OUTPUT: usize = 2;
pub const ASH: usize = 3;
pub const SLOTS: usize = 4;
/// `progress >= 200` smelts.
pub const TICKS: u32 = 200;
/// `processAsh`'s threshold (line 89).
pub const ASH_THRESHOLD: u32 = 2000;
/// `getInventoryStackLimit()`.
pub const STACK: u32 = 64;

/// `burnSpeed` (lines 36-44): what the input adds to the progress a tick.
pub fn burn_speed(input: &str) -> u32 {
    match input {
        "minecraft:clay_ball" | "ntm:ball_fireclay" | "minecraft:netherrack" => 4,
        "minecraft:cobblestone" | "minecraft:sand" | "minecraft:red_sand" => 2,
        i if i.starts_with("minecraft:")
            && i.ends_with("_log")
            && !i.starts_with("minecraft:stripped_") =>
        {
            2
        }
        _ => 1,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrickFurnace {
    pub slots: Slots,
    pub burn_time: u32,
    pub max_burn_time: u32,
    pub progress: u32,
    pub ash_wood: u32,
    pub ash_coal: u32,
    pub ash_misc: u32,
}

impl Default for BrickFurnace {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            burn_time: 0,
            max_burn_time: 0,
            progress: 0,
            ash_wood: 0,
            ash_coal: 0,
            ash_misc: 0,
        }
    }
}

fn same(a: &Stack, id: &str) -> bool {
    a.id == id && a.damage == 0 && a.custom.is_none()
}

impl BrickFurnace {
    /// `canSmelt` (lines 178-192).
    pub fn can_smelt(&self) -> Option<&'static recipes::Smelt> {
        let input = self.slots[INPUT].as_ref().filter(|s| s.count > 0)?;
        let r = recipes::furnace().smelt(&input.id)?;
        match &self.slots[OUTPUT] {
            None => Some(r),
            Some(o) if same(o, &r.output) && o.count + r.count <= STACK => Some(r),
            _ => None,
        }
    }

    /// `smeltItem` (lines 194-210).
    fn smelt(&mut self) {
        let Some(r) = self.can_smelt() else { return };
        match &mut self.slots[OUTPUT] {
            Some(o) => o.count += r.count,
            None => self.slots[OUTPUT] = Some(Stack::new(&r.output, r.count)),
        }
        if let Some(i) = &mut self.slots[INPUT] {
            i.count -= 1;
            if i.count == 0 {
                self.slots[INPUT] = None;
            }
        }
    }

    /// `processAsh` (lines 163-176): one ash of `kind` when the level is
    /// at the threshold and slot 3 takes it.
    fn process_ash(slots: &mut Slots, level: u32, kind: Ash) -> bool {
        if level < ASH_THRESHOLD {
            return false;
        }
        match &mut slots[ASH] {
            None => {
                slots[ASH] = Some(Stack::new(kind.item(), 1));
                true
            }
            Some(s) if s.count < STACK && same(s, kind.item()) => {
                s.count += 1;
                true
            }
            _ => false,
        }
    }

    /// One `updateEntity` (lines 65-125); true when it changed anything.
    pub fn step(&mut self) -> bool {
        let before = (self.burn_time, self.progress);
        let mut dirty = false;
        if self.burn_time > 0 {
            self.burn_time -= 1;
        }
        let has_both = self.slots[FUEL].is_some() && self.slots[INPUT].is_some();
        if self.burn_time != 0 || has_both {
            if self.burn_time == 0 && self.can_smelt().is_some() {
                let time = self.slots[FUEL]
                    .as_ref()
                    .map_or(0, |s| fuel::burn_time(&s.id));
                self.max_burn_time = time;
                self.burn_time = time;
                if time > 0 {
                    dirty = true;
                    if let Some(f) = self.slots[FUEL].as_mut() {
                        let id = f.id.clone();
                        f.count -= 1;
                        match fuel::ash(&id) {
                            Ash::Wood => self.ash_wood += time,
                            Ash::Coal => self.ash_coal += time,
                            Ash::Misc => self.ash_misc += time,
                            // `getAshFromFuel` gives only these three.
                            Ash::Fly | Ash::Soot => {}
                        }
                        if Self::process_ash(&mut self.slots, self.ash_wood, Ash::Wood) {
                            self.ash_wood -= ASH_THRESHOLD;
                        }
                        if Self::process_ash(&mut self.slots, self.ash_coal, Ash::Coal) {
                            self.ash_coal -= ASH_THRESHOLD;
                        }
                        if Self::process_ash(&mut self.slots, self.ash_misc, Ash::Misc) {
                            self.ash_misc -= ASH_THRESHOLD;
                        }
                        if self.slots[FUEL].as_ref().is_some_and(|s| s.count == 0) {
                            self.slots[FUEL] = fuel::container_item(&id).map(|c| Stack::new(c, 1));
                        }
                    }
                }
            }
            if self.burn_time > 0 && self.can_smelt().is_some() {
                let speed = self.slots[INPUT].as_ref().map_or(1, |s| burn_speed(&s.id));
                self.progress += speed;
                if self.progress >= TICKS {
                    self.progress = 0;
                    self.smelt();
                    dirty = true;
                }
            } else {
                self.progress = 0;
            }
        }
        dirty || before != (self.burn_time, self.progress)
    }

    /// The screen's values: the burn left, the burn of the last fuel, the
    /// progress (`GUIFurnaceBrick`).
    pub fn screen_data(&self) -> Vec<i32> {
        vec![
            self.burn_time as i32,
            self.max_burn_time as i32,
            self.progress as i32,
        ]
    }
}

impl Slotted for BrickFurnace {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        BrickFurnace::screen_data(self)
    }
    /// The burn, the progress and the three ash levels ride with the slots:
    /// a crash cannot leave the fuel gone without the burn it gave, the
    /// input smelted without its progress reset, or the ash item without its
    /// level.
    fn save_state(&self) -> Vec<i32> {
        [
            self.burn_time,
            self.max_burn_time,
            self.progress,
            self.ash_wood,
            self.ash_coal,
            self.ash_misc,
        ]
        .map(|v| v as i32)
        .to_vec()
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [b, m, p, w, c, a, ..] = v {
            let [b, m, p, w, c, a] = [b, m, p, w, c, a].map(|x| (*x).max(0) as u32);
            (self.burn_time, self.max_burn_time, self.progress) = (b, m, p);
            (self.ash_wood, self.ash_coal, self.ash_misc) = (w, c, a);
        }
    }
}

impl Logic for BrickFurnace {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn level(&self, max: u8) -> u8 {
        if self.burn_time == 0 {
            0
        } else {
            fill_level(
                u64::from(self.burn_time),
                u64::from(self.max_burn_time.max(self.burn_time)),
                max,
            )
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Forno de tijolos")
            .bar(
                "Queima",
                u64::from(self.burn_time),
                u64::from(self.max_burn_time),
                "t",
            )
            .bar("Progresso", u64::from(self.progress), u64::from(TICKS), "")
            .text("Os itens entram pela tela com slots.", view::GREY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn furnace(input: Option<(&str, u32)>, fuel_: Option<(&str, u32)>) -> BrickFurnace {
        let mut f = BrickFurnace::default();
        f.slots[INPUT] = input.map(|(i, n)| Stack::new(i, n));
        f.slots[FUEL] = fuel_.map(|(i, n)| Stack::new(i, n));
        f
    }

    #[test]
    fn it_does_nothing_without_fuel_or_input() {
        let mut f = furnace(Some(("minecraft:raw_iron", 4)), None);
        assert!(!f.step());
        let mut f = furnace(None, Some(("minecraft:coal", 4)));
        assert!(!f.step());
        assert_eq!(f.slots[FUEL], Some(Stack::new("minecraft:coal", 4)));
    }

    #[test]
    fn a_coal_burns_1600_ticks_and_smelts_8_ingots() {
        let mut f = furnace(
            Some(("minecraft:raw_iron", 64)),
            Some(("minecraft:coal", 2)),
        );
        // The tick that takes the fuel also counts the first step of progress.
        f.step();
        assert_eq!((f.burn_time, f.max_burn_time, f.progress), (1600, 1600, 1));
        assert_eq!(f.slots[FUEL], Some(Stack::new("minecraft:coal", 1)));
        for _ in 0..1599 {
            f.step();
        }
        // 1600 ticks at one progress a tick: 8 smelts of 200.
        assert_eq!(f.slots[OUTPUT], Some(Stack::new("minecraft:iron_ingot", 8)));
        assert_eq!(f.slots[INPUT], Some(Stack::new("minecraft:raw_iron", 56)));
        assert_eq!(f.burn_time, 1);
    }

    #[test]
    fn the_burn_time_decrements_before_the_check() {
        let mut f = furnace(
            Some(("minecraft:raw_iron", 1)),
            Some(("minecraft:stick", 1)),
        );
        f.step();
        assert_eq!(f.burn_time, 100);
        f.step();
        assert_eq!(f.burn_time, 99);
    }

    #[test]
    fn clay_and_logs_cook_faster() {
        assert_eq!(burn_speed("minecraft:clay_ball"), 4);
        assert_eq!(burn_speed("minecraft:sand"), 2);
        // `Blocks.sand` of any metadata: the red sand too.
        assert_eq!(burn_speed("minecraft:red_sand"), 2);
        assert_eq!(burn_speed("minecraft:oak_log"), 2);
        assert_eq!(burn_speed("minecraft:raw_iron"), 1);
        let mut f = furnace(Some(("minecraft:raw_iron", 1)), Some(("minecraft:coal", 1)));
        let mut g = furnace(
            Some(("minecraft:clay_ball", 1)),
            Some(("minecraft:coal", 1)),
        );
        f.step();
        g.step();
        assert_eq!((f.progress, g.progress), (1, 4));
    }

    #[test]
    fn it_stops_when_the_output_cannot_take_the_result() {
        let mut f = furnace(Some(("minecraft:raw_iron", 4)), Some(("minecraft:coal", 1)));
        f.slots[OUTPUT] = Some(Stack::new("minecraft:iron_ingot", 64));
        f.step();
        assert_eq!((f.burn_time, f.progress), (0, 0));
        assert_eq!(f.slots[FUEL], Some(Stack::new("minecraft:coal", 1)));
        f.slots[OUTPUT] = Some(Stack::new("minecraft:gold_ingot", 1));
        assert!(f.can_smelt().is_none());
    }

    #[test]
    fn progress_resets_when_the_input_goes_while_burning() {
        let mut f = furnace(Some(("minecraft:raw_iron", 1)), Some(("minecraft:coal", 1)));
        for _ in 0..50 {
            f.step();
        }
        assert_eq!(f.progress, 50);
        f.slots[INPUT] = None;
        f.step();
        assert_eq!(f.progress, 0);
        assert_eq!(f.burn_time, 1550);
    }

    #[test]
    fn ash_comes_per_2000_burn_ticks_of_a_kind() {
        let mut f = furnace(
            Some(("minecraft:raw_iron", 64)),
            Some(("minecraft:coal", 2)),
        );
        f.step();
        // 1600 of coal ash: none yet.
        assert_eq!((f.ash_coal, f.slots[ASH].clone()), (1600, None));
        for _ in 0..1600 {
            f.step();
        }
        // The second coal brings it to 3200: one ash, 1200 left.
        assert_eq!(
            (f.ash_coal, f.slots[ASH].clone()),
            (1200, Some(Stack::new("ntm:powder_ash__coal", 1)))
        );
        assert_eq!(f.slots[FUEL], None);
    }

    #[test]
    fn a_full_ash_slot_keeps_the_level_for_later() {
        let mut f = furnace(
            Some(("minecraft:raw_iron", 64)),
            Some(("minecraft:coal", 3)),
        );
        f.slots[ASH] = Some(Stack::new("ntm:powder_ash__coal", 64));
        f.ash_coal = 2000;
        f.step();
        assert_eq!(f.ash_coal, 3600);
        assert_eq!(f.slots[ASH], Some(Stack::new("ntm:powder_ash__coal", 64)));
        f.slots[ASH] = Some(Stack::new("ntm:powder_ash__wood", 1));
        for _ in 0..1600 {
            f.step();
        }
        // A pile of another ash does not take it either.
        assert!(f.ash_coal >= 3600);
        assert_eq!(f.slots[ASH], Some(Stack::new("ntm:powder_ash__wood", 1)));
    }

    #[test]
    fn a_lava_bucket_leaves_its_bucket() {
        let mut f = furnace(
            Some(("minecraft:raw_iron", 64)),
            Some(("minecraft:lava_bucket", 1)),
        );
        f.step();
        assert_eq!(f.burn_time, 20_000);
        assert_eq!(f.slots[FUEL], Some(Stack::new("minecraft:bucket", 1)));
        // One ash a kind a fuel taken (`processAsh` runs once): the rest waits.
        assert_eq!(f.ash_misc, 20_000 - 2000);
        assert_eq!(f.slots[ASH], Some(Stack::new("ntm:powder_ash__misc", 1)));
    }

    #[test]
    fn the_ash_levels_survive_a_save() {
        let f = BrickFurnace {
            burn_time: 5,
            max_burn_time: 1600,
            progress: 77,
            ash_wood: 1,
            ash_coal: 1999,
            ash_misc: 7,
            ..Default::default()
        };
        let v = Slotted::save_state(&f);
        let mut g = BrickFurnace::default();
        Slotted::load_state(&mut g, &v);
        assert_eq!((g.burn_time, g.max_burn_time, g.progress), (5, 1600, 77));
        assert_eq!((g.ash_wood, g.ash_coal, g.ash_misc), (1, 1999, 7));
    }

    #[test]
    fn it_glows_while_it_burns() {
        let mut f = furnace(Some(("minecraft:raw_iron", 1)), Some(("minecraft:coal", 1)));
        assert_eq!(f.level(5), 0);
        f.step();
        assert_eq!(f.level(5), 5);
    }
}

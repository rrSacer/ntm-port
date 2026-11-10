//! The K3 furnaces with several slots: `furnace_iron`
//! (`tileentity/machine/TileEntityFurnaceIron.java`), and the machines that
//! follow it in this file. All keep their items as the engine's slots.

use crate::{
    fuel::{self, Family, Module},
    furnace_combination::{self, Combination},
    furnace_steel::{self, Steel},
    machine::{Logic, fill_level},
    recipes,
    server::container_bridge::{Slots, Slotted, Stack},
    upgrades,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

/// `getInventoryStackLimit()`.
pub const STACK: u32 = 64;

pub const IRON: &str = "ntm:furnace_iron";
pub const STEEL: &str = "ntm:furnace_steel";
pub const COMBINATION: &str = "ntm:furnace_combination";

/// Every block this file runs.
pub fn of(block: &str) -> Option<Furnace> {
    match block {
        IRON => Some(Furnace::Iron(Iron::default())),
        STEEL => Some(Furnace::Steel(Steel::default())),
        COMBINATION => Some(Furnace::Combination(Box::default())),
        _ => None,
    }
}

pub fn is_furnace(block: &str) -> bool {
    of(block).is_some()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "f")]
pub enum Furnace {
    Iron(Iron),
    Steel(Steel),
    Combination(Box<Combination>),
}

impl Furnace {
    pub fn slotted(&mut self) -> &mut dyn Slotted {
        match self {
            Furnace::Iron(m) => m,
            Furnace::Steel(m) => m,
            Furnace::Combination(m) => &mut **m,
        }
    }
    fn logic(&self) -> &dyn Logic {
        match self {
            Furnace::Iron(m) => m,
            Furnace::Steel(m) => m,
            Furnace::Combination(m) => &**m,
        }
    }
    fn logic_mut(&mut self) -> &mut dyn Logic {
        match self {
            Furnace::Iron(m) => m,
            Furnace::Steel(m) => m,
            Furnace::Combination(m) => &mut **m,
        }
    }
}

impl Logic for Furnace {
    fn tick(&mut self) -> bool {
        match self {
            Furnace::Iron(m) => m.step(),
            Furnace::Steel(m) => m.step(),
            Furnace::Combination(m) => m.step(),
        }
    }
    fn fluid(&mut self) -> Option<&mut dyn crate::fluid::FluidNode> {
        self.logic_mut().fluid()
    }
    fn level(&self, max: u8) -> u8 {
        self.logic().level(max)
    }
    fn view(&self) -> ViewModel {
        self.logic().view()
    }
}

/// `tryPullHeat` of the furnaces that sit on a heat source: full, it takes
/// none; a hotter source gives `diffusion` of the difference, rounded up;
/// otherwise it cools by a thousandth, at least 1. Returns what the source
/// gave up.
pub(crate) fn pull_heat(heat: &mut u32, max: u32, diffusion: f64, source: Option<u32>) -> u32 {
    if *heat >= max {
        return 0;
    }
    if let Some(s) = source {
        if s == *heat {
            return 0;
        }
        if s > *heat {
            let diff = (f64::from(s - *heat) * diffusion).ceil() as u32;
            *heat = (*heat + diff).min(max);
            return diff;
        }
    }
    *heat = heat.saturating_sub((*heat / 1000).max(1));
    0
}

pub(crate) fn key(s: &Option<Stack>) -> Option<(String, u32)> {
    s.as_ref().map(|s| (s.id.clone(), s.damage))
}

fn same(a: &Stack, id: &str) -> bool {
    a.id == id && a.damage == 0 && a.custom.is_none()
}

/// The smelting both furnaces share: whether the result of `input` fits in
/// `output` (`canSmelt`'s last lines).
pub(crate) fn fits(
    input: &Option<Stack>,
    output: &Option<Stack>,
) -> Option<&'static recipes::Smelt> {
    let input = input.as_ref().filter(|s| s.count > 0)?;
    let r = recipes::furnace().smelt(&input.id)?;
    match output {
        None => Some(r),
        Some(o) if same(o, &r.output) && o.count + r.count <= STACK => Some(r),
        _ => None,
    }
}

// ------------------------------------------------------------ iron furnace

pub mod iron {
    pub const INPUT: usize = 0;
    pub const FUELS: std::ops::Range<usize> = 1..3;
    pub const OUTPUT: usize = 3;
    pub const UPGRADE: usize = 4;
    pub const SLOTS: usize = 5;
    /// `baseTime`.
    pub const BASE_TIME: u32 = 160;
}

/// `new ModuleBurnTime().setLigniteTimeMod(1.25).setCoalTimeMod(1.25)
/// .setCokeTimeMod(1.5).setSolidTimeMod(2).setRocketTimeMod(2)
/// .setBalefireTimeMod(2)` (lines 49-55).
pub const IRON_MODULE: Module = Module::NONE
    .with(Family::Lignite, 1.25)
    .with(Family::Coal, 1.25)
    .with(Family::Coke, 1.5)
    .with(Family::Solid, 2.0)
    .with(Family::Rocket, 2.0)
    .with(Family::Balefire, 2.0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Iron {
    pub slots: Slots,
    pub max_burn_time: u32,
    pub burn_time: u32,
    pub progress: u32,
    pub processing_time: u32,
    pub was_on: bool,
}

impl Default for Iron {
    fn default() -> Self {
        Self {
            slots: vec![None; iron::SLOTS],
            max_burn_time: 0,
            burn_time: 0,
            progress: 0,
            processing_time: iron::BASE_TIME,
            was_on: false,
        }
    }
}

impl Iron {
    /// `canSmelt` (lines 138-152): it needs burn time left.
    pub fn can_smelt(&self) -> Option<&'static recipes::Smelt> {
        if self.burn_time == 0 {
            return None;
        }
        fits(&self.slots[iron::INPUT], &self.slots[iron::OUTPUT])
    }

    /// One `updateEntity` (lines 67-123); true when it changed anything.
    pub fn step(&mut self) -> bool {
        let before = (
            self.burn_time,
            self.progress,
            self.was_on,
            self.processing_time,
        );
        let mut moved = false;
        // `baseTime - ((baseTime / 2) * speedLevel / 3)`.
        let speed = upgrades::level(&self.slots[iron::UPGRADE..], "speed", 3);
        self.processing_time = iron::BASE_TIME - (iron::BASE_TIME / 2) * speed / 3;
        self.was_on = false;
        if self.burn_time == 0 {
            for i in iron::FUELS {
                let Some(s) = self.slots[i].as_mut() else {
                    continue;
                };
                let fuel = IRON_MODULE.burn_time(&s.id);
                if fuel == 0 {
                    continue;
                }
                self.max_burn_time = fuel;
                self.burn_time = fuel;
                let id = s.id.clone();
                s.count -= 1;
                if s.count == 0 {
                    self.slots[i] = fuel::container_item(&id).map(|c| Stack::new(c, 1));
                }
                moved = true;
                break;
            }
        }
        if let Some(r) = self.can_smelt() {
            self.was_on = true;
            self.progress += 1;
            self.burn_time -= 1;
            if self.progress >= self.processing_time {
                match &mut self.slots[iron::OUTPUT] {
                    Some(o) => o.count += r.count,
                    o => *o = Some(Stack::new(&r.output, r.count)),
                }
                if let Some(i) = &mut self.slots[iron::INPUT] {
                    i.count -= 1;
                    if i.count == 0 {
                        self.slots[iron::INPUT] = None;
                    }
                }
                self.progress = 0;
                moved = true;
            }
        } else {
            self.progress = 0;
        }
        moved
            || before
                != (
                    self.burn_time,
                    self.progress,
                    self.was_on,
                    self.processing_time,
                )
    }

    /// The screen: progress, its total, the burn left, the last fuel's, and
    /// whether it can smelt (`GUIFurnaceIron`).
    pub fn screen_data(&self) -> Vec<i32> {
        vec![
            self.progress as i32,
            self.processing_time as i32,
            self.burn_time as i32,
            self.max_burn_time as i32,
            i32::from(self.can_smelt().is_some()),
        ]
    }
}

impl Slotted for Iron {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Iron::screen_data(self)
    }
    /// The burn and the progress ride with the slots: a fuel taken from a
    /// slot is never without its burn after a crash.
    fn save_state(&self) -> Vec<i32> {
        [self.burn_time, self.max_burn_time, self.progress]
            .map(|v| v as i32)
            .to_vec()
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [b, m, p, ..] = v {
            let [b, m, p] = [b, m, p].map(|x| (*x).max(0) as u32);
            (self.burn_time, self.max_burn_time, self.progress) = (b, m, p);
        }
    }
}

impl Logic for Iron {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn level(&self, max: u8) -> u8 {
        if self.was_on {
            fill_level(
                u64::from(self.burn_time.max(1)),
                u64::from(self.max_burn_time.max(1)),
                max,
            )
            .max(1)
        } else {
            0
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Forno de ferro")
            .bar(
                "Queima",
                u64::from(self.burn_time),
                u64::from(self.max_burn_time),
                "t",
            )
            .bar(
                "Progresso",
                u64::from(self.progress),
                u64::from(self.processing_time),
                "t",
            )
            .text("Os itens entram pela tela com slots.", view::GREY)
    }
}

/// What a hand and a hopper may do with a slot of these furnaces; `None` when
/// `block` is not one of them. The plain `Slot`s of the containers take any
/// item from a hand (1.7.10's `Slot.isItemValid` is true; the outputs are
/// `SlotSmelting`/`SlotCraftingOutput`, the upgrade `SlotUpgrade`); a hopper
/// asks `isItemValidForSlot` and `canExtractItem` of the tile entity.
pub fn allows(block: &str, slot: usize, item: &str, insert: bool, manual: bool) -> Option<bool> {
    let smelts = || recipes::furnace().smelt(item).is_some();
    let rule = match block {
        IRON if insert => match slot {
            iron::INPUT => manual || smelts(),
            s if iron::FUELS.contains(&s) => manual || IRON_MODULE.burn_time(item) > 0,
            iron::UPGRADE => manual && upgrades::is_upgrade(item),
            _ => false,
        },
        IRON => manual || slot == iron::OUTPUT,
        STEEL if insert => furnace_steel::INPUTS.contains(&slot) && (manual || smelts()),
        STEEL => manual || furnace_steel::OUTPUTS.contains(&slot),
        COMBINATION if insert => match slot {
            furnace_combination::INPUT => manual || furnace_combination::output(item).is_some(),
            furnace_combination::CONTAINER_IN => manual,
            _ => false,
        },
        COMBINATION => manual || slot == furnace_combination::OUTPUT,
        _ => return None,
    };
    Some(rule)
}

/// The container of a furnace block (`getAccessibleSlotsFromSide`).
pub fn container(block: &str) -> Option<crate::native::Container> {
    let (slots, reach, inputs, outputs): (usize, Vec<usize>, Vec<usize>, Vec<usize>) = match block {
        IRON => (
            iron::SLOTS,
            vec![0, 1, 2, 3],
            vec![0, 1, 2],
            vec![iron::OUTPUT],
        ),
        STEEL => (
            furnace_steel::SLOTS,
            (0..6).collect(),
            vec![0, 1, 2],
            vec![3, 4, 5],
        ),
        COMBINATION => (
            furnace_combination::SLOTS,
            vec![0, 1],
            vec![0],
            vec![furnace_combination::OUTPUT],
        ),
        _ => return None,
    };
    Some(crate::native::Container {
        slots,
        faces: std::array::from_fn(|_| reach.clone()),
        inputs,
        outputs,
        menu: Some("minecraft:generic_9x3"),
    })
}

/// The static rules of its slots (the conditional ones are [`allows`]); the
/// containers' `transferStackInSlot` merges a shift-click into slot 0 first,
/// so nothing narrows it.
pub fn filters(block: &str) -> Vec<crate::native::SlotFilter> {
    use crate::native::SlotFilter;
    let plain = SlotFilter::default();
    let output = SlotFilter {
        output: true,
        ..Default::default()
    };
    match block {
        IRON => vec![
            plain.clone(),
            plain.clone(),
            plain,
            output,
            SlotFilter {
                accept: Some(upgrades::ITEMS.iter().map(|s| s.to_string()).collect()),
                max: Some(1),
                ..Default::default()
            },
        ],
        STEEL => vec![
            plain.clone(),
            plain.clone(),
            plain,
            output.clone(),
            output.clone(),
            output,
        ],
        COMBINATION => vec![plain.clone(), output.clone(), plain, output],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iron_with(input: Option<(&str, u32)>, fuel_: Option<(&str, u32)>) -> Iron {
        let mut f = Iron::default();
        f.slots[iron::INPUT] = input.map(|(i, n)| Stack::new(i, n));
        f.slots[2] = fuel_.map(|(i, n)| Stack::new(i, n));
        f
    }

    #[test]
    fn it_loads_a_fuel_without_input_and_keeps_it_unspent() {
        let mut f = iron_with(None, Some(("minecraft:coal", 2)));
        f.step();
        assert_eq!((f.burn_time, f.max_burn_time), (2000, 2000));
        for _ in 0..50 {
            f.step();
        }
        assert_eq!(f.burn_time, 2000, "só queima enquanto funde");
        assert_eq!(f.slots[2], Some(Stack::new("minecraft:coal", 1)));
        assert!(!f.was_on);
    }

    #[test]
    fn a_coal_fuses_12_ingots_and_leaves_80_ticks() {
        let mut f = iron_with(
            Some(("minecraft:raw_iron", 64)),
            Some(("minecraft:coal", 1)),
        );
        for _ in 0..2000 {
            f.step();
        }
        assert_eq!(
            f.slots[iron::OUTPUT],
            Some(Stack::new("minecraft:iron_ingot", 12))
        );
        assert_eq!((f.burn_time, f.progress), (0, 80));
    }

    #[test]
    fn it_takes_the_first_fuel_slot_that_burns() {
        let mut f = iron_with(
            Some(("minecraft:raw_iron", 1)),
            Some(("minecraft:stick", 3)),
        );
        f.slots[1] = Some(Stack::new("minecraft:coal", 1));
        f.step();
        assert_eq!(f.max_burn_time, 2000);
        assert_eq!(f.slots[1], None);
        assert_eq!(f.slots[2], Some(Stack::new("minecraft:stick", 3)));
    }

    #[test]
    fn the_speed_upgrade_cuts_the_time_by_a_sixth_a_level() {
        let mut f = iron_with(None, None);
        for (item, time) in [
            (None, 160),
            (Some("ntm:upgrade_speed_1"), 134),
            (Some("ntm:upgrade_speed_2"), 107),
            (Some("ntm:upgrade_speed_3"), 80),
        ] {
            f.slots[iron::UPGRADE] = item.map(|i| Stack::new(i, 1));
            f.step();
            assert_eq!(f.processing_time, time);
        }
        // Another type changes nothing.
        f.slots[iron::UPGRADE] = Some(Stack::new("ntm:upgrade_power_3", 1));
        f.step();
        assert_eq!(f.processing_time, 160);
    }

    #[test]
    fn a_full_output_or_another_item_stops_it() {
        let mut f = iron_with(Some(("minecraft:raw_iron", 5)), Some(("minecraft:coal", 1)));
        f.slots[iron::OUTPUT] = Some(Stack::new("minecraft:iron_ingot", 64));
        f.step();
        assert_eq!(f.progress, 0);
        f.slots[iron::OUTPUT] = Some(Stack::new("minecraft:gold_ingot", 1));
        assert!(f.can_smelt().is_none());
    }

    #[test]
    fn it_never_smelts_without_burn_time() {
        let mut f = iron_with(
            Some(("minecraft:raw_iron", 5)),
            Some(("minecraft:stone", 1)),
        );
        for _ in 0..20 {
            f.step();
        }
        assert_eq!((f.progress, f.burn_time), (0, 0));
    }

    #[test]
    fn its_rules_are_the_slots_and_the_sided_inventorys() {
        let a = |slot, item, insert, manual| allows(IRON, slot, item, insert, manual);
        // A hand puts anything in the plain slots; a hopper only what fits them.
        assert_eq!(a(0, "minecraft:diamond", true, true), Some(true));
        assert_eq!(a(0, "minecraft:diamond", true, false), Some(false));
        assert_eq!(a(0, "minecraft:raw_iron", true, false), Some(true));
        assert_eq!(a(1, "minecraft:coal", true, false), Some(true));
        assert_eq!(a(2, "minecraft:stone", true, false), Some(false));
        assert_eq!(a(2, "minecraft:stone", true, true), Some(true));
        assert_eq!(a(3, "minecraft:iron_ingot", true, true), Some(false));
        assert_eq!(a(4, "ntm:upgrade_speed_1", true, true), Some(true));
        assert_eq!(a(4, "ntm:upgrade_speed_1", true, false), Some(false));
        assert_eq!(a(4, "minecraft:stone", true, true), Some(false));
        assert_eq!(a(0, "x", false, false), Some(false));
        assert_eq!(a(3, "x", false, false), Some(true));
        assert_eq!(a(0, "x", false, true), Some(true));
        assert_eq!(allows("ntm:machine_press", 0, "x", true, true), None);
        let c = |slot, item, insert, manual| allows(COMBINATION, slot, item, insert, manual);
        assert_eq!(c(0, "minecraft:coal", true, false), Some(true));
        assert_eq!(c(0, "minecraft:stone", true, false), Some(false));
        assert_eq!(c(2, "minecraft:bucket", true, true), Some(true));
        assert_eq!(c(2, "minecraft:bucket", true, false), Some(false));
        assert_eq!(c(3, "minecraft:bucket", true, true), Some(false));
        assert_eq!(c(1, "x", false, false), Some(true));
        assert_eq!(c(2, "x", false, false), Some(false));
    }

    #[test]
    fn the_state_survives_a_save() {
        let f = Iron {
            burn_time: 900,
            max_burn_time: 2000,
            progress: 33,
            ..Default::default()
        };
        let mut g = Iron::default();
        Slotted::load_state(&mut g, &Slotted::save_state(&f));
        assert_eq!((g.burn_time, g.max_burn_time, g.progress), (900, 2000, 33));
    }
}

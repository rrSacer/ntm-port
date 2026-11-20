//! The combination oven (`furnace_combination`), from
//! `tileentity/machine/TileEntityFurnaceCombination.java`: an item into an
//! item and a fluid, on the heat of the block under it. Its items are the
//! engine's slots: 0 the input, 1 the output, 2 an empty container, 3 the
//! full one; the 24 000 mB tank is its own and a sender of the fluid network.
//!
//! Left out: the soot and the smoke tanks of `TileEntityMachinePolluting`
//! (the port has no pollution yet), the fire it sets on entities above it,
//! and its sounds.

use crate::{
    fluid::{self, FluidNode, Tank},
    furnaces::{STACK, pull_heat},
    machine::{Logic, fill_level},
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const INPUT: usize = 0;
pub const OUTPUT: usize = 1;
pub const CONTAINER_IN: usize = 2;
pub const CONTAINER_OUT: usize = 3;
pub const SLOTS: usize = 4;
/// `processTime`, `maxHeat`, `diffusion` and the tank (lines 38-44).
pub const PROCESS_TIME: u32 = 20_000;
pub const MAX_HEAT: u32 = 100_000;
pub const DIFFUSION: f64 = 0.25;
pub const TANK: u32 = 24_000;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Recipe {
    pub ids: Vec<String>,
    /// An OreDict key: tried after the exact items (`getOutput`).
    #[serde(default)]
    pub dict: Option<String>,
    pub item: Option<(String, u32)>,
    pub fluid: Option<(String, u32)>,
    pub label: String,
}

#[derive(serde::Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

fn table() -> &'static [Recipe] {
    static T: OnceLock<Vec<Recipe>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str::<Table>(include_str!("../data/machines/combination.json"))
            .expect("data/machines/combination.json")
            .recipes
    })
}

/// `CombinationRecipes.getOutput` (lines 79-110): the exact item first,
/// then its OreDict keys.
pub fn output(item: &str) -> Option<&'static Recipe> {
    table()
        .iter()
        .find(|r| r.dict.is_none() && r.ids.iter().any(|i| i == item))
        .or_else(|| {
            table()
                .iter()
                .find(|r| r.dict.is_some() && r.ids.iter().any(|i| i == item))
        })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Combination {
    pub slots: Slots,
    pub progress: u32,
    pub heat: u32,
    pub was_on: bool,
    pub tank: Tank,
}

impl Default for Combination {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            progress: 0,
            heat: 0,
            was_on: false,
            tank: Tank::new(TANK),
        }
    }
}

impl Combination {
    /// `tryPullHeat` (lines 119-136).
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        pull_heat(&mut self.heat, MAX_HEAT, DIFFUSION, source)
    }

    /// `canSmelt` (lines 99-117).
    pub fn can_smelt(&self) -> Option<&'static Recipe> {
        let input = self.slots[INPUT].as_ref().filter(|s| s.count > 0)?;
        let r = output(&input.id)?;
        if let (Some((out, n)), Some(o)) = (&r.item, &self.slots[OUTPUT])
            && (o.id != *out || o.damage != 0 || o.count + n > STACK)
        {
            return None;
        }
        if let Some((f, n)) = &r.fluid {
            if self.tank.fluid != *f && self.tank.mb > 0 {
                return None;
            }
            if self.tank.fluid == *f && self.tank.mb + n > self.tank.cap {
                return None;
            }
        }
        Some(r)
    }

    /// `tank.unloadTank(2, 3, slots)`: one empty container a tick takes what
    /// the tank holds, when it fits in the full slot.
    fn fill_container(&mut self) {
        let Some(id) = self.slots[CONTAINER_IN].as_ref().map(|s| s.id.clone()) else {
            return;
        };
        let Some((full, mb)) =
            fluid::fill_container(&id, &self.tank.fluid).filter(|(_, mb)| self.tank.mb >= *mb)
        else {
            return;
        };
        if self.slots[CONTAINER_OUT]
            .as_ref()
            .is_some_and(|o| o.id != full || o.count >= STACK)
        {
            return;
        }
        self.tank.drain(mb);
        if let Some(s) = &mut self.slots[CONTAINER_IN] {
            s.count -= 1;
            if s.count == 0 {
                self.slots[CONTAINER_IN] = None;
            }
        }
        match &mut self.slots[CONTAINER_OUT] {
            Some(o) => o.count += 1,
            o => *o = Some(Stack::new(&full, 1)),
        }
    }

    /// One `updateEntity` (lines 65-107), after `tryPullHeat`.
    pub fn step(&mut self) -> bool {
        let before = (self.heat, self.progress, self.was_on, self.tank.mb);
        let slots_before = (
            self.slots[INPUT].clone(),
            self.slots[OUTPUT].clone(),
            self.slots[CONTAINER_IN].clone(),
        );
        self.was_on = false;
        self.fill_container();
        if let Some(r) = self.can_smelt() {
            let burn = self.heat / 100;
            if burn > 0 {
                self.was_on = true;
                self.progress += burn;
                self.heat -= burn;
                if self.progress >= PROCESS_TIME {
                    self.progress -= PROCESS_TIME;
                    if let Some((out, n)) = &r.item {
                        match &mut self.slots[OUTPUT] {
                            Some(o) => o.count += n,
                            o => *o = Some(Stack::new(out, *n)),
                        }
                    }
                    if let Some((f, n)) = &r.fluid {
                        self.tank.set_type(f);
                        self.tank.mb += n;
                    }
                    if let Some(i) = &mut self.slots[INPUT] {
                        i.count -= 1;
                        if i.count == 0 {
                            self.slots[INPUT] = None;
                        }
                    }
                }
            }
        } else {
            self.progress = 0;
        }
        before != (self.heat, self.progress, self.was_on, self.tank.mb)
            || slots_before
                != (
                    self.slots[INPUT].clone(),
                    self.slots[OUTPUT].clone(),
                    self.slots[CONTAINER_IN].clone(),
                )
    }

    /// The screen: progress, heat, the tank's fill, capacity and colour
    /// (`GUIFurnaceCombo`).
    pub fn screen_data(&self) -> Vec<i32> {
        vec![
            self.progress as i32,
            self.heat as i32,
            self.tank.mb as i32,
            self.tank.cap as i32,
            fluid::get(&self.tank.fluid).map_or(0, |f| f.color as i32),
        ]
    }
}

impl Slotted for Combination {
    /// Its record keeps its slots (the model's JSON): a restore from the journal has them.
    fn slots_recorded(&self) -> bool {
        true
    }
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Combination::screen_data(self)
    }
    /// The progress, the heat and the tank ride with the slots: a crash
    /// cannot leave the input smelted without the fluid it gave. The tank's
    /// type is its fluid's number in `data/fluids.json`.
    fn save_state(&self) -> Vec<i32> {
        let id = fluid::get(&self.tank.fluid).map_or(-1, |f| i32::from(f.id));
        vec![
            self.progress as i32,
            self.heat as i32,
            self.tank.mb as i32,
            id,
        ]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [p, h, mb, id, ..] = v {
            self.progress = (*p).max(0) as u32;
            self.heat = (*h).max(0) as u32;
            self.tank.fluid = fluid::types()
                .iter()
                .find(|f| i32::from(f.id) == *id)
                .map_or_else(String::new, |f| f.string_id.clone());
            self.tank.mb = (*mb).clamp(0, self.tank.cap as i32) as u32;
        }
    }
    fn held_contents(&self) -> Vec<(String, u32)> {
        Vec::new()
    }
}

/// `IFluidStandardSenderMK2`: the tank only gives.
impl FluidNode for Combination {
    fn fluid_offer(&self, f: &str) -> u64 {
        if self.tank.fluid == f {
            u64::from(self.tank.mb)
        } else {
            0
        }
    }
    fn fluid_demand(&self, _f: &str) -> u64 {
        0
    }
    fn fluid_types(&self) -> Vec<String> {
        if self.tank.fluid.is_empty() {
            Vec::new()
        } else {
            vec![self.tank.fluid.clone()]
        }
    }
    fn fluid_give(&mut self, _f: &str, _mb: u64) {}
    fn fluid_take(&mut self, _f: &str, mb: u64) {
        self.tank.drain(mb.min(u64::from(u32::MAX)) as u32);
    }
}

impl Logic for Combination {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.was_on {
            fill_level(u64::from(self.heat), u64::from(MAX_HEAT), max).max(1)
        } else {
            0
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Forno de combinação")
            .bar(
                "Progresso",
                u64::from(self.progress),
                u64::from(PROCESS_TIME),
                "TU",
            )
            .bar("Calor", u64::from(self.heat), u64::from(MAX_HEAT), "TU")
            .text(self.tank.line(), view::GREY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(input: &str, n: u32, heat: u32) -> Combination {
        let mut f = Combination {
            heat,
            ..Default::default()
        };
        f.slots[INPUT] = Some(Stack::new(input, n));
        f
    }

    #[test]
    fn it_pulls_a_quarter_of_the_difference_rounded_up() {
        let mut f = Combination::default();
        assert_eq!(f.pull_heat(Some(1000)), 250);
        assert_eq!(f.heat, 250);
        assert_eq!(f.pull_heat(Some(1000)), 188, "a quarter of 750, rounded up");
        let mut g = Combination {
            heat: 10,
            ..Default::default()
        };
        assert_eq!(g.pull_heat(Some(11)), 1, "never less than one");
        assert_eq!(g.heat, 11);
    }

    #[test]
    fn the_exact_item_comes_before_its_keys() {
        assert_eq!(
            output("minecraft:coal").unwrap().item,
            Some(("ntm:coke__coal".into(), 1))
        );
        assert_eq!(
            output("minecraft:coal").unwrap().fluid,
            Some(("coalcreosote".into(), 100))
        );
        assert_eq!(
            output("ntm:briquette__coal").unwrap().fluid,
            Some(("coalcreosote".into(), 150))
        );
        assert_eq!(
            output("minecraft:oak_log").unwrap().item,
            Some(("minecraft:charcoal".into(), 1))
        );
        assert_eq!(output("ntm:powder_sodium").unwrap().item, None);
        assert!(output("minecraft:stone").is_none());
        assert_eq!(
            output("ntm:bedrock_ore__base_light").unwrap().fluid,
            Some(("vitriol".into(), 50))
        );
    }

    #[test]
    fn at_full_heat_a_coal_takes_23_ticks_and_a_fifth_of_the_heat() {
        let mut f = with("minecraft:coal", 3, MAX_HEAT);
        let mut ticks = 0;
        while f.slots[OUTPUT].is_none() {
            f.step();
            ticks += 1;
            assert!(ticks < 2000);
        }
        // A hundredth of the heat a tick, falling: 20 000 TU take 23 ticks and
        // the furnace keeps 79 370 of its 100 000.
        assert_eq!((ticks, f.heat, f.progress), (23, 79_370, 630));
        assert_eq!(f.slots[OUTPUT], Some(Stack::new("ntm:coke__coal", 1)));
        assert_eq!((f.tank.fluid.as_str(), f.tank.mb), ("coalcreosote", 100));
        assert_eq!(f.slots[INPUT], Some(Stack::new("minecraft:coal", 2)));
    }

    #[test]
    fn it_burns_nothing_below_a_hundred_heat_and_keeps_the_progress() {
        let mut f = with("minecraft:coal", 1, 99);
        f.progress = 500;
        f.step();
        // The heat is the source's to pull (`tryPullHeat` runs apart): none is spent.
        assert_eq!((f.progress, f.heat), (500, 99));
        // Nothing to smelt resets the progress.
        f.slots[INPUT] = None;
        f.step();
        assert_eq!(f.progress, 0);
    }

    #[test]
    fn a_full_or_other_tank_stops_it() {
        let mut f = with("minecraft:coal", 1, MAX_HEAT);
        f.tank.fluid = "chlorine".into();
        f.tank.mb = 1;
        assert!(f.can_smelt().is_none());
        f.tank.fluid = "coalcreosote".into();
        f.tank.mb = TANK - 99;
        assert!(f.can_smelt().is_none());
        f.tank.mb = TANK - 100;
        assert!(f.can_smelt().is_some());
        // An emptied tank of another type is free again.
        f.tank.fluid = "chlorine".into();
        f.tank.mb = 0;
        assert!(f.can_smelt().is_some());
    }

    #[test]
    fn the_output_must_take_the_result() {
        let mut f = with("minecraft:coal", 1, MAX_HEAT);
        f.slots[OUTPUT] = Some(Stack::new("ntm:coke__coal", 64));
        assert!(f.can_smelt().is_none());
        f.slots[OUTPUT] = Some(Stack::new("ntm:coke__lignite", 1));
        assert!(f.can_smelt().is_none());
        // A recipe without an item ignores the output slot.
        let mut g = with("ntm:powder_sodium", 1, MAX_HEAT);
        g.slots[OUTPUT] = Some(Stack::new("minecraft:stone", 64));
        assert!(g.can_smelt().is_some());
    }

    #[test]
    fn it_fills_a_container_from_its_tank_a_tick() {
        let mut f = Combination::default();
        f.tank.fluid = "water".into();
        f.tank.mb = 1500;
        f.slots[CONTAINER_IN] = Some(Stack::new("minecraft:bucket", 3));
        f.step();
        assert_eq!(
            f.slots[CONTAINER_OUT],
            Some(Stack::new("minecraft:water_bucket", 1))
        );
        assert_eq!(
            (f.tank.mb, f.slots[CONTAINER_IN].as_ref().unwrap().count),
            (500, 2)
        );
        // 500 mB is not a bucket.
        f.step();
        assert_eq!(f.slots[CONTAINER_IN].as_ref().unwrap().count, 2);
    }

    #[test]
    fn the_state_survives_a_save() {
        let f = Combination {
            progress: 1234,
            heat: 5678,
            tank: Tank {
                fluid: "coalcreosote".into(),
                mb: 300,
                cap: TANK,
                pressure: 0,
            },
            ..Default::default()
        };
        let mut g = Combination::default();
        Slotted::load_state(&mut g, &Slotted::save_state(&f));
        assert_eq!(
            (g.progress, g.heat, g.tank.fluid.as_str(), g.tank.mb),
            (1234, 5678, "coalcreosote", 300)
        );
    }

    #[test]
    fn it_only_sends_fluid() {
        let mut f = Combination::default();
        f.tank.fluid = "coalcreosote".into();
        f.tank.mb = 400;
        assert_eq!(
            (
                f.fluid_offer("coalcreosote"),
                f.fluid_offer("water"),
                f.fluid_demand("coalcreosote")
            ),
            (400, 0, 0)
        );
        f.fluid_take("coalcreosote", 150);
        assert_eq!(f.tank.mb, 250);
    }
}

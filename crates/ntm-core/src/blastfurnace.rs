//! The blast furnace (docs/maquinas.md): `ntm-src/src/main/java/com/hbm/
//! tileentity/machine/TileEntityMachineBlastFurnace.java` and
//! `inventory/recipes/BlastFurnaceRecipesNT.java` (the current one; the
//! old `machine_difurnace` is deprecated in the NTM).
//!
//! No HE: it burns solid fuel (`ModuleBurnTime` with wood worth nothing),
//! half a coal per operation. Its slots are the engine's (`Slotted`): 0
//! fuel, 1 and 2 inputs, 3 and 4 outputs. Hot air (`airblast`, 4 000 mB,
//! 5% lost a tick) speeds it up to 5×; each operation makes flue gas
//! (`flue`, 1 000 mB). Left out (docs/maquinas.md): tilting
//! (`checkTilt`, off by default in the NTM), flue spilling into the world
//! when its tank is full (it just stops at the top), sound and particles.

use crate::{
    fluid::{FluidNode, Tank},
    server::container_bridge::{Slots, Stack},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SLOTS: usize = 5;
pub const FUEL: usize = 0;
pub const INPUTS: [usize; 2] = [1, 2];
pub const OUTPUTS: [usize; 2] = [3, 4];
/// `FUEL_COAL`, `FUEL_RATE` (half a coal an operation), `MAX_FUEL`, `FLUE_GAS`.
pub const FUEL_COAL: u32 = 200 * 8;
pub const FUEL_RATE: u32 = 200 * 4;
pub const MAX_FUEL: u32 = FUEL_COAL * 24;
pub const FLUE_GAS: u32 = 8;
pub const AIRBLAST: &str = "airblast";
pub const FLUE: &str = "flue";
pub const AIRBLAST_CAP: u32 = 4_000;
pub const FLUE_CAP: u32 = 1_000;
pub const STACK: u32 = 64;

/// An input: any of `ids` (OreDict alternatives), `n` of them.
#[derive(Clone, Debug, Deserialize)]
pub struct Input {
    pub ids: Vec<String>,
    pub n: u32,
}

impl Input {
    /// `AStack.matchesRecipe(stack, ignoreSize)`.
    fn matches(&self, s: Option<&Stack>, ignore_size: bool) -> bool {
        s.is_some_and(|s| self.ids.contains(&s.id) && (ignore_size || s.count >= self.n))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub duration: u32,
    #[serde(rename = "in")]
    pub inputs: Vec<Input>,
    #[serde(rename = "out")]
    pub outputs: Vec<(String, u32)>,
}

/// The recipes in the NTM's order, and what each fuel is worth.
#[derive(Clone, Debug, Default)]
pub struct Tables {
    pub recipes: Vec<Recipe>,
    pub fuels: HashMap<String, u32>,
}

impl Tables {
    /// `BlastFurnaceRecipesNT.getRecipe(s0, s1)`: the first that fits.
    pub fn recipe(&self, s0: Option<&Stack>, s1: Option<&Stack>) -> Option<&Recipe> {
        self.recipes.iter().find(|r| match r.inputs.as_slice() {
            [a] => (s1.is_none() && a.matches(s0, false)) || (s0.is_none() && a.matches(s1, false)),
            [a, b] => (a.matches(s0, true) && b.matches(s1, false)) || (b.matches(s0, true) && a.matches(s1, false)),
            _ => false,
        })
    }

    /// Whether an item may go in an input slot: some recipe takes it.
    pub fn takes(&self, item: &str) -> bool {
        self.recipes.iter().any(|r| r.inputs.iter().any(|i| i.ids.iter().any(|x| x == item)))
    }

    /// Heat units a fuel item gives (0: not a fuel here).
    pub fn fuel(&self, item: &str) -> u32 {
        self.fuels.get(item).copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlastFurnace {
    pub slots: Slots,
    pub fuel: u32,
    pub progress: f32,
    pub speed: f32,
    pub working: bool,
    pub airblast: Tank,
    pub flue: Tank,
}

impl Default for BlastFurnace {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            fuel: 0,
            progress: 0.0,
            speed: 0.0,
            working: false,
            airblast: Tank::typed(AIRBLAST, AIRBLAST_CAP),
            flue: Tank::typed(FLUE, FLUE_CAP),
        }
    }
}

impl BlastFurnace {
    fn slot(&self, i: usize) -> Option<&Stack> {
        self.slots[i].as_ref()
    }

    fn take(&mut self, i: usize, n: u32) {
        if let Some(s) = &mut self.slots[i] {
            s.count = s.count.saturating_sub(n);
            if s.count == 0 {
                self.slots[i] = None;
            }
        }
    }

    /// `hasQuantities`: both inputs there, in either slot.
    fn has_quantities(&self, r: &Recipe) -> bool {
        let (s1, s2) = (self.slot(1), self.slot(2));
        match r.inputs.as_slice() {
            [a] => a.matches(s1, false) || a.matches(s2, false),
            [a, b] => (a.matches(s1, false) && b.matches(s2, false)) || (a.matches(s2, false) && b.matches(s1, false)),
            _ => false,
        }
    }

    /// `canOutput`: each output fits its slot.
    fn can_output(&self, r: &Recipe) -> bool {
        r.outputs.iter().enumerate().all(|(i, (id, n))| match self.slot(OUTPUTS[i]) {
            None => true,
            Some(s) => s.id == *id && s.count + n <= STACK,
        })
    }

    /// `process`: outputs in, inputs out.
    fn process(&mut self, r: &Recipe) {
        for (i, (id, n)) in r.outputs.iter().enumerate() {
            match &mut self.slots[OUTPUTS[i]] {
                Some(s) => s.count += n,
                s => *s = Some(Stack::new(id, *n)),
            }
        }
        match r.inputs.as_slice() {
            [a] => {
                let slot = if a.matches(self.slot(1), false) { 1 } else { 2 };
                self.take(slot, a.n);
            }
            [a, b] => {
                if a.matches(self.slot(1), false) && b.matches(self.slot(2), false) {
                    self.take(1, a.n);
                    self.take(2, b.n);
                } else {
                    self.take(2, a.n);
                    self.take(1, b.n);
                }
            }
            _ => {}
        }
    }

    /// One tick (`updateEntity`); true when its items or state changed.
    pub fn step(&mut self, t: &Tables) -> bool {
        let before = (self.working, self.fuel);
        let mut moved = false;
        if let Some(s) = self.slot(FUEL) {
            let burn = t.fuel(&s.id);
            if burn > 0 && burn <= MAX_FUEL - self.fuel {
                self.fuel += burn;
                self.take(FUEL, 1);
                moved = true;
            }
        }
        self.speed = 0.0;
        let recipe = t.recipe(self.slot(1), self.slot(2)).cloned();
        match recipe {
            Some(r) if self.fuel >= FUEL_RATE && self.has_quantities(&r) && self.can_output(&r) => {
                self.speed = (1.0 + self.airblast.mb as f32 * 8.0 / AIRBLAST_CAP as f32).clamp(1.0, 5.0);
                self.working = true;
                self.progress += self.speed / r.duration as f32;
                if self.progress >= 1.0 {
                    self.process(&r);
                    self.progress = 0.0;
                    self.fuel -= FUEL_RATE;
                    let gas = (FLUE_GAS as f32 * (r.duration as f32 / self.speed)) as u32;
                    self.flue.mb = (self.flue.mb + gas).min(FLUE_CAP);
                    moved = true;
                }
            }
            _ => {
                self.working = false;
                self.progress = 0.0;
            }
        }
        if self.airblast.mb > 0 {
            self.airblast.mb = (self.airblast.mb as f32 * 0.95) as u32;
        }
        moved || before != (self.working, self.fuel)
    }

    /// The screen's values (`horizon_screen`).
    pub fn screen_data(&self) -> Vec<i32> {
        vec![
            self.fuel as i32,
            MAX_FUEL as i32,
            (self.progress * 1000.0) as i32,
            1000,
            (self.speed * 100.0) as i32,
            self.airblast.mb as i32,
            AIRBLAST_CAP as i32,
            self.flue.mb as i32,
            FLUE_CAP as i32,
            i32::from(self.working),
        ]
    }
}

impl crate::server::container_bridge::Slotted for BlastFurnace {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        BlastFurnace::screen_data(self)
    }
}

impl FluidNode for BlastFurnace {
    fn fluid_offer(&self, f: &str) -> u64 {
        if f == FLUE { u64::from(self.flue.mb) } else { 0 }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if f == AIRBLAST { u64::from(self.airblast.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        vec![AIRBLAST.into(), FLUE.into()]
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if f == AIRBLAST {
            self.airblast.mb = (self.airblast.mb + mb.min(u64::from(AIRBLAST_CAP)) as u32).min(AIRBLAST_CAP);
        }
    }
    fn fluid_take(&mut self, f: &str, mb: u64) {
        if f == FLUE {
            self.flue.mb = self.flue.mb.saturating_sub(mb.min(u64::from(FLUE_CAP)) as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(ids: &[&str], n: u32) -> Input {
        Input { ids: ids.iter().map(|s| s.to_string()).collect(), n }
    }

    /// Two of `BlastFurnaceRecipesNT`'s, as they are.
    fn tables() -> Tables {
        Tables {
            recipes: vec![
                Recipe {
                    name: "blast.steelFromIngot".into(),
                    duration: 800,
                    inputs: vec![input(&["minecraft:iron_ingot"], 2), input(&["minecraft:sand", "minecraft:red_sand"], 1)],
                    outputs: vec![("ntm:ingot_steel".into(), 2), ("ntm:ingot_raw__slag".into(), 1)],
                },
                Recipe {
                    name: "blast.mingrade".into(),
                    duration: 400,
                    inputs: vec![input(&["minecraft:copper_ingot"], 1), input(&["minecraft:redstone"], 1)],
                    outputs: vec![("ntm:ingot_red_copper".into(), 2)],
                },
            ],
            fuels: [("minecraft:coal", 1600), ("minecraft:coal_block", 16000)].into_iter().map(|(a, b)| (a.to_string(), b)).collect(),
        }
    }

    fn st(id: &str, n: u32) -> Option<Stack> {
        Some(Stack::new(id, n))
    }

    fn run(b: &mut BlastFurnace, t: &Tables, ticks: u32) {
        for _ in 0..ticks {
            b.step(t);
        }
    }

    #[test]
    fn steel_from_iron_and_sand_in_either_slot() {
        let t = tables();
        let mut b = BlastFurnace::default();
        b.slots[0] = st("minecraft:coal", 2);
        b.slots[1] = st("minecraft:sand", 3);
        b.slots[2] = st("minecraft:iron_ingot", 4);
        // 800 ticks at speed 1 (the progress is a float sum: a tick of margin).
        run(&mut b, &t, 801);
        assert_eq!(b.slots[3], st("ntm:ingot_steel", 2));
        assert_eq!(b.slots[4], st("ntm:ingot_raw__slag", 1));
        assert_eq!((b.slots[1].clone(), b.slots[2].clone()), (st("minecraft:sand", 2), st("minecraft:iron_ingot", 2)));
        // Fuel: a coal goes in when it fits (the second one too: 2 × 1600 ≤ 38 400), half a coal an operation.
        assert_eq!(b.slots[0], None);
        assert_eq!(b.fuel, 2 * FUEL_COAL - FUEL_RATE);
        // 8 mB a tick of the operation (6 400) into a tank of 1 000: full.
        assert_eq!(b.flue.mb, FLUE_CAP);
    }

    #[test]
    fn hot_air_speeds_it_up_and_wears_off() {
        let t = tables();
        let mut b = BlastFurnace { fuel: MAX_FUEL, ..Default::default() };
        b.slots[1] = st("minecraft:copper_ingot", 1);
        b.slots[2] = st("minecraft:redstone", 1);
        b.airblast.mb = AIRBLAST_CAP;
        b.step(&t);
        assert_eq!(b.speed, 5.0, "tanque cheio: 5×");
        // 5% a tick lost without a network feeding it.
        assert_eq!(b.airblast.mb, 3800);
        let mut ticks = 1;
        while b.slots[3].is_none() && ticks < 1000 {
            b.step(&t);
            ticks += 1;
        }
        assert!(ticks < 400, "o ar quente não acelerou: {ticks} ticks");
        assert_eq!(b.slots[3], st("ntm:ingot_red_copper", 2));
    }

    #[test]
    fn no_fuel_no_room_or_wrong_items_no_work() {
        let t = tables();
        let mut b = BlastFurnace::default();
        b.slots[1] = st("minecraft:iron_ingot", 2);
        b.slots[2] = st("minecraft:sand", 1);
        run(&mut b, &t, 900);
        assert_eq!(b.slots[3], None, "sem combustível");
        // Wood isn't fuel here (`setWoodHeatMod(0)`): stays in its slot.
        b.slots[0] = st("minecraft:oak_log", 1);
        b.step(&t);
        assert_eq!(b.slots[0], st("minecraft:oak_log", 1));
        b.fuel = MAX_FUEL;
        // One iron ingot short.
        b.slots[1] = st("minecraft:iron_ingot", 1);
        run(&mut b, &t, 900);
        assert_eq!(b.slots[3], None);
        // A full output stops it.
        b.slots[1] = st("minecraft:iron_ingot", 2);
        b.slots[3] = st("ntm:ingot_steel", 63);
        run(&mut b, &t, 900);
        assert_eq!(b.slots[3], st("ntm:ingot_steel", 63));
        assert!(!b.working);
        // A full fuel tank takes no more coal.
        b.slots[0] = st("minecraft:coal", 1);
        b.step(&t);
        assert_eq!(b.slots[0], st("minecraft:coal", 1));
    }

    #[test]
    fn the_recipe_lookup_is_the_ntms() {
        let t = tables();
        // Iron (2) first and sand (1) second, or the other way.
        assert!(t.recipe(st("minecraft:iron_ingot", 1).as_ref(), st("minecraft:sand", 1).as_ref()).is_some());
        assert!(t.recipe(st("minecraft:sand", 1).as_ref(), st("minecraft:iron_ingot", 2).as_ref()).is_some());
        assert!(t.recipe(st("minecraft:sand", 1).as_ref(), st("minecraft:iron_ingot", 1).as_ref()).is_none());
        assert!(t.recipe(st("minecraft:sand", 1).as_ref(), None).is_none());
        assert!(t.takes("minecraft:red_sand") && !t.takes("minecraft:dirt"));
    }

    #[test]
    fn fluids_go_through_the_network_node() {
        let mut b = BlastFurnace::default();
        assert_eq!(b.fluid_demand(AIRBLAST), 4000);
        b.fluid_give(AIRBLAST, 9999);
        assert_eq!(b.airblast.mb, 4000);
        b.flue.mb = 700;
        assert_eq!(b.fluid_offer(FLUE), 700);
        b.fluid_take(FLUE, 500);
        assert_eq!(b.flue.mb, 200);
        assert_eq!(b.fluid_demand(FLUE), 0);
    }
}

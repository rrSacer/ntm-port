//! The rotary furnace (`ntm:machine_rotary_furnace`, docs/content-foundry.md):
//! `tileentity/machine/TileEntityMachineRotaryFurnace.java` and
//! `inventory/recipes/RotaryFurnaceRecipes.java`.
//!
//! Three unordered input slots (`0..3`), an optional fluid tank (`tank_in`,
//! 16 000 mB, typed by the fluid identifier in slot `3`), solid fuel in slot `4`
//! (halved burn time, `ROTARY_FURNACE` burn/heat module), powered by steam
//! (`steam`, 12 000 mB -> `spentsteam`, 120 mB, 100:1) rather than electricity,
//! melting into a single molten `output` buffer (`10 368` quanta max = 16
//! blocks). `molten_tick` pours up to `72` quanta/tick (`INGOT.q(1)`, `safe =
//! true`) from `2.875` blocks toward `dir.getRotation(DOWN)` (`y + 1.25`).
//! On block break, `output` is lost (R8: Java does not override `breakBlock`
//! to drop molten scraps).

use crate::{
    fluid::{self, FluidNode, Tank},
    foundry::{Tables, quanta_name},
    fuel,
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const BLOCK: &str = "ntm:machine_rotary_furnace";
/// `TileEntityMachineRotaryFurnace.maxOutput` = `MaterialShapes.BLOCK.q(16)`.
pub const MAX_OUTPUT: u32 = 10_368;
/// `MaterialShapes.INGOT.q(1)` poured per tick.
pub const POUR: u32 = 72;
pub const TANK_IN_CAP: u32 = 16_000;
pub const STEAM_CAP: u32 = 12_000;
pub const SPENT_STEAM_CAP: u32 = 120;

pub mod slot {
    pub const INPUTS: std::ops::Range<usize> = 0..3;
    pub const FLUID_ID: usize = 3;
    pub const FUEL: usize = 4;
    pub const COUNT: usize = 5;
}

#[derive(Clone, Debug, Deserialize)]
pub struct Ingredient {
    pub ids: Vec<String>,
    pub n: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub label: String,
    pub duration: u32,
    pub steam: u32,
    #[serde(rename = "in")]
    pub inputs: Vec<Ingredient>,
    pub fluid: Option<(String, u32)>,
    pub out: (String, u32),
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn recipes() -> &'static [Recipe] {
    static T: OnceLock<Vec<Recipe>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str::<Table>(include_str!("../data/machines/rotary_furnace.json"))
            .expect("data/machines/rotary_furnace.json")
            .recipes
    })
}

/// `RotaryFurnaceRecipes.getRecipe(FluidType, ItemStack...):78-118`:
/// every non-empty slot in `0..3` must match an unused ingredient in
/// `recipe.inputs` with `count >= g.n`, all ingredients must be matched, and
/// if `recipe.fluid` is required then `tank_in.fluid` must match it.
pub fn match_recipe(tank_fluid: &str, slots: &[Option<Stack>]) -> Option<&'static Recipe> {
    let inputs: Vec<Option<&Stack>> = slot::INPUTS
        .map(|i| slots.get(i).and_then(|s| s.as_ref()).filter(|s| s.count > 0))
        .collect();
    recipes().iter().find(|r| {
        if let Some((f, _)) = &r.fluid
            && tank_fluid != f
        {
            return false;
        }
        let mut left: Vec<&Ingredient> = r.inputs.iter().collect();
        for stack in inputs.iter().flatten() {
            let Some(pos) = left
                .iter()
                .position(|g| g.ids.contains(&stack.id) && stack.count >= g.n)
            else {
                return false;
            };
            left.remove(pos);
        }
        left.is_empty()
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RotaryFurnace {
    /// Engine-managed slots (`0..5`).
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    pub tank_in: Tank,
    pub steam: Tank,
    pub spent_steam: Tank,
    pub progress: f32,
    pub burn_time: u32,
    pub max_burn_time: u32,
    pub burn_heat: f64,
    pub steam_used: i32,
    #[serde(default)]
    pub output: Option<(String, u32)>,
    #[serde(skip)]
    pub is_progressing: bool,
}

fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

impl Default for RotaryFurnace {
    fn default() -> Self {
        Self {
            slots: empty_slots(),
            tank_in: Tank::new(TANK_IN_CAP),
            steam: Tank::typed("steam", STEAM_CAP),
            spent_steam: Tank::typed("spentsteam", SPENT_STEAM_CAP),
            progress: 0.0,
            burn_time: 0,
            max_burn_time: 0,
            burn_heat: 1.0,
            steam_used: 0,
            output: None,
            is_progressing: false,
        }
    }
}

impl RotaryFurnace {
    pub fn recipe(&self) -> Option<&'static Recipe> {
        match_recipe(&self.tank_in.fluid, &self.slots)
    }

    /// `TileEntityMachineRotaryFurnace.canProcess`:324-338.
    pub fn can_process(&self, r: &Recipe, steam_use_mult: f32) -> bool {
        if self.burn_time == 0 {
            return false;
        }
        if let Some((f, mb)) = &r.fluid
            && (self.tank_in.fluid != *f || self.tank_in.mb < *mb)
        {
            return false;
        }
        let need_steam = r.steam as f32 * steam_use_mult;
        if (self.steam.mb as f32) < need_steam {
            return false;
        }
        if (self.spent_steam.free() as f32) < need_steam / 100.0 {
            return false;
        }
        if self.steam_used > 100 {
            return false;
        }
        if let Some((mat, q)) = &self.output
            && (*mat != r.out.0 || q + r.out.1 > MAX_OUTPUT)
        {
            return false;
        }
        true
    }

    /// `TileEntityMachineRotaryFurnace.consumeItems`:339-352.
    fn consume_items(&mut self, r: &Recipe) {
        let mut used = [false; 3];
        for g in &r.inputs {
            for (i, slot_used) in used.iter_mut().enumerate() {
                if *slot_used {
                    continue;
                }
                let Some(s) = &mut self.slots[i] else {
                    continue;
                };
                if g.ids.contains(&s.id) && s.count >= g.n {
                    s.count -= g.n;
                    if s.count == 0 {
                        self.slots[i] = None;
                    }
                    *slot_used = true;
                    break;
                }
            }
        }
        if let Some((_, mb)) = &r.fluid {
            self.tank_in.drain(*mb);
        }
    }

    /// One tick (`TileEntityMachineRotaryFurnace.updateEntity`:150, 174-227).
    pub fn step(&mut self) -> bool {
        if self.slots.len() < slot::COUNT {
            self.slots.resize(slot::COUNT, None);
        }
        let before = (
            self.is_progressing,
            self.progress.to_bits(),
            self.burn_time,
            self.max_burn_time,
            self.burn_heat.to_bits(),
            self.steam_used,
            self.tank_in.clone(),
            self.steam.mb,
            self.spent_steam.mb,
            self.output.clone(),
        );
        crate::combustion::type_from_identifier(&mut self.tank_in, &self.slots, slot::FLUID_ID, |_| true);
        self.steam.set_type("steam");
        self.spent_steam.set_type("spentsteam");

        self.is_progressing = false;
        if let Some(r) = self.recipe() {
            if self.burn_time == 0
                && let Some(s) = &self.slots[slot::FUEL]
                && fuel::burn_time(&s.id) > 0
            {
                self.burn_heat = fuel::Module::ROTARY_FURNACE.heat_mod(&s.id);
                let burn = fuel::Module::ROTARY_FURNACE.burn_time(&s.id) / 2;
                self.max_burn_time = burn;
                self.burn_time = burn;
                if let Some(s) = &mut self.slots[slot::FUEL] {
                    s.count = s.count.saturating_sub(1);
                    if s.count == 0 {
                        self.slots[slot::FUEL] = None;
                    }
                }
            }

            let process_speed = (self.burn_heat as f32).max(1.0);
            let steam_use_mult = 10.0 * process_speed.log10() + 1.0;

            if self.can_process(r, steam_use_mult) {
                self.progress += process_speed / r.duration.max(1) as f32;
                let used = r.steam as f32 * steam_use_mult;
                self.steam.mb = ((self.steam.mb as f32 - used) as i32).max(0) as u32;
                self.steam_used = (self.steam_used as f32 + used) as i32;
                self.is_progressing = true;

                if self.progress >= 1.0 {
                    self.progress -= 1.0;
                    self.consume_items(r);
                    match &mut self.output {
                        Some((_, q)) => *q += r.out.1,
                        None => self.output = Some((r.out.0.clone(), r.out.1)),
                    }
                }

                if self.burn_time > 0 {
                    self.burn_time -= 1;
                }
            } else {
                self.progress = 0.0;
            }

            if self.steam_used >= 100 {
                let steam_return = self.steam_used / 100;
                let can_return = self.spent_steam.free() as i32;
                let does_return = steam_return.min(can_return);
                self.steam_used -= does_return * 100;
                self.spent_steam.mb += does_return as u32;
            }
        } else {
            self.progress = 0.0;
        }

        let after = (
            self.is_progressing,
            self.progress.to_bits(),
            self.burn_time,
            self.max_burn_time,
            self.burn_heat.to_bits(),
            self.steam_used,
            self.tank_in.clone(),
            self.steam.mb,
            self.spent_steam.mb,
            self.output.clone(),
        );
        before != after
    }

    /// Pour up to `POUR` (`72`) quanta into `accept` (`CrucibleUtil.pourSingleStack`:156).
    /// Returns `true` if any quanta moved.
    pub fn pour(&mut self, mut accept: impl FnMut(&str, u32) -> Option<u32>) -> bool {
        let Some((mat, q)) = self.output.clone() else {
            return false;
        };
        if q == 0 {
            self.output = None;
            return false;
        }
        let n = q.min(POUR);
        let Some(left) = accept(&mat, n) else {
            return false;
        };
        let taken = n - left.min(n);
        if taken == 0 {
            return false;
        }
        let rem = q - taken;
        self.output = (rem > 0).then_some((mat, rem));
        true
    }
}

impl FluidNode for RotaryFurnace {
    fn accepts_any(&self) -> bool {
        false
    }

    fn fluid_types(&self) -> Vec<String> {
        let mut out = Vec::new();
        for t in [&self.tank_in, &self.steam, &self.spent_steam] {
            if !t.fluid.is_empty() && !out.contains(&t.fluid) {
                out.push(t.fluid.clone());
            }
        }
        out
    }

    fn fluid_offer(&self, fluid: &str) -> u64 {
        if fluid == "spentsteam" {
            u64::from(self.spent_steam.mb)
        } else {
            0
        }
    }

    fn fluid_demand(&self, fluid: &str) -> u64 {
        if fluid == "steam" {
            u64::from(self.steam.free())
        } else if !self.tank_in.fluid.is_empty() && self.tank_in.fluid == fluid {
            u64::from(self.tank_in.free())
        } else {
            0
        }
    }

    fn fluid_give(&mut self, fluid: &str, mb: u64) {
        let amt = mb.min(u64::from(u32::MAX)) as u32;
        if fluid == "steam" {
            self.steam.fill(fluid, amt);
        } else if !self.tank_in.fluid.is_empty() && self.tank_in.fluid == fluid {
            self.tank_in.fill(fluid, amt);
        }
    }

    fn fluid_take(&mut self, fluid: &str, mb: u64) {
        if fluid == "spentsteam" {
            self.spent_steam.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

impl Logic for RotaryFurnace {
    fn tick(&mut self) -> bool {
        self.step()
    }

    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }

    fn level(&self, _max: u8) -> u8 {
        u8::from(self.is_progressing)
    }

    fn view(&self) -> ViewModel {
        let out = match &self.output {
            Some((m, q)) if *q > 0 => format!("{m}: {}", quanta_name(*q)),
            _ => "vazio".into(),
        };
        ViewModel::new("Forno rotativo")
            .text(format!("Entrada: {}", self.tank_in.line()), view::GREY)
            .text(format!("Vapor: {}", self.steam.line()), view::GREY)
            .text(format!("Saída: {out}"), view::GREY)
    }
}

impl Slotted for RotaryFurnace {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }

    /// `horizon_screen/machine_rotary_furnace.json` indices `0..15`.
    fn screen_data(&self) -> Vec<i32> {
        let t = Tables::ntm();
        let prog = (self.progress.clamp(0.0, 1.0) * 10_000.0) as i32;
        let (out_q, out_col) = match &self.output {
            Some((m, q)) if *q > 0 => (*q as i32, t.color(m) as i32),
            _ => (0, 0),
        };
        let fluid_col = |f: &str| fluid::get(f).map_or(0, |ft| ft.color as i32);
        vec![
            prog,
            self.burn_time as i32,
            self.max_burn_time as i32,
            out_q,
            MAX_OUTPUT as i32,
            out_col,
            self.tank_in.mb as i32,
            TANK_IN_CAP as i32,
            fluid_col(&self.tank_in.fluid),
            self.steam.mb as i32,
            STEAM_CAP as i32,
            fluid_col(&self.steam.fluid),
            self.spent_steam.mb as i32,
            SPENT_STEAM_CAP as i32,
            fluid_col(&self.spent_steam.fluid),
        ]
    }

    fn save_state(&self) -> Vec<i32> {
        let t = Tables::ntm();
        let heat_bits = self.burn_heat.to_bits();
        let in_id = fluid::get(&self.tank_in.fluid).map_or(0, |f| i32::from(f.id));
        let (out_id, out_q) = match &self.output {
            Some((m, q)) if *q > 0 => (t.mat_id(Some(m)), *q as i32),
            _ => (0, 0),
        };
        vec![
            self.progress.to_bits() as i32,
            self.burn_time as i32,
            self.max_burn_time as i32,
            (heat_bits >> 32) as i32,
            heat_bits as i32,
            self.steam_used,
            in_id,
            self.tank_in.mb as i32,
            self.steam.mb as i32,
            self.spent_steam.mb as i32,
            out_id,
            out_q,
        ]
    }

    fn load_state(&mut self, v: &[i32]) {
        if v.len() < 12 {
            return;
        }
        let t = Tables::ntm();
        let prog = f32::from_bits(v[0] as u32);
        self.progress = if prog.is_finite() && prog >= 0.0 { prog } else { 0.0 };
        self.burn_time = v[1].max(0) as u32;
        self.max_burn_time = v[2].max(0) as u32;
        let heat_bits = ((v[3] as u32 as u64) << 32) | (v[4] as u32 as u64);
        let heat = f64::from_bits(heat_bits);
        self.burn_heat = if heat.is_finite() && heat > 0.0 { heat } else { 1.0 };
        self.steam_used = v[5].max(0);
        if let Some(ft) = fluid::types().iter().find(|f| i32::from(f.id) == v[6] && f.id != 0) {
            self.tank_in.fluid = ft.string_id.clone();
            self.tank_in.mb = (v[7].max(0) as u32).min(TANK_IN_CAP);
        } else {
            self.tank_in.fluid.clear();
            self.tank_in.mb = 0;
        }
        self.steam.mb = (v[8].max(0) as u32).min(STEAM_CAP);
        self.spent_steam.mb = (v[9].max(0) as u32).min(SPENT_STEAM_CAP);
        self.output = if v[10] > 0 && v[11] > 0 {
            t.mat_name(v[10]).map(|m| (m, (v[11] as u32).min(MAX_OUTPUT)))
        } else {
            None
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_stack(fluid: &str) -> Stack {
        Stack {
            id: "ntm:fluid_identifier_multi".into(),
            count: 1,
            damage: 0,
            custom: Some(format!("{{ntm_fluid:\"{fluid}\"}}")),
            tagged: false,
        }
    }

    #[test]
    fn loads_all_13_recipes_without_empty() {
        let list = recipes();
        assert_eq!(list.len(), 13);
        for r in list {
            assert!(!r.inputs.is_empty() || r.fluid.is_some(), "{}", r.name);
            assert!(r.duration > 0, "{}", r.name);
            assert!(r.out.1 > 0, "{}", r.name);
        }
    }

    #[test]
    fn recipe_matching_is_unordered_and_rejects_extra_items() {
        // Order does not matter: coal in slot 0, iron ingot in slot 2.
        let mut slots = empty_slots();
        slots[0] = Some(Stack::new("minecraft:coal", 1));
        slots[2] = Some(Stack::new("minecraft:iron_ingot", 1));
        let r = match_recipe("", &slots).expect("steel recipe");
        assert_eq!(r.out, ("steel".into(), 72));

        // Extra unmatched item in slot 1 breaks the match (Java :108-110).
        slots[1] = Some(Stack::new("minecraft:dirt", 1));
        assert!(match_recipe("", &slots).is_none());

        // Insufficient count fails.
        slots[1] = None;
        slots[0] = Some(Stack::new("ntm:powder_flux", 1));
        slots[2] = Some(Stack::new("ntm:fragment__iron", 8));
        assert!(match_recipe("", &slots).is_none());
    }

    #[test]
    fn fluid_identifier_types_tank_and_runs_recipe() {
        let mut f = RotaryFurnace::default();
        f.slots[0] = Some(Stack::new("ntm:powder_desh_ready", 1));
        f.slots[slot::FLUID_ID] = Some(id_stack("lightoil"));
        f.slots[slot::FUEL] = Some(Stack::new("minecraft:coal", 1));
        f.steam.mb = 12_000;

        // First step types tank_in to lightoil, lights coal (1600/2 = 800 ticks),
        // but tank_in is still 0 mB so canProcess is false (burn_time stays 800!).
        f.step();
        assert_eq!(f.tank_in.fluid, "lightoil");
        assert_eq!(f.burn_time, 800);
        assert!(!f.is_progressing);

        // Give lightoil and run 101 ticks -> 1 desh ingot (72 quanta; Java float 100 * 0.01f = 0.99999934f < 1f).
        f.fluid_give("lightoil", 500);
        assert_eq!(f.tank_in.mb, 500);
        for _ in 0..101 {
            f.steam.mb = STEAM_CAP;
            f.spent_steam.mb = 0;
            f.step();
        }
        assert_eq!(f.output, Some(("desh".into(), 72)));
        assert_eq!(f.tank_in.mb, 400);
        assert_eq!(f.slots[0], None);
        assert_eq!(f.burn_time, 699);
    }

    #[test]
    fn steam_use_mult_and_spent_steam_return_exact() {
        // Use rotary.0 (steel: iron + coal, duration 100, steam 100) with solid_fuel
        // (time 1.5 -> 4800 / 2 = 2400; heat 1.5 -> steamUseMult = 10*log10(1.5)+1).
        let mut f = RotaryFurnace::default();
        f.slots[0] = Some(Stack::new("minecraft:iron_ingot", 4));
        f.slots[1] = Some(Stack::new("minecraft:coal", 4));
        f.slots[slot::FUEL] = Some(Stack::new("ntm:solid_fuel", 1));
        f.steam.mb = 12_000;

        f.step();
        assert_eq!(f.burn_heat, 1.5);
        assert_eq!(f.max_burn_time, 2400);
        assert_eq!(f.burn_time, 2399);
        assert!(f.is_progressing);

        let mult = 10.0 * 1.5_f32.log10() + 1.0;
        let expected_used = (100.0 * mult) as i32;
        let expected_spent = expected_used / 100;
        assert_eq!(f.spent_steam.mb, expected_spent as u32);
        assert_eq!(f.steam_used, expected_used - expected_spent * 100);
        assert_eq!(f.steam.mb, ((12_000_f32 - 100.0 * mult) as i32) as u32);
    }

    #[test]
    fn spent_steam_full_blocks_processing_without_consuming_burn_time() {
        let mut f = RotaryFurnace::default();
        f.slots[0] = Some(Stack::new("minecraft:iron_ingot", 1));
        f.slots[1] = Some(Stack::new("minecraft:coal", 1));
        f.slots[slot::FUEL] = Some(Stack::new("minecraft:coal", 1));
        f.steam.mb = 12_000;
        f.spent_steam.mb = SPENT_STEAM_CAP; // 120/120 full -> free() = 0 < 100/100

        f.step();
        assert!(!f.is_progressing);
        assert_eq!(f.burn_time, 800, "Java does not decrement burnTime when canProcess is false");
        assert_eq!(f.progress, 0.0);
    }

    /// `canProcess`:337: more than 100 mB of steam used and not yet returned stops it; exactly 100 does not.
    #[test]
    fn it_stops_only_above_100_steam_used_and_not_returned() {
        let mut f = RotaryFurnace { burn_time: 100, ..Default::default() };
        f.steam.mb = 12_000;
        let r = &recipes()[0];
        f.steam_used = 100;
        assert!(f.can_process(r, 1.0), "100 is allowed");
        f.steam_used = 101;
        assert!(!f.can_process(r, 1.0), "101 is not");
    }

    /// `consumeItems`:339-352: each ingredient takes its own count out of a slot (nine fragments, not one).
    #[test]
    fn an_ingredient_takes_its_own_count_out_of_the_slot() {
        let mut f = RotaryFurnace::default();
        f.slots[0] = Some(Stack::new("ntm:bedrock_ore_fragment__iron", 20));
        f.slots[1] = Some(Stack::new("minecraft:coal", 3));
        let r = recipes().iter().find(|r| r.name == "rotary.2").unwrap();
        f.consume_items(r);
        assert_eq!(f.slots[0], Some(Stack::new("ntm:bedrock_ore_fragment__iron", 11)));
        assert_eq!(f.slots[1], Some(Stack::new("minecraft:coal", 2)));
    }

    /// `updateEntity`:197: progress grows by `processSpeed / duration` (the fuel's heat modifier), not by one tick's worth.
    #[test]
    fn progress_grows_with_the_process_speed() {
        let mut f = RotaryFurnace::default();
        f.slots[0] = Some(Stack::new("minecraft:iron_ingot", 4));
        f.slots[1] = Some(Stack::new("minecraft:coal", 4));
        f.slots[slot::FUEL] = Some(Stack::new("ntm:solid_fuel", 1)); // heat x1.5
        f.steam.mb = 12_000;
        f.step();
        assert_eq!(f.progress, 1.5 / 100.0);
    }

    #[test]
    fn r8_molten_output_is_lost_on_break() {
        let f = RotaryFurnace { output: Some(("steel".into(), 720)), ..Default::default() };
        assert!(f.held_contents().is_empty());
        assert!(f.contents().is_empty());
        assert!(Logic::scraps(&f).is_empty(), "the Java loses it: no scrap either");
    }

    #[test]
    fn save_and_load_state_roundtrip() {
        let mut f = RotaryFurnace {
            progress: 0.375,
            burn_time: 1234,
            max_burn_time: 2400,
            burn_heat: 3.0,
            steam_used: 77,
            ..Default::default()
        };
        f.tank_in.fluid = "lightoil".into();
        f.tank_in.mb = 4321;
        f.steam.mb = 9876;
        f.spent_steam.mb = 45;
        f.output = Some(("steel".into(), 216));

        let saved = f.save_state();
        assert_eq!(f.screen_data().len() + saved.len(), 27);
        assert!(f.screen_data().len() + saved.len() <= 32);

        let mut loaded = RotaryFurnace::default();
        loaded.load_state(&saved);
        assert_eq!(loaded.progress, f.progress);
        assert_eq!(loaded.burn_time, f.burn_time);
        assert_eq!(loaded.max_burn_time, f.max_burn_time);
        assert_eq!(loaded.burn_heat, f.burn_heat);
        assert_eq!(loaded.steam_used, f.steam_used);
        assert_eq!(loaded.tank_in.fluid, f.tank_in.fluid);
        assert_eq!(loaded.tank_in.mb, f.tank_in.mb);
        assert_eq!(loaded.steam.mb, f.steam.mb);
        assert_eq!(loaded.spent_steam.mb, f.spent_steam.mb);
        assert_eq!(loaded.output, f.output);
    }
}

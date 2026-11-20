//! The radiolysis chamber (`machine_radiolysis`), from `tileentity/machine/TileEntityMachineRadiolysis.java`: an RTG that
//! cracks fluids with its heat and sterilizes items.
//!
//! 15 slots (line 51): 0..9 RTG pellets, 10 and 11 the fluid identifier in and out, 12 and 13 the sterilization in and out, 14 the
//! battery; three tanks of 2 000 mB (the input, and two outputs typed by the recipe of the input). A tick (`updateEntity`, lines
//! 107-141): power goes to the battery item, the pellets give `heat` (`RTGUtil.updateRTGs`) and `heat * 10` HE, the input tank is
//! typed by the identifier (`FluidTank.setType(10, 11, slots)`) and the output tanks by its recipe (`setupTanks`); above 100 heat
//! it cracks 100 mB every `max(30 - 0.1 * (heat - 100), 5)` ticks (`crack`) and from 200 heat it sterilizes every 100 ticks.
//!
//! Recipes and item lists come from `data/machines/radiolysis.json` (`tools/content-gen/radiolysis_data.py`).

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    radioisotopes,
    server::container_bridge::{Slots, Slotted, Stack},
    snbt,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ops::Range};

pub const BLOCK: &str = "ntm:machine_radiolysis";
pub const SLOTS: usize = 15;
pub const RTG: Range<usize> = 0..10;
pub const FLUID_IN: usize = 10;
pub const FLUID_OUT: usize = 11;
pub const STERILIZE_IN: usize = 12;
pub const STERILIZE_OUT: usize = 13;
pub const BATTERY: usize = 14;
pub const MAX_POWER: u64 = 1_000_000;
pub const TANK: u32 = 2_000;
/// The NBT boolean `ntmContagion` (`ModEventHandler`, `EntityEffectHandler`), here a field of the item's custom data.
pub const CONTAGION: &str = "ntmContagion";

#[derive(Debug, Deserialize)]
struct Recipe {
    out: [(String, u32); 2],
}
#[derive(Debug, Deserialize)]
struct Foods {
    ntm: Vec<String>,
    vanilla: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct Table {
    cracking: BTreeMap<String, Recipe>,
    foods: Foods,
    depleted: Vec<String>,
}

fn table() -> &'static Table {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/radiolysis.json")).expect("data/machines/radiolysis.json"))
}

/// `RadiolysisRecipes.getRadiolysis`: what 100 mB of `fluid` become, as two fluid stacks (`""` is `NONE`).
pub fn recipe(fluid: &str) -> Option<[(&'static str, u32); 2]> {
    let r = table().cracking.get(fluid)?;
    let name = |f: &'static String| if f == "none" { "" } else { f.as_str() };
    Some([(name(&r.out[0].0), r.out[0].1), (name(&r.out[1].0), r.out[1].1)])
}

/// `instanceof ItemFood` and not the pancake (`sterilize`, line 198).
pub fn is_eaten(item: &str) -> bool {
    let f = &table().foods;
    f.ntm.iter().chain(&f.vanilla).any(|i| i == item)
}

/// `instanceof ItemRTGPelletDepleted`.
pub fn is_depleted(item: &str) -> bool {
    table().depleted.iter().any(|i| i == item)
}

/// `hasTagCompound && getBoolean("ntmContagion")` (`checkIfValid`).
pub fn has_contagion(s: &Stack) -> bool {
    s.custom
        .as_deref()
        .and_then(snbt::fields)
        .and_then(|f| f.get(CONTAGION).cloned())
        .is_some_and(|v| v.trim_end_matches(['b', 'B']).parse::<i64>().is_ok_and(|n| n != 0))
}

/// `removeTag("ntmContagion")`, and no tags left is no compound.
fn cleaned(s: &mut Stack) {
    s.custom = s.custom.as_deref().and_then(|c| snbt::without_field(c, CONTAGION));
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Radiolysis {
    pub slots: Slots,
    /// The input, the first output and the second.
    pub tanks: [Tank; 3],
    pub power: u64,
    /// What the heat made this tick: it joins the store after the network has taken its offer, as Java provides first.
    #[serde(default)]
    pub pending: u64,
    pub heat: i32,
    /// `getTotalWorldTime`, which the machines' tick does not get: its own count of ticks.
    #[serde(default)]
    pub clock: u64,
}

impl Default for Radiolysis {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], tanks: std::array::from_fn(|_| Tank::new(TANK)), power: 0, pending: 0, heat: 0, clock: 0 }
    }
}

/// The ticks between two cracks at `heat` (`(int) Math.max(-0.1 * (heat - 100) + 30, 5)`, line 125).
pub fn crack_period(heat: i32) -> u64 {
    (-0.1 * f64::from(heat - 100) + 30.0).max(5.0) as u64
}

impl Radiolysis {
    /// `Library.chargeItemsFromTE` on the battery slot: the creative battery takes everything; the port has no other charge in items.
    fn charge_battery(&mut self) {
        if self.slots[BATTERY].as_ref().is_some_and(|s| s.id == crate::machine::CREATIVE_BATTERY) {
            self.power = 0;
        } else {
            self.power = self.power.min(MAX_POWER);
        }
    }

    /// `FluidTank.setType(10, 11, slots)`: an identifier of another type than the tank's types it (emptying it) and goes to the
    /// out slot, which has to be empty.
    fn identify(&mut self) {
        let Some(kind) = self.slots[FLUID_IN].as_ref().and_then(crate::combustion::identifier_type) else { return };
        if self.slots[FLUID_OUT].is_none() && self.tanks[0].fluid != kind {
            self.tanks[0].set_type(&kind);
            self.slots[FLUID_OUT] = self.slots[FLUID_IN].take();
        }
    }

    /// `setupTanks`: the outputs take the types of the input's recipe, and an input that has none loses its type.
    fn setup_tanks(&mut self) {
        match recipe(&self.tanks[0].fluid) {
            Some([a, b]) => {
                self.tanks[1].set_type(a.0);
                self.tanks[2].set_type(b.0);
            }
            None => self.tanks.iter_mut().for_each(|t| t.set_type("")),
        }
    }

    /// `crack`: 100 mB of the input into the recipe's two stacks when both outputs have room.
    fn crack(&mut self) {
        let Some([(_, left), (_, right)]) = recipe(&self.tanks[0].fluid) else { return };
        if self.tanks[0].mb >= 100 && self.tanks[1].mb + left <= self.tanks[1].cap && self.tanks[2].mb + right <= self.tanks[2].cap {
            self.tanks[0].mb -= 100;
            self.tanks[1].mb += left;
            self.tanks[2].mb += right;
        }
    }

    /// `sterilize`: a food in the first slot is eaten (one), then an item with the contagion goes to the second without it.
    fn sterilize(&mut self) {
        let Some(held) = &mut self.slots[STERILIZE_IN] else { return };
        if is_eaten(&held.id) {
            held.count -= 1;
            if held.count == 0 {
                self.slots[STERILIZE_IN] = None;
            }
        }
        let Some(held) = self.slots[STERILIZE_IN].as_ref().filter(|s| has_contagion(s)) else { return };
        let mut output = Stack { count: 1, ..held.clone() };
        match &mut self.slots[STERILIZE_OUT] {
            None => {
                cleaned(&mut output);
                self.slots[STERILIZE_OUT] = Some(output);
            }
            // `isItemEqual` (item and damage) and the stack limit; the port knows no item's own limit: 64.
            Some(out) if out.id == output.id && out.damage == output.damage && out.count < 64 => {
                out.count += 1;
                cleaned(out);
            }
            Some(_) => return,
        }
        let held = self.slots[STERILIZE_IN].as_mut().expect("checked");
        held.count -= 1;
        if held.count == 0 {
            self.slots[STERILIZE_IN] = None;
        }
    }

    /// One `updateEntity`; true while it does something.
    pub fn step(&mut self) -> bool {
        let before = (self.power, self.tanks.clone(), self.slots[FLUID_IN..=STERILIZE_OUT].to_vec());
        // The battery took its share after the network's offer, the heat's power joins after it.
        self.charge_battery();
        self.power = (self.power + std::mem::take(&mut self.pending)).min(MAX_POWER);
        self.heat = radioisotopes::update_rtgs(&mut self.slots[RTG], true, false);
        self.pending = (self.heat * 10).max(0) as u64;
        self.identify();
        self.setup_tanks();
        if self.heat > 100 {
            if self.clock.is_multiple_of(crack_period(self.heat)) {
                self.crack();
            }
            if self.heat >= 200 && self.clock.is_multiple_of(100) {
                self.sterilize();
            }
        }
        self.clock = self.clock.wrapping_add(1);
        self.heat > 0 || before != (self.power, self.tanks.clone(), self.slots[FLUID_IN..=STERILIZE_OUT].to_vec())
    }
}

impl Logic for Radiolysis {
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
        if self.heat > 100 { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Câmara de radiólise")
            .text(format!("{} / {} HE", self.power, MAX_POWER), view::GREY)
            .text(format!("Calor: {}", self.heat), view::GREY)
    }
}

impl EnergyNode for Radiolysis {
    fn role(&self) -> Role {
        Role::Provider
    }
    fn offer(&self) -> u64 {
        self.power
    }
    fn demand(&self) -> u64 {
        0
    }
    fn give(&mut self, _he: u64) {}
    fn take(&mut self, he: u64) {
        self.power = self.power.saturating_sub(he);
    }
}

/// `getReceivingTanks` is the input, `getSendingTanks` the two outputs.
impl FluidNode for Radiolysis {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.tanks[1..].iter().filter(|t| !f.is_empty() && t.fluid == f).map(|t| u64::from(t.mb)).sum()
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if !f.is_empty() && self.tanks[0].fluid == f { u64::from(self.tanks[0].free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in &self.tanks {
            if !t.fluid.is_empty() && !out.contains(&t.fluid) {
                out.push(t.fluid.clone());
            }
        }
        out
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.tanks[0].fluid == f {
            self.tanks[0].fill(f, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take(&mut self, f: &str, mb: u64) {
        let mut left = mb.min(u64::from(u32::MAX)) as u32;
        for t in &mut self.tanks[1..] {
            if t.fluid == f && left > 0 {
                left -= t.drain(left);
            }
        }
    }
}

impl Slotted for Radiolysis {
    /// Its record keeps its slots (the model's JSON): a restore from the journal has them.
    fn slots_recorded(&self) -> bool {
        true
    }
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// The power and its limit, the heat, then each tank's mB, capacity and color.
    fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![self.power as i32, MAX_POWER as i32, self.heat];
        for t in &self.tanks {
            v.extend([t.mb as i32, t.cap as i32, crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)]);
        }
        v
    }
}

/// `isItemValidForSlot` and `canExtractItem`, and the menu's slots: a hand puts anything into the plain slots (0..9, 10, 12 and 14,
/// not the two take-only ones) and takes from every one; a hopper puts a pellet into the pellet slots and anything into the
/// sterilization slot, and takes the depleted pellets and the sterilized items.
pub fn allows(slot: usize, item: &str, insert: bool, manual: bool) -> bool {
    if slot >= SLOTS {
        return false;
    }
    match (insert, manual) {
        (true, true) => slot != FLUID_OUT && slot != STERILIZE_OUT,
        (true, false) => slot == STERILIZE_IN || (slot < 10 && radioisotopes::pellet(item).is_some()),
        (false, true) => true,
        (false, false) => (slot < 10 && is_depleted(item)) || slot == STERILIZE_OUT,
    }
}

/// `slot_io`: every face reaches the pellets and the two sterilization slots.
pub fn container() -> crate::native::Container {
    let io: Vec<usize> = (0..10).chain([STERILIZE_IN, STERILIZE_OUT]).collect();
    crate::native::Container {
        slots: SLOTS,
        faces: std::array::from_fn(|_| io.clone()),
        inputs: (0..10).chain([STERILIZE_IN]).collect(),
        outputs: (0..10).chain([STERILIZE_OUT]).collect(),
        menu: Some("minecraft:generic_9x3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(r: &mut Radiolysis, slot: usize, id: &str, n: u32) {
        r.slots[slot] = Some(Stack::new(id, n));
    }
    fn identifier(fluid: &str) -> Stack {
        Stack { custom: Some(format!("{{ntm_fluid:\"{fluid}\"}}")), ..Stack::new("ntm:fluid_identifier_multi", 1) }
    }
    /// A chamber hot enough to crack at every tick.
    fn hot() -> Radiolysis {
        let mut r = Radiolysis::default();
        for i in 0..10 {
            put(&mut r, i, "ntm:pellet_rtg_polonium", 1);
        }
        r
    }
    fn contaminated(id: &str) -> Stack {
        Stack { custom: Some("{ntmContagion:1b}".into()), ..Stack::new(id, 1) }
    }

    #[test]
    fn the_table_has_water_and_the_cracking_recipes() {
        assert_eq!(recipe("water"), Some([("peroxide", 80), ("hydrogen", 20)]));
        assert_eq!(recipe("oil"), Some([("crackoil", 80), ("petroleum", 20)]));
        assert_eq!(recipe("kerosene"), Some([("petroleum", 60), ("", 0)]), "no second output is NONE, nothing");
        assert_eq!(recipe("lava"), None);
        assert_eq!(table().cracking.len(), 13);
        assert!(is_eaten("minecraft:bread") && is_eaten("ntm:ingot_smore") && !is_eaten("ntm:pancake") && !is_eaten("minecraft:stick"));
        assert!(is_depleted("ntm:pellet_rtg_depleted__lead") && !is_depleted("ntm:pellet_rtg"));
    }

    #[test]
    fn the_heat_makes_ten_he_each_and_arrives_after_the_offer() {
        let mut r = Radiolysis::default();
        put(&mut r, 0, "ntm:pellet_rtg_radium", 1);
        let per = radioisotopes::pellet("ntm:pellet_rtg_radium").unwrap().heat_tu.decay_enabled;
        r.step();
        assert_eq!((r.heat, r.power, r.pending), (per, 0, per as u64 * 10), "a tick's heat is not in the store the network offers");
        r.step();
        assert_eq!(r.power, per as u64 * 10);
        // The store stops at the million.
        r.power = MAX_POWER - 1;
        r.step();
        assert_eq!(r.power, MAX_POWER);
    }

    #[test]
    fn the_creative_battery_takes_all_the_store() {
        let mut r = hot();
        r.power = 5_000;
        r.slots[BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        r.step();
        assert_eq!(r.power, 0, "taken after the offer, before the heat's power");
        r.step();
        assert_eq!(r.power, (r.heat * 10) as u64, "only the last tick's heat is left to offer, then taken again");
        r.step();
        assert_eq!(r.power, (r.heat * 10) as u64);
    }

    #[test]
    fn an_identifier_types_the_input_and_goes_to_the_out_slot() {
        let mut r = Radiolysis::default();
        r.slots[FLUID_IN] = Some(identifier("water"));
        r.step();
        assert_eq!(r.tanks[0].fluid, "water");
        assert_eq!(r.slots[FLUID_IN], None);
        assert_eq!(r.slots[FLUID_OUT].as_ref().map(|s| s.id.as_str()), Some("ntm:fluid_identifier_multi"));
        assert_eq!((r.tanks[1].fluid.as_str(), r.tanks[2].fluid.as_str()), ("peroxide", "hydrogen"), "the outputs are typed by the recipe");
        // With the out slot taken the next identifier does nothing (`FluidTank.setType`, line 157: `slots[out] == null`).
        r.tanks[0].mb = 500;
        r.slots[FLUID_IN] = Some(identifier("oil"));
        r.step();
        assert_eq!((r.tanks[0].fluid.as_str(), r.tanks[0].mb), ("water", 500));
        // The same type does not move it.
        r.slots[FLUID_OUT] = None;
        r.slots[FLUID_IN] = Some(identifier("water"));
        r.step();
        assert!(r.slots[FLUID_IN].is_some() && r.slots[FLUID_OUT].is_none());
    }

    #[test]
    fn a_fluid_with_no_recipe_loses_its_type_and_all_three_tanks_clear() {
        let mut r = Radiolysis::default();
        r.slots[FLUID_IN] = Some(identifier("lava"));
        r.step();
        assert_eq!(r.tanks.iter().map(|t| t.fluid.as_str()).collect::<Vec<_>>(), ["", "", ""]);
        // The outputs typed by oil's recipe and holding some, the input then given a type with no recipe.
        r.tanks[0].set_type("oil");
        r.step();
        r.tanks[0].mb = 700;
        r.tanks[1].mb = 5;
        r.tanks[2].mb = 9;
        assert_eq!((r.tanks[1].fluid.as_str(), r.tanks[2].fluid.as_str()), ("crackoil", "petroleum"));
        r.tanks[0].set_type("lava");
        r.step();
        assert!(r.tanks.iter().all(|t| t.mb == 0 && t.fluid.is_empty()), "{:?}", r.tanks);
        // A tank that already has no type keeps what it holds (`setTankType` is a no-op for the same type).
        r.tanks[1].mb = 5;
        r.step();
        assert_eq!(r.tanks[1].mb, 5);
    }

    #[test]
    fn it_cracks_a_hundred_at_a_time_above_a_hundred_heat() {
        let mut r = hot();
        r.slots[FLUID_IN] = Some(identifier("water"));
        r.step();
        r.tanks[0].mb = 1_000;
        let before = r.tanks[0].mb;
        for _ in 0..40 {
            r.step();
        }
        assert!(r.tanks[0].mb < before, "heat {} cracks", r.heat);
        assert_eq!(r.tanks[0].mb % 100, 0, "a hundred at a time");
        let cracked = (before - r.tanks[0].mb) / 100;
        assert_eq!(cracked, 8, "500 heat cracks every 5 ticks: ticks 5, 10 ... 40 of the clock");
        assert_eq!((r.tanks[1].mb, r.tanks[2].mb), (cracked * 80, cracked * 20));
        // Not enough heat: nothing.
        let mut cold = Radiolysis::default();
        put(&mut cold, 0, "ntm:pellet_rtg_radium", 1);
        cold.slots[FLUID_IN] = Some(identifier("water"));
        cold.step();
        cold.tanks[0].mb = 1_000;
        for _ in 0..200 {
            cold.step();
        }
        assert!(cold.heat <= 100 && cold.tanks[0].mb == 1_000, "{} heat", cold.heat);
    }

    #[test]
    fn it_waits_when_an_output_is_full() {
        let mut r = hot();
        r.slots[FLUID_IN] = Some(identifier("water"));
        r.step();
        r.tanks[0].mb = 1_000;
        r.tanks[2].mb = TANK - 10;
        for _ in 0..60 {
            r.step();
        }
        assert_eq!(r.tanks[0].mb, 1_000, "20 mB would not fit in the second output");
        r.tanks[2].mb = TANK - 20;
        for _ in 0..60 {
            r.step();
            if r.tanks[0].mb < 1_000 {
                break;
            }
        }
        assert_eq!((r.tanks[0].mb, r.tanks[2].mb), (900, TANK));
    }

    #[test]
    fn the_period_shortens_with_the_heat_down_to_five_ticks() {
        let period = crack_period;
        assert_eq!((period(101), period(200), period(300), period(350), period(1_000)), (29, 20, 10, 5, 5));
    }

    #[test]
    fn sterilizing_eats_food_and_cleans_the_contagion() {
        let mut r = hot();
        assert!(r.heat == 0);
        r.step();
        assert!(r.heat >= 200, "ten radium pellets give {}", r.heat);
        // A bread in the first slot is eaten, one per 100 ticks (and nothing else happens: no contagion).
        put(&mut r, STERILIZE_IN, "minecraft:bread", 3);
        r.clock = 99;
        r.step();
        assert_eq!(r.slots[STERILIZE_IN], Some(Stack::new("minecraft:bread", 3)), "not at tick 99");
        r.step();
        assert_eq!(r.slots[STERILIZE_IN], Some(Stack::new("minecraft:bread", 2)), "at tick 100 one is eaten");
        // The pancake is not eaten.
        put(&mut r, STERILIZE_IN, "ntm:pancake", 1);
        r.clock = 200;
        r.step();
        assert_eq!(r.slots[STERILIZE_IN], Some(Stack::new("ntm:pancake", 1)));
        // A contaminated item comes out clean in the second slot, one at a time.
        r.slots[STERILIZE_IN] = Some(Stack { count: 2, ..contaminated("minecraft:iron_sword") });
        r.clock = 300;
        r.step();
        assert_eq!(r.slots[STERILIZE_OUT], Some(Stack::new("minecraft:iron_sword", 1)), "the tag is gone and no tags left is no data");
        assert_eq!(r.slots[STERILIZE_IN].as_ref().map(|s| s.count), Some(1));
        // Another of the same joins it; its other data stays.
        r.slots[STERILIZE_IN] = Some(Stack { custom: Some("{ntmContagion:1b,keep:5}".into()), ..Stack::new("minecraft:iron_sword", 1) });
        r.clock = 400;
        r.step();
        assert_eq!(r.slots[STERILIZE_OUT].as_ref().map(|s| (s.count, s.custom.clone())), Some((2, None)));
        // A different item blocks it, and the contaminated one stays.
        put(&mut r, STERILIZE_OUT, "minecraft:diamond_sword", 1);
        r.slots[STERILIZE_IN] = Some(contaminated("minecraft:iron_sword"));
        r.clock = 500;
        r.step();
        assert!(r.slots[STERILIZE_IN].is_some());
        // An item with no contagion is left alone.
        r.slots[STERILIZE_IN] = Some(Stack::new("minecraft:iron_sword", 1));
        r.clock = 600;
        r.step();
        assert_eq!(r.slots[STERILIZE_IN], Some(Stack::new("minecraft:iron_sword", 1)));
    }

    #[test]
    fn a_contaminated_food_is_eaten_and_not_cleaned() {
        let mut r = hot();
        r.step();
        r.slots[STERILIZE_IN] = Some(contaminated("minecraft:bread"));
        r.clock = 100;
        r.step();
        assert_eq!((r.slots[STERILIZE_IN].clone(), r.slots[STERILIZE_OUT].clone()), (None, None), "line 199: eaten first, then nothing to check");
    }

    #[test]
    fn the_slots_a_hand_and_a_hopper_may_use() {
        let pellet = "ntm:pellet_rtg_radium";
        assert!(allows(0, "minecraft:dirt", true, true) && allows(10, "minecraft:dirt", true, true) && allows(14, "minecraft:dirt", true, true));
        assert!(!allows(11, "x", true, true) && !allows(13, "x", true, true), "the take-only slots");
        assert!(allows(0, pellet, true, false) && !allows(0, "minecraft:dirt", true, false) && !allows(10, pellet, true, false));
        assert!(allows(12, "minecraft:dirt", true, false), "any item into the sterilization slot (`i == 12`)");
        assert!(allows(0, "ntm:pellet_rtg_depleted__lead", false, false) && !allows(0, pellet, false, false));
        assert!(allows(13, "x", false, false) && !allows(12, "x", false, false) && !allows(11, "x", false, false));
        assert!((0..SLOTS).all(|s| allows(s, "x", false, true)));
        let c = container();
        assert!(c.faces.iter().all(|f| f == &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 13]));
    }

    #[test]
    fn what_the_screen_shows() {
        let mut r = Radiolysis { power: 123, heat: 45, ..Default::default() };
        r.tanks[0].set_type("water");
        r.tanks[0].mb = 600;
        let d = r.screen_data();
        assert_eq!((d.len(), d[0], d[1], d[2], d[3], d[4]), (12, 123, 1_000_000, 45, 600, 2_000));
    }

    #[test]
    fn the_network_sees_the_input_as_a_demand_and_the_outputs_as_an_offer() {
        let mut r = Radiolysis::default();
        r.tanks[0].set_type("water");
        r.tanks[1].set_type("peroxide");
        r.tanks[1].mb = 300;
        r.tanks[2].set_type("hydrogen");
        r.tanks[2].mb = 40;
        assert_eq!((r.fluid_demand("water"), r.fluid_demand("peroxide")), (2_000, 0));
        assert_eq!((r.fluid_offer("peroxide"), r.fluid_offer("hydrogen"), r.fluid_offer("water")), (300, 40, 0));
        r.fluid_take("peroxide", 100);
        r.fluid_give("water", 250);
        assert_eq!((r.tanks[1].mb, r.tanks[0].mb), (200, 250));
        assert_eq!(r.fluid_offer("water"), 0, "the input is never offered");
        assert_eq!(r.fluid_demand(""), 0, "no type is no demand");
        assert_eq!((r.role(), r.offer(), r.demand()), (Role::Provider, 0, 0));
    }

    #[test]
    fn the_menu_and_the_hoppers_reach_the_rules_through_native() {
        use crate::native::{SlotAction, SlotOrigin, slot_allows, slot_filters};
        let f = slot_filters(BLOCK);
        assert_eq!(f.len(), SLOTS);
        assert_eq!(f.iter().enumerate().filter(|(_, r)| r.output).map(|(i, _)| i).collect::<Vec<_>>(), [FLUID_OUT, STERILIZE_OUT]);
        let slots: Slots = vec![None; SLOTS];
        let pellet = Stack::new("ntm:pellet_rtg_polonium", 1);
        let dirt = Stack::new("minecraft:dirt", 1);
        let can = |slot, item: &Stack, a, o| slot_allows(BLOCK, &slots, slot, item, a, o);
        assert!(can(0, &pellet, SlotAction::Insert, SlotOrigin::Automation) && !can(0, &dirt, SlotAction::Insert, SlotOrigin::Automation));
        assert!(can(0, &dirt, SlotAction::Insert, SlotOrigin::Manual) && !can(13, &dirt, SlotAction::Insert, SlotOrigin::Manual));
        assert!(can(13, &dirt, SlotAction::Extract, SlotOrigin::Automation) && !can(12, &dirt, SlotAction::Extract, SlotOrigin::Automation));
        assert_eq!(crate::native::container(BLOCK), Some(container()));
    }

    #[test]
    fn the_thresholds_are_over_a_hundred_heat_to_crack_and_two_hundred_to_sterilize() {
        let chamber = |pellets: usize| {
            let mut r = Radiolysis::default();
            for i in 0..pellets {
                put(&mut r, i, "ntm:pellet_rtg_polonium", 1);
            }
            r.slots[FLUID_IN] = Some(identifier("water"));
            r.step();
            r.tanks[0].mb = 1_000;
            // A tick of the clock that is a multiple of every period here (30, 25 and 20), then one of the sterilization's.
            r.clock = 300;
            r.step();
            r.slots[STERILIZE_IN] = Some(contaminated("minecraft:iron_sword"));
            r.clock = 400;
            r.step();
            r
        };
        let (two, three, four) = (chamber(2), chamber(3), chamber(4));
        assert_eq!((two.heat, three.heat, four.heat), (100, 150, 200));
        assert_eq!(two.tanks[0].mb, 1_000, "exactly 100 does not crack");
        assert_eq!(three.tanks[0].mb, 800, "150 cracks on both ticks (multiples of its period 25)");
        assert!(three.slots[STERILIZE_IN].is_some() && four.slots[STERILIZE_IN].is_none(), "199 does not sterilize, 200 does");
    }

    #[test]
    fn the_sterilized_stack_stops_at_sixty_four() {
        let mut r = hot();
        r.step();
        for (have, left) in [(63, 0), (64, 1)] {
            r.slots[STERILIZE_OUT] = Some(Stack::new("minecraft:iron_sword", have));
            r.slots[STERILIZE_IN] = Some(contaminated("minecraft:iron_sword"));
            r.clock = 100;
            r.step();
            assert_eq!(r.slots[STERILIZE_IN].as_ref().map_or(0, |s| s.count), left, "{have} in the second slot");
        }
    }

    #[test]
    fn the_edges_of_what_it_cracks_and_where_it_fits() {
        let mut r = hot();
        r.slots[FLUID_IN] = Some(identifier("water"));
        r.step();
        // Exactly a hundred mB is enough, and an output with room for exactly the recipe's share takes it.
        r.tanks[0].mb = 100;
        r.tanks[1].mb = TANK - 80;
        r.tanks[2].mb = TANK - 20;
        r.clock = 5;
        r.step();
        assert_eq!((r.tanks[0].mb, r.tanks[1].mb, r.tanks[2].mb), (0, TANK, TANK));
        // Ninety-nine is not.
        r.tanks[0].mb = 99;
        r.tanks[1].mb = 0;
        r.tanks[2].mb = 0;
        r.clock = 10;
        r.step();
        assert_eq!((r.tanks[0].mb, r.tanks[1].mb), (99, 0));
    }

    #[test]
    fn a_contaminated_item_already_in_the_second_slot_is_cleaned_when_another_joins() {
        let mut r = hot();
        r.step();
        r.slots[STERILIZE_OUT] = Some(contaminated("minecraft:iron_sword"));
        r.slots[STERILIZE_IN] = Some(contaminated("minecraft:iron_sword"));
        r.clock = 100;
        r.step();
        assert_eq!(r.slots[STERILIZE_OUT], Some(Stack::new("minecraft:iron_sword", 2)));
    }

    #[test]
    fn a_tank_with_no_type_asks_for_nothing() {
        let r = Radiolysis::default();
        assert_eq!((r.fluid_demand(""), r.fluid_offer("")), (0, 0));
    }
}

//! The cyclotron (`machine_cyclotron`), from `tileentity/machine/TileEntityMachineCyclotron.java`: three lanes that bombard an
//! ingredient with a particle, turning it into another powder and some antimatter, with a lot of water as coolant.
//!
//! 12 slots (line 66): 0 to 2 the particles, 3 to 5 the targets, 6 to 8 the outputs, 9 the battery, 10 and 11 the upgrades; three
//! tanks (lines 68-72): water in (32 000 mB), spent steam out (32 000 mB) and antimatter out (8 000 mB); 100 000 000 HE.
//!
//! A tick (`updateEntity`, lines 109-137): the battery gives power; when it can process (power for the tick, coolant, room for the
//! steam and a lane whose recipe has room in its output) the progress runs by the speed, the power goes, the coolant becomes steam,
//! and at 690 every lane that can makes its output and adds its antimatter (`process`, lines 189-224).
//!
//! Recipes come from `data/machines/cyclotron.json` (`tools/content-gen/cyclotron_data.py`).

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::{Logic, charge_from_slot},
    server::container_bridge::{Slots, Slotted, Stack},
    upgrades,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_cyclotron";
pub const SLOTS: usize = 12;
pub const PARTICLES: std::ops::Range<usize> = 0..3;
pub const TARGETS: std::ops::Range<usize> = 3..6;
pub const OUTPUTS: std::ops::Range<usize> = 6..9;
pub const BATTERY: usize = 9;
pub const UPGRADES: std::ops::Range<usize> = 10..12;
pub const MAX_POWER: u64 = 100_000_000;
pub const CONSUMPTION: i32 = 1_000_000;
pub const DURATION: i32 = 690;
pub const WATER: &str = "water";
pub const STEAM: &str = "spentsteam";
pub const ANTIMATTER: &str = "amat";

#[derive(Debug, Deserialize)]
pub struct Recipe {
    pub particle: String,
    pub ingredients: Vec<String>,
    pub output: String,
    pub output_stack: u32,
    pub amat: u32,
}

#[derive(Debug, Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn recipes() -> &'static [Recipe] {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    &T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/cyclotron.json")).expect("data/machines/cyclotron.json")).recipes
}

/// `CyclotronRecipes.getOutput(target, particle)`: the recipe of that particle and ingredient. The Java map is a `HashMap`; no
/// stack is taken by two recipes of one particle (`collisions` is empty), so the order does not matter.
pub fn recipe(particle: &str, target: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|r| r.particle == particle && r.ingredients.iter().any(|i| i == target))
}

/// Every particle (the first key of a recipe), for the menu's shift-click.
pub fn particles() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in recipes() {
        if !out.contains(&r.particle) {
            out.push(r.particle.clone());
        }
    }
    out
}

/// Every ingredient of any recipe.
pub fn targets() -> Vec<String> {
    let mut out: Vec<String> = recipes().iter().flat_map(|r| r.ingredients.iter().cloned()).collect();
    out.sort();
    out.dedup();
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cyclotron {
    pub slots: Slots,
    /// Water in, spent steam out, antimatter out.
    pub tanks: [Tank; 3],
    pub power: u64,
    pub progress: i32,
}

impl Default for Cyclotron {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            tanks: [Tank::typed(WATER, 32_000), Tank::typed(STEAM, 32_000), Tank::typed(ANTIMATTER, 8_000)],
            power: 0,
            progress: 0,
        }
    }
}

impl Cyclotron {
    fn level(&self, kind: &str) -> i32 {
        upgrades::level(&self.slots[UPGRADES], kind, 3) as i32
    }

    /// `getSpeed`: 1 plus the speed upgrades.
    pub fn speed(&self) -> i32 {
        self.level("speed") + 1
    }

    /// `getConsumption`: a million HE less a hundred thousand for each power upgrade.
    pub fn consumption(&self) -> i32 {
        CONSUMPTION - 100_000 * self.level("power")
    }

    /// `getCoolantConsumption`: `500 / (effect + 1) * speed`, in Java `int`s.
    pub fn coolant(&self) -> i32 {
        500 / (self.level("effect") + 1) * self.speed()
    }

    fn id(&self, slot: usize) -> Option<&str> {
        self.slots[slot].as_ref().map(|s| s.id.as_str())
    }

    /// What lane `i` would make (`getOutput(slots[i + 3], slots[i])`), when the output slot has room (lines 155-171).
    fn lane(&self, i: usize) -> Option<&'static Recipe> {
        let r = recipe(self.id(i)?, self.id(i + 3)?)?;
        match &self.slots[i + 6] {
            None => Some(r),
            Some(o) if o.id == r.output && o.damage == 0 && o.count < r.output_stack => Some(r),
            Some(_) => None,
        }
    }

    pub fn can_process(&self) -> bool {
        let convert = self.coolant().max(0) as u32;
        if self.power < self.consumption().max(0) as u64 || self.tanks[0].mb < convert || self.tanks[1].mb + convert > self.tanks[1].cap {
            return false;
        }
        (0..3).any(|i| self.lane(i).is_some())
    }

    fn take_one(&mut self, slot: usize) {
        let held = self.slots[slot].as_mut().expect("a recipe needs it");
        held.count -= 1;
        if held.count == 0 {
            self.slots[slot] = None;
        }
    }

    /// `process` (lines 189-224): every lane with a recipe and room.
    fn process(&mut self) {
        for i in 0..3 {
            let Some(r) = self.lane(i) else { continue };
            self.take_one(i);
            self.take_one(i + 3);
            match &mut self.slots[i + 6] {
                None => self.slots[i + 6] = Some(Stack::new(&r.output, 1)),
                Some(o) => o.count += 1,
            }
            self.tanks[2].mb += r.amat;
        }
        self.tanks[2].mb = self.tanks[2].mb.min(self.tanks[2].cap);
    }

    /// One `updateEntity`; true while it does something.
    pub fn step(&mut self) -> bool {
        let before = (self.power, self.progress, self.tanks.clone(), self.slots.clone());
        self.power = charge_from_slot(&self.slots, BATTERY, self.power, MAX_POWER);
        if self.can_process() {
            let convert = self.coolant() as u32;
            self.progress += self.speed();
            self.power -= self.consumption() as u64;
            self.tanks[0].mb -= convert;
            self.tanks[1].mb += convert;
            if self.progress >= DURATION {
                self.process();
                self.progress = 0;
            }
        } else {
            self.progress = 0;
        }
        self.progress > 0 || before != (self.power, self.progress, self.tanks.clone(), self.slots.clone())
    }
}

impl Logic for Cyclotron {
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
        if self.progress > 0 { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Cíclotron")
            .text(format!("{} / {} HE", self.power, MAX_POWER), view::GREY)
            .text(format!("Progresso: {} / {}", self.progress, DURATION), view::GREY)
    }
}

impl EnergyNode for Cyclotron {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        MAX_POWER.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power = (self.power + he).min(MAX_POWER);
    }
    fn take(&mut self, _he: u64) {}
}

/// `getReceivingTanks` is the water, `getSendingTanks` the steam and the antimatter.
impl FluidNode for Cyclotron {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.tanks[1..].iter().filter(|t| t.fluid == f).map(|t| u64::from(t.mb)).sum()
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if self.tanks[0].fluid == f { u64::from(self.tanks[0].free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        self.tanks.iter().map(|t| t.fluid.clone()).collect()
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

impl Slotted for Cyclotron {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// The power and its limit, the progress scaled to the arrow's 34 pixels (`getProgressScaled`) and those 34, then each tank's mB,
    /// capacity and color.
    fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![self.power as i32, MAX_POWER as i32, (i64::from(self.progress) * 34 / i64::from(DURATION)) as i32, 34];
        for t in &self.tanks {
            v.extend([t.mb as i32, t.cap as i32, crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)]);
        }
        v
    }
    /// What lives outside the slots rides with them: a crash cannot leave a lane's inputs consumed with no output, or the
    /// coolant spent without its steam.
    fn save_state(&self) -> Vec<i32> {
        vec![self.power as i32, self.progress, self.tanks[0].mb as i32, self.tanks[1].mb as i32, self.tanks[2].mb as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [power, progress, water, steam, amat, ..] = v {
            self.power = (*power).max(0) as u64;
            self.progress = (*progress).clamp(0, DURATION);
            for (t, mb) in self.tanks.iter_mut().zip([water, steam, amat]) {
                t.mb = (*mb).clamp(0, t.cap as i32) as u32;
            }
        }
    }
}

/// `isItemValidForSlot` (lines 304-323) and the menu's slots for a hand and a hopper: a hand puts anything into the plain slots
/// (not the three outputs) and takes from every one; a hopper puts a particle of any recipe into a particle slot and an
/// ingredient of any recipe into a target slot, and takes the outputs.
pub fn allows(slot: usize, item: &str, insert: bool, manual: bool) -> bool {
    if slot >= SLOTS {
        return false;
    }
    match (insert, manual) {
        (true, true) => !OUTPUTS.contains(&slot),
        (false, true) => true,
        (false, false) => OUTPUTS.contains(&slot),
        (true, false) => {
            if PARTICLES.contains(&slot) {
                recipes().iter().any(|r| r.particle == item)
            } else if TARGETS.contains(&slot) {
                recipes().iter().any(|r| r.ingredients.iter().any(|i| i == item))
            } else {
                false
            }
        }
    }
}

/// `getAccessibleSlotsFromSide(x, y, z, side)` (lines 345-356): the three cells of each side of the box reach a lane each
/// (`{0, 3}`, `{1, 4}`, `{2, 5}`) and the three outputs. The engine's rules go by the machine, not by the cell the hopper touches
/// (`docs/native-slot-policy.md`), so every cell reaches all of them.
pub fn container() -> crate::native::Container {
    let reach: Vec<usize> = (0..9).collect();
    crate::native::Container {
        slots: SLOTS,
        faces: std::array::from_fn(|_| reach.clone()),
        inputs: (0..6).collect(),
        outputs: (6..9).collect(),
        menu: Some("minecraft:generic_9x3"),
    }
}

/// The twelve ports (`getAllPorts`, lines 91-107 of the block) in the placer's frame, each with the face it opens on: three on each side
/// of the 5x5 footprint, the NTM's `(x, z)` at `(-x, -z)` (`rotatePorts`) and its face turned the same way.
pub const PORTS: [([i32; 3], [i32; 3]); 12] = [
    ([-2, 0, -1], [-1, 0, 0]),
    ([-2, 0, 0], [-1, 0, 0]),
    ([-2, 0, 1], [-1, 0, 0]),
    ([2, 0, -1], [1, 0, 0]),
    ([2, 0, 0], [1, 0, 0]),
    ([2, 0, 1], [1, 0, 0]),
    ([-1, 0, -2], [0, 0, -1]),
    ([0, 0, -2], [0, 0, -1]),
    ([1, 0, -2], [0, 0, -1]),
    ([-1, 0, 2], [0, 0, 1]),
    ([0, 0, 2], [0, 0, 1]),
    ([1, 0, 2], [0, 0, 1]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn put(c: &mut Cyclotron, slot: usize, id: &str, n: u32) {
        c.slots[slot] = Some(Stack::new(id, n));
    }
    fn at(c: &Cyclotron, slot: usize) -> Option<(&str, u32)> {
        c.slots[slot].as_ref().map(|s| (s.id.as_str(), s.count))
    }
    /// A lane 0 that makes beryllium powder from lithium dust, on a creative battery with plenty of coolant (a cycle drinks 345 000 mB
    /// and makes as much steam, so the tanks are far bigger than the machine's).
    fn ready() -> Cyclotron {
        let mut c = Cyclotron::default();
        put(&mut c, 0, "ntm:part_lithium", 3);
        put(&mut c, 3, "ntm:powder_lithium", 3);
        put(&mut c, BATTERY, crate::machine::CREATIVE_BATTERY, 1);
        c.tanks[0].cap = 2_000_000;
        c.tanks[1].cap = 2_000_000;
        c.tanks[0].mb = 2_000_000;
        c
    }

    #[test]
    fn the_recipes_are_the_catalogs_thirty_eight() {
        assert_eq!(recipes().len(), 38);
        let r = recipe("ntm:part_lithium", "ntm:powder_lithium").unwrap();
        assert_eq!((r.output.as_str(), r.amat), ("ntm:powder_beryllium", 50));
        assert_eq!(recipe("ntm:part_plutonium", "ntm:pellet_charged").map(|r| (r.output.as_str(), r.amat)), Some(("ntm:nugget_schrabidium", 1000)));
        assert_eq!(recipe("ntm:part_carbon", "ntm:powder_lead").map(|r| r.output.as_str()), Some("ntm:powder_ra226"), "the unspaced line of the source is there too");
        assert!(recipe("ntm:part_copper", "ntm:powder_lithium").is_none() && recipe("ntm:part_lithium", "minecraft:dirt").is_none());
    }

    #[test]
    fn the_upgrades_set_the_speed_the_power_and_the_coolant() {
        let cases = [
            (&[][..], (1, 1_000_000, 500)),
            (&["ntm:upgrade_speed_1"][..], (2, 1_000_000, 1_000)),
            (&["ntm:upgrade_speed_3"][..], (4, 1_000_000, 2_000)),
            (&["ntm:upgrade_power_2"][..], (1, 800_000, 500)),
            (&["ntm:upgrade_power_3"][..], (1, 700_000, 500)),
            (&["ntm:upgrade_effect_1"][..], (1, 1_000_000, 250)),
            (&["ntm:upgrade_effect_2"][..], (1, 1_000_000, 166)),
            (&["ntm:upgrade_effect_3"][..], (1, 1_000_000, 125)),
            (&["ntm:upgrade_speed_3", "ntm:upgrade_effect_3"][..], (4, 1_000_000, 500)),
            (&["ntm:upgrade_speed_2", "ntm:upgrade_speed_3"][..], (4, 1_000_000, 2_000)),
        ];
        for (ups, (speed, consumption, coolant)) in cases {
            let mut c = Cyclotron::default();
            for (i, u) in ups.iter().enumerate() {
                put(&mut c, 10 + i, u, 1);
            }
            assert_eq!((c.speed(), c.consumption(), c.coolant()), (speed, consumption, coolant), "{ups:?}");
        }
    }

    #[test]
    fn six_hundred_and_ninety_ticks_make_a_powder_and_antimatter() {
        let mut c = ready();
        for _ in 0..689 {
            assert!(c.step());
        }
        assert_eq!((at(&c, 6), c.progress), (None, 689));
        assert_eq!((c.tanks[0].mb, c.tanks[1].mb), (2_000_000 - 689 * 500, 689 * 500));
        c.step();
        assert_eq!((at(&c, 6), at(&c, 0), at(&c, 3), c.progress, c.tanks[2].mb), (Some(("ntm:powder_beryllium", 1)), Some(("ntm:part_lithium", 2)), Some(("ntm:powder_lithium", 2)), 0, 50));
    }

    #[test]
    fn the_speed_upgrade_runs_the_progress_faster_and_drinks_more() {
        let mut c = ready();
        put(&mut c, 10, "ntm:upgrade_speed_3", 1);
        for _ in 0..172 {
            c.step();
        }
        assert_eq!((c.progress, c.tanks[1].mb), (688, 172 * 2_000));
        c.step();
        assert_eq!((at(&c, 6), c.progress), (Some(("ntm:powder_beryllium", 1)), 0), "ceil(690 / 4) ticks");
    }

    #[test]
    fn every_lane_with_a_recipe_makes_its_output_at_once() {
        let mut c = ready();
        put(&mut c, 1, "ntm:part_beryllium", 1);
        put(&mut c, 4, "ntm:powder_thorium", 1);
        put(&mut c, 2, "ntm:part_carbon", 1);
        put(&mut c, 5, "minecraft:dirt", 1);
        for _ in 0..690 {
            c.step();
        }
        assert_eq!((at(&c, 6), at(&c, 7), at(&c, 8)), (Some(("ntm:powder_beryllium", 1)), Some(("ntm:powder_uranium", 1)), None));
        assert_eq!(c.tanks[2].mb, 50 + 25, "lithium's 50 and beryllium's 25 antimatter");
        assert_eq!((at(&c, 2), at(&c, 5)), (Some(("ntm:part_carbon", 1)), Some(("minecraft:dirt", 1))), "the lane with no recipe is untouched");
    }

    #[test]
    fn it_waits_for_what_it_needs() {
        // No power, no coolant, no room for the steam, a full or other output: nothing runs and the progress is lost.
        let setups: [fn(&mut Cyclotron); 5] = [
            |c| c.slots[BATTERY] = None,
            |c| c.tanks[0].mb = 499,
            |c| c.tanks[1].mb = c.tanks[1].cap - 499,
            |c| c.slots[6] = Some(Stack::new("ntm:powder_beryllium", 64)),
            |c| c.slots[6] = Some(Stack::new("minecraft:dirt", 1)),
        ];
        for (n, setup) in setups.iter().enumerate() {
            let mut c = ready();
            c.power = 0;
            if n != 0 {
                c.power = MAX_POWER;
                c.slots[BATTERY] = None;
            }
            setup(&mut c);
            if n == 0 {
                c.power = 999_999;
            }
            c.progress = 40;
            let (tanks, power) = (c.tanks.clone(), c.power);
            c.step();
            assert_eq!((c.progress, c.tanks.clone(), c.power), (0, tanks, power), "setup {n}: the progress is lost and nothing is spent");
        }
        // Exactly the coolant, the room and the power of one tick is enough.
        let mut c = ready();
        c.slots[BATTERY] = None;
        c.power = 1_000_000;
        c.tanks[0].mb = 500;
        c.tanks[1].mb = c.tanks[1].cap - 500;
        let cap = c.tanks[1].cap;
        assert!(c.step());
        assert_eq!((c.progress, c.power, c.tanks[0].mb, c.tanks[1].mb), (1, 0, 0, cap));
    }

    #[test]
    fn a_lane_takes_a_stack_of_outputs_up_to_the_limit() {
        let mut c = ready();
        put(&mut c, 6, "ntm:powder_beryllium", 63);
        for _ in 0..690 {
            c.step();
        }
        assert_eq!(at(&c, 6), Some(("ntm:powder_beryllium", 64)));
        for _ in 0..5 {
            c.step();
        }
        assert_eq!(c.progress, 0, "a full output holds the lane");
    }

    #[test]
    fn the_antimatter_stops_at_its_tank() {
        let mut c = ready();
        c.tanks[2].mb = 7_990;
        for _ in 0..690 {
            c.step();
        }
        assert_eq!(c.tanks[2].mb, 8_000);
    }

    #[test]
    fn the_state_outside_the_slots_survives_a_save() {
        let mut c = Cyclotron::default();
        put(&mut c, 0, "ntm:part_lithium", 3);
        put(&mut c, 3, "ntm:powder_lithium", 3);
        put(&mut c, BATTERY, crate::machine::CREATIVE_BATTERY, 1);
        c.tanks[0].mb = 32_000;
        for _ in 0..30 {
            c.step();
        }
        let mut d = Cyclotron::default();
        d.load_state(&c.save_state());
        assert_eq!((d.power, d.progress, d.tanks.clone()), (c.power, c.progress, c.tanks.clone()));
        let mut e = Cyclotron::default();
        e.load_state(&[-5, 99_999, 99_999_999, -1, 9]);
        assert_eq!((e.power, e.progress, e.tanks[0].mb, e.tanks[1].mb, e.tanks[2].mb), (0, DURATION, 32_000, 0, 9));
    }

    #[test]
    fn what_the_screen_shows() {
        let mut c = Cyclotron { progress: 345, ..Default::default() };
        c.tanks[0].mb = 1_234;
        let d = c.screen_data();
        assert_eq!((d.len(), d[1], d[2], d[3], d[4], d[5], d[7], d[8], d[10], d[11]), (13, 100_000_000, 17, 34, 1_234, 32_000, 0, 32_000, 0, 8_000));
        c.progress = 20;
        assert_eq!(c.screen_data()[2], 0, "the lamp and the arrow wait for 21 ticks");
    }

    #[test]
    fn what_a_hand_and_a_hopper_may_do() {
        assert!(allows(0, "minecraft:dirt", true, true) && allows(9, "minecraft:dirt", true, true) && allows(11, "minecraft:dirt", true, true));
        assert!(!allows(6, "x", true, true) && !allows(8, "x", true, true) && allows(7, "x", false, true));
        assert!(allows(1, "ntm:part_copper", true, false) && !allows(1, "ntm:powder_lithium", true, false));
        assert!(allows(4, "ntm:powder_lithium", true, false) && !allows(4, "ntm:part_copper", true, false) && !allows(4, "minecraft:dirt", true, false));
        assert!(!allows(9, "ntm:part_copper", true, false) && !allows(10, "ntm:part_copper", true, false) && !allows(6, "ntm:part_copper", true, false));
        assert!(allows(7, "x", false, false) && allows(6, "x", false, false) && !allows(0, "x", false, false) && !allows(9, "x", false, false));
        assert!(!allows(12, "x", false, true));
    }

    #[test]
    fn the_menu_reaches_the_rules_through_native() {
        use crate::native::{SlotAction, SlotOrigin, slot_allows, slot_filters};
        let f = slot_filters(BLOCK);
        assert_eq!(f.len(), SLOTS);
        assert_eq!(f.iter().enumerate().filter(|(_, r)| r.output).map(|(i, _)| i).collect::<Vec<_>>(), [6, 7, 8]);
        // A shift-click puts a particle in a particle slot and an ingredient in a target slot, the battery and the upgrades in theirs.
        let q = |i: usize| f[i].quick_move.clone().unwrap_or_default();
        assert!(q(0).contains(&"ntm:part_lithium".to_string()) && !q(0).contains(&"ntm:powder_lithium".to_string()));
        assert!(q(3).contains(&"ntm:powder_lithium".to_string()) && !q(3).contains(&"ntm:part_lithium".to_string()));
        assert_eq!(q(9), vec![crate::machine::CREATIVE_BATTERY.to_string()]);
        assert!(q(10).contains(&"ntm:upgrade_speed_1".to_string()) && q(11).contains(&"ntm:upgrade_effect_3".to_string()));
        let slots: Slots = vec![None; SLOTS];
        let p = Stack::new("ntm:part_copper", 1);
        assert!(slot_allows(BLOCK, &slots, 0, &p, SlotAction::Insert, SlotOrigin::Automation));
        assert!(!slot_allows(BLOCK, &slots, 3, &p, SlotAction::Insert, SlotOrigin::Automation));
        assert!(slot_allows(BLOCK, &slots, 6, &p, SlotAction::Extract, SlotOrigin::Automation));
        assert_eq!(crate::native::container(BLOCK), Some(container()));
        let c = container();
        assert!(c.faces.iter().all(|f| f == &(0..9).collect::<Vec<_>>()));
        assert_eq!((c.inputs, c.outputs), ((0..6).collect::<Vec<_>>(), vec![6, 7, 8]));
    }

    #[test]
    fn the_network_sees_the_water_as_a_demand_and_the_steam_and_antimatter_as_offers() {
        let mut c = Cyclotron::default();
        c.tanks[1].mb = 300;
        c.tanks[2].mb = 40;
        assert_eq!((c.fluid_demand(WATER), c.fluid_demand(STEAM)), (32_000, 0));
        assert_eq!((c.fluid_offer(STEAM), c.fluid_offer(ANTIMATTER), c.fluid_offer(WATER)), (300, 40, 0));
        c.fluid_take(STEAM, 100);
        c.fluid_give(WATER, 250);
        assert_eq!((c.tanks[1].mb, c.tanks[0].mb), (200, 250));
        let mut e = Cyclotron::default();
        e.give(MAX_POWER + 5);
        assert_eq!((e.power, e.demand(), e.role(), e.offer()), (MAX_POWER, 0, Role::Consumer, 0));
    }

    #[test]
    fn the_twelve_ports_are_three_on_each_side_of_the_footprint_opening_outwards() {
        for (cell, face) in PORTS {
            assert_eq!(cell[1], 0);
            assert!(cell[0].abs() == 2 || cell[2].abs() == 2, "{cell:?} is on the edge");
            assert!(cell[0].abs() <= 2 && cell[2].abs() <= 2 && (cell[0].abs() != 2 || cell[2].abs() != 2), "{cell:?} is not a corner");
            let out = [cell[0] + face[0], cell[1], cell[2] + face[2]];
            assert!(out[0].abs() > 2 || out[2].abs() > 2, "{cell:?} opens outside the 5x5");
        }
    }
}

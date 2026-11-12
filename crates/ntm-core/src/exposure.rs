//! The exposure chamber (`machine_exposure_chamber`), from `tileentity/machine/TileEntityMachineExposureChamber.java`: a particle
//! capsule exposes eight ingots of its recipe's ingredient, one at a time.
//!
//! The Java inventory has eight slots (line 91): 0 the particle, 1 the loaded particle (internal), 2 the empty capsule it leaves, 3
//! the ingredient, 4 the output, 5 the battery, 6 and 7 the upgrades. The menu shows all but the internal one, and so do the
//! engine's slots here: 0 particle, 1 empty capsule, 2 ingredient, 3 output, 4 battery, 5 and 6 upgrades. The internal one is
//! a ghost with a stack size of 0 in Java (it only names the particle), so the port keeps it as the id of the capsule in `loaded`.
//!
//! A tick (`updateEntity`, lines 109-192): the battery gives power; the upgrades set the time and the consumption; with no particle
//! loaded and a capsule and an ingredient that make a recipe, the capsule is loaded (its empty one goes to slot 1, which must take
//! it) and gives 8 uses; with a particle loaded, power for one tick and room in the output, the progress runs, and at the end of
//! the time an ingredient becomes the output and a use is spent; with no uses left the particle is unloaded.
//!
//! Recipes come from `data/machines/exposure.json` (`tools/content-gen/exposure_data.py`).

use crate::{
    energy::{EnergyNode, Role},
    machine::{Logic, charge_from_slot},
    server::container_bridge::{Slots, Slotted, Stack},
    upgrades,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_exposure_chamber";
pub const SLOTS: usize = 7;
pub const PARTICLE: usize = 0;
pub const CAPSULE: usize = 1;
pub const INGREDIENT: usize = 2;
pub const OUTPUT: usize = 3;
pub const BATTERY: usize = 4;
pub const UPGRADES: std::ops::Range<usize> = 5..7;
pub const MAX_POWER: u64 = 1_000_000;
pub const PROCESS_TIME: i32 = 200;
pub const CONSUMPTION: i32 = 10_000;
pub const MAX_PARTICLES: i32 = 8;

#[derive(Debug, Deserialize)]
pub struct Recipe {
    pub particle: String,
    pub ingredients: Vec<String>,
    pub output: (String, u32),
    pub output_stack: u32,
    pub container: Option<String>,
    pub container_stack: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn recipes() -> &'static [Recipe] {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    &T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/exposure.json")).expect("data/machines/exposure.json")).recipes
}

/// `ExposureChamberRecipes.getRecipe`: the first recipe of that particle and ingredient.
pub fn recipe(particle: &str, ingredient: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|r| r.particle == particle && r.ingredients.iter().any(|i| i == ingredient))
}

/// `GeneralConfig`-free `getCode`: the recipes' particles, for saving which one is loaded.
fn particle_code(id: &str) -> i32 {
    recipes().iter().position(|r| r.particle == id).map_or(0, |i| i as i32 + 1)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Exposure {
    pub slots: Slots,
    /// `power`, `progress` and `savedParticles` (lines 49-51, 59, 63).
    pub power: u64,
    pub progress: i32,
    pub saved: i32,
    /// The internal particle (Java's slot 1): the id of the capsule that was loaded.
    pub loaded: Option<String>,
    /// `processTime` and `consumption` of the last tick (the upgrades' effect).
    pub process_time: i32,
    pub consumption: i32,
    pub on: bool,
}

impl Exposure {
    pub fn new() -> Self {
        Self { slots: vec![None; SLOTS], process_time: PROCESS_TIME, consumption: CONSUMPTION, ..Default::default() }
    }

    fn id(&self, slot: usize) -> Option<&str> {
        self.slots.get(slot).and_then(|s| s.as_ref()).map(|s| s.id.as_str())
    }

    /// The time and the consumption with the upgrades (lines 121-131), all in Java `int`s.
    fn upgrade_effects(&mut self) {
        let speed = upgrades::level(&self.slots[UPGRADES], "speed", 3) as i32;
        let power = upgrades::level(&self.slots[UPGRADES], "power", 3) as i32;
        let overdrive = upgrades::level(&self.slots[UPGRADES], "overdrive", 3) as i32;
        let mut consumption = CONSUMPTION;
        let mut time = PROCESS_TIME - PROCESS_TIME / 4 * speed;
        consumption *= speed / 2 + 1;
        time *= power / 2 + 1;
        consumption /= power + 1;
        time /= overdrive + 1;
        consumption *= overdrive * 2 + 1;
        self.process_time = time;
        self.consumption = consumption;
    }

    /// The capsule goes in: its empty one into slot 1 when that takes it (lines 134-160).
    fn load(&mut self) {
        let (Some(particle), Some(ingredient)) = (self.id(PARTICLE).map(str::to_string), self.id(INGREDIENT).map(str::to_string)) else { return };
        if self.loaded.is_some() || self.saved > 0 {
            return;
        }
        let Some(r) = recipe(&particle, &ingredient) else { return };
        let can_store = match &r.container {
            None => true,
            Some(c) => match self.slots[CAPSULE].as_mut() {
                None => {
                    self.slots[CAPSULE] = Some(Stack::new(c, 1));
                    true
                }
                Some(held) if held.id == *c && held.damage == 0 && held.count < r.container_stack.unwrap_or(64) => {
                    held.count += 1;
                    true
                }
                Some(_) => false,
            },
        };
        if can_store {
            self.loaded = Some(particle);
            let held = self.slots[PARTICLE].as_mut().expect("checked");
            held.count -= 1;
            if held.count == 0 {
                self.slots[PARTICLE] = None;
            }
            self.saved = MAX_PARTICLES;
        }
    }

    /// One `updateEntity`; true while it does something.
    pub fn step(&mut self) -> bool {
        let before = (self.power, self.progress, self.saved, self.loaded.clone(), self.slots.clone());
        self.on = false;
        self.power = charge_from_slot(&self.slots, BATTERY, self.power, MAX_POWER);
        self.upgrade_effects();
        self.load();
        let consumption = u64::from(self.consumption.max(0) as u32);
        if let Some(particle) = self.loaded.clone().filter(|_| self.saved > 0 && self.power >= consumption) {
            let r = self.id(INGREDIENT).and_then(|i| recipe(&particle, i));
            let room = |out: &Option<Stack>, r: &Recipe| match out {
                None => true,
                Some(o) => o.id == r.output.0 && o.damage == 0 && o.count + r.output.1 <= r.output_stack,
            };
            match r {
                Some(r) if room(&self.slots[OUTPUT], r) => {
                    self.progress += 1;
                    self.power -= consumption;
                    self.on = true;
                    if self.progress >= self.process_time {
                        self.progress = 0;
                        self.saved -= 1;
                        let held = self.slots[INGREDIENT].as_mut().expect("a recipe needs it");
                        held.count -= 1;
                        if held.count == 0 {
                            self.slots[INGREDIENT] = None;
                        }
                        match &mut self.slots[OUTPUT] {
                            None => self.slots[OUTPUT] = Some(Stack::new(&r.output.0, r.output.1)),
                            Some(o) => o.count += r.output.1,
                        }
                    }
                }
                _ => self.progress = 0,
            }
        } else {
            self.progress = 0;
        }
        if self.saved <= 0 {
            self.loaded = None;
        }
        self.on || before != (self.power, self.progress, self.saved, self.loaded.clone(), self.slots.clone())
    }
}

impl Logic for Exposure {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.on { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Câmara de exposição")
            .text(format!("{} / {} HE", self.power, MAX_POWER), view::GREY)
            .text(format!("Partículas: {} / {}", self.saved, MAX_PARTICLES), view::GREY)
    }
}

impl EnergyNode for Exposure {
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

impl Slotted for Exposure {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// The power and its limit, the progress and `processTime + 1` (the bar's divisor, `GUIMachineExposureChamber` line 54), the
    /// particles left and their limit, and whether the power covers a tick (the lamp).
    fn screen_data(&self) -> Vec<i32> {
        vec![
            self.power as i32,
            MAX_POWER as i32,
            self.progress,
            self.process_time + 1,
            self.saved,
            MAX_PARTICLES,
            i32::from(u64::from(self.consumption.max(0) as u32) <= self.power),
        ]
    }
    /// What lives outside the slots rides with them (power, progress, uses, which particle): a crash cannot leave a capsule
    /// consumed with no uses, or its uses with the capsule still in the slot.
    fn save_state(&self) -> Vec<i32> {
        vec![self.power as i32, self.progress, self.saved, self.loaded.as_deref().map_or(0, particle_code)]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [power, progress, saved, code, ..] = v {
            self.power = (*power).max(0) as u64;
            self.progress = (*progress).max(0);
            self.saved = (*saved).clamp(0, MAX_PARTICLES);
            self.loaded = usize::try_from(*code - 1).ok().and_then(|i| recipes().get(i)).map(|r| r.particle.clone());
        }
    }
}

/// `isItemValidForSlot` (lines 196-231) and the menu's slots for a hand and a hopper. A hand puts anything into the plain slots
/// (the empty capsule's and the output's are take-only); a hopper takes the empty capsules and the output out, and puts a
/// particle or an ingredient in only as the recipes ask. The Java reference for "a particle already loaded" is the internal
/// slot, which the engine's slots do not have: the particle slot stands for it (the port accepts an ingredient the Java refuses
/// while a capsule is loaded and its slot is empty).
pub fn allows(slots: &[Option<Stack>], slot: usize, item: &str, insert: bool, manual: bool) -> bool {
    if slot >= SLOTS {
        return false;
    }
    match (insert, manual) {
        (true, true) => slot != CAPSULE && slot != OUTPUT,
        (false, true) => true,
        (false, false) => slot == CAPSULE || slot == OUTPUT,
        (true, false) => {
            let id = |i: usize| slots.get(i).and_then(|s| s.as_ref()).map(|s| s.id.as_str());
            let (particle, ingredient) = (id(PARTICLE), id(INGREDIENT));
            match slot {
                PARTICLE | INGREDIENT => {
                    // A slot already holding something takes more of it (the stack merge skips the validity check).
                    if (slot == PARTICLE && particle.is_some()) || (slot == INGREDIENT && ingredient.is_some()) {
                        return true;
                    }
                    if slot == PARTICLE && ingredient.is_some() {
                        return ingredient.is_some_and(|i| recipe(item, i).is_some());
                    }
                    if slot == INGREDIENT && particle.is_some() {
                        return particle.is_some_and(|p| recipe(p, item).is_some());
                    }
                    // Nothing at all: any item some recipe takes in that place.
                    particle.is_none()
                        && ingredient.is_none()
                        && recipes().iter().any(|r| if slot == PARTICLE { r.particle == item } else { r.ingredients.iter().any(|i| i == item) })
                }
                _ => false,
            }
        }
    }
}

/// `getAccessibleSlotsFromSide`: `{0, 2, 3, 4}` of Java's, here the particle, the empty capsule, the ingredient and the output.
pub fn container() -> crate::native::Container {
    let all: Vec<usize> = vec![PARTICLE, CAPSULE, INGREDIENT, OUTPUT];
    crate::native::Container {
        slots: SLOTS,
        faces: std::array::from_fn(|_| all.clone()),
        inputs: vec![PARTICLE, INGREDIENT],
        outputs: vec![CAPSULE, OUTPUT],
        menu: Some("minecraft:generic_9x3"),
    }
}

/// The five ports (`getPorts`, lines 81-96) in the placer's frame, with the face each one opens on: the two legs' ends on the
/// right, `{x, z, face}`: the NTM's `(-7, 1)`, `(-7, -1)`, `(-8, 1)`, `(-8, -1)` and `(-8, 0)` at `(-x, -z)` (`rotatePorts`).
pub const PORTS: [([i32; 3], [i32; 3]); 5] = [
    ([7, 0, -1], [0, 0, -1]),
    ([7, 0, 1], [0, 0, 1]),
    ([8, 0, -1], [0, 0, -1]),
    ([8, 0, 1], [0, 0, 1]),
    ([8, 0, 0], [1, 0, 0]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn chamber() -> Exposure {
        let mut e = Exposure::new();
        e.power = MAX_POWER;
        e
    }
    fn put(e: &mut Exposure, slot: usize, id: &str, n: u32) {
        e.slots[slot] = Some(Stack::new(id, n));
    }
    fn at(e: &Exposure, slot: usize) -> Option<(&str, u32)> {
        e.slots[slot].as_ref().map(|s| (s.id.as_str(), s.count))
    }
    fn ready() -> Exposure {
        let mut e = chamber();
        put(&mut e, PARTICLE, "ntm:particle_higgs", 2);
        put(&mut e, INGREDIENT, "ntm:ingot_uranium", 20);
        e
    }

    #[test]
    fn the_recipes_are_the_four_defaults_in_order() {
        let r = recipes();
        assert_eq!(r.len(), 4);
        assert_eq!((r[0].particle.as_str(), r[0].ingredients.as_slice(), r[0].output.0.as_str()), ("ntm:particle_higgs", &["ntm:ingot_uranium".to_string()][..], "ntm:ingot_schraranium"));
        assert_eq!(recipe("ntm:particle_higgs", "ntm:ingot_u238").map(|r| r.output.0.as_str()), Some("ntm:ingot_schrabidium"));
        assert_eq!(recipe("ntm:particle_dark", "ntm:ingot_plutonium").map(|r| r.output.0.as_str()), Some("ntm:ingot_euphemium"));
        assert_eq!(recipe("ntm:particle_sparkticle", "ntm:ingot_schrabidate").map(|r| r.output.0.as_str()), Some("ntm:ingot_dineutronium"));
        assert!(recipe("ntm:particle_dark", "ntm:ingot_uranium").is_none());
    }

    #[test]
    fn a_capsule_is_loaded_with_eight_uses_and_leaves_its_empty_one() {
        let mut e = ready();
        e.step();
        assert_eq!((e.loaded.as_deref(), e.saved), (Some("ntm:particle_higgs"), 8));
        assert_eq!((at(&e, PARTICLE), at(&e, CAPSULE)), (Some(("ntm:particle_higgs", 1)), Some(("ntm:particle_empty", 1))));
        // The next capsule waits while this one has uses left.
        e.step();
        assert_eq!(at(&e, PARTICLE), Some(("ntm:particle_higgs", 1)));
    }

    #[test]
    fn two_hundred_ticks_make_an_ingot_and_use_one_of_eight() {
        let mut e = ready();
        e.slots[BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        for _ in 0..199 {
            e.step();
        }
        assert_eq!((at(&e, OUTPUT), e.saved, e.progress), (None, 8, 199), "the loading tick is also the first of the 200");
        e.step();
        assert_eq!((at(&e, OUTPUT), at(&e, INGREDIENT), e.saved, e.progress), (Some(("ntm:ingot_schraranium", 1)), Some(("ntm:ingot_uranium", 19)), 7, 0));
        assert_eq!(e.power, MAX_POWER - 10_000, "the battery refills what the tick takes");
        // With no battery, a million HE last a hundred ticks of ten thousand.
        let mut weak = ready();
        for _ in 0..150 {
            weak.step();
        }
        assert_eq!((weak.progress, weak.power), (0, 0));
    }

    #[test]
    fn a_particle_makes_eight_and_is_unloaded_when_spent() {
        let mut e = ready();
        e.slots[BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        for _ in 0..200 * 8 + 5 {
            e.step();
        }
        assert_eq!(at(&e, OUTPUT), Some(("ntm:ingot_schraranium", 8)));
        assert_eq!(at(&e, INGREDIENT), Some(("ntm:ingot_uranium", 12)));
        // The second capsule was loaded once the first had none left.
        assert_eq!((e.loaded.as_deref(), at(&e, PARTICLE)), (Some("ntm:particle_higgs"), None));
        assert_eq!(at(&e, CAPSULE), Some(("ntm:particle_empty", 2)));
    }

    #[test]
    fn no_power_no_progress() {
        let mut e = ready();
        e.power = 9_999;
        for _ in 0..50 {
            e.step();
        }
        assert_eq!((e.progress, e.power, e.on), (0, 9_999, false));
        e.power = 10_000;
        e.step();
        assert_eq!((e.progress, e.power, e.on), (1, 0, true));
    }

    #[test]
    fn a_full_or_other_output_stops_the_progress() {
        for out in [("ntm:ingot_schrabidium", 1), ("ntm:ingot_schraranium", 64)] {
            let mut e = ready();
            put(&mut e, OUTPUT, out.0, out.1);
            for _ in 0..20 {
                e.step();
            }
            assert_eq!((e.progress, e.on, at(&e, OUTPUT)), (0, false, Some(out)), "{out:?}");
        }
        let mut e = ready();
        e.slots[BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        put(&mut e, OUTPUT, "ntm:ingot_schraranium", 63);
        for _ in 0..201 {
            e.step();
        }
        assert_eq!(at(&e, OUTPUT), Some(("ntm:ingot_schraranium", 64)));
    }

    #[test]
    fn the_capsule_waits_when_the_empty_slot_cannot_take_its_empty_one() {
        for held in [("minecraft:dirt", 1), ("ntm:particle_empty", 64)] {
            let mut e = ready();
            put(&mut e, CAPSULE, held.0, held.1);
            e.step();
            assert_eq!((e.loaded.clone(), e.saved, at(&e, PARTICLE), at(&e, CAPSULE)), (None, 0, Some(("ntm:particle_higgs", 2)), Some(held)));
        }
    }

    #[test]
    fn the_upgrades_set_the_time_and_the_consumption_in_java_ints() {
        let cases = [
            (&[][..], (200, 10_000)),
            (&["ntm:upgrade_speed_1"][..], (150, 10_000)),
            (&["ntm:upgrade_speed_2"][..], (100, 20_000)),
            (&["ntm:upgrade_speed_3"][..], (50, 20_000)),
            (&["ntm:upgrade_power_1"][..], (200, 5_000)),
            (&["ntm:upgrade_power_2"][..], (400, 3_333)),
            (&["ntm:upgrade_power_3"][..], (400, 2_500)),
            (&["ntm:upgrade_overdrive_1"][..], (100, 30_000)),
            (&["ntm:upgrade_overdrive_3"][..], (50, 70_000)),
            (&["ntm:upgrade_speed_3", "ntm:upgrade_power_3"][..], (100, 5_000)),
        ];
        for (ups, (time, consumption)) in cases {
            let mut e = chamber();
            for (i, u) in ups.iter().enumerate() {
                put(&mut e, 5 + i, u, 1);
            }
            e.step();
            assert_eq!((e.process_time, e.consumption), (time, consumption), "{ups:?}");
        }
    }

    #[test]
    fn the_state_outside_the_slots_survives_a_save() {
        let mut e = ready();
        e.step();
        for _ in 0..30 {
            e.step();
        }
        let saved = e.save_state();
        let mut g = Exposure::new();
        g.load_state(&saved);
        assert_eq!((g.power, g.progress, g.saved, g.loaded.clone()), (e.power, e.progress, e.saved, e.loaded.clone()));
        let mut h = Exposure::new();
        h.load_state(&[5, -3, 99, 77]);
        assert_eq!((h.progress, h.saved, h.loaded), (0, MAX_PARTICLES, None), "nonsense is held to what makes sense");
    }

    #[test]
    fn what_the_screen_shows() {
        let mut e = ready();
        e.step();
        let d = e.screen_data();
        assert_eq!(d, vec![MAX_POWER as i32 - 10_000, 1_000_000, 1, 201, 8, 8, 1]);
        e.power = 100;
        assert_eq!(e.screen_data()[6], 0, "the lamp is off without a tick's power");
        e.power = 10_000;
        assert_eq!(e.screen_data()[6], 1, "exactly a tick's power lights it (`consumption <= power`)");
    }

    #[test]
    fn what_a_hand_and_a_hopper_may_do() {
        let empty: Slots = vec![None; SLOTS];
        let mut with_particle = empty.clone();
        with_particle[PARTICLE] = Some(Stack::new("ntm:particle_higgs", 1));
        let mut with_ingot = empty.clone();
        with_ingot[INGREDIENT] = Some(Stack::new("ntm:ingot_uranium", 1));
        // A hand: anything into the plain slots, nothing into the take-only ones.
        assert!(allows(&empty, PARTICLE, "minecraft:dirt", true, true) && allows(&empty, 5, "minecraft:dirt", true, true));
        assert!(!allows(&empty, CAPSULE, "x", true, true) && !allows(&empty, OUTPUT, "x", true, true));
        assert!(allows(&empty, OUTPUT, "x", false, true));
        // A hopper takes the empty capsules and the output.
        assert!(allows(&empty, CAPSULE, "x", false, false) && allows(&empty, OUTPUT, "x", false, false) && !allows(&empty, PARTICLE, "x", false, false));
        // With nothing at all, a particle or an ingredient of any recipe.
        assert!(allows(&empty, PARTICLE, "ntm:particle_dark", true, false) && !allows(&empty, PARTICLE, "ntm:ingot_uranium", true, false));
        assert!(allows(&empty, INGREDIENT, "ntm:ingot_plutonium", true, false) && !allows(&empty, INGREDIENT, "ntm:particle_dark", true, false));
        // With a particle, an ingredient of its recipe; with an ingredient, a particle of its recipe.
        assert!(allows(&with_particle, INGREDIENT, "ntm:ingot_u238", true, false) && !allows(&with_particle, INGREDIENT, "ntm:ingot_plutonium", true, false));
        assert!(allows(&with_ingot, PARTICLE, "ntm:particle_higgs", true, false) && !allows(&with_ingot, PARTICLE, "ntm:particle_dark", true, false));
        // A slot already holding something takes more of it.
        assert!(allows(&with_particle, PARTICLE, "ntm:particle_dark", true, false));
        // No other slot takes anything from a hopper.
        assert!(![CAPSULE, OUTPUT, BATTERY, 5, 6].iter().any(|&s| allows(&empty, s, "ntm:particle_higgs", true, false)));
    }

    #[test]
    fn the_menu_reaches_the_rules_through_native() {
        use crate::native::{SlotAction, SlotOrigin, slot_allows, slot_filters};
        let f = slot_filters(BLOCK);
        assert_eq!(f.len(), SLOTS);
        assert_eq!(f.iter().enumerate().filter(|(_, r)| r.output).map(|(i, _)| i).collect::<Vec<_>>(), [CAPSULE, OUTPUT]);
        let slots: Slots = vec![None; SLOTS];
        let d = Stack::new("ntm:particle_dark", 1);
        assert!(slot_allows(BLOCK, &slots, PARTICLE, &d, SlotAction::Insert, SlotOrigin::Automation));
        assert!(!slot_allows(BLOCK, &slots, PARTICLE, &Stack::new("minecraft:dirt", 1), SlotAction::Insert, SlotOrigin::Automation));
        assert!(slot_allows(BLOCK, &slots, OUTPUT, &d, SlotAction::Extract, SlotOrigin::Automation));
        assert!(!slot_allows(BLOCK, &slots, PARTICLE, &d, SlotAction::Extract, SlotOrigin::Automation));
        assert_eq!(crate::native::container(BLOCK), Some(container()));
    }

    #[test]
    fn the_energy_demand_is_the_room() {
        let mut e = Exposure::new();
        e.give(MAX_POWER + 5);
        assert_eq!((e.power, e.demand(), e.role(), e.offer()), (MAX_POWER, 0, Role::Consumer, 0));
    }

    #[test]
    fn the_container_lets_hoppers_reach_the_four_slots_of_java() {
        let c = container();
        assert_eq!(c.slots, SLOTS);
        assert!(c.faces.iter().all(|f| f == &[PARTICLE, CAPSULE, INGREDIENT, OUTPUT]), "`getAccessibleSlotsFromSide` is {{0, 2, 3, 4}} on every side");
        assert_eq!((c.inputs, c.outputs), (vec![PARTICLE, INGREDIENT], vec![CAPSULE, OUTPUT]));
    }
}

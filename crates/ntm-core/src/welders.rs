//! The arc welder (`machine_arc_welder`, `TileEntityMachineArcWelder.java`)
//! and the soldering station (`machine_soldering_station`,
//! `TileEntityMachineSolderingStation.java`): one recipe machine that powers
//! a recipe found from its slots, with an optional fluid and the speed, power
//! and overdrive upgrades. Its items are the engine's slots.
//!
//! Arc welder (8 slots): 0..3 the inputs, 3 the output, 4 the battery, 5 the
//! fluid identifier, 6 and 7 upgrades; a 24 000 mB tank. Soldering station
//! (11 slots): 0..3 toppings, 3..5 the pcb, 5 the solder, 6 the output, 7 the
//! battery, 8 the identifier, 9 and 10 upgrades; an 8 000 mB tank; a button
//! that refuses a fluid the recipe does not use (`collisionPrevention`).
//!
//! The tank is typed by the fluid identifier in its slot, as the Java does
//! (`tank.setType(5, slots)` / `setType(8, slots)`): without one it takes no
//! fluid, and another identifier empties it. Left out: the particles and the
//! recipe's output ghost in the screen.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{self, FluidNode, Tank},
    machine::{Logic, charge_from_slot},
    server::container_bridge::{Slots, Slotted, Stack},
    upgrades,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const ARC: &str = "ntm:machine_arc_welder";
pub const SOLDER: &str = "ntm:machine_soldering_station";
const STACK: u32 = 64;

fn default_stack() -> u32 {
    STACK
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Model {
    Arc,
    Solder,
}

impl Model {
    pub fn of(block: &str) -> Option<Model> {
        match block {
            ARC => Some(Model::Arc),
            SOLDER => Some(Model::Solder),
            _ => None,
        }
    }
    pub fn slots(self) -> usize {
        match self {
            Model::Arc => 8,
            Model::Solder => 11,
        }
    }
    /// The output, battery and upgrade slots.
    pub fn output(self) -> usize {
        match self {
            Model::Arc => 3,
            Model::Solder => 6,
        }
    }
    pub fn battery(self) -> usize {
        match self {
            Model::Arc => 4,
            Model::Solder => 7,
        }
    }
    pub fn upgrade_slots(self) -> std::ops::Range<usize> {
        match self {
            Model::Arc => 6..8,
            Model::Solder => 9..11,
        }
    }
    pub fn inputs(self) -> std::ops::Range<usize> {
        match self {
            Model::Arc => 0..3,
            Model::Solder => 0..6,
        }
    }
    /// The fluid identifier's slot (`setType(5, slots)`, `setType(8, slots)`).
    pub fn identifier(self) -> usize {
        match self {
            Model::Arc => 5,
            Model::Solder => 8,
        }
    }
    fn tank(self) -> u32 {
        match self {
            Model::Arc => 24_000,
            Model::Solder => 8_000,
        }
    }
    fn title(self) -> &'static str {
        match self {
            Model::Arc => "Soldadora a arco",
            Model::Solder => "Estação de solda",
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Ingredient {
    pub ids: Vec<String>,
    pub n: u32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Recipe {
    pub out: (String, u32),
    /// The product's `getMaxStackSize()`: the output slot stops there.
    #[serde(default = "default_stack")]
    pub stack: u32,
    pub duration: u32,
    pub power: u64,
    pub fluid: Option<(String, u32)>,
    #[serde(default, rename = "in")]
    pub arc: Vec<Ingredient>,
    #[serde(default)]
    pub toppings: Vec<Ingredient>,
    #[serde(default)]
    pub pcb: Vec<Ingredient>,
    #[serde(default)]
    pub solder: Vec<Ingredient>,
    pub label: String,
}

#[derive(serde::Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn recipes(model: Model) -> &'static [Recipe] {
    static A: OnceLock<Vec<Recipe>> = OnceLock::new();
    static S: OnceLock<Vec<Recipe>> = OnceLock::new();
    let table = |json: &str| {
        serde_json::from_str::<Table>(json)
            .expect("data/machines")
            .recipes
    };
    match model {
        Model::Arc => A.get_or_init(|| table(include_str!("../data/machines/arc_welder.json"))),
        Model::Solder => S.get_or_init(|| table(include_str!("../data/machines/soldering.json"))),
    }
}

/// `SerializableRecipe.matchesIngredients`: every non-empty input takes the
/// first still-unmatched ingredient that wants it (an id of its and enough of
/// them), and all ingredients end matched.
pub fn matches(inputs: &[Option<&Stack>], recipe: &[Ingredient]) -> bool {
    let mut left: Vec<&Ingredient> = recipe.iter().collect();
    for input in inputs.iter().flatten() {
        let Some(at) = left
            .iter()
            .position(|g| g.ids.contains(&input.id) && input.count >= g.n)
        else {
            return false;
        };
        left.remove(at);
    }
    left.is_empty()
}

fn recipe_for(model: Model, slots: &[Option<Stack>]) -> Option<&'static Recipe> {
    let at = |range: std::ops::Range<usize>| {
        range
            .map(|i| slots[i].as_ref().filter(|s| s.count > 0))
            .collect::<Vec<_>>()
    };
    recipes(model).iter().find(|r| match model {
        Model::Arc => matches(&at(0..3), &r.arc),
        Model::Solder => {
            matches(&at(0..3), &r.toppings)
                && matches(&at(3..5), &r.pcb)
                && matches(&at(5..6), &r.solder)
        }
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Welder {
    pub model: Model,
    pub slots: Slots,
    pub power: u64,
    pub max_power: u64,
    pub consumption: u64,
    pub progress: u32,
    pub process_time: u32,
    pub tank: Tank,
    /// The soldering station's `collisionPrevention`.
    pub collision_prevention: bool,
}

impl Welder {
    pub fn of(block: &str) -> Option<Self> {
        let model = Model::of(block)?;
        Some(Self {
            model,
            slots: vec![None; model.slots()],
            power: 0,
            max_power: 2_000,
            consumption: 0,
            progress: 0,
            process_time: 1,
            tank: Tank::new(model.tank()),
            collision_prevention: false,
        })
    }

    pub fn recipe(&self) -> Option<&'static Recipe> {
        recipe_for(self.model, &self.slots)
    }

    /// `canProcess`: power, the fluid, room in the output.
    fn can_process(&self, r: &Recipe) -> bool {
        if self.power < self.consumption {
            return false;
        }
        if let Some((f, mb)) = &r.fluid
            && (self.tank.fluid != *f || self.tank.mb < *mb)
        {
            return false;
        }
        if self.model == Model::Solder
            && self.collision_prevention
            && r.fluid.is_none()
            && self.tank.mb > 0
        {
            return false;
        }
        match &self.slots[self.model.output()] {
            Some(o) => o.id == r.out.0 && o.damage == 0 && o.count + r.out.1 <= r.stack,
            None => true,
        }
    }

    /// `consumeItems`: each ingredient takes from the first slot of its group that
    /// matches it and holds enough.
    fn consume(&mut self, r: &Recipe) {
        let groups: Vec<(std::ops::Range<usize>, &Vec<Ingredient>)> = match self.model {
            Model::Arc => vec![(0..3, &r.arc)],
            Model::Solder => vec![(0..3, &r.toppings), (3..5, &r.pcb), (5..6, &r.solder)],
        };
        for (range, list) in groups {
            for g in list {
                for i in range.clone() {
                    let Some(s) = &mut self.slots[i] else {
                        continue;
                    };
                    if g.ids.contains(&s.id) && s.count >= g.n {
                        s.count -= g.n;
                        if s.count == 0 {
                            self.slots[i] = None;
                        }
                        break;
                    }
                }
            }
        }
        if let Some((_, mb)) = &r.fluid {
            self.tank.drain(*mb);
        }
    }

    /// One `updateEntity`; true when it changed anything.
    pub fn step(&mut self) -> bool {
        let before = (
            self.power,
            self.max_power,
            self.consumption,
            self.progress,
            self.process_time,
            self.tank.mb,
            self.tank.fluid.clone(),
        );
        let items_before = (
            self.slots[self.model.output()].clone(),
            self.recipe().map(|r| r.out.0.clone()),
        );
        crate::combustion::type_from_identifier(&mut self.tank, &self.slots, self.model.identifier(), |_| true);
        self.power = charge_from_slot(
            &self.slots,
            self.model.battery(),
            self.power,
            self.max_power,
        );
        let up = &self.slots[self.model.upgrade_slots()];
        let (red, blue, black) = (
            upgrades::level(up, "speed", 3),
            upgrades::level(up, "power", 3),
            upgrades::level(up, "overdrive", 3),
        );
        let intended;
        if let Some(r) = self.recipe() {
            let d = u64::from(r.duration);
            self.process_time = (d - d * u64::from(red) / 6 + d * u64::from(blue) / 3) as u32;
            let c = r.power;
            self.consumption = c + c * u64::from(red) - c * u64::from(blue) / 6;
            self.consumption *= 2u64.pow(black);
            intended = self.consumption * 20;
            if self.can_process(r) {
                self.progress += 1 + black;
                self.power -= self.consumption.min(self.power);
                if self.progress >= self.process_time {
                    self.progress = 0;
                    self.consume(r);
                    match &mut self.slots[self.model.output()] {
                        Some(o) => o.count += r.out.1,
                        o => *o = Some(Stack::new(&r.out.0, r.out.1)),
                    }
                }
            } else {
                self.progress = 0;
            }
        } else {
            self.progress = 0;
            self.consumption = 100;
            intended = 2_000;
        }
        self.max_power = intended.max(self.power);
        before
            != (
                self.power,
                self.max_power,
                self.consumption,
                self.progress,
                self.process_time,
                self.tank.mb,
                self.tank.fluid.clone(),
            )
            || items_before
                != (
                    self.slots[self.model.output()].clone(),
                    self.recipe().map(|r| r.out.0.clone()),
                )
    }

    /// The screen: power, its maximum, progress, process time, the lamp
    /// (power for the next tick), consumption, the tank's fill, capacity and
    /// colour, and `collisionPrevention`.
    pub fn screen_data(&self) -> Vec<i32> {
        let cap = |v: u64| v.min(i32::MAX as u64) as i32;
        vec![
            cap(self.power),
            cap(self.max_power),
            self.progress as i32,
            self.process_time as i32,
            i32::from(self.power >= self.consumption),
            cap(self.consumption),
            self.tank.mb as i32,
            self.tank.cap as i32,
            fluid::get(&self.tank.fluid).map_or(0, |f| f.color as i32),
            i32::from(self.collision_prevention),
        ]
    }
}

impl Slotted for Welder {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Welder::screen_data(self)
    }
    /// The soldering station's button (id 0): `collisionPrevention`.
    fn button(&mut self, id: u32) -> bool {
        if id == 0 && self.model == Model::Solder {
            self.collision_prevention = !self.collision_prevention;
        }
        false
    }
    /// Power, progress and the tank ride with the slots: a crash cannot
    /// leave the inputs consumed without the output, or the fluid spent twice.
    fn save_state(&self) -> Vec<i32> {
        let id = fluid::get(&self.tank.fluid).map_or(-1, |f| i32::from(f.id));
        // The Java keeps these as `long`: the low 31 bits where a save of before kept the capped value,
        // the rest in three words after the others.
        let lo = |v: u64| (v & 0x7fff_ffff) as i32;
        let hi = |v: u64| (v >> 31).min(i32::MAX as u64) as i32;
        vec![
            lo(self.power),
            self.progress as i32,
            self.tank.mb as i32,
            id,
            i32::from(self.collision_prevention),
            lo(self.max_power),
            lo(self.consumption),
            self.process_time as i32,
            hi(self.power),
            hi(self.max_power),
            hi(self.consumption),
        ]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [p, pr, mb, id, cp, mx, c, t, rest @ ..] = v {
            let wide = |lo: i32, hi: Option<&i32>| (lo.max(0) as u64) | ((hi.copied().unwrap_or(0).max(0) as u64) << 31);
            self.power = wide(*p, rest.first());
            self.progress = (*pr).max(0) as u32;
            self.tank.fluid = fluid::types()
                .iter()
                .find(|f| i32::from(f.id) == *id)
                .map_or_else(String::new, |f| f.string_id.clone());
            self.tank.mb = (*mb).clamp(0, self.tank.cap as i32) as u32;
            self.collision_prevention = *cp != 0;
            self.max_power = wide(*mx, rest.get(1));
            self.consumption = wide(*c, rest.get(2));
            self.process_time = (*t).max(1) as u32;
        }
    }
}

impl EnergyNode for Welder {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        self.max_power.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

/// `IFluidStandardReceiverMK2`: the tank takes what a recipe of the machine uses.
impl FluidNode for Welder {
    fn fluid_offer(&self, _f: &str) -> u64 {
        0
    }
    /// `getReceivingTanks`: the tank takes the fluid its identifier typed it to
    /// (an empty tank stays typed; another identifier changes it).
    fn fluid_demand(&self, f: &str) -> u64 {
        if self.tank.fluid == f {
            u64::from(self.tank.free())
        } else {
            0
        }
    }
    fn fluid_types(&self) -> Vec<String> {
        if self.tank.fluid.is_empty() {
            Vec::new()
        } else {
            vec![self.tank.fluid.clone()]
        }
    }
    fn accepts_any(&self) -> bool {
        false
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.tank.fluid == f {
            self.tank.fill(f, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take(&mut self, _f: &str, _mb: u64) {}
}

impl Logic for Welder {
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
        if self.progress > 0 {
            crate::machine::fill_level(
                u64::from(self.progress),
                u64::from(self.process_time.max(1)),
                max,
            )
            .max(1)
        } else {
            0
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new(self.model.title())
            .bar("Energia", self.power, self.max_power, "HE")
            .bar(
                "Progresso",
                u64::from(self.progress),
                u64::from(self.process_time),
                "t",
            )
            .text(self.tank.line(), view::GREY)
    }
}

/// The ids a group of any recipe takes (`SolderingRecipes.toppings/pcb/solder`).
fn group_ids(group: u8) -> &'static std::collections::HashSet<String> {
    static T: OnceLock<[std::collections::HashSet<String>; 3]> = OnceLock::new();
    let t = T.get_or_init(|| {
        let mut sets: [std::collections::HashSet<String>; 3] = Default::default();
        for r in recipes(Model::Solder) {
            for (set, list) in sets.iter_mut().zip([&r.toppings, &r.pcb, &r.solder]) {
                set.extend(list.iter().flat_map(|g| g.ids.iter().cloned()));
            }
        }
        sets
    });
    &t[usize::from(group)]
}

/// What a hand and a hopper may do with a slot of these machines; `None`
/// for another block. The arc welder's `Slot`s take anything from a hand and
/// its hopper rule is `slot < 3`; the soldering station's inputs are
/// `SlotNonRetarded`: both go through `isItemValidForSlot` (a topping, a pcb
/// or a solder, never the same item in two slots of its group). The upgrade
/// slots are `SlotUpgrade` (the hand only); the output is take-only.
pub fn allows(
    block: &str,
    slots: &[Option<Stack>],
    slot: usize,
    item: &str,
    insert: bool,
    manual: bool,
) -> Option<bool> {
    let model = Model::of(block)?;
    if !insert {
        return Some(manual || slot == model.output());
    }
    if model.upgrade_slots().contains(&slot) {
        return Some(manual && upgrades::is_upgrade(item));
    }
    Some(match model {
        Model::Arc => match slot {
            0..3 => true,
            3 => false,
            _ => manual,
        },
        Model::Solder => {
            let (group, range) = match slot {
                0..3 => (0, 0..3),
                3..5 => (1, 3..5),
                5 => (2, 5..6),
                6 => return Some(false),
                _ => return Some(manual),
            };
            let twin = range.into_iter().any(|i| {
                i != slot
                    && slots
                        .get(i)
                        .and_then(Option::as_ref)
                        .is_some_and(|s| s.id == item)
            });
            !twin && group_ids(group).contains(item)
        }
    })
}

/// The container: the arc welder lists, per multiblock cell (`getAccessibleSlotsFromSide(x,y,z,side)`),
/// one input and the output (the core 1, two cells 0, one 1, two 2; the layer above, none), which the
/// engine's one container per machine cannot split; here every port cell reaches all inputs and the
/// output, and the cells above the core are no ports (`inventory_ports`). The soldering station
/// reaches 0..6 on every side.
pub fn container(block: &str) -> Option<crate::native::Container> {
    let model = Model::of(block)?;
    let (reach, inputs): (Vec<usize>, Vec<usize>) = match model {
        Model::Arc => ((0..4).collect(), (0..3).collect()),
        Model::Solder => ((0..7).collect(), (0..6).collect()),
    };
    Some(crate::native::Container {
        slots: model.slots(),
        faces: std::array::from_fn(|_| reach.clone()),
        inputs,
        outputs: vec![model.output()],
        menu: Some("minecraft:generic_9x3"),
    })
}

pub fn filters(block: &str) -> Vec<crate::native::SlotFilter> {
    use crate::native::SlotFilter;
    let Some(model) = Model::of(block) else {
        return Vec::new();
    };
    let plain = SlotFilter::default();
    let upgrade = SlotFilter {
        accept: Some(upgrades::ITEMS.iter().map(|s| s.to_string()).collect()),
        max: Some(1),
        ..Default::default()
    };
    let output = SlotFilter {
        output: true,
        ..Default::default()
    };
    match model {
        Model::Arc => vec![
            plain.clone(),
            plain.clone(),
            plain.clone(),
            output,
            plain.clone(),
            plain,
            upgrade.clone(),
            upgrade,
        ],
        Model::Solder => {
            let accept = |g: u8| SlotFilter {
                accept: Some(group_ids(g).iter().cloned().collect()),
                ..Default::default()
            };
            let (t, p, s) = (accept(0), accept(1), accept(2));
            vec![
                t.clone(),
                t.clone(),
                t,
                p.clone(),
                p,
                s,
                output,
                plain.clone(),
                plain,
                upgrade.clone(),
                upgrade,
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arc() -> Welder {
        Welder::of(ARC).unwrap()
    }

    fn put(w: &mut Welder, i: usize, id: &str, n: u32) {
        w.slots[i] = Some(Stack::new(id, n));
    }

    /// A fluid identifier of `f` in the machine's identifier slot (`{ntm_fluid:"..."}`).
    fn identify(w: &mut Welder, f: &str) {
        let mut id = Stack::new("ntm:fluid_identifier_multi", 1);
        id.custom = Some(format!("{{ntm_fluid:\"{f}\"}}"));
        w.slots[w.model.identifier()] = Some(id);
    }

    #[test]
    fn the_tables_come_from_the_source() {
        assert_eq!(recipes(Model::Arc).len(), 24);
        assert_eq!(recipes(Model::Solder).len(), 27);
        let r = &recipes(Model::Arc)[0];
        assert_eq!(
            (r.out.0.as_str(), r.out.1, r.duration, r.power),
            ("ntm:motor", 2, 100, 400)
        );
    }

    #[test]
    fn ingredients_match_in_any_order_and_all_must_be_used() {
        let ing = |id: &str, n| Ingredient {
            ids: vec![id.into()],
            n,
        };
        let recipe = vec![ing("a", 2), ing("b", 1)];
        let (a, b, c) = (Stack::new("a", 2), Stack::new("b", 1), Stack::new("c", 1));
        assert!(matches(&[Some(&a), Some(&b)], &recipe));
        assert!(matches(&[Some(&b), None, Some(&a)], &recipe));
        assert!(!matches(&[Some(&a)], &recipe), "falta o b");
        assert!(
            !matches(&[Some(&a), Some(&b), Some(&c)], &recipe),
            "o c não é da receita"
        );
        assert!(
            !matches(&[Some(&Stack::new("a", 1)), Some(&b)], &recipe),
            "poucos a"
        );
    }

    #[test]
    fn it_welds_a_recipe_on_the_power_it_is_given() {
        let mut w = arc();
        // The first recipe: two steel plates and two dense wires into two motors.
        let r0 = &recipes(Model::Arc)[0];
        for (i, g) in r0.arc.iter().enumerate() {
            put(&mut w, i, &g.ids[0], g.n);
        }
        w.step();
        assert_eq!(
            (w.consumption, w.process_time, w.max_power),
            (400, 100, 8_000)
        );
        assert_eq!(w.progress, 0, "sem energia não anda");
        w.give(50_000);
        for _ in 0..100 {
            w.step();
        }
        assert_eq!(w.slots[3], Some(Stack::new("ntm:motor", 2)));
        assert!(w.slots[..3].iter().all(Option::is_none));
        assert_eq!(w.power, 50_000 - 100 * 400);
    }

    #[test]
    fn upgrades_shorten_the_time_and_raise_the_draw() {
        let mut w = arc();
        let r0 = &recipes(Model::Arc)[0];
        for (i, g) in r0.arc.iter().enumerate() {
            put(&mut w, i, &g.ids[0], g.n);
        }
        put(&mut w, 6, "ntm:upgrade_speed_3", 1);
        w.step();
        // 100 - 100·3/6 = 50 ticks, 400 + 400·3 = 1 600 HE a tick.
        assert_eq!((w.process_time, w.consumption), (50, 1_600));
        put(&mut w, 7, "ntm:upgrade_overdrive_1", 1);
        w.step();
        // The overdrive doubles the draw and the step.
        assert_eq!(w.consumption, 3_200);
        w.give(1_000_000);
        w.step();
        assert_eq!(w.progress, 2);
    }

    #[test]
    fn a_fluid_recipe_needs_the_fluid_and_spends_it() {
        let mut w = arc();
        let (i, r) = recipes(Model::Arc)
            .iter()
            .enumerate()
            .find(|(_, r)| r.fluid.is_some())
            .unwrap();
        let _ = i;
        for (k, g) in r.arc.iter().enumerate() {
            put(&mut w, k, &g.ids[0], g.n);
        }
        w.power = r.power * u64::from(r.duration) + 1;
        w.step();
        assert_eq!(w.progress, 0, "sem o fluido");
        let (f, mb) = r.fluid.clone().unwrap();
        identify(&mut w, &f);
        w.step();
        w.fluid_give(&f, u64::from(mb) + 500);
        for _ in 0..r.duration {
            w.step();
        }
        assert_eq!(
            w.slots[3].as_ref().map(|s| s.id.as_str()),
            Some(r.out.0.as_str())
        );
        assert_eq!(w.tank.mb, 500);
    }

    #[test]
    fn the_output_must_take_the_result() {
        let mut w = arc();
        let r0 = &recipes(Model::Arc)[0];
        for (i, g) in r0.arc.iter().enumerate() {
            put(&mut w, i, &g.ids[0], g.n);
        }
        w.give(100_000);
        put(&mut w, 3, "ntm:motor", 63);
        w.step();
        assert_eq!(w.progress, 0);
        put(&mut w, 3, "minecraft:stone", 1);
        w.step();
        assert_eq!(w.progress, 0);
    }

    #[test]
    fn the_soldering_station_reads_its_three_groups_and_the_collision_button() {
        let mut w = Welder::of(SOLDER).unwrap();
        let r = &recipes(Model::Solder)[0];
        for (k, g) in r.toppings.iter().enumerate() {
            put(&mut w, k, &g.ids[0], g.n);
        }
        for (k, g) in r.pcb.iter().enumerate() {
            put(&mut w, 3 + k, &g.ids[0], g.n);
        }
        for (k, g) in r.solder.iter().enumerate() {
            put(&mut w, 5 + k, &g.ids[0], g.n);
        }
        w.give(1_000_000);
        for _ in 0..r.duration {
            w.step();
        }
        assert_eq!(
            w.slots[6].as_ref().map(|s| s.id.as_str()),
            Some(r.out.0.as_str())
        );
        // With the button on, a recipe without fluid refuses a tank with some.
        for (k, g) in r.toppings.iter().enumerate() {
            put(&mut w, k, &g.ids[0], g.n);
        }
        for (k, g) in r.pcb.iter().enumerate() {
            put(&mut w, 3 + k, &g.ids[0], g.n);
        }
        for (k, g) in r.solder.iter().enumerate() {
            put(&mut w, 5 + k, &g.ids[0], g.n);
        }
        w.slots[6] = None;
        w.tank.fluid = "water".into();
        w.tank.mb = 100;
        Slotted::button(&mut w, 0);
        w.progress = 0;
        w.step();
        assert_eq!(w.progress, 0, "colisão evitada");
        Slotted::button(&mut w, 0);
        w.step();
        assert_eq!(w.progress, 1);
    }

    #[test]
    fn the_state_survives_a_save() {
        let mut w = arc();
        w.power = 5_000;
        w.progress = 17;
        w.max_power = 9_000;
        w.consumption = 400;
        w.process_time = 100;
        w.tank = Tank {
            fluid: "oxygen".into(),
            mb: 700,
            cap: 24_000,
        };
        let mut g = arc();
        Slotted::load_state(&mut g, &Slotted::save_state(&w));
        assert_eq!(
            (
                g.power,
                g.progress,
                g.max_power,
                g.consumption,
                g.process_time,
                g.tank.mb
            ),
            (5_000, 17, 9_000, 400, 100, 700)
        );
        assert_eq!(g.tank.fluid, "oxygen");
    }

    #[test]
    fn without_an_identifier_the_tank_takes_no_fluid() {
        let w = arc();
        assert_eq!(w.fluid_demand("oxygen"), 0);
        assert!(!w.accepts_any());
        let mut w = arc();
        w.fluid_give("oxygen", 500);
        assert_eq!(w.tank.mb, 0, "an untyped tank refuses what is pushed in");
    }

    #[test]
    fn the_identifier_types_the_tank_and_another_one_empties_it() {
        for model in [ARC, SOLDER] {
            let mut w = Welder::of(model).unwrap();
            let fluids: Vec<String> = {
                let mut v: Vec<String> = Vec::new();
                for r in recipes(w.model) {
                    if let Some((f, _)) = &r.fluid
                        && !v.contains(f)
                    {
                        v.push(f.clone());
                    }
                }
                v
            };
            let (first, second) = (fluids[0].clone(), fluids[1].clone());
            identify(&mut w, &first);
            w.step();
            assert_eq!(w.tank.fluid, first);
            assert_eq!(w.fluid_demand(&first), u64::from(w.tank.cap));
            assert_eq!(w.fluid_demand(&second), 0);
            w.fluid_give(&first, 1_000);
            assert_eq!(w.tank.mb, 1_000);
            // The tank runs dry and stays typed, as in the Java: only the identifier changes it.
            w.tank.drain(1_000);
            assert_eq!(w.fluid_demand(&second), 0);
            identify(&mut w, &second);
            w.step();
            assert_eq!((w.tank.fluid.as_str(), w.tank.mb), (second.as_str(), 0));
            assert!(w.fluid_demand(&second) > 0);
            // With some left, another identifier takes it away (`FluidTank.setType`).
            w.fluid_give(&second, 300);
            identify(&mut w, &first);
            w.step();
            assert_eq!((w.tank.fluid.as_str(), w.tank.mb), (first.as_str(), 0));
        }
    }

    #[test]
    fn the_output_stops_at_the_products_stack_limit() {
        let mut w = Welder::of(SOLDER).unwrap();
        let (n, r) = recipes(Model::Solder)
            .iter()
            .enumerate()
            .find(|(_, r)| r.out.0 == "ntm:sat_chip")
            .unwrap();
        let _ = n;
        assert_eq!(r.stack, 1, "a satellite chip stacks to 1");
        let fill = |w: &mut Welder| {
            for (k, g) in r.toppings.iter().enumerate() {
                put(w, k, &g.ids[0], g.n);
            }
            for (k, g) in r.pcb.iter().enumerate() {
                put(w, 3 + k, &g.ids[0], g.n);
            }
            for (k, g) in r.solder.iter().enumerate() {
                put(w, 5 + k, &g.ids[0], g.n);
            }
        };
        fill(&mut w);
        w.give(10_000_000);
        for _ in 0..r.duration {
            w.step();
        }
        assert_eq!(w.slots[6].as_ref().map(|s| (s.id.as_str(), s.count)), Some(("ntm:sat_chip", 1)));
        fill(&mut w);
        w.progress = 0;
        w.step();
        assert_eq!(w.progress, 0, "the output holds its one chip: the Java stops");
        for model_stack in recipes(Model::Solder).iter().chain(recipes(Model::Arc)) {
            assert!(model_stack.stack >= 1);
        }
        let singles = recipes(Model::Solder).iter().filter(|r| r.stack == 1).count();
        let sixteens = recipes(Model::Solder).iter().filter(|r| r.stack == 16).count();
        assert_eq!((singles, sixteens), (16, 2), "15 upgrades and the chip, and radius and health");
    }

    #[test]
    fn the_energy_the_java_keeps_as_a_long_survives_a_save() {
        let mut w = Welder::of(ARC).unwrap();
        w.power = 3_200_000_000;
        w.max_power = 32_000_000_000;
        w.consumption = 1_600_000_000;
        let mut g = Welder::of(ARC).unwrap();
        Slotted::load_state(&mut g, &Slotted::save_state(&w));
        assert_eq!((g.power, g.max_power, g.consumption), (3_200_000_000, 32_000_000_000, 1_600_000_000));
        // A save of before (eight values, the energy capped at an i32) still reads.
        let mut g = Welder::of(ARC).unwrap();
        Slotted::load_state(&mut g, &[5_000, 17, 700, -1, 0, 9_000, 400, 100]);
        assert_eq!((g.power, g.max_power, g.consumption), (5_000, 9_000, 400));
    }

    #[test]
    fn a_hand_and_a_hopper_follow_the_slot_rules() {
        let slots = |model: Model| vec![None; model.slots()];
        // The arc welder: any item by hand in an input, `slot < 3` for a hopper, nothing in the output.
        let arc_slots = slots(Model::Arc);
        assert_eq!(allows(ARC, &arc_slots, 0, "minecraft:stone", true, true), Some(true));
        assert_eq!(allows(ARC, &arc_slots, 2, "minecraft:stone", true, false), Some(true));
        assert_eq!(allows(ARC, &arc_slots, 3, "ntm:motor", true, true), Some(false));
        assert_eq!(allows(ARC, &arc_slots, 4, "ntm:battery_creative", true, true), Some(true));
        assert_eq!(allows(ARC, &arc_slots, 4, "ntm:battery_creative", true, false), Some(false));
        assert_eq!(allows(ARC, &arc_slots, 6, "ntm:upgrade_speed_1", true, true), Some(true));
        assert_eq!(allows(ARC, &arc_slots, 6, "ntm:upgrade_speed_1", true, false), Some(false));
        assert_eq!(allows(ARC, &arc_slots, 6, "minecraft:stone", true, true), Some(false));
        // Taking: a hand anywhere, a hopper only from the output.
        assert_eq!(allows(ARC, &arc_slots, 0, "x", false, true), Some(true));
        assert_eq!(allows(ARC, &arc_slots, 0, "x", false, false), Some(false));
        assert_eq!(allows(ARC, &arc_slots, 3, "x", false, false), Some(true));
        assert_eq!(allows("minecraft:chest", &arc_slots, 0, "x", true, true), None);
        // The soldering station: a topping, a pcb or a solder of its recipes, and never the same item twice in a group.
        let r = &recipes(Model::Solder)[0];
        let topping = r.toppings[0].ids[0].as_str();
        let pcb = r.pcb[0].ids[0].as_str();
        let mut st = slots(Model::Solder);
        assert_eq!(allows(SOLDER, &st, 0, "minecraft:stone", true, true), Some(false));
        assert_eq!(allows(SOLDER, &st, 0, topping, true, true), Some(true));
        assert_eq!(allows(SOLDER, &st, 3, topping, true, true), Some(false), "a topping is no pcb");
        assert_eq!(allows(SOLDER, &st, 3, pcb, true, false), Some(true));
        st[0] = Some(Stack::new(topping, 1));
        assert_eq!(allows(SOLDER, &st, 1, topping, true, true), Some(false), "the twin of slot 0");
        assert_eq!(allows(SOLDER, &st, 0, topping, true, true), Some(true), "its own slot is no twin");
        assert_eq!(allows(SOLDER, &st, 6, topping, true, true), Some(false));
    }

    #[test]
    fn a_new_identifier_counts_as_a_change_of_the_step() {
        let mut w = arc();
        w.step();
        assert!(!w.step(), "idle: nothing changes");
        identify(&mut w, "oxygen");
        assert!(w.step(), "the type changed: the saved data and the screen must follow");
        assert!(!w.step());
    }

    #[test]
    fn the_slots_are_the_javas() {
        // `tank.setType(5, slots)` and `setType(8, slots)`; the battery, the output and the upgrades beside them.
        assert_eq!((Model::Arc.identifier(), Model::Arc.battery(), Model::Arc.output()), (5, 4, 3));
        assert_eq!((Model::Solder.identifier(), Model::Solder.battery(), Model::Solder.output()), (8, 7, 6));
        assert_eq!((Model::Arc.upgrade_slots(), Model::Solder.upgrade_slots()), (6..8, 9..11));
        assert_eq!((Model::Arc.slots(), Model::Solder.slots()), (8, 11));
    }

    #[test]
    fn the_containers_reach_every_input_and_only_the_output_comes_out() {
        let arc = container(ARC).unwrap();
        assert_eq!((arc.slots, arc.inputs.clone(), arc.outputs.clone()), (8, vec![0, 1, 2], vec![3]));
        assert!(arc.faces.iter().all(|f| *f == vec![0, 1, 2, 3]));
        let st = container(SOLDER).unwrap();
        assert_eq!((st.slots, st.inputs.clone(), st.outputs.clone()), (11, vec![0, 1, 2, 3, 4, 5], vec![6]));
        assert!(st.faces.iter().all(|f| *f == vec![0, 1, 2, 3, 4, 5, 6]));
        assert!(container("minecraft:chest").is_none());
    }

    #[test]
    fn the_collision_button_leaves_a_fluid_recipe_alone() {
        let mut w = Welder::of(SOLDER).unwrap();
        let r = recipes(Model::Solder).iter().find(|r| r.fluid.is_some()).unwrap();
        for (k, g) in r.toppings.iter().enumerate() {
            put(&mut w, k, &g.ids[0], g.n);
        }
        for (k, g) in r.pcb.iter().enumerate() {
            put(&mut w, 3 + k, &g.ids[0], g.n);
        }
        for (k, g) in r.solder.iter().enumerate() {
            put(&mut w, 5 + k, &g.ids[0], g.n);
        }
        let (f, mb) = r.fluid.clone().unwrap();
        identify(&mut w, &f);
        w.step();
        w.fluid_give(&f, u64::from(mb) + 100);
        w.give(10_000_000);
        Slotted::button(&mut w, 0);
        assert!(w.collision_prevention);
        w.step();
        assert_eq!(w.progress, 1, "the recipe uses the fluid: nothing to avoid");
    }
}

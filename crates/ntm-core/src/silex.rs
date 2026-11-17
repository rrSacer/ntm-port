//! The laser isotope separation chamber (`machine_silex`), from `tileentity/machine/TileEntitySILEX.java`: an item (or the
//! fluid of its tank) is loaded as a fill of up to 16 000 mB, and a free-electron laser (`machine_fel`) of at least the
//! recipe's strength runs it into weighted outputs.
//!
//! 11 slots (lines 46-51): 0 the input, 1 the fluid identifier, 2 and 3 the full and empty containers, 4 the output, 5..10
//! the queue. A tank of 16 000 mB, peroxide at first. A tick (`updateEntity`, lines 69-89): the identifier types the tank
//! (`setType(1, 1)`), a container fills it (`loadTank(2, 3)`), `loadFluid` loads the fill, `process` runs it (or the
//! progress goes back to 0), `dequeue` moves the output to the queue, an empty fill forgets its item, and the laser's mode
//! goes back to NULL: the FEL sets it again each of its ticks (`TileEntityFEL:161-165`).
//!
//! Recipes and their translations come from `data/machines/silex.json` (`tools/content-gen/silex_data.py`). An item is a
//! catalog id; a fluid's icon (`ModItems.fluid_icon` of its id) is `fluid:<name>`.

use crate::{
    fel::Wavelength,
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BLOCK: &str = "ntm:machine_silex";
pub const SLOTS: usize = 11;
pub const INPUT: usize = 0;
pub const IDENTIFIER: usize = 1;
pub const CONTAINER_IN: usize = 2;
pub const CONTAINER_OUT: usize = 3;
pub const OUTPUT: usize = 4;
pub const QUEUE: std::ops::Range<usize> = 5..11;
pub const TANK: u32 = 16_000;
/// `maxFill`.
pub const MAX_FILL: i32 = 16_000;
/// `processTime`.
pub const PROCESS_TIME: i32 = 100;
/// What `recipeIndex` moves by after each output.
pub const PRIME: i32 = 137;
pub const PEROXIDE: &str = "peroxide";
/// `fluidConversion` (lines 129-133): the fluids whose tank loads its own icon, whatever their recipe.
pub const CONVERSION: [&str; 3] = ["uf6", "puf6", "death"];

#[derive(Debug, Deserialize)]
struct Output {
    item: String,
    count: u32,
    weight: i32,
    stack: u32,
}
#[derive(Debug, Deserialize)]
struct Input {
    item: Option<String>,
    ore: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Row {
    input: Input,
    produced: i32,
    consumed: i32,
    laser: u8,
    outputs: Vec<Output>,
}
#[derive(Debug, Deserialize)]
struct Data {
    recipes: Vec<Row>,
    item_translation: Vec<(String, String)>,
    dict_translation: Vec<(String, String)>,
    tiny_waste: Vec<(String, String)>,
}
struct Table {
    rows: Vec<Row>,
    by_item: BTreeMap<String, usize>,
    by_ore: BTreeMap<String, usize>,
    items: BTreeMap<String, String>,
    dicts: BTreeMap<String, String>,
    tiny: BTreeMap<String, String>,
    /// Every key a fill can be of, sorted: what `save_state` keeps by index.
    keys: Vec<String>,
    limits: BTreeMap<String, u32>,
}

fn table() -> &'static Table {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let d: Data = serde_json::from_str(include_str!("../data/machines/silex.json")).expect("data/machines/silex.json");
        let mut t = Table {
            rows: Vec::new(),
            by_item: BTreeMap::new(),
            by_ore: BTreeMap::new(),
            items: d.item_translation.into_iter().collect(),
            dicts: d.dict_translation.into_iter().collect(),
            tiny: d.tiny_waste.into_iter().collect(),
            keys: Vec::new(),
            limits: BTreeMap::new(),
        };
        let mut keys = std::collections::BTreeSet::new();
        for (i, r) in d.recipes.into_iter().enumerate() {
            if let Some(item) = &r.input.item {
                t.by_item.insert(item.clone(), i);
                keys.insert(item.clone());
            }
            if let Some(ore) = &r.input.ore {
                t.by_ore.insert(ore.clone(), i);
                keys.extend(crate::oredict::items(ore).iter().cloned());
            }
            for o in &r.outputs {
                t.limits.insert(o.item.clone(), o.stack);
            }
            t.rows.push(r);
        }
        for (from, _) in t.items.iter().chain(t.tiny.iter()) {
            keys.insert(from.clone());
        }
        for (from, _) in &t.dicts {
            keys.extend(crate::oredict::items(from).iter().cloned());
        }
        keys.extend(CONVERSION.iter().map(|f| format!("fluid:{f}")));
        t.keys = keys.into_iter().collect();
        t
    })
}

/// A recipe as `getOutput` gives it: a tiny waste's has a ninth of the whole one's fluid.
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub produced: i32,
    pub consumed: i32,
    /// `laserStrength`'s ordinal.
    pub laser: u8,
    row: usize,
}

impl Recipe {
    /// The weighted outputs: (item, count, weight).
    pub fn outputs(&self) -> impl Iterator<Item = (&'static str, u32, i32)> {
        table().rows[self.row].outputs.iter().map(|o| (o.item.as_str(), o.count, o.weight))
    }
}

fn row(i: usize) -> Recipe {
    let r = &table().rows[i];
    Recipe { produced: r.produced, consumed: r.consumed, laser: r.laser, row: i }
}

/// `SILEXRecipes.getOutput` (lines 680-716): the translated item's recipe, else that of one of its ore names (translated),
/// else, for a tiny waste, the whole one's with `(fluidProduced / 900) * 100`.
pub fn output(key: &str) -> Option<Recipe> {
    let t = table();
    let comp = t.items.get(key).map_or(key, String::as_str);
    if let Some(&i) = t.by_item.get(comp) {
        return Some(row(i));
    }
    for name in crate::oredict::names(comp) {
        let name = t.dicts.get(name).unwrap_or(name);
        if let Some(&i) = t.by_ore.get(name) {
            return Some(row(i));
        }
    }
    let whole = output(t.tiny.get(comp)?)?;
    Some(Recipe { produced: (whole.produced / 900) * 100, ..whole })
}

/// `getMaxStackSize` of an output (64 for anything else).
pub fn stack_limit(item: &str) -> u32 {
    table().limits.get(item).copied().unwrap_or(64)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Silex {
    pub slots: Slots,
    pub tank: Tank,
    /// `current`: what the fill is of.
    pub current: Option<String>,
    /// `currentFill`.
    pub fill: i32,
    pub progress: i32,
    /// The FEL's wavelength for this tick (`mode`), NULL after it.
    pub mode: Wavelength,
    pub recipe_index: i32,
    pub load_delay: i32,
}

impl Default for Silex {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            tank: Tank::typed(PEROXIDE, TANK),
            current: None,
            fill: 0,
            progress: 0,
            mode: Wavelength::Null,
            recipe_index: 0,
            load_delay: 0,
        }
    }
}

impl Silex {
    /// `loadFluid` (lines 137-217).
    fn load_fluid(&mut self) {
        let direct = format!("fluid:{}", self.tank.fluid);
        let loads = CONVERSION.contains(&self.tank.fluid.as_str()) || output(&direct).is_some();
        if loads {
            if self.fill == 0 {
                self.current = Some(direct.clone());
            }
            if self.current.as_deref() == Some(direct.as_str()) {
                let n = 50.min((MAX_FILL - self.fill).min(self.tank.mb as i32));
                self.fill += n;
                self.tank.mb -= n as u32;
            }
        }
        self.load_delay += 1;
        if self.load_delay > 20 {
            self.load_delay = 0;
        }
        let Some(item) = self.slots[INPUT].as_ref().map(|s| s.id.clone()) else { return };
        if self.load_delay != 0 || self.tank.fluid != PEROXIDE || self.current.as_ref().is_some_and(|c| *c != item) {
            return;
        }
        let Some(recipe) = output(&item) else { return };
        let load = recipe.produced;
        if load <= MAX_FILL - self.fill && load <= self.tank.mb as i32 {
            self.fill += load;
            self.current = Some(item);
            self.tank.mb -= load as u32;
            take_one(&mut self.slots, INPUT);
        }
    }

    /// `process` (lines 219-268): false when nothing runs (the progress goes back to 0).
    fn process(&mut self) -> bool {
        let Some(current) = &self.current else { return false };
        if self.fill <= 0 {
            return false;
        }
        let Some(recipe) = output(current) else { return false };
        if recipe.laser > self.mode as u8 || self.fill < recipe.consumed || self.slots[OUTPUT].is_some() {
            return false;
        }
        // `(int) Math.pow(2, mode - laser + 1) / 2`.
        self.progress += 1 << (self.mode as u8 - recipe.laser);
        if self.progress >= PROCESS_TIME {
            self.fill -= recipe.consumed;
            let total: i32 = recipe.outputs().map(|o| o.2).sum();
            self.recipe_index %= total.max(1);
            let mut weight = 0;
            for (item, count, w) in recipe.outputs() {
                weight += w;
                if self.recipe_index < weight {
                    self.slots[OUTPUT] = Some(Stack::new(item, count));
                    break;
                }
            }
            self.progress = 0;
            self.recipe_index += PRIME;
        }
        true
    }

    /// `dequeue` (lines 273-296): one of the output onto a queue stack of the same item below its limit, else the whole
    /// output into the first empty queue slot.
    fn dequeue(&mut self) {
        let Some(out) = self.slots[OUTPUT].clone() else { return };
        for i in QUEUE {
            if let Some(q) = &mut self.slots[i]
                && q.count < stack_limit(&q.id)
                && q.id == out.id
                && q.damage == out.damage
                && q.custom == out.custom
            {
                q.count += 1;
                take_one(&mut self.slots, OUTPUT);
                return;
            }
        }
        for i in QUEUE {
            if self.slots[i].is_none() {
                self.slots[i] = self.slots[OUTPUT].take();
                return;
            }
        }
    }

    pub fn step(&mut self) -> bool {
        let before = self.clone();
        crate::combustion::type_from_identifier(&mut self.tank, &self.slots, IDENTIFIER, |_| true);
        crate::combustion::load_tank(&mut self.tank, &mut self.slots, CONTAINER_IN, CONTAINER_OUT);
        self.load_fluid();
        if !self.process() {
            self.progress = 0;
        }
        self.dequeue();
        if self.fill <= 0 {
            self.current = None;
        }
        self.mode = Wavelength::Null;
        // The load delay turns every tick: only what it gates counts as work.
        *self != Self { load_delay: self.load_delay, ..before }
    }
}

fn take_one(slots: &mut Slots, i: usize) {
    if let Some(s) = &mut slots[i] {
        s.count -= 1;
        if s.count == 0 {
            slots[i] = None;
        }
    }
}

impl Logic for Silex {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.progress > 0 { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Câmara SILEX")
            .text(format!("{} / {} mB", self.fill, MAX_FILL), view::GREY)
            .text(format!("Progresso: {} / {}", self.progress, PROCESS_TIME), view::GREY)
    }
}

/// `getReceivingTanks`: the tank, of its type.
impl FluidNode for Silex {
    fn fluid_offer(&self, _f: &str) -> u64 {
        0
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if !f.is_empty() && self.tank.fluid == f { u64::from(self.tank.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        if self.tank.fluid.is_empty() { Vec::new() } else { vec![self.tank.fluid.clone()] }
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.tank.fluid == f {
            self.tank.fill(f, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take(&mut self, _f: &str, _mb: u64) {}
}

fn fluid_id(name: &str) -> i32 {
    crate::fluid::get(name).map_or(0, |f| i32::from(f.id))
}

impl Slotted for Silex {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// The fill and its limit, the progress and its length, the laser, then the tank's mB, capacity and color.
    fn screen_data(&self) -> Vec<i32> {
        vec![
            self.fill,
            MAX_FILL,
            self.progress,
            PROCESS_TIME,
            self.mode as i32,
            self.tank.mb as i32,
            self.tank.cap as i32,
            crate::fluid::get(&self.tank.fluid).map_or(0, |f| f.color as i32),
        ]
    }
    /// The fill (its item as an index of the possible ones, -1 for none), the progress, the recipe index, the load
    /// delay, the tank's type (its fluid id) and mB.
    fn save_state(&self) -> Vec<i32> {
        let current = self.current.as_ref().and_then(|c| table().keys.iter().position(|k| k == c)).map_or(-1, |i| i as i32);
        vec![self.fill, current, self.progress, self.recipe_index, self.load_delay, fluid_id(&self.tank.fluid), self.tank.mb as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [fill, current, progress, index, delay, fluid, mb, ..] = v {
            self.fill = (*fill).clamp(0, MAX_FILL);
            self.current = usize::try_from(*current).ok().and_then(|i| table().keys.get(i)).cloned();
            self.progress = (*progress).clamp(0, PROCESS_TIME);
            self.recipe_index = (*index).max(0);
            self.load_delay = (*delay).clamp(0, 20);
            let name = crate::fluid::types().iter().find(|f| i32::from(f.id) == *fluid).map_or(String::new(), |f| f.string_id.clone());
            self.tank = Tank { fluid: if *fluid == 0 { String::new() } else { name }, mb: 0, cap: TANK };
            self.tank.mb = (*mb).clamp(0, TANK as i32) as u32;
        }
    }
}

/// `isItemValidForSlot` and `canExtractItem` (lines 298-315), and the menu (`ContainerSILEX`): a hand puts anything into
/// the input, the identifier and the full container slots and takes from every one; a hopper puts an item with a recipe
/// into the input and takes from the queue.
pub fn allows(slot: usize, item: &str, insert: bool, manual: bool) -> bool {
    if slot >= SLOTS {
        return false;
    }
    match (insert, manual) {
        (true, true) => slot <= CONTAINER_IN,
        (true, false) => slot == INPUT && output(item).is_some(),
        (false, true) => true,
        (false, false) => slot >= QUEUE.start,
    }
}

/// `getAccessibleSlotsFromSide`: every face reaches the input and the queue.
pub fn container() -> crate::native::Container {
    let io: Vec<usize> = std::iter::once(INPUT).chain(QUEUE).collect();
    crate::native::Container {
        slots: SLOTS,
        faces: std::array::from_fn(|_| io.clone()),
        inputs: vec![INPUT],
        outputs: QUEUE.collect(),
        menu: Some("minecraft:generic_9x3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(s: &mut Silex, slot: usize, id: &str, n: u32) {
        s.slots[slot] = Some(Stack::new(id, n));
    }

    fn peroxide(mb: u32) -> Silex {
        let mut s = Silex::default();
        s.tank.mb = mb;
        s
    }

    #[test]
    fn the_table_has_its_recipes_translations_and_tiny_waste() {
        let r = output("ntm:ingot_uranium").expect("ingotUranium");
        assert_eq!((r.produced, r.consumed, r.laser), (900, 100, Wavelength::Visible as u8));
        assert_eq!(r.outputs().collect::<Vec<_>>(), [("ntm:nugget_u235", 1, 1), ("ntm:nugget_u238", 1, 11)]);
        // fluid_icon of UF6 is the ingot (itemTranslation), and the ingot's ore name has the recipe.
        assert_eq!(output("fluid:uf6").map(|r| r.produced), Some(900));
        assert_eq!(output("fluid:puf6").map(|r| r.laser), Some(Wavelength::Visible as u8));
        let tiny = output("ntm:nuclear_waste_long_tiny__uranium235").unwrap();
        let whole = output("ntm:nuclear_waste_long__uranium235").unwrap();
        assert_eq!(tiny.produced, (whole.produced / 900) * 100);
        assert_eq!((tiny.consumed, tiny.laser), (whole.consumed, whole.laser));
        // dustUranium is translated to ingotUranium (dictTranslation).
        assert_eq!(output("ntm:powder_uranium").map(|r| (r.produced, r.laser)), Some((900, Wavelength::Visible as u8)));
        assert!(output("minecraft:stone").is_none());
        assert!(output("fluid:water").is_none());
        assert!(output("fluid:redmud").is_some());
    }

    #[test]
    fn an_item_loads_every_21_ticks_into_the_fill_with_peroxide() {
        let mut s = peroxide(16_000);
        put(&mut s, INPUT, "ntm:ingot_uranium", 3);
        s.step();
        assert_eq!((s.fill, s.tank.mb, s.slots[INPUT].as_ref().unwrap().count), (0, 16_000, 3), "the delay starts at 1");
        for _ in 0..19 {
            s.step();
        }
        assert_eq!(s.fill, 0);
        s.step();
        assert_eq!((s.fill, s.tank.mb, s.slots[INPUT].as_ref().unwrap().count), (900, 15_100, 2));
        assert_eq!(s.current.as_deref(), Some("ntm:ingot_uranium"));
        for _ in 0..21 {
            s.step();
        }
        assert_eq!((s.fill, s.slots[INPUT].as_ref().unwrap().count), (1_800, 1));
    }

    #[test]
    fn only_peroxide_loads_an_item_and_only_onto_its_own_fill() {
        let mut s = Silex::default();
        s.tank = Tank { fluid: "water".into(), mb: 16_000, cap: TANK };
        s.load_delay = 20;
        put(&mut s, INPUT, "ntm:ingot_uranium", 1);
        s.step();
        assert_eq!(s.fill, 0);
        let mut s = peroxide(16_000);
        s.load_delay = 20;
        s.fill = 500;
        s.current = Some("ntm:ingot_plutonium".into());
        put(&mut s, INPUT, "ntm:ingot_uranium", 1);
        s.step();
        assert_eq!((s.fill, s.slots[INPUT].as_ref().map(|x| x.count)), (500, Some(1)));
        // Not more than the room or the peroxide there is.
        let mut s = peroxide(899);
        s.load_delay = 20;
        put(&mut s, INPUT, "ntm:ingot_uranium", 1);
        s.step();
        assert_eq!(s.fill, 0);
        let mut s = peroxide(16_000);
        s.load_delay = 20;
        s.fill = MAX_FILL - 899;
        s.current = Some("ntm:ingot_uranium".into());
        put(&mut s, INPUT, "ntm:ingot_uranium", 1);
        s.step();
        assert_eq!(s.slots[INPUT].as_ref().map(|x| x.count), Some(1));
    }

    #[test]
    fn a_converted_fluid_loads_50_a_tick_as_its_own_icon() {
        let mut s = Silex::default();
        s.tank = Tank { fluid: "uf6".into(), mb: 120, cap: TANK };
        s.step();
        assert_eq!((s.fill, s.tank.mb, s.current.as_deref()), (50, 70, Some("fluid:uf6")));
        s.step();
        s.step();
        assert_eq!((s.fill, s.tank.mb), (120, 0));
        // A fluid with a recipe of its own does the same; one without, nothing.
        let mut s = Silex::default();
        s.tank = Tank { fluid: "redmud".into(), mb: 1_000, cap: TANK };
        s.step();
        assert_eq!((s.fill, s.current.as_deref()), (50, Some("fluid:redmud")));
        let mut s = Silex::default();
        s.tank = Tank { fluid: "water".into(), mb: 1_000, cap: TANK };
        s.step();
        assert_eq!((s.fill, s.tank.mb, s.current), (0, 1_000, None));
        // A fill of another item keeps it: the fluid waits.
        let mut s = Silex::default();
        s.tank = Tank { fluid: "uf6".into(), mb: 1_000, cap: TANK };
        s.fill = 900;
        s.current = Some("ntm:ingot_uranium".into());
        s.step();
        assert_eq!((s.fill, s.tank.mb, s.current.as_deref()), (900, 1_000, Some("ntm:ingot_uranium")));
    }

    #[test]
    fn the_laser_must_be_strong_enough_and_a_stronger_one_doubles_the_speed() {
        let mut s = peroxide(0);
        s.fill = 900;
        s.current = Some("ntm:ingot_uranium".into());
        s.mode = Wavelength::Ir;
        s.step();
        assert_eq!(s.progress, 0, "visible needed");
        for (mode, speed) in [(Wavelength::Visible, 1), (Wavelength::Uv, 2), (Wavelength::Gamma, 4), (Wavelength::Drx, 8)] {
            s.progress = 0;
            s.mode = mode;
            s.step();
            assert_eq!(s.progress, speed, "{mode:?}");
            assert_eq!(s.mode, Wavelength::Null, "the mode is the FEL's for one tick");
        }
        s.progress = 50;
        s.step();
        assert_eq!(s.progress, 0, "no laser, the progress goes");
    }

    #[test]
    fn a_run_takes_the_consumed_fill_and_picks_by_the_prime_stepped_index() {
        let mut s = peroxide(0);
        s.fill = 900;
        s.current = Some("ntm:ingot_uranium".into());
        let mut got = Vec::new();
        for _ in 0..9 {
            s.progress = PROCESS_TIME - 1;
            s.mode = Wavelength::Visible;
            s.step();
            got.push(s.slots[QUEUE.start].take().or_else(|| s.slots[OUTPUT].take()).map(|x| x.id));
        }
        // Weights 1 (u235) and 11 (u238): index 0, 137 % 12 = 5, 142 % 12 = 10, ...
        let mut index = 0i32;
        let expected: Vec<_> = (0..9)
            .map(|_| {
                index %= 12;
                let item = if index < 1 { "ntm:nugget_u235" } else { "ntm:nugget_u238" };
                index += PRIME;
                Some(item.to_string())
            })
            .collect();
        assert_eq!(got, expected);
        assert_eq!(s.fill, 0);
        assert_eq!(s.current, None, "an empty fill forgets its item");
    }

    #[test]
    fn a_full_output_or_too_little_fill_holds_the_run() {
        let mut s = peroxide(0);
        s.fill = 99;
        s.current = Some("ntm:ingot_uranium".into());
        s.mode = Wavelength::Visible;
        s.step();
        assert_eq!(s.progress, 0);
        s.fill = 100;
        for i in QUEUE {
            put(&mut s, i, "minecraft:dirt", 64);
        }
        put(&mut s, OUTPUT, "ntm:nugget_u238", 1);
        s.mode = Wavelength::Visible;
        s.step();
        assert_eq!((s.progress, s.fill), (0, 100));
    }

    #[test]
    fn the_queue_piles_one_a_tick_then_takes_the_whole_output() {
        let mut s = peroxide(0);
        put(&mut s, OUTPUT, "ntm:powder_boron_tiny", 3);
        put(&mut s, QUEUE.start, "ntm:powder_boron_tiny", 62);
        s.step();
        assert_eq!((s.slots[QUEUE.start].as_ref().unwrap().count, s.slots[OUTPUT].as_ref().unwrap().count), (63, 2));
        s.step();
        assert_eq!((s.slots[QUEUE.start].as_ref().unwrap().count, s.slots[OUTPUT].as_ref().unwrap().count), (64, 1));
        s.step();
        assert_eq!(s.slots[OUTPUT], None);
        assert_eq!(s.slots[QUEUE.start + 1], Some(Stack::new("ntm:powder_boron_tiny", 1)));
        assert_eq!(stack_limit("ntm:powder_boron_tiny"), 64);
        // Another item's stack is passed by.
        let mut s = peroxide(0);
        put(&mut s, OUTPUT, "ntm:nugget_u238", 1);
        put(&mut s, QUEUE.start, "ntm:nugget_u235", 3);
        s.step();
        assert_eq!(s.slots[QUEUE.start + 1], Some(Stack::new("ntm:nugget_u238", 1)));
        assert_eq!(s.slots[QUEUE.start], Some(Stack::new("ntm:nugget_u235", 3)));
    }

    #[test]
    fn the_identifier_types_the_tank_and_a_container_fills_it() {
        let mut s = peroxide(400);
        let mut id = Stack::new("ntm:fluid_identifier_multi", 1);
        id.custom = Some("{ntm_fluid:\"uf6\"}".into());
        s.slots[IDENTIFIER] = Some(id);
        s.step();
        assert_eq!(s.tank.fluid, "uf6");
        assert!(s.slots[IDENTIFIER].is_some(), "the identifier stays");
        // A full tank of peroxide empties into it, one a tick, the empty one out.
        let mut s = peroxide(0);
        put(&mut s, CONTAINER_IN, "ntm:fluid_tank_full__peroxide", 2);
        s.step();
        assert_eq!((s.tank.mb, s.slots[CONTAINER_OUT].clone()), (1_000, Some(Stack::new("ntm:fluid_tank_empty", 1))));
    }

    #[test]
    fn the_state_outside_the_slots_round_trips() {
        let mut s = Silex::default();
        s.fill = 1_234;
        s.current = Some("fluid:uf6".into());
        s.progress = 42;
        s.recipe_index = 274;
        s.load_delay = 7;
        s.tank = Tank { fluid: "uf6".into(), mb: 555, cap: TANK };
        let mut back = Silex::default();
        back.load_state(&s.save_state());
        assert_eq!(back, s);
        let mut none = Silex::default();
        none.tank.mb = 10;
        let mut back = Silex::default();
        back.load_state(&none.save_state());
        assert_eq!(back, none);
    }

    #[test]
    fn hands_and_hoppers_reach_their_slots() {
        assert!(allows(INPUT, "minecraft:dirt", true, true));
        assert!(allows(CONTAINER_IN, "minecraft:dirt", true, true));
        assert!(!allows(CONTAINER_OUT, "minecraft:dirt", true, true));
        assert!(!allows(OUTPUT, "minecraft:dirt", true, true));
        assert!(allows(INPUT, "ntm:ingot_uranium", true, false));
        assert!(!allows(INPUT, "minecraft:dirt", true, false));
        assert!(!allows(QUEUE.start, "ntm:ingot_uranium", true, false));
        assert!(allows(QUEUE.end - 1, "x", false, false));
        assert!(!allows(OUTPUT, "x", false, false));
        assert!(allows(OUTPUT, "x", false, true));
        assert_eq!(container().faces[0], vec![0, 5, 6, 7, 8, 9, 10]);
    }
}

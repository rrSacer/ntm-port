//! The shredder (docs/maquinas.md): `ntm-src/src/main/java/com/hbm/
//! tileentity/machine/TileEntityMachineShredder.java`.
//!
//! Its 30 slots as the NTM has them, which are the engine's slots (the
//! machine keeps its items as them; `server::container_bridge`): 0..9
//! inputs, 9..27 outputs, 27 and 28 the blades, 29 a battery (kept, not
//! used: the port has no charge in items). Every 60 ticks with power
//! (5 HE/t, 10 000 HE) and two whole blades, each input slot gives one item
//! to its recipe's output (`processItem`), and each blade with a durability
//! takes 1 damage (`minecraft:damage`, in the item). A blade at full damage
//! stops it.
//!
//! Recipes come from `data/machines/shredder.json` (`ShredderRecipes`:
//! the explicit ones and those made from the OreDict in `registerPost`).

use crate::{
    energy::{EnergyNode, Role},
    machine::{Logic, fill_level},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SLOTS: usize = 30;
pub const INPUTS: std::ops::Range<usize> = 0..9;
pub const OUTPUTS: std::ops::Range<usize> = 9..27;
pub const BLADES: [usize; 2] = [27, 28];
pub const BATTERY: usize = 29;
/// `processingSpeed`, `maxPower` and the HE a tick of work takes.
pub const TICKS: u32 = 60;
pub const MAX_POWER: u64 = 10_000;
pub const HE_PER_TICK: u64 = 5;
/// Items a slot piles up to (`getMaxStackSize` of the NTM's dusts).
pub const STACK: u32 = 64;

/// A blade's durability (`ItemBlades`, from the table; 0: never wears).
pub fn blade_durability(item: &str) -> Option<u32> {
    table().blades.iter().find(|(id, _)| id == item).map(|(_, d)| *d)
}

/// What an item without a recipe becomes (`getShredderResult`: never null,
/// so any item but a blade goes in).
pub const SCRAP: (&str, u32) = ("ntm:scrap", 1);

/// What each input becomes: (output, count).
#[derive(Clone, Debug, Default)]
pub struct Recipes(pub HashMap<String, (String, u32)>);

#[derive(serde::Deserialize)]
struct Table {
    recipes: HashMap<String, (String, u32)>,
    blades: Vec<(String, u32)>,
}

fn table() -> &'static Table {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/shredder.json")).expect("data/machines/shredder.json"))
}

impl Recipes {
    /// The NTM's table (`data/machines/shredder.json`).
    pub fn ntm() -> &'static Recipes {
        static R: std::sync::OnceLock<Recipes> = std::sync::OnceLock::new();
        R.get_or_init(|| Recipes(table().recipes.clone()))
    }

    pub fn result(&self, item: &str) -> Option<(&str, u32)> {
        if blade_durability(item).is_some() {
            return None;
        }
        Some(self.0.get(item).map_or(SCRAP, |(o, n)| (o.as_str(), *n)))
    }
}

/// Whether `item` may go in input slot `slot` (`isItemValidForSlot`).
pub fn accepts(slot: usize, item: &str) -> bool {
    match slot {
        s if INPUTS.contains(&s) => blade_durability(item).is_none(),
        s if BLADES.contains(&s) => blade_durability(item).is_some(),
        _ => false,
    }
}

/// Slots and stacks as the engine has them (`minecraft:damage` too).
pub use crate::server::container_bridge::{Slots, Stack};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shredder {
    pub slots: Slots,
    pub power: u64,
    pub progress: u32,
    #[serde(skip)]
    pub sound_cycle: u8,
    #[serde(skip)]
    pub sound_hit: bool,
}

impl Default for Shredder {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], power: 0, progress: 0, sound_cycle: 0, sound_hit: false }
    }
}

/// A blade's state on the screen (`getGearLeft`/`getGearRight`): 0 none,
/// 1 good, 2 worn past half, 3 broken.
pub fn gear(stack: &Option<Stack>) -> u8 {
    let Some((max, wear)) = stack.as_ref().and_then(|s| blade_durability(&s.id).map(|d| (d, s.damage))) else { return 0 };
    if max == 0 || wear < max / 2 {
        1
    } else if wear != max {
        2
    } else {
        3
    }
}

impl Shredder {
    pub fn gears(&self) -> [u8; 2] {
        BLADES.map(|b| gear(&self.slots[b]))
    }

    /// The screen's values (`horizon_screen`): energy, its maximum,
    /// progress, its maximum, the blades' states.
    pub fn screen_data(&self) -> Vec<i32> {
        let [l, r] = self.gears();
        vec![self.power as i32, MAX_POWER as i32, self.progress as i32, TICKS as i32, i32::from(l), i32::from(r)]
    }

    /// Room in the outputs for one result of `item` (`hasSpace`).
    fn has_space(&self, r: &Recipes, item: &str) -> bool {
        let Some((out, n)) = r.result(item) else { return false };
        self.slots[OUTPUTS].iter().any(|s| match s {
            None => true,
            Some(s) => s.id == out && s.count + n <= STACK,
        })
    }

    /// Both blades whole and an input with room (`canProcess`).
    pub fn can_process(&self, r: &Recipes) -> bool {
        self.gears().iter().all(|g| (1..=2).contains(g)) && self.slots[INPUTS].iter().flatten().any(|s| s.count > 0 && self.has_space(r, &s.id))
    }

    /// One pass of `processItem`: each input slot with room gives one item.
    fn process(&mut self, r: &Recipes) {
        for i in INPUTS {
            let Some(id) = self.slots[i].as_ref().map(|s| s.id.clone()) else { continue };
            if !self.has_space(r, &id) {
                continue;
            }
            let Some((out, n)) = r.result(&id) else { continue };
            // A pile of the same first, else the first empty slot.
            let slot = OUTPUTS
                .clone()
                .find(|&o| self.slots[o].as_ref().is_some_and(|s| s.id == out && s.count + n <= STACK))
                .or_else(|| OUTPUTS.clone().find(|&o| self.slots[o].is_none()));
            let Some(o) = slot else { continue };
            match &mut self.slots[o] {
                Some(s) => s.count += n,
                s => *s = Some(Stack::new(out, n)),
            }
            if let Some(s) = &mut self.slots[i] {
                s.count -= 1;
                if s.count == 0 {
                    self.slots[i] = None;
                }
            }
        }
    }

    /// One tick (`updateEntity`); true when its items changed.
    pub fn step(&mut self, r: &Recipes) -> bool {
        self.sound_hit = false;
        if self.progress == 0 { self.sound_cycle = 0; }
        let mut processed = false;
        if self.power > 0 && self.can_process(r) {
            self.progress += 1;
            self.power = self.power.saturating_sub(HE_PER_TICK);
            if self.progress == TICKS {
                for b in BLADES {
                    if let Some(s) = &mut self.slots[b]
                        && blade_durability(&s.id).is_some_and(|d| d > 0)
                    {
                        s.damage += 1;
                    }
                }
                self.progress = 0;
                self.process(r);
                processed = true;
            }
            self.sound_hit = self.sound_cycle == 0;
            self.sound_cycle = (self.sound_cycle + 1) % 50;
        } else {
            self.progress = 0;
        }
        processed
    }
}

impl EnergyNode for Shredder {
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

impl crate::server::container_bridge::Slotted for Shredder {
    /// Its record keeps its slots (the model's JSON): a restore from the journal has them.
    fn slots_recorded(&self) -> bool {
        true
    }
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Shredder::screen_data(self)
    }
}

impl Logic for Shredder {
    fn tick(&mut self) -> bool {
        let before = (self.power > 0, self.progress == 0);
        let moved = self.step(Recipes::ntm());
        moved || before != (self.power > 0, self.progress == 0)
    }

    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }

    fn level(&self, max: u8) -> u8 {
        if self.progress == 0 { 0 } else { fill_level(u64::from(self.progress), u64::from(TICKS), max) }
    }

    /// No item screen here: the items go in and out through the engine's
    /// slots (the machine screen of `horizon_screen`), not transactions.
    fn view(&self) -> ViewModel {
        let state = |g: u8| ["sem lâmina", "boa", "gasta", "quebrada"][usize::from(g.min(3))];
        let [l, r] = self.gears();
        ViewModel::new("Triturador")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .bar("Progresso", u64::from(self.progress), u64::from(TICKS), "ticks")
            .text(format!("Lâminas: {}, {}", state(l), state(r)), view::GREY)
            .text("Os itens entram e saem pela tela com slots (motor com o patch ntm-visual).", view::GREY)
    }

    /// What it holds, dropped when it is broken (the engine's slots spill
    /// themselves; this is for a game without them).
    fn contents(&self) -> Vec<(String, u32)> {
        self.slots.iter().flatten().map(|s| (s.id.clone(), s.count)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipes() -> Recipes {
        Recipes(
            [("minecraft:iron_ingot", ("ntm:powder_iron", 1)), ("minecraft:iron_ore", ("ntm:powder_iron", 2)), ("ntm:block_steel", ("ntm:powder_steel", 9))]
                .into_iter()
                .map(|(a, (b, n))| (a.to_string(), (b.to_string(), n)))
                .collect(),
        )
    }

    fn with_blades(steel_wear: u32) -> Shredder {
        let mut s = Shredder::default();
        s.slots[27] = Some(Stack { damage: steel_wear, ..Stack::new("ntm:blades_steel", 1) });
        s.slots[28] = Some(Stack::new("ntm:blades_desh", 1));
        s.power = MAX_POWER;
        s
    }

    fn st(id: &str, n: u32) -> Option<Stack> {
        Some(Stack::new(id, n))
    }

    fn damage(s: &Shredder, slot: usize) -> u32 {
        s.slots[slot].as_ref().map_or(0, |s| s.damage)
    }

    fn cycle(s: &mut Shredder, r: &Recipes) {
        for _ in 0..TICKS {
            s.step(r);
        }
    }

    #[test]
    fn the_ntm_table_is_embedded() {
        let r = Recipes::ntm();
        assert!(r.0.len() >= 700, "{} receitas", r.0.len());
        assert_eq!(r.result("minecraft:iron_ingot"), Some(("ntm:powder_iron", 1)));
        assert_eq!(r.result("ntm:ingot_steel"), Some(("ntm:powder_steel", 1)));
        // Explicit (registerDefaults) wins over the OreDict one.
        assert_eq!(r.result("minecraft:diamond_ore"), Some(("ntm:gravel_diamond", 2)));
        let blades: Vec<(&str, u32)> = table().blades.iter().map(|(b, d)| (b.as_str(), *d)).collect();
        assert_eq!(blades, [("ntm:blades_steel", 400), ("ntm:blades_titanium", 500), ("ntm:blades_desh", 0)], "ModItems.java:2759-2761");
        // Every output is an item of the content or vanilla.
        assert!(r.0.values().all(|(o, n)| (o.starts_with("ntm:") || o.starts_with("minecraft:")) && *n > 0));
    }

    #[test]
    fn each_input_slot_gives_one_item_per_cycle() {
        let r = recipes();
        let mut s = with_blades(0);
        s.slots[0] = st("minecraft:iron_ingot", 3);
        s.slots[1] = st("minecraft:iron_ore", 1);
        cycle(&mut s, &r);
        // Ingot ×1 then ore ×2 onto the same pile.
        assert_eq!(s.slots[9], st("ntm:powder_iron", 3));
        assert_eq!(s.slots[0], st("minecraft:iron_ingot", 2));
        assert_eq!(s.slots[1], None);
        assert_eq!((damage(&s, 27), damage(&s, 28)), (1, 0), "aço gasta, desh não");
        assert_eq!(s.screen_data(), vec![(MAX_POWER - 300) as i32, 10_000, 0, 60, 1, 1]);
        assert_eq!(s.power, MAX_POWER - TICKS as u64 * HE_PER_TICK);
    }

    #[test]
    fn no_power_or_blades_no_work() {
        let r = recipes();
        let mut s = with_blades(0);
        s.power = 0;
        s.slots[0] = st("minecraft:iron_ingot", 1);
        cycle(&mut s, &r);
        assert_eq!(s.slots[0], st("minecraft:iron_ingot", 1));
        let mut s = with_blades(0);
        s.slots[28] = None;
        s.slots[0] = st("minecraft:iron_ingot", 1);
        assert!(!s.can_process(&r));
    }

    #[test]
    fn a_broken_blade_stops_it() {
        let r = recipes();
        let mut s = with_blades(399);
        s.slots[0] = st("minecraft:iron_ingot", 5);
        assert_eq!(s.gears(), [2, 1]);
        cycle(&mut s, &r);
        assert_eq!(s.gears(), [3, 1]);
        assert!(!s.can_process(&r));
        assert_eq!(s.slots[0], st("minecraft:iron_ingot", 4));
    }

    #[test]
    fn anything_without_a_recipe_becomes_scrap_but_blades() {
        let r = recipes();
        assert_eq!(r.result("minecraft:dirt"), Some(SCRAP));
        assert_eq!(r.result("ntm:blades_steel"), None);
        assert!(accepts(0, "minecraft:dirt") && !accepts(0, "ntm:blades_desh"));
        assert!(accepts(27, "ntm:blades_titanium") && !accepts(28, "minecraft:dirt"));
        let mut s = with_blades(0);
        s.slots[0] = st("minecraft:dirt", 1);
        cycle(&mut s, &r);
        assert_eq!(s.slots[9], st("ntm:scrap", 1));
    }

    #[test]
    fn full_outputs_stop_it() {
        let r = recipes();
        let mut s = with_blades(0);
        for o in OUTPUTS {
            s.slots[o] = st("ntm:scrap", 64);
        }
        s.slots[0] = st("minecraft:iron_ingot", 1);
        assert!(!s.can_process(&r));
        // A pile of the same with room is enough.
        s.slots[20] = st("ntm:powder_iron", 63);
        assert!(s.can_process(&r));
        cycle(&mut s, &r);
        assert_eq!(s.slots[20], st("ntm:powder_iron", 64));
        // A block (×9) needs a pile with room for 9.
        s.slots[1] = st("ntm:block_steel", 1);
        assert!(!s.can_process(&r));
    }
}

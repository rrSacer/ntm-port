//! The funnel (`machine_funnel`), from `tileentity/machine/TileEntityMachineFunnel.java`: 18 slots, the top nine
//! take items and the bottom nine get them back compressed. Every tick each of the nine columns turns 9 of its
//! item into the result of a full 3x3 crafting grid of it, or else 4 into that of a full 2x2 grid
//! (`updateEntity`, lines 38-64). The recipes are `CraftingManager`'s (`getFrom9`, `getFrom4`, lines 120-142);
//! the port reads them from `data/machines/funnel.json` (`tools/content-gen/funnel_compression.py`).
//!
//! The button of its screen (`receiveControl`, line 166) cycles the mode: 3x3 then 2x2, 3x3 only, 2x2 only.

use crate::{
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BLOCK: &str = "ntm:machine_funnel";
pub const INPUTS: usize = 9;
pub const SLOTS: usize = 18;

/// `MODE_ALL`, `MODE_3x3`, `MODE_2x2` (lines 28-30).
pub const MODE_ALL: i32 = 0;
pub const MODE_3X3: i32 = 1;
pub const MODE_2X2: i32 = 2;

#[derive(Clone, Debug, Deserialize)]
pub struct Entry {
    pub output: (String, u32),
    /// The result's `getMaxStackSize`.
    pub stack: u32,
}

#[derive(Debug, Deserialize)]
pub struct Table {
    pub from9: BTreeMap<String, Entry>,
    pub from4: BTreeMap<String, Entry>,
}

/// The compression recipes (`data/machines/funnel.json`).
pub fn table() -> &'static Table {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/funnel.json")).expect("data/machines/funnel.json"))
}

/// `getFrom9`: the result of a full 3x3 grid of `item`.
pub fn from9(item: &str) -> Option<&'static Entry> {
    table().from9.get(item)
}

/// `getFrom4`: the result of a full 2x2 grid of `item`.
pub fn from4(item: &str) -> Option<&'static Entry> {
    table().from4.get(item)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Funnel {
    pub slots: Slots,
    pub mode: i32,
}

impl Default for Funnel {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], mode: MODE_ALL }
    }
}

impl Funnel {
    /// One `updateEntity`; true when an item moved.
    pub fn step(&mut self) -> bool {
        let mut moved = false;
        for i in 0..INPUTS {
            let Some(held) = &self.slots[i] else { continue };
            let count = held.count;
            let mut size = 9;
            let mut found = if self.mode == MODE_2X2 || count < 9 { None } else { from9(&held.id) };
            if found.is_none() {
                found = if self.mode == MODE_3X3 || count < 4 { None } else { from4(&held.id) };
                size = 4;
            }
            let Some(found) = found else { continue };
            if count < size {
                continue;
            }
            let (id, n) = (&found.output.0, found.output.1);
            let room = match &self.slots[i + INPUTS] {
                None => true,
                Some(o) => o.id == *id && o.damage == 0 && o.count + n <= found.stack,
            };
            if !room {
                continue;
            }
            match &mut self.slots[i + INPUTS] {
                Some(o) => o.count += n,
                slot => *slot = Some(Stack::new(id, n)),
            }
            let input = self.slots[i].as_mut().expect("checked");
            input.count -= size;
            if input.count == 0 {
                self.slots[i] = None;
            }
            moved = true;
        }
        moved
    }
}

impl Logic for Funnel {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Funil")
            .text("Em cima: o que comprimir. Embaixo: o resultado.", view::GREY)
            .text(mode_label(self.mode), view::GREY)
    }
}

/// The tooltip of the mode button (`GUIFunnel.drawScreen`, line 33).
pub fn mode_label(mode: i32) -> &'static str {
    match mode {
        MODE_3X3 => "Mode: 3x3 only",
        MODE_2X2 => "Mode: 2x2 only",
        _ => "Mode: 3x3 then 2x2",
    }
}

impl Slotted for Funnel {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![self.mode]
    }
    /// `receiveControl`: `mode++`, back to 0 after 2.
    fn button(&mut self, _id: u32) -> bool {
        self.mode = if self.mode + 1 > MODE_2X2 { MODE_ALL } else { self.mode + 1 };
        false
    }
    fn save_state(&self) -> Vec<i32> {
        vec![self.mode]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [mode, ..] = v {
            self.mode = (*mode).clamp(MODE_ALL, MODE_2X2);
        }
    }
}

/// `isItemValidForSlot` and `canExtractItem` for what a hand and a hopper may do. A hand (the menu's `Slot`s) puts
/// into the top nine and takes from all; the bottom nine are `SlotCraftingOutput`s that take nothing in. A hopper
/// puts into the top nine only, and an item that no recipe takes only into an occupied slot (lines 85-90).
pub fn allows(slots: &[Option<Stack>], slot: usize, item: &str, insert: bool, manual: bool) -> bool {
    if slot >= SLOTS {
        return false;
    }
    if !insert {
        return manual || slot >= INPUTS;
    }
    slot < INPUTS && (manual || slots.get(slot).is_some_and(Option::is_some) || from9(item).is_some() || from4(item).is_some())
}

/// `getAccessibleSlotsFromSide`: the bottom reaches the results, every other face the inputs.
pub fn container() -> crate::native::Container {
    let top: Vec<usize> = (0..INPUTS).collect();
    let bottom: Vec<usize> = (INPUTS..SLOTS).collect();
    crate::native::Container {
        slots: SLOTS,
        faces: [bottom.clone(), top.clone(), top.clone(), top.clone(), top.clone(), top.clone()],
        inputs: top,
        outputs: bottom,
        menu: Some("minecraft:generic_9x3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(f: &mut Funnel, slot: usize, id: &str, n: u32) {
        f.slots[slot] = Some(Stack::new(id, n));
    }
    fn at(f: &Funnel, slot: usize) -> Option<(&str, u32)> {
        f.slots[slot].as_ref().map(|s| (s.id.as_str(), s.count))
    }

    #[test]
    fn nine_become_a_block_and_the_rest_stays() {
        let mut f = Funnel::default();
        put(&mut f, 0, "minecraft:iron_ingot", 20);
        assert!(f.step());
        assert_eq!((at(&f, 0), at(&f, 9)), (Some(("minecraft:iron_ingot", 11)), Some(("minecraft:iron_block", 1))));
        f.step();
        assert_eq!((at(&f, 0), at(&f, 9)), (Some(("minecraft:iron_ingot", 2)), Some(("minecraft:iron_block", 2))));
        assert!(!f.step(), "two ingots are too few");
        put(&mut f, 0, "minecraft:iron_ingot", 9);
        f.step();
        assert_eq!(at(&f, 0), None, "an emptied slot is empty, not a stack of zero");
    }

    #[test]
    fn four_become_the_2x2_result_when_there_are_not_nine() {
        let mut f = Funnel::default();
        put(&mut f, 3, "minecraft:glowstone_dust", 8);
        f.step();
        assert_eq!((at(&f, 3), at(&f, 12)), (Some(("minecraft:glowstone_dust", 4)), Some(("minecraft:glowstone", 1))));
        // The recipe's own count is taken whole: 4 stone make 4 stone bricks.
        put(&mut f, 4, "minecraft:stone", 4);
        f.step();
        assert_eq!((at(&f, 4), at(&f, 13)), (None, Some(("minecraft:stone_bricks", 4))));
    }

    /// An item with both a 3x3 and a 2x2 recipe (the catalog has some, e.g. an ingot of a block and a plate).
    fn both() -> &'static str {
        table().from9.keys().find(|k| table().from4.contains_key(*k)).expect("an item with both recipes")
    }

    #[test]
    fn nine_prefers_the_3x3_and_the_modes_limit_it() {
        let both = both();
        let (big, small) = (from9(both).unwrap(), from4(both).unwrap());
        // Nine or more: the 3x3 first (the 9 items go, line 44), unless the mode is 2x2 only.
        for (mode, out, taken) in [(MODE_ALL, &big.output.0, 9), (MODE_3X3, &big.output.0, 9), (MODE_2X2, &small.output.0, 4)] {
            let mut f = Funnel { mode, ..Default::default() };
            put(&mut f, 0, both, 9);
            f.step();
            assert_eq!(9 - at(&f, 0).map_or(0, |s| s.1), taken, "mode {mode}");
            assert_eq!(at(&f, 9).map(|s| s.0), Some(out.as_str()), "mode {mode}");
        }
        // Four to eight: the 2x2, unless the mode is 3x3 only.
        for (mode, taken) in [(MODE_ALL, 4), (MODE_2X2, 4), (MODE_3X3, 0)] {
            let mut f = Funnel { mode, ..Default::default() };
            put(&mut f, 0, both, 8);
            f.step();
            assert_eq!(8 - at(&f, 0).map_or(0, |s| s.1), taken, "mode {mode}");
        }
        // An item with a 2x2 only is used from four on, even in a stack of nine.
        let mut f = Funnel::default();
        put(&mut f, 0, "minecraft:clay_ball", 9);
        f.step();
        assert_eq!(at(&f, 0), Some(("minecraft:clay_ball", 5)));
    }

    #[test]
    fn a_blocked_output_holds_the_compression_without_falling_back() {
        // A different item in the result slot, or a full one, stops it (lines 51-58).
        let mut f = Funnel::default();
        put(&mut f, 0, "minecraft:iron_ingot", 9);
        put(&mut f, 9, "minecraft:dirt", 1);
        assert!(!f.step());
        assert_eq!(at(&f, 0), Some(("minecraft:iron_ingot", 9)));
        put(&mut f, 9, "minecraft:iron_block", 64);
        assert!(!f.step(), "the result slot is full");
        put(&mut f, 9, "minecraft:iron_block", 63);
        assert!(f.step());
        assert_eq!(at(&f, 9), Some(("minecraft:iron_block", 64)));
        // With the 3x3 result blocked, a stack of nine does not try the 2x2 recipe of the same item (line 44 chose it).
        let both = both();
        let mut f = Funnel::default();
        put(&mut f, 0, both, 9);
        put(&mut f, 9, "minecraft:dirt", 1);
        assert!(!f.step());
        assert_eq!(at(&f, 0), Some((both, 9)));
    }

    #[test]
    fn the_results_own_stack_limit_applies() {
        let mut f = Funnel::default();
        let (id, entry) = table().from9.iter().find(|(_, e)| e.stack < 64).expect("a result that stacks to less");
        put(&mut f, 0, id, 9);
        put(&mut f, 9, &entry.output.0, entry.stack);
        assert!(!f.step());
    }

    #[test]
    fn the_columns_work_together_and_alone() {
        let mut f = Funnel::default();
        for i in 0..INPUTS {
            put(&mut f, i, "minecraft:coal", 9);
        }
        assert!(f.step());
        for i in 0..INPUTS {
            assert_eq!((at(&f, i), at(&f, i + INPUTS)), (None, Some(("minecraft:coal_block", 1))));
        }
    }

    #[test]
    fn the_button_cycles_the_three_modes() {
        let mut f = Funnel::default();
        let seen: Vec<i32> = (0..4).map(|_| { f.button(0); f.mode }).collect();
        assert_eq!(seen, [1, 2, 0, 1]);
        let mut g = Funnel::default();
        g.load_state(&[2]);
        assert_eq!((g.mode, g.save_state()), (2, vec![2]));
        g.load_state(&[9]);
        assert_eq!(g.mode, MODE_2X2, "a mode out of range is held to the last");
    }

    #[test]
    fn what_a_hand_and_a_hopper_may_do() {
        let slots: Slots = vec![None; SLOTS];
        // A hand puts anything in the top nine, nothing in the results, and takes from every slot.
        assert!(allows(&slots, 0, "minecraft:dirt", true, true));
        assert!(!allows(&slots, 9, "minecraft:iron_ingot", true, true));
        assert!(allows(&slots, 9, "x", false, true) && allows(&slots, 0, "x", false, true));
        // A hopper puts only what a recipe takes into an empty slot, anything into an occupied one.
        assert!(allows(&slots, 0, "minecraft:iron_ingot", true, false));
        assert!(!allows(&slots, 0, "minecraft:dirt", true, false));
        let mut held = slots.clone();
        held[0] = Some(Stack::new("minecraft:dirt", 1));
        assert!(allows(&held, 0, "minecraft:dirt", true, false));
        assert!(!allows(&held, 9, "minecraft:iron_ingot", true, false));
        // A hopper takes from the results only.
        assert!(allows(&slots, 9, "x", false, false) && !allows(&slots, 0, "x", false, false));
    }

    #[test]
    fn the_table_has_the_vanilla_and_the_catalog_recipes() {
        assert_eq!(from9("minecraft:gold_ingot").map(|e| e.output.0.as_str()), Some("minecraft:gold_block"));
        assert_eq!(from9("minecraft:gold_nugget").map(|e| e.output.0.as_str()), Some("minecraft:gold_ingot"));
        assert_eq!(from4("minecraft:oak_planks").map(|e| e.output.0.as_str()), Some("minecraft:crafting_table"));
        assert!(from9("minecraft:coal").is_some() && from9("minecraft:charcoal").is_none(), "1.7.10's coal block takes coal, not charcoal");
        assert!(from9("ntm:ingot_uranium").is_some());
        assert!(from9("minecraft:dirt").is_none() && from4("minecraft:dirt").is_none());
    }
}

//! The locksmith table (`machine_keyforge`), from
//! `tileentity/machine/TileEntityMachineKeyForge.java`: three slots. A key or
//! lock in the first slot gives its pin configuration to the one in the
//! second, and the one in the third gets a new random one every tick
//! (`updateEntity`, lines 187-201). The pins are the `pins` int of the item's
//! NBT (`ItemKeyPin`); here the `pins` int of its `custom_data`. `key_fake`
//! can neither be changed nor copied (`canTransfer`).

use crate::{
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    snbt,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_keyforge";
pub const SLOTS: usize = 3;
pub const SOURCE: usize = 0;
pub const TARGET: usize = 1;
pub const RANDOM: usize = 2;

/// The `ItemKeyPin`s (`ItemKey` and `ItemLock`): whether `canTransfer`.
pub const PIN_ITEMS: [(&str, bool); 6] = [
    ("ntm:key", true),
    ("ntm:key_fake", false),
    ("ntm:padlock_rusty", true),
    ("ntm:padlock", true),
    ("ntm:padlock_reinforced", true),
    ("ntm:padlock_unbreakable", true),
];

/// An `ItemKeyPin`.
pub fn is_pin_item(id: &str) -> bool {
    PIN_ITEMS.iter().any(|(i, _)| *i == id)
}

/// `ItemKeyPin.canTransfer`: every one but `key_fake`.
pub fn can_transfer(id: &str) -> bool {
    PIN_ITEMS.iter().any(|(i, t)| *i == id && *t)
}

/// `ItemKeyPin.getPins`: the `pins` int, 0 when there is none.
pub fn pins(stack: &Stack) -> i32 {
    stack
        .custom
        .as_deref()
        .and_then(snbt::fields)
        .and_then(|f| f.get("pins").cloned())
        .and_then(|v| v.trim_end_matches(['i', 'I']).parse().ok())
        .unwrap_or(0)
}

/// `ItemKeyPin.setPins`: the other data of the item stays.
pub fn set_pins(stack: &mut Stack, pins: i32) {
    stack.custom = Some(snbt::with_field(stack.custom.as_deref(), "pins", &pins.to_string()));
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyForge {
    pub slots: Slots,
    /// The state of the pin generator (`worldObj.rand`).
    #[serde(default = "seed")]
    pub rng: u64,
}

fn seed() -> u64 {
    0x9e37_79b9_7f4a_7c15
}

impl Default for KeyForge {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], rng: seed() }
    }
}

impl KeyForge {
    /// `rand.nextInt(900) + 100`: 100 to 999.
    fn random_pins(&mut self) -> i32 {
        // xorshift64*: the port has no world random in the machines' step, and nothing else depends on this one.
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        100 + (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as i32 % 900
    }

    fn transferable(&self, slot: usize) -> bool {
        self.slots[slot].as_ref().is_some_and(|s| can_transfer(&s.id))
    }

    /// One `updateEntity`; true when an item changed.
    pub fn step(&mut self) -> bool {
        let mut changed = false;
        if self.transferable(SOURCE) && self.transferable(TARGET) {
            let copy = pins(self.slots[SOURCE].as_ref().expect("checked"));
            let target = self.slots[TARGET].as_mut().expect("checked");
            if pins(target) != copy {
                set_pins(target, copy);
                changed = true;
            }
        }
        if self.transferable(RANDOM) {
            let random = self.random_pins();
            set_pins(self.slots[RANDOM].as_mut().expect("checked"), random);
            changed = true;
        }
        changed
    }
}

impl Logic for KeyForge {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Mesa do chaveiro")
            .text("1: copia os pinos da chave ou do cadeado", view::GREY)
            .text("2: recebe os pinos do primeiro", view::GREY)
            .text("3: recebe pinos aleatórios", view::GREY)
    }
}

impl Slotted for KeyForge {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Vec::new()
    }
}

/// What a hand and a hopper may do with a slot: `isItemValidForSlot` is false for every slot (a hopper puts nothing in),
/// the `Slot`s of its container take anything from a hand, and `canExtractItem` is true.
pub fn allows(slot: usize, insert: bool, manual: bool) -> bool {
    slot < SLOTS && (!insert || manual)
}

/// `getAccessibleSlotsFromSide`: the top reaches the first slot, the bottom the second, the sides the third.
pub fn container() -> crate::native::Container {
    crate::native::Container {
        slots: SLOTS,
        faces: [vec![TARGET], vec![SOURCE], vec![RANDOM], vec![RANDOM], vec![RANDOM], vec![RANDOM]],
        inputs: Vec::new(),
        outputs: vec![SOURCE, TARGET, RANDOM],
        menu: Some("minecraft:generic_9x3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str, pins_: Option<i32>) -> Option<Stack> {
        let mut s = Stack::new(id, 1);
        if let Some(p) = pins_ {
            set_pins(&mut s, p);
        }
        Some(s)
    }

    #[test]
    fn the_pins_are_the_int_in_the_custom_data() {
        let mut s = Stack::new("ntm:key", 1);
        assert_eq!(pins(&s), 0);
        set_pins(&mut s, 482);
        assert_eq!((pins(&s), s.custom.as_deref()), (482, Some("{pins:482}")));
        // Other data of the item stays when the pins change.
        s.custom = Some("{note:\"a\",pins:7}".into());
        set_pins(&mut s, 900);
        assert_eq!(pins(&s), 900);
        assert!(s.custom.as_deref().unwrap().contains("note:\"a\""));
    }

    #[test]
    fn the_first_slot_gives_its_pins_to_the_second() {
        let mut f = KeyForge::default();
        f.slots[SOURCE] = key("ntm:key", Some(321));
        f.slots[TARGET] = key("ntm:padlock", None);
        assert!(f.step());
        assert_eq!(pins(f.slots[TARGET].as_ref().unwrap()), 321);
        assert!(!f.step(), "nothing left to copy");
        assert_eq!(pins(f.slots[SOURCE].as_ref().unwrap()), 321, "the source is untouched");
    }

    #[test]
    fn a_fake_key_is_neither_copied_nor_changed() {
        for (a, b, c) in [("ntm:key_fake", "ntm:key", "ntm:key"), ("ntm:key", "ntm:key_fake", "ntm:key_fake")] {
            let mut f = KeyForge::default();
            f.slots[SOURCE] = key(a, Some(111));
            f.slots[TARGET] = key(b, Some(222));
            f.slots[RANDOM] = key(c, Some(333));
            f.step();
            assert_eq!(pins(f.slots[TARGET].as_ref().unwrap()), 222, "{a} to {b}");
            let third = pins(f.slots[RANDOM].as_ref().unwrap());
            assert_eq!(third == 333, c == "ntm:key_fake", "the third slot's {c}");
        }
    }

    #[test]
    fn the_third_slot_is_set_to_a_random_configuration_of_three_digits_every_tick() {
        let mut f = KeyForge::default();
        f.slots[RANDOM] = key("ntm:padlock_reinforced", None);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..200 {
            assert!(f.step());
            let p = pins(f.slots[RANDOM].as_ref().unwrap());
            assert!((100..=999).contains(&p), "{p}");
            seen.insert(p);
        }
        assert!(seen.len() > 100, "{} different ones in 200 ticks", seen.len());
    }

    #[test]
    fn what_is_no_key_or_lock_is_left_alone() {
        let mut f = KeyForge::default();
        f.slots[SOURCE] = key("minecraft:stick", None);
        f.slots[TARGET] = key("ntm:key", None);
        f.slots[RANDOM] = key("minecraft:stick", None);
        assert!(!f.step());
        assert_eq!((pins(f.slots[TARGET].as_ref().unwrap()), f.slots[RANDOM].as_ref().unwrap().custom.clone()), (0, None));
        assert!(!f.transferable(SOURCE) && f.transferable(TARGET));
    }

    #[test]
    fn the_hand_puts_anything_and_the_hopper_nothing() {
        for slot in 0..3 {
            assert!(allows(slot, true, true) && !allows(slot, true, false), "{slot}");
            assert!(allows(slot, false, true) && allows(slot, false, false), "{slot}");
        }
        assert!(!allows(3, true, true));
        let c = container();
        assert_eq!((c.slots, c.inputs.len(), c.outputs.clone()), (3, 0, vec![0, 1, 2]));
        assert!(c.faces.iter().all(|f| f.len() == 1));
        assert_eq!((c.faces[0].clone(), c.faces[1].clone(), c.faces[2].clone()), (vec![1], vec![0], vec![2]));
        assert!(PIN_ITEMS.iter().filter(|(_, t)| !*t).count() == 1 && is_pin_item("ntm:key_fake"));
    }
}

//! The filing cabinet (`ntm:filing_cabinet`), from `tileentity/machine/storage/TileEntityFileCabinet.java`,
//! `ContainerFileCabinet.java` and `BlockDecoContainer.java`: eight slots in two rows of four, which a hand fills and
//! empties and a hopper or an inserter cannot touch (`isItemValidForSlot`, `canInsertItem` and `canExtractItem` are all
//! false: "No automation, it's a filing cabinet"). A shift-click moves a stack between the cabinet and the inventory
//! (`transferStackInSlot`), and a broken cabinet spills every slot (`BlockDecoContainer.breakBlock`: the engine does it).
//!
//! Left out, as declared in `docs/content-cabinet.md`: the lock (`TileEntityLockableBase`: a padlock or a key kit on
//! the block, which waits for the lock slice to expose it), the drawers' animation (`lowerExtent` and `upperExtent`,
//! the `RenderFileCabinet` model that opens with the players using it), its sounds (`block.crateOpen` and
//! `block.crateClose`) and the custom name an anvil gives a crate.

use crate::{
    machine::Logic,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:filing_cabinet";
/// `super(8)`: two rows of four (`ContainerFileCabinet`, slot `j + i * 4`).
pub const SLOTS: usize = 8;

/// What a hand and a hopper may do with a slot: a hand puts in and takes out of any (`Slot`), a hopper does neither
/// (`isItemValidForSlot`, `canInsertItem` and `canExtractItem` are false).
pub fn allows(slot: usize, manual: bool) -> bool {
    slot < SLOTS && manual
}

/// `getAccessibleSlotsFromSide` reaches every slot from every side, and then `canInsertItem` and `canExtractItem` say no:
/// no face is open here, and a hopper finds nothing to take or to put.
pub fn container() -> crate::native::Container {
    crate::native::Container {
        slots: SLOTS,
        faces: Default::default(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        menu: Some("minecraft:generic_9x3"),
    }
}

/// The eight slots. Its record has none (they are the block entity's alone: an item keeps every component the model
/// does not read, a name, an enchantment, whatever its data is).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cabinet {
    #[serde(skip, default = "empty")]
    pub slots: Slots,
}

fn empty() -> Slots {
    vec![None; SLOTS]
}

impl Default for Cabinet {
    fn default() -> Self {
        Self { slots: empty() }
    }
}

impl Logic for Cabinet {
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Arquivo").text("Oito lugares; só uma mão põe e tira (nada de funil nem inserter)", view::GREY)
    }
}

impl Slotted for Cabinet {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::Stack;

    #[test]
    fn it_has_eight_slots_and_only_a_hand_reaches_them() {
        let c = Cabinet::default();
        assert_eq!(c.slots.len(), 8);
        for slot in 0..SLOTS {
            assert!(allows(slot, true), "a hand, slot {slot}");
            assert!(!allows(slot, false), "a hopper, slot {slot}");
        }
        assert!(!allows(SLOTS, true), "no ninth slot");
    }

    #[test]
    fn no_face_of_it_is_open_to_a_hopper() {
        let c = container();
        assert_eq!(c.slots, SLOTS);
        assert!(c.faces.iter().all(Vec::is_empty) && c.inputs.is_empty() && c.outputs.is_empty());
    }

    /// Its record keeps no slots: a model the journal restored takes them from the block entity, whole.
    #[test]
    fn its_record_keeps_no_slots() {
        let mut c = Cabinet::default();
        c.slots[3] = Some(Stack::new("minecraft:diamond", 7));
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("diamond"), "{json}");
        let back: Cabinet = serde_json::from_str(&json).unwrap();
        assert_eq!(back.slots, vec![None; SLOTS]);
        assert!(!Slotted::slots_recorded(&c));
    }
}

//! The ashpit (`TileEntityAshpit.java`, Onda 1 slice 6): five slots and a level of each kind of ash
//! (`ashLevelWood/Coal/Misc/Fly/Soot`), which the firebox and the heating oven over it fill
//! (`TileEntityFireboxBase.java:85-91`). Every tick, one ash of each kind whose level reached its threshold
//! (`updateEntity:62-66`); all the slots come out from every side and nothing goes in
//! (`getAccessibleSlotsFromSide`, `canExtractItem`; `isItemValidForSlot` is false, `SlotTakeOnly`).
//!
//! The Java's `processAsh` (`:82-97`) is followed as it is: a new stack in an empty slot takes the threshold off
//! the wood's level whatever its kind, and the caller then takes it off the kind's own (the wood's level can go
//! below zero); an empty slot comes before a stack of the same kind further on. The chimneys' fly ash and soot
//! need the smoke the port's machines don't give (decision D1): their levels are kept, nothing fills them.

use crate::{
    fuel::Ash,
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const SLOTS: usize = 5;
/// `thresholdWood/Coal/Misc/Fly` (2000) and `thresholdSoot` (8000).
pub const THRESHOLD: i32 = 2000;
pub const SOOT_THRESHOLD: i32 = 8000;
/// `powder_ash`'s stack.
const STACK: u32 = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ashpit {
    pub slots: Slots,
    pub wood: i32,
    pub coal: i32,
    pub misc: i32,
    pub fly: i32,
    pub soot: i32,
}

impl Default for Ashpit {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], wood: 0, coal: 0, misc: 0, fly: 0, soot: 0 }
    }
}

impl Ashpit {
    /// What a firebox over it adds when it lights a fuel: the fuel's base time, as Java ints add.
    pub fn add(&mut self, ash: Ash, time: i32) {
        let level = match ash {
            Ash::Wood => &mut self.wood,
            Ash::Coal => &mut self.coal,
            Ash::Misc => &mut self.misc,
            Ash::Fly => &mut self.fly,
            Ash::Soot => &mut self.soot,
        };
        *level = level.wrapping_add(time);
    }

    /// `processAsh(level, type, threshold)`, the wood's level taken with a new stack included.
    fn process(&mut self, level: i32, ash: Ash, threshold: i32) -> bool {
        if level < threshold {
            return false;
        }
        // The engine's slots: five from its machine file; none (an engine without it) leaves the level as it is.
        for i in 0..self.slots.len().min(SLOTS) {
            match &mut self.slots[i] {
                None => {
                    self.slots[i] = Some(Stack::new(ash.item(), 1));
                    self.wood = self.wood.wrapping_sub(threshold);
                    return true;
                }
                Some(s) if s.count < STACK && s.id == ash.item() => {
                    s.count += 1;
                    return true;
                }
                Some(_) => {}
            }
        }
        false
    }

    /// One `updateEntity`; true when it changed anything.
    pub fn step(&mut self) -> bool {
        let before = self.clone();
        if self.process(self.wood, Ash::Wood, THRESHOLD) {
            self.wood = self.wood.wrapping_sub(THRESHOLD);
        }
        if self.process(self.coal, Ash::Coal, THRESHOLD) {
            self.coal = self.coal.wrapping_sub(THRESHOLD);
        }
        if self.process(self.misc, Ash::Misc, THRESHOLD) {
            self.misc = self.misc.wrapping_sub(THRESHOLD);
        }
        if self.process(self.fly, Ash::Fly, THRESHOLD) {
            self.fly = self.fly.wrapping_sub(THRESHOLD);
        }
        if self.process(self.soot, Ash::Soot, SOOT_THRESHOLD) {
            self.soot = self.soot.wrapping_sub(SOOT_THRESHOLD);
        }
        *self != before
    }

    /// `isFull`: some slot holds ash (the model shows its filled inside).
    pub fn full(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }
}

impl Logic for Ashpit {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn level(&self, max: u8) -> u8 {
        u8::from(self.full()).min(max)
    }
    /// No item screen here: the ash comes out through the engine's slots.
    fn view(&self) -> ViewModel {
        ViewModel::new("Poço de cinzas").text(if self.full() { "Com cinzas" } else { "Vazio" }, view::GREY)
    }
}

/// `ContainerAshpit`: the five slots, no values on the screen.
impl Slotted for Ashpit {
    /// Its record keeps its slots (the model's JSON): a restore from the journal has them.
    fn slots_recorded(&self) -> bool {
        true
    }
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Vec::new()
    }
    /// The levels (`writeToNBT`), saved with the slots.
    fn save_state(&self) -> Vec<i32> {
        vec![self.wood, self.coal, self.misc, self.fly, self.soot]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [wood, coal, misc, fly, soot, ..] = v {
            (self.wood, self.coal, self.misc, self.fly, self.soot) = (*wood, *coal, *misc, *fly, *soot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(a: &Ashpit, i: usize) -> Option<(&str, u32)> {
        a.slots[i].as_ref().map(|s| (s.id.as_str(), s.count))
    }

    /// One ash a tick per kind at its threshold; a new stack takes the threshold off the wood's level too.
    #[test]
    fn a_new_stack_takes_the_threshold_off_the_wood_too() {
        let mut a = Ashpit::default();
        a.add(Ash::Coal, 4500);
        assert!(a.step());
        assert_eq!(count(&a, 0), Some(("ntm:powder_ash__coal", 1)));
        assert_eq!((a.coal, a.wood), (2500, -2000), "TileEntityAshpit.processAsh: ashLevelWood -= threshold");
        a.step();
        assert_eq!(count(&a, 0), Some(("ntm:powder_ash__coal", 2)), "the same stack grows: the wood's level stays");
        assert_eq!((a.coal, a.wood), (500, -2000));
        assert!(!a.step(), "500 is under the threshold");
    }

    /// The wood's own: its new stack takes the threshold off twice.
    #[test]
    fn the_woods_new_stack_takes_twice() {
        let mut a = Ashpit::default();
        a.add(Ash::Wood, 4000);
        a.step();
        assert_eq!((a.wood, count(&a, 0)), (0, Some(("ntm:powder_ash__wood", 1))));
    }

    /// The first empty slot comes before a stack of the same kind further on; soot waits for 8000.
    #[test]
    fn an_empty_slot_comes_first_and_soot_waits_for_8000() {
        let mut a = Ashpit::default();
        a.slots[1] = Some(Stack::new("ntm:powder_ash__misc", 3));
        a.add(Ash::Misc, 2000);
        a.add(Ash::Soot, 7999);
        a.step();
        assert_eq!((count(&a, 0), count(&a, 1)), (Some(("ntm:powder_ash__misc", 1)), Some(("ntm:powder_ash__misc", 3))));
        assert!(a.slots[2].is_none(), "7999 soot is under 8000");
        a.add(Ash::Soot, 1);
        a.step();
        assert_eq!(count(&a, 2), Some(("ntm:powder_ash__soot", 1)));
    }

    /// No slots from the engine (a machine file it lacks): nothing made, nothing lost, no panic.
    #[test]
    fn without_slots_the_level_waits() {
        let mut a = Ashpit { slots: Vec::new(), ..Ashpit::default() };
        a.add(Ash::Coal, 2000);
        assert!(!a.step());
        assert_eq!((a.coal, a.wood), (2000, 0));
    }

    /// Full slots of other kinds or full stacks: the level waits.
    #[test]
    fn a_full_pit_keeps_the_level() {
        let mut a = Ashpit::default();
        for i in 0..SLOTS {
            a.slots[i] = Some(Stack::new("ntm:powder_ash__wood", 64));
        }
        a.add(Ash::Coal, 2000);
        assert!(!a.step());
        assert_eq!(a.coal, 2000);
        let back = {
            let mut b = Ashpit::default();
            b.load_state(&a.save_state());
            b.coal
        };
        assert_eq!(back, 2000);
    }
}

//! What the native bundle's helpers call (docs/nativo.md): a machine's block
//! entity as plain data. The game keeps its state (this module's JSON, in
//! the block entity's compound), its item slots and its menu's data slots;
//! the helpers turn the game's types into these and back. Nothing here
//! knows the game.
//!
//! Stage L1 of the migration: the electric furnace, in the vanilla furnace
//! menu (input, battery, result; the flame shows its power, the arrow its
//! progress). The other machines come in L3.

use crate::{
    machine::{CREATIVE_BATTERY, ElectricFurnace, Logic, Machine, charge_from_slot},
    registry,
    server::container_bridge::Stack,
};

/// The machine blocks whose block entity this module runs.
pub const BLOCKS: &[&str] = &["ntm:machine_electric_furnace_off"];

/// A machine block's slots, as a `WorldlyContainer` says them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Container {
    pub slots: usize,
    /// The slots each face reaches: down, up, north, south, west, east.
    pub faces: [Vec<usize>; 6],
    /// Slots hoppers may put into, and take from.
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
    /// The vanilla menu that shows them.
    pub menu: Option<&'static str>,
}

/// The electric furnace's slots (`ContainerElectricFurnace`): what it
/// smelts, its battery, what it made.
pub mod furnace {
    pub const INPUT: usize = 0;
    pub const BATTERY: usize = 1;
    pub const OUTPUT: usize = 2;
    pub const SLOTS: usize = 3;
}

pub fn handles(block: &str) -> bool {
    BLOCKS.contains(&block)
}

/// As a vanilla furnace: the input from above, the battery from the sides,
/// the result from below.
pub fn container(block: &str) -> Option<Container> {
    use furnace::*;
    handles(block).then(|| Container {
        slots: SLOTS,
        faces: [vec![OUTPUT], vec![INPUT], vec![BATTERY], vec![BATTERY], vec![BATTERY], vec![BATTERY]],
        inputs: vec![INPUT, BATTERY],
        outputs: vec![OUTPUT],
        menu: Some("minecraft:furnace"),
    })
}

/// A new machine's state for `block`, facing `facing` when it has one.
pub fn new_state(block: &str, facing: Option<&str>) -> Option<String> {
    if !handles(block) {
        return None;
    }
    let spec = registry::spec(block)?;
    serde_json::to_string(&Machine::new(spec.kind, block, facing.map(str::to_string))).ok()
}

/// One server tick of the machine `block` whose state is `state`, its items
/// `items` (as many as its container's slots) and its menu's `data`: true
/// when anything changed. A state that doesn't read starts over as new.
pub fn tick(block: &str, state: &mut String, items: &mut [Option<Stack>], data: &mut Vec<i32>) -> bool {
    let Some(spec) = registry::spec(block).filter(|_| handles(block)) else { return false };
    let mut m: Machine = serde_json::from_str(state).unwrap_or_else(|_| Machine::new(spec.kind, block, None));
    let before = (state.clone(), items.to_vec(), data.clone());
    if let crate::machine::Data::ElectricFurnace(f) = &mut m.data {
        furnace_tick(f, items, data);
    }
    *state = serde_json::to_string(&m).unwrap_or_default();
    before != (state.clone(), items.to_vec(), data.clone())
}

fn pile(s: &Option<Stack>) -> Option<(String, u32)> {
    s.as_ref().filter(|s| s.count > 0).map(|s| (s.id.clone(), s.count))
}

fn stack(p: &Option<(String, u32)>) -> Option<Stack> {
    p.as_ref().map(|(i, n)| Stack::new(i, *n))
}

/// The slots in, a tick of `ElectricFurnace`, the slots out. The battery
/// charges it (`chargeTEFromItems`; only the creative one: the port has no
/// other charge in items). Data: power, its maximum, progress, its total.
fn furnace_tick(f: &mut ElectricFurnace, items: &mut [Option<Stack>], data: &mut Vec<i32>) {
    use furnace::*;
    f.input = pile(&items[INPUT]);
    f.output = pile(&items[OUTPUT]);
    f.power = charge_from_slot(items, BATTERY, f.power, ElectricFurnace::MAX);
    f.tick();
    items[INPUT] = stack(&f.input);
    items[OUTPUT] = stack(&f.output);
    let ticks = crate::recipes::furnace().ticks;
    *data = vec![f.power.min(i32::MAX as u64) as i32, ElectricFurnace::MAX as i32, f.progress as i32, ticks as i32];
}

/// Whether `item` may go into `slot` of `block` (the furnace: smeltable in,
/// the battery in its slot, nothing into the result).
pub fn accepts(block: &str, slot: usize, item: &str) -> bool {
    use furnace::*;
    handles(block)
        && match slot {
            INPUT => crate::recipes::furnace().smelt(item).is_some(),
            BATTERY => item == CREATIVE_BATTERY,
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: &str = "ntm:machine_electric_furnace_off";

    /// A new furnace, a creative battery and iron ore: an ingot in 100
    /// ticks; its state reads back.
    #[test]
    fn the_electric_furnace_smelts_on_its_battery_through_plain_data() {
        let mut state = new_state(F, Some("north")).unwrap();
        let mut items = vec![Some(Stack::new("minecraft:iron_ore", 2)), Some(Stack::new(CREATIVE_BATTERY, 1)), None];
        let mut data = Vec::new();
        assert!(tick(F, &mut state, &mut items, &mut data));
        assert_eq!(data, vec![100_000 - 50, 100_000, 1, 100]);
        for _ in 0..99 {
            tick(F, &mut state, &mut items, &mut data);
        }
        assert_eq!(items[2], Some(Stack::new("minecraft:iron_ingot", 1)));
        assert_eq!(items[0], Some(Stack::new("minecraft:iron_ore", 1)));
        // Saved and read again: the progress goes on.
        let mut again = state.clone();
        tick(F, &mut again, &mut items, &mut data);
        assert_eq!(data[2], 1);
        assert!(again.contains("\"progress\":1"));
    }

    /// Without the battery nothing smelts; a state that doesn't read starts
    /// over; other blocks aren't this module's.
    #[test]
    fn a_furnace_without_power_waits() {
        let mut state = "{".to_string();
        let mut items = vec![Some(Stack::new("minecraft:iron_ore", 1)), None, None];
        let mut data = Vec::new();
        tick(F, &mut state, &mut items, &mut data);
        assert_eq!((data[0], data[2]), (0, 0));
        assert!(new_state("ntm:machine_boiler", None).is_none() && container("minecraft:stone").is_none());
        assert!(accepts(F, 0, "minecraft:iron_ore") && !accepts(F, 0, "minecraft:dirt") && accepts(F, 1, CREATIVE_BATTERY) && !accepts(F, 2, "minecraft:iron_ore"));
    }
}

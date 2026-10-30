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

/// A machine block (any the registry runs): its block entity carries the
/// machine's state, slots and data, and the server's phases run it.
pub fn machine_block(block: &str) -> bool {
    !handles(block) && registry::spec(block).is_some_and(|s| s.kind != crate::machine::Kind::Inert)
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

/// A slot's rule, as the game's `ModSlotFilter` takes it: the items it
/// takes (none: any), those it refuses, its limit, and whether players
/// only take from it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotFilter {
    pub accept: Option<Vec<String>>,
    pub reject: Vec<String>,
    pub max: Option<u32>,
    pub output: bool,
}

/// The rules of block `block`'s slots, in order (menu, clicks and hoppers
/// alike): the furnace takes what it smelts, one creative battery, and
/// gives its result.
pub fn slot_filters(block: &str) -> Vec<SlotFilter> {
    if !handles(block) {
        return Vec::new();
    }
    let mut smeltable: Vec<String> = crate::recipes::furnace().inputs().map(str::to_string).collect();
    smeltable.sort();
    vec![
        SlotFilter { accept: Some(smeltable), ..Default::default() },
        SlotFilter { accept: Some(vec![CREATIVE_BATTERY.into()]), max: Some(1), ..Default::default() },
        SlotFilter { output: true, ..Default::default() },
    ]
}

/// The machine files (`horizon_machine/<block>.json` of the content, by
/// `tools/machine-gen/build.py`): each block's slot rules and synced data
/// indices, as `{"slots": [...], "synced": [...]}`.
fn machine_files() -> &'static std::collections::HashMap<String, serde_json::Value> {
    static T: std::sync::OnceLock<std::collections::HashMap<String, serde_json::Value>> = std::sync::OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/machine_files.json")).expect("machine_files.json"))
}

/// Block `block`'s slot rules, from its machine file (none without one).
pub fn block_slots(block: &str) -> Vec<SlotFilter> {
    machine_files().get(block).map(|v| file_slots(v.to_string().as_bytes())).unwrap_or_default()
}

/// Block `block`'s synced data indices.
pub fn block_synced(block: &str) -> Vec<usize> {
    machine_files().get(block).map(|v| synced(v.to_string().as_bytes())).unwrap_or_default()
}

/// The slots of a machine file (`horizon_machine/<block>.json`, the same
/// the screens and the old engine read): one rule a slot, a `grid`
/// `[x, y, columns, rows]` one rule for each of its cells.
pub fn file_slots(file: &[u8]) -> Vec<SlotFilter> {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(file) else { return Vec::new() };
    let mut out = Vec::new();
    for s in v.get("slots").and_then(|s| s.as_array()).into_iter().flatten() {
        let list = |k: &str| s.get(k).and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_string)).collect::<Vec<_>>());
        let rule = SlotFilter {
            accept: list("accept"),
            reject: list("reject").unwrap_or_default(),
            max: s.get("max").and_then(|m| m.as_u64()).map(|m| m as u32),
            output: s.get("output").and_then(|o| o.as_bool()).unwrap_or(false),
        };
        let cells = match s.get("grid").and_then(|g| g.as_array()) {
            Some(g) if g.len() == 4 => (g[2].as_u64().unwrap_or(1) * g[3].as_u64().unwrap_or(1)) as usize,
            _ => 1,
        };
        out.extend(std::iter::repeat_n(rule, cells));
    }
    out
}

/// The container of a machine with these slots: hoppers put into the
/// inputs and take from the outputs, through any face. `menu` is only the
/// wire's envelope (the client lays the slots out itself, API 19).
pub fn file_container(slots: &[SlotFilter]) -> Container {
    let all: Vec<usize> = (0..slots.len()).collect();
    Container {
        slots: slots.len(),
        faces: std::array::from_fn(|_| all.clone()),
        inputs: all.iter().copied().filter(|&i| !slots[i].output).collect(),
        outputs: all.iter().copied().filter(|&i| slots[i].output).collect(),
        menu: Some("minecraft:generic_9x3"),
    }
}

// ------------------------------------------------------- client sync

/// Where block `block`'s machine file is among the content's data
/// (`data/ntm/horizon_machine/machine_press.json`).
pub fn machine_file(block: &str) -> Option<String> {
    let (ns, path) = block.split_once(':')?;
    Some(format!("data/{ns}/horizon_machine/{path}.json"))
}

/// The data indices a machine file's `synced` names: those whose change the
/// clients' models must see (the rest only matter to the open screen).
pub fn synced(machine_file: &[u8]) -> Vec<usize> {
    serde_json::from_slice::<serde_json::Value>(machine_file)
        .ok()
        .and_then(|v| v.get("synced")?.as_array().cloned())
        .map(|a| a.iter().filter_map(|i| i.as_u64()).map(|i| i as usize).collect())
        .unwrap_or_default()
}

/// Whether the clients need the machine's data again: a synced index that
/// changed (or data never sent).
pub fn sync_changed(sent: Option<&[i32]>, now: &[i32], synced: &[usize]) -> bool {
    let Some(sent) = sent else { return !synced.is_empty() };
    synced.iter().any(|&i| sent.get(i) != now.get(i))
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

    /// Only a synced index's change asks for an update; never sent, any
    /// synced machine does.
    #[test]
    fn the_clients_hear_of_synced_values_only() {
        let file = br#"{"data": 4, "synced": [0, 2]}"#;
        let s = synced(file);
        assert_eq!(s, vec![0, 2]);
        assert!(!sync_changed(Some(&[1, 5, 3]), &[1, 9, 3], &s), "o índice 1 não é sincronizado");
        assert!(sync_changed(Some(&[1, 5, 3]), &[1, 5, 4], &s));
        assert!(sync_changed(None, &[0], &s) && !sync_changed(None, &[0], &[]));
        assert_eq!(machine_file("ntm:machine_press").as_deref(), Some("data/ntm/horizon_machine/machine_press.json"));
        assert!(synced(b"{").is_empty());
    }

    /// The furnace's slots: smeltables, one battery, a result to take.
    #[test]
    fn the_furnace_slots_take_what_they_should() {
        let f = slot_filters(F);
        assert!(f[0].accept.as_ref().is_some_and(|a| a.iter().any(|i| i == "minecraft:iron_ore")));
        assert_eq!((f[1].max, f[2].output), (Some(1), true));
        assert!(slot_filters("minecraft:stone").is_empty());
    }

    /// A machine file's slots: a grid's cells each with its rule.
    #[test]
    fn a_machine_file_gives_each_slot_its_rule() {
        let f = br#"{"slots": [{"grid": [44, 18, 3, 3], "reject": ["ntm:blades_steel"]}, {"grid": [116, 18, 3, 6], "output": true}, {"x": 44, "y": 108, "accept": ["ntm:blades_steel"], "max": 1}]}"#;
        let s = file_slots(f);
        assert_eq!(s.len(), 9 + 18 + 1);
        assert_eq!(s[0].reject, vec!["ntm:blades_steel".to_string()]);
        assert!(s[9].output && s[26].output && !s[8].output);
        assert_eq!((s[27].accept.as_deref(), s[27].max), (Some(&["ntm:blades_steel".to_string()][..]), Some(1)));
        let c = file_container(&s);
        assert_eq!((c.slots, c.inputs.len(), c.outputs.len()), (28, 10, 18));
        // The content's own files, embedded.
        let press = block_slots("ntm:machine_press");
        assert_eq!((press.len(), press[1].max, press[3].output), (4, Some(1), true));
        assert_eq!(block_synced("ntm:machine_press"), vec![0, 1]);
        assert!(block_slots("ntm:machine_boiler").is_empty() || block_slots("ntm:machine_boiler").len() < 64);
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

/// The machines of the old screen only, through their native screens.
#[cfg(test)]
mod screens {
    use crate::fluidbox::FluidStore;
    use crate::machine::{Battery, BatteryMode, CREATIVE_BATTERY, Logic, WoodBurner};
    use crate::server::container_bridge::{Slotted, Stack};

    /// The battery: the creative battery in slot 0 fills it; buttons 0 and
    /// 1 turn the two modes.
    #[test]
    fn the_battery_fills_from_its_slot_and_turns_its_modes() {
        let mut b = Battery::new(1_000_000);
        b.slots[0] = Some(Stack::new(CREATIVE_BATTERY, 1));
        b.sync_in();
        assert_eq!(b.screen_data(), vec![1_000_000, 1_000_000, 1, 2, 1]);
        b.button(0);
        b.button(1);
        assert_eq!((b.mode, b.mode_high), (BatteryMode::Output, BatteryMode::None));
    }

    /// The wood burner burns what its fuel slot holds; button 0 stops it.
    #[test]
    fn the_wood_burner_burns_its_slot() {
        let mut w = WoodBurner::default();
        w.slots[0] = Some(Stack::new("minecraft:coal", 2));
        w.sync_in();
        w.tick();
        w.sync_out();
        assert_eq!(w.slots[0], Some(Stack::new("minecraft:coal", 1)));
        assert_eq!(w.screen_data()[..5], [100, 100_000, 1599, 1600, 1]);
        w.button(0);
        assert!(!w.on);
    }

    /// The barrel: a full tank in slot 2 pours, its empty one out at 3; an
    /// empty one in slot 4 fills, out at 5.
    #[test]
    fn the_barrel_pours_and_fills_containers_in_its_slots() {
        let mut f = FluidStore::new(16_000, 0);
        f.slots[2] = Some(Stack::new("ntm:fluid_tank_full__water", 2));
        f.sync_in();
        assert_eq!((f.tank.fluid.as_str(), f.tank.mb), ("water", 1_000));
        assert_eq!(f.slots[3], Some(Stack::new("ntm:fluid_tank_empty", 1)));
        f.slots[4] = Some(Stack::new("ntm:fluid_tank_empty", 1));
        f.sync_in();
        assert_eq!(f.slots[5], Some(Stack::new("ntm:fluid_tank_full__water", 1)));
        assert_eq!(f.tank.mb, 1_000, "um entrou e um saiu");
        assert_eq!(f.screen_data()[3], 0);
        f.button(0);
        assert_eq!(f.screen_data()[3], 1);
    }
}

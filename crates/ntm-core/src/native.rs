//! What the native bundle's helpers call (docs/nativo.md): a machine's
//! block entity as plain data (its slots, their rules, its menu's data
//! indices the clients' models need). The server (`server/native_server.rs`)
//! runs the machines; nothing here knows the game.

use crate::{
    machine::{CREATIVE_BATTERY, ElectricFurnace},
    registry,
};

/// A machine block's slots, as a `WorldlyContainer` says them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Container {
    pub slots: usize,
    /// The slots each face reaches: down, up, north, south, west, east.
    pub faces: [Vec<usize>; 6],
    /// Slots hoppers may put into, and take from.
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
    /// The vanilla menu that shows them (only the wire's envelope for a
    /// native screen, API 19).
    pub menu: Option<&'static str>,
}

const FURNACE: &str = "ntm:machine_electric_furnace_off";

/// A machine block (any the registry runs): its block entity carries the
/// machine's state, slots and data, and the server's phases run it.
pub fn machine_block(block: &str) -> bool {
    registry::spec(block).is_some_and(|s| s.kind != crate::machine::Kind::Inert)
}

/// Block `block`'s container: the electric furnace's is a vanilla
/// furnace's (input from above, battery from the sides, result from below);
/// the others' come from their machine files.
pub fn container(block: &str) -> Option<Container> {
    use crate::machine::furnace_slot::*;
    if block == FURNACE {
        return Some(Container {
            slots: SLOTS,
            faces: [vec![OUTPUT], vec![INPUT], vec![BATTERY], vec![BATTERY], vec![BATTERY], vec![BATTERY]],
            inputs: vec![INPUT, BATTERY],
            outputs: vec![OUTPUT],
            menu: Some("minecraft:furnace"),
        });
    }
    let slots = block_slots(block);
    (machine_block(block) && !slots.is_empty()).then(|| file_container(&slots))
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
/// gives its result; the others' are their machine files'.
pub fn slot_filters(block: &str) -> Vec<SlotFilter> {
    if block != FURNACE {
        return if machine_block(block) { block_slots(block) } else { Vec::new() };
    }
    let mut smeltable: Vec<String> = crate::recipes::furnace().inputs().map(str::to_string).collect();
    smeltable.sort();
    vec![
        SlotFilter { accept: Some(smeltable), ..Default::default() },
        SlotFilter { accept: Some(vec![CREATIVE_BATTERY.into()]), max: Some(1), ..Default::default() },
        SlotFilter { output: true, ..Default::default() },
    ]
}

/// The furnace's menu data: power, its maximum, progress, its total (the
/// vanilla furnace's flame and arrow).
pub fn furnace_data(f: &ElectricFurnace) -> Vec<i32> {
    let ticks = crate::recipes::furnace().ticks;
    vec![f.power.min(i32::MAX as u64) as i32, ElectricFurnace::MAX as i32, f.progress as i32, ticks as i32]
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
    if block == "ntm:pile_device__loader" { return vec![SlotFilter { accept: Some(crate::pile::ROD_NAMES.iter().map(|n| format!("ntm:pile_rod__{n}")).collect()), max: Some(1), ..Default::default() }]; }
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

// ------------------------------------------------------- recipe selection

/// The menu button that clears a machine's recipe.
pub const RECIPE_CLEAR: u32 = 9999;
/// The menu button that picks recipe `i` of [`recipes`]: `RECIPE_BASE + i`.
pub const RECIPE_BASE: u32 = 10_000;

/// A row of a machine's recipe list. The tables are the port's, compiled
/// into both sides, so the client lays out, pages and searches the list
/// itself; only the pick (a menu button) reaches the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeRow {
    /// Its place in the machine's table (the button is `RECIPE_BASE + index`).
    pub index: u32,
    pub label: String,
    /// The item shown: its first item output, or the fluid identifier when
    /// it makes only fluids.
    pub icon: String,
    /// (item, count): the first of an ingredient's alternatives.
    pub inputs: Vec<(String, u32)>,
    pub outputs: Vec<(String, u32)>,
    /// (fluid, mB).
    pub fluids_in: Vec<(String, u32)>,
    pub fluids_out: Vec<(String, u32)>,
}

const FLUID_ICON: &str = "ntm:fluid_identifier_multi";

/// The recipes block `block`'s machine picks from (None: it has no list,
/// its recipe follows what it holds). An anvil lists its tier's
/// constructions, each with its index in the whole table.
pub fn recipes(block: &str) -> Option<Vec<RecipeRow>> {
    let icon = |out: &[(String, u32)]| out.first().map_or(FLUID_ICON.to_string(), |o| o.0.clone());
    if let Some(st) = crate::crafter::Station::of(block) {
        if st.picks_by_input() {
            return None;
        }
        let rows = st.recipes().iter().enumerate().map(|(i, r)| {
            let outputs: Vec<(String, u32)> = r.items_out.iter().map(|o| (o.0.clone(), o.1)).collect();
            RecipeRow {
                index: i as u32,
                label: r.label.clone(),
                icon: icon(&outputs),
                inputs: r.items_in.iter().filter_map(|i| Some((i.ids.first()?.clone(), i.n))).collect(),
                outputs,
                fluids_in: r.fluids_in.clone(),
                fluids_out: r.fluids_out.clone(),
            }
        });
        return Some(rows.collect());
    }
    if registry::spec(block)?.kind != crate::machine::Kind::Anvil {
        return None;
    }
    let t = crate::anvil::Tables::ntm();
    let rows = t.listed(crate::anvil::tier(block)).into_iter().map(|i| {
        let c = &t.construction[i];
        let outputs: Vec<(String, u32)> = c.outputs.iter().map(|o| (o.0.clone(), o.1)).collect();
        RecipeRow {
            index: i as u32,
            label: if c.label.is_empty() { icon(&outputs) } else { c.label.clone() },
            icon: icon(&outputs),
            inputs: c.inputs.iter().filter_map(|i| Some((i.ids.first()?.clone(), i.count))).collect(),
            outputs,
            fluids_in: Vec::new(),
            fluids_out: Vec::new(),
        }
    });
    Some(rows.collect())
}

/// The index in block `block`'s menu data that holds its recipe + 1 (0:
/// none), for the list to mark it.
pub fn recipe_slot(block: &str) -> Option<usize> {
    recipes(block)?;
    if crate::crafter::Station::of(block).is_some() {
        Some(crate::server::container_bridge::CRAFTER_RECIPE)
    } else {
        Some(crate::anvil::RECIPE_DATA)
    }
}

/// A recipe menu button: `Some(None)` clears, `Some(Some(i))` picks recipe
/// `i`, None is another button.
pub fn recipe_button(button: u32) -> Option<Option<usize>> {
    match button {
        RECIPE_CLEAR => Some(None),
        b if b >= RECIPE_BASE => Some(Some((b - RECIPE_BASE) as usize)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: &str = "ntm:machine_electric_furnace_off";

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
        assert_eq!(container(F).map(|c| (c.slots, c.menu)), Some((3, Some("minecraft:furnace"))));
        assert_eq!(container("ntm:machine_press").map(|c| c.slots), Some(4));
        assert!(container("minecraft:stone").is_none() && machine_block("ntm:machine_press"));
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

    /// The electric furnace: a creative battery and iron ore, an ingot in
    /// 100 ticks; its data, the furnace menu's flame and arrow.
    #[test]
    fn the_electric_furnace_smelts_on_its_battery_slot() {
        let mut f = crate::machine::ElectricFurnace {
            slots: vec![Some(Stack::new("minecraft:iron_ore", 2)), Some(Stack::new(CREATIVE_BATTERY, 1)), None],
            ..Default::default()
        };
        for _ in 0..100 {
            f.sync_in();
            f.tick();
            f.sync_out();
        }
        assert_eq!(f.slots[2], Some(Stack::new("minecraft:iron_ingot", 1)));
        assert_eq!(f.slots[0], Some(Stack::new("minecraft:iron_ore", 1)));
        assert_eq!(f.screen_data(), vec![100_000 - 50, 100_000, 0, 100], "cheia antes do tick, que gasta 50");
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

    /// The recipe lists: the crafters that pick (not those that follow
    /// their input), the anvils by tier; the pick and the clear by button,
    /// the pick + 1 in the menu data.
    #[test]
    fn recipe_lists_pick_by_button() {
        use super::{RECIPE_BASE, RECIPE_CLEAR, recipe_button, recipe_slot, recipes};
        use crate::crafter::{Crafter, Station};
        const ASM: &str = "ntm:machine_assembly_machine";
        let rows = recipes(ASM).unwrap();
        assert_eq!(rows.len(), Station::Assembly.recipes().len());
        assert!(rows.iter().enumerate().all(|(i, r)| r.index == i as u32 && !r.icon.is_empty()));
        assert!(recipes("ntm:machine_centrifuge").is_none());
        assert!(recipes("ntm:machine_battery").is_none());
        assert_eq!(recipe_slot(ASM), Some(30));
        let mut c = Crafter::new(Station::Assembly);
        let r = &Station::Assembly.recipes()[3];
        c.set_recipe(r);
        let data = crate::server::container_bridge::crafter_data(&c);
        assert_eq!(data.len(), 31);
        assert_eq!(data[30], 4);
        assert_eq!(recipe_button(RECIPE_BASE + 3), Some(Some(3)));
        assert_eq!(recipe_button(RECIPE_CLEAR), Some(None));
        assert_eq!(recipe_button(1), None);

        let iron = recipes("ntm:anvil_iron").unwrap();
        let t = crate::anvil::Tables::ntm();
        assert!(!iron.is_empty() && iron.len() < t.construction.len());
        assert_eq!(recipe_slot("ntm:anvil_iron"), Some(crate::anvil::RECIPE_DATA));
        let mut a = crate::anvil::Anvil::new(1);
        let pick = iron[2].index;
        a.press(t, RECIPE_BASE + pick);
        assert_eq!(a.screen_data(t)[crate::anvil::RECIPE_DATA], pick as i32 + 1);
        // Above its tier: nothing picked.
        let high = (0..t.construction.len()).find(|&i| !t.construction[i].tier_valid(1)).unwrap();
        a.press(t, RECIPE_BASE + high as u32);
        assert_eq!(a.screen_data(t)[crate::anvil::RECIPE_DATA], 0);
        a.press(t, RECIPE_BASE + pick);
        a.press(t, RECIPE_CLEAR);
        assert_eq!(a.screen_data(t)[crate::anvil::RECIPE_DATA], 0);
    }
}

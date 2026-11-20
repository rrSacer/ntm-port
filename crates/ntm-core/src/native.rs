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
/// The electric furnace, lit or not (`machine_electric_furnace_off`, `_on`).
pub fn is_electric_furnace(block: &str) -> bool {
    matches!(block, FURNACE | "ntm:machine_electric_furnace_on")
}

/// The brick furnace, lit or not (`machine_furnace_brick_off`, `_on`).
pub fn is_brick_furnace(block: &str) -> bool {
    matches!(block, "ntm:machine_furnace_brick_off" | "ntm:machine_furnace_brick_on")
}

pub fn machine_block(block: &str) -> bool {
    registry::spec(block).is_some_and(|s| s.kind != crate::machine::Kind::Inert)
}

/// A machine's block entity items sized to its slots (`block_slots`): the engine reads a saved entity's items back
/// only up to the last one kept (`decode_mod_block_entity`: "the game sizes them to the block's container when it uses
/// them"), so the glue sizes them before any reader or commit uses them (RBMK T08 review P3-1).
pub fn size_slots<T: Clone>(block: &str, items: &mut Vec<Option<T>>) {
    let n = block_slots(block).len();
    if items.len() < n {
        items.resize(n, None);
    }
}

/// Block `block`'s container: the electric furnace's is a vanilla
/// furnace's (input from above, battery from the sides, result from below);
/// the others' come from their machine files.
pub fn container(block: &str) -> Option<Container> {
    use crate::machine::furnace_slot::*;
    if block == crate::radioisotopes::RTG {
        return Some(Container {slots:15,faces:std::array::from_fn(|_|(0..15).collect()),inputs:vec![],outputs:vec![],menu:Some("minecraft:generic_9x3")});
    }
    if block == crate::zirnox::BLOCK {
        let rods:Vec<_>=(0..24).collect();
        return Some(Container {slots:28,faces:std::array::from_fn(|_|rods.clone()),inputs:rods.clone(),outputs:rods,menu:Some("minecraft:generic_9x3")});
    }
    if is_electric_furnace(block) {
        return Some(Container {
            slots: SLOTS,
            faces: [vec![OUTPUT], vec![INPUT], vec![BATTERY], vec![BATTERY], vec![BATTERY], vec![BATTERY]],
            inputs: vec![INPUT, BATTERY],
            outputs: vec![OUTPUT],
            menu: Some("minecraft:furnace"),
        });
    }
    if let Some(c) = crate::furnaces::container(block).or_else(|| crate::welders::container(block)) {
        return Some(c);
    }
    if is_brick_furnace(block) {
        // `TileEntityFurnaceBrick.getAccessibleSlotsFromSide`: the input from
        // above, the fuel from the sides, the output, fuel and ash below;
        // `canExtractItem` only lets slots 2 and 3 out.
        use crate::furnace_brick::*;
        return Some(Container {
            slots: SLOTS,
            faces: [vec![OUTPUT, FUEL, ASH], vec![INPUT], vec![FUEL], vec![FUEL], vec![FUEL], vec![FUEL]],
            inputs: vec![INPUT, FUEL],
            outputs: vec![OUTPUT, ASH],
            menu: Some("minecraft:generic_9x3"),
        });
    }
    let slots = block_slots(block);
    // A screen with no slot of the machine's (RBMK T06: the control rods, `ContainerRBMKControlAuto`; T08: the boiler,
    // `ContainerRBMKGeneric`): a menu all the same, which the screen's session, buttons and the player's inventory need.
    (machine_block(block) && (!slots.is_empty() || crate::rbmk::screen::has_slotless_menu(block))).then(|| file_container(&slots))
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
    /// The items a shift-click may move in (none: as `accept`).
    pub quick_move: Option<Vec<String>>,
    /// A shift-click fills this run of slots from its last to its first (`mergeItemStack(.., true)`).
    pub quick_reverse: bool,
}

/// The rules of block `block`'s slots, in order (menu, clicks and hoppers
/// alike): the furnace takes what it smelts, one creative battery, and
/// gives its result; the others' are their machine files'.
pub fn slot_filters(block: &str) -> Vec<SlotFilter> {
    if crate::furnaces::is_furnace(block) {
        return crate::furnaces::filters(block);
    }
    if crate::welders::Model::of(block).is_some() {
        return crate::welders::filters(block);
    }
    if is_brick_furnace(block) {
        // `ContainerFurnaceBrick.transferStackInSlot`: a fuel is merged into
        // slot 1 first and then slot 0, anything else into slot 0 only: the
        // two slots are visited last to first, slot 1 taking fuels alone.
        return vec![
            SlotFilter { quick_reverse: true, ..Default::default() },
            SlotFilter { quick_move: Some(crate::fuel::fuels()), quick_reverse: true, ..Default::default() },
            SlotFilter { output: true, ..Default::default() },
            SlotFilter { output: true, ..Default::default() },
        ];
    }
    if !is_electric_furnace(block) {
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

/// Origin of an inventory operation. Shift-click and every screen gesture are Manual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotOrigin { Manual, Automation }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotAction { Insert, Extract }

/// Additional pure restriction on a slot, composed with static filters, limits and faces.
/// `slots` must be the current complete container, never a cached tick snapshot.
/// Manual ZIRNOX insertion and extraction are unrestricted (vanilla `Slot`s); automation
/// follows `isItemValidForSlot` (rods only, and only into the rod slots) and `canExtractItem`.
pub fn slot_allows(block: &str, slots: &[Option<crate::server::container_bridge::Stack>], slot: usize,
    item: &crate::server::container_bridge::Stack, action: SlotAction, origin: SlotOrigin) -> bool {
    if matches!(block, crate::radioisotopes::RTG | crate::radioisotopes::RADGEN) {
        return match (action,origin) {
            (SlotAction::Insert,SlotOrigin::Manual) => crate::radioisotopes::can_insert_manual(block,slot),
            (SlotAction::Insert,SlotOrigin::Automation) => crate::radioisotopes::can_insert(block,slots,slot,item),
            (SlotAction::Extract,SlotOrigin::Manual) => true,
            (SlotAction::Extract,SlotOrigin::Automation) => crate::radioisotopes::can_extract(block,slot),
        };
    }
    if let Some(allowed) = crate::welders::allows(block, slots, slot, &item.id, action == SlotAction::Insert, origin == SlotOrigin::Manual) {
        return allowed;
    }
    if let Some(allowed) = crate::furnaces::allows(block, slot, &item.id, action == SlotAction::Insert, origin == SlotOrigin::Manual) {
        return allowed;
    }
    if is_brick_furnace(block) {
        // The plain slots 0 and 1 take anything from a hand (1.7.10's
        // `Slot.isItemValid`); a hopper asks `isItemValidForSlot`: any input,
        // only a fuel in slot 1, nothing in the outputs; `canExtractItem`:
        // the automation takes slots 2 and 3.
        return match action {
            SlotAction::Insert => match slot {
                crate::furnace_brick::INPUT => true,
                crate::furnace_brick::FUEL => origin == SlotOrigin::Manual || crate::fuel::burn_time(&item.id) > 0,
                _ => false,
            },
            SlotAction::Extract => origin == SlotOrigin::Manual || slot >= crate::furnace_brick::OUTPUT,
        };
    }
    // The RBMK columns' automation (`getAccessibleSlotsFromSide`, `canInsertItem`, `canExtractItem`), through the core
    // and the proxies of their ports: the storage takes rods and gives anything, the outgasser takes into its input
    // only what has a recipe (`isItemValidForSlot`, T09: one whose product the port lacks has none) and gives only its
    // output (`canExtractItem`), the fuel channels and the heater show no slot. The hand (T05): a fuel
    // channel's menu takes or gives nothing in survival while the rod in it is hotter than 200 °C (`ContainerRBMKRod.
    // slotClick` and `transferStackInSlot`: every gesture on the slot, its vanilla `Slot` taking any item otherwise); a
    // creative player passes (the glue's `creative_bypass`). The other columns' menus are their tickets'.
    if crate::rbmk::persist::is_column(block) {
        if origin == SlotOrigin::Manual && crate::rbmk::persist::is_channel(block) {
            return crate::rbmk::persist::cold_enough_for_manual(slots.first().and_then(Option::as_ref));
        }
        return origin == SlotOrigin::Manual
            || match (block, action) {
                ("ntm:rbmk_storage", SlotAction::Insert) => crate::rbmk::persist::rod_of(&item.id).is_some(),
                ("ntm:rbmk_storage", SlotAction::Extract) => true,
                ("ntm:rbmk_outgasser", SlotAction::Insert) => slot == 0 && crate::rbmk::outgasser::recipe_of_port(&item.id).is_some(),
                ("ntm:rbmk_outgasser", SlotAction::Extract) => slot == 1,
                _ => false,
            };
    }
    block != crate::zirnox::BLOCK || origin == SlotOrigin::Manual
        || match action {
            SlotAction::Insert => slot < 24 && crate::zirnox::fuel(&item.id).is_some(),
            SlotAction::Extract => crate::zirnox::can_extract(slot, &item.id),
        }
}

/// The ported blocks whose Java block has `hasComparatorInputOverride`:
/// `MachineBattery` (by charge), `MachineFluidTank` and `BlockFluidBarrel` (by fluid).
pub fn has_comparator(block: &str) -> bool {
    matches!(block, "ntm:machine_battery" | "ntm:machine_fluidtank" | "ntm:barrel_steel")
}

/// `getComparatorPower` of the machine saved as `json`, 0 to 15: nothing stored is 0,
/// else `floor(fraction * 15) + 1` (`TileEntityMachineBattery`, `TileEntityBarrel`,
/// `TileEntityMachineFluidTank`), so a full store is 15.
pub fn comparator_signal(json: &str) -> u8 {
    use crate::machine::{Data, Machine};
    let Ok(machine) = serde_json::from_str::<Machine>(json) else { return 0 };
    let (stored, max) = match &machine.data {
        Data::Battery(b) => (b.power as f64, b.max as f64),
        Data::FluidStore(f) => (f64::from(f.tank.mb), f64::from(f.tank.cap)),
        _ => return 0,
    };
    if stored == 0. {
        return 0;
    }
    ((stored / max * 15.) as i32).saturating_add(1).clamp(0, 15) as u8
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
    if block == crate::zirnox::BLOCK {
        // Java's `ContainerReactorZirnox` builds slots 0..23 as plain `Slot`s: a hand
        // may place anything, one item at a time. Only `transferStackInSlot` (shift-click)
        // and the automation (`slot_allows`) are limited to the rods.
        let rods=crate::zirnox::table().fuels.iter().map(|f|f.item.clone()).collect();
        let mut slots=vec![SlotFilter {max:Some(1), quick_move:Some(rods), quick_reverse:true, ..Default::default()};24];
        // Slots 24 and 25 are vanilla `Slot`s too; their `transferStackInSlot` branches
        // send only carbon dioxide and water containers, respectively.
        for f in ["carbondioxide","water"] {slots.push(SlotFilter {quick_move:Some(crate::fluid::all_full_containers().into_iter().filter(|s|crate::fluid::contents(s).is_some_and(|c|c.fluid==f)).collect()),..Default::default()});}
        // Slots 26 and 27 are `SlotTakeOnly`, yet 1.7.10's `mergeItemStack(24, 26)` and `(26, 28)` reach them
        // too: the carbon dioxide containers go to 24 and then 26, the water ones to 25 and then 27.
        let (co2,water)=(slots[24].quick_move.clone(),slots[25].quick_move.clone());
        slots.extend([SlotFilter {output:true,quick_move:co2,..Default::default()},SlotFilter {output:true,quick_move:water,..Default::default()}]);
        return slots;
    }
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
            quick_move: None,
            quick_reverse: false,
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

    /// `getComparatorPower`: empty is 0, else `floor(fraction * 15) + 1`, full 15.
    #[test]
    fn comparator_signals_follow_the_java_formulas() {
        use crate::machine::{Data, Kind, Machine};
        let battery = |power: u64| {
            let mut m = Machine::new(Kind::Battery, "ntm:machine_battery", Some("north".into()));
            let max = if let Data::Battery(b) = &mut m.data {
                b.power = power.min(b.max);
                b.max
            } else {
                panic!()
            };
            (serde_json::to_string(&m).unwrap(), max)
        };
        let (_, max) = battery(0);
        let signal = |power: u64| comparator_signal(&battery(power).0);
        assert_eq!(signal(0), 0);
        assert_eq!(signal(1), 1);
        assert_eq!(signal(max.div_ceil(15)), 2);
        assert_eq!(signal(max / 2), 8);
        assert_eq!(signal(max - 1), 15);
        assert_eq!(signal(max), 15);
        let tank = |mb: u32| {
            let mut m = Machine::new(Kind::FluidStore, "ntm:machine_fluidtank", None);
            let Data::FluidStore(f) = &mut m.data else { panic!() };
            f.tank.mb = mb.min(f.tank.cap);
            serde_json::to_string(&m).unwrap()
        };
        assert_eq!(comparator_signal(&tank(0)), 0);
        assert_eq!(comparator_signal(&tank(1)), 1);
        assert_eq!(comparator_signal(&tank(128_000)), 8);
        assert_eq!(comparator_signal(&tank(256_000)), 15);
        assert_eq!(comparator_signal("{}"), 0);
        assert!(has_comparator("ntm:barrel_steel") && !has_comparator("ntm:machine_zirnox"));
    }

    #[test]
    fn zirnox_slot_policy_keeps_active_rods_and_allows_manual_removal() {
        let active = crate::server::container_bridge::Stack::new(&crate::zirnox::table().fuels[0].item, 1);
        let spent = crate::server::container_bridge::Stack::new(&crate::zirnox::table().fuels[0].output, 1);
        let slots = vec![Some(active.clone()), Some(spent.clone())];
        assert!(!slot_allows(crate::zirnox::BLOCK, &slots, 0, &active, SlotAction::Extract, SlotOrigin::Automation));
        assert!(slot_allows(crate::zirnox::BLOCK, &slots, 1, &spent, SlotAction::Extract, SlotOrigin::Automation));
        assert!(!slot_allows(crate::zirnox::BLOCK, &slots, 24, &spent, SlotAction::Extract, SlotOrigin::Automation));
        assert!(slot_allows(crate::zirnox::BLOCK, &slots, 0, &active, SlotAction::Extract, SlotOrigin::Manual));
        assert!(slot_allows(crate::zirnox::BLOCK, &slots, 0, &active, SlotAction::Insert, SlotOrigin::Automation));
        assert_eq!(slots, vec![Some(active), Some(spent)]);
    }

    /// `ContainerReactorZirnox` builds slots 0..23 as plain `Slot`s, so a hand places any
    /// item; the rods-only rule is `isItemValidForSlot` (automation) and `transferStackInSlot`.
    #[test]
    fn zirnox_manual_insertion_is_unrestricted_but_automation_and_shift_click_take_rods() {
        use crate::server::container_bridge::Stack;
        let rod = Stack::new(&crate::zirnox::table().fuels[0].item, 1);
        let spent = Stack::new(&crate::zirnox::table().fuels[0].output, 1);
        let junk = Stack::new("minecraft:dirt", 1);
        let allows = |slot, item: &Stack, origin| slot_allows(crate::zirnox::BLOCK, &[], slot, item, SlotAction::Insert, origin);
        for item in [&rod, &spent, &junk] {
            assert!(allows(0, item, SlotOrigin::Manual));
        }
        assert!(allows(0, &rod, SlotOrigin::Automation));
        assert!(!allows(0, &spent, SlotOrigin::Automation));
        assert!(!allows(23, &junk, SlotOrigin::Automation));
        let slots = block_slots(crate::zirnox::BLOCK);
        let quick = |s: &SlotFilter, id: &str| s.quick_move.as_ref().is_none_or(|q| q.iter().any(|x| x == id));
        for s in &slots[..24] {
            assert!(s.accept.is_none() && s.max == Some(1) && s.quick_reverse);
            assert!(quick(s, &rod.id) && !quick(s, &junk.id) && !quick(s, &spent.id));
        }
        // The fluid slots are vanilla `Slot`s as well: any item by hand, one kind of
        // container by shift-click, nothing by automation.
        let co2 = &slots[24].quick_move;
        let water = &slots[25].quick_move;
        assert!(slots[24].accept.is_none() && slots[25].accept.is_none());
        assert!(co2.as_ref().is_some_and(|q| !q.is_empty() && q.iter().all(|i| !i.contains("water"))));
        assert!(water.as_ref().is_some_and(|q| !q.is_empty() && q.iter().all(|i| !i.contains("carbon"))));
        assert!(allows(24, &junk, SlotOrigin::Manual) && allows(25, &junk, SlotOrigin::Manual));
        assert!(!allows(24, &rod, SlotOrigin::Automation) && !allows(25, &junk, SlotOrigin::Automation));
        // Java fills the rods from the last slot back (`mergeItemStack(0, 24, true)`); the fluid slots in order.
        assert!(slots[24..].iter().all(|s| !s.quick_reverse));
        // The take-only slots take the same containers by shift-click, after their input slots.
        assert!(slots[26].output && slots[26].quick_move == slots[24].quick_move);
        assert!(slots[27].output && slots[27].quick_move == slots[25].quick_move);
    }

    #[test]
    fn zirnox_automation_reaches_rod_outputs_but_not_container_slots() {
        let c = container(crate::zirnox::BLOCK).unwrap();
        assert_eq!(c.outputs, (0..24).collect::<Vec<_>>());
        assert!(c.faces.iter().all(|face| face == &c.outputs));
        assert_eq!(c.inputs, c.outputs);
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


    /// The lit furnaces are the unlit ones: the same slots, faces and rules.
    #[test]
    fn the_lit_furnaces_are_the_unlit_ones() {
        assert_eq!(container("ntm:machine_electric_furnace_on"), container("ntm:machine_electric_furnace_off"));
        assert_eq!(slot_filters("ntm:machine_electric_furnace_on"), slot_filters("ntm:machine_electric_furnace_off"));
        assert_eq!(container("ntm:machine_furnace_brick_on"), container("ntm:machine_furnace_brick_off"));
        assert_eq!(slot_filters("ntm:machine_furnace_brick_on"), slot_filters("ntm:machine_furnace_brick_off"));
    }

    /// `TileEntityFurnaceBrick`: the input from above, the fuel from the sides,
    /// the output, the fuel and the ash from below; only slots 2 and 3 leave.
    #[test]
    fn the_brick_furnace_follows_its_sided_inventory() {
        use crate::server::container_bridge::Stack;
        let c = container("ntm:machine_furnace_brick_off").unwrap();
        assert_eq!(c.faces[0], vec![2, 1, 3]);
        assert_eq!(c.faces[1], vec![0]);
        assert!(c.faces[2..].iter().all(|f| f == &vec![1]));
        assert_eq!((c.inputs, c.outputs), (vec![0, 1], vec![2, 3]));
        let b = "ntm:machine_furnace_brick_off";
        let s: Vec<Option<Stack>> = vec![None; 4];
        let (coal, stone, ingot) = (Stack::new("minecraft:coal", 1), Stack::new("minecraft:stone", 1), Stack::new("minecraft:iron_ingot", 1));
        let allows = |slot, item: &Stack, action, origin| slot_allows(b, &s, slot, item, action, origin);
        use crate::native::{SlotAction::*, SlotOrigin::*};
        assert!(allows(0, &stone, Insert, Manual) && allows(0, &stone, Insert, Automation));
        assert!(allows(1, &coal, Insert, Manual) && allows(1, &stone, Insert, Manual) && !allows(1, &stone, Insert, Automation));
        assert!(!allows(2, &ingot, Insert, Manual) && !allows(3, &coal, Insert, Automation));
        assert!(allows(0, &stone, Extract, Manual) && !allows(0, &stone, Extract, Automation));
        assert!(!allows(1, &coal, Extract, Automation) && allows(1, &coal, Extract, Manual));
        assert!(allows(2, &ingot, Extract, Automation) && allows(3, &ingot, Extract, Automation));
        let f = slot_filters(b);
        assert_eq!(f.iter().map(|x| x.output).collect::<Vec<_>>(), vec![false, false, true, true]);
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
        // As the Java tile starts: `redLow = 0` (input only), `redHigh = 2` (output).
        assert_eq!(b.screen_data(), vec![1_000_000, 1_000_000, 0, 2, 1]);
        assert_eq!((b.mode, b.mode_high), (BatteryMode::Input, BatteryMode::Output));
        b.button(0);
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

    /// The wood burner burns what its fuel slot holds; button 0 stops it. The tick it lights a fuel it makes
    /// nothing (`TileEntityMachineWoodBurner.updateEntity:97-123`: the burn is an `else if`).
    #[test]
    fn the_wood_burner_burns_its_slot() {
        let mut w = WoodBurner::default();
        w.slots[0] = Some(Stack::new("minecraft:coal", 2));
        w.sync_in();
        w.tick();
        w.sync_out();
        assert_eq!(w.slots[0], Some(Stack::new("minecraft:coal", 1)));
        assert_eq!(w.screen_data()[..5], [0, 100_000, 1600, 1600, 1]);
        w.sync_in();
        w.tick();
        assert_eq!(w.screen_data()[..5], [100, 100_000, 1599, 1600, 1]);
        w.button(0);
        assert!(!w.on);
    }

    /// Slice 6: the module's time (a log x4: 1200) goes into the fuel's kind of ash, one ash in slot 1 each 2000;
    /// off, it still lights the next fuel once its burn is out, and its burn then stays.
    #[test]
    fn the_wood_burner_leaves_its_ash_in_slot_1_on_or_off() {
        let mut w = WoodBurner { on: false, ..WoodBurner::default() };
        w.slots[0] = Some(Stack::new("minecraft:oak_log", 3));
        let run = |w: &mut WoodBurner| {
            w.sync_in();
            w.tick();
            w.sync_out();
        };
        run(&mut w);
        assert_eq!((w.burn, w.ash_wood, w.slots[0].as_ref().map(|s| s.count)), (1200, 1200, Some(2)), "off, it lit the log");
        run(&mut w);
        assert_eq!((w.burn, w.slots[0].as_ref().map(|s| s.count)), (1200, Some(2)), "off: the burn stays, nothing more lit");
        w.on = true;
        // Drained as it goes: a full buffer stops the burn (`power < maxPower`).
        for _ in 0..1200 {
            w.power = 0;
            run(&mut w);
        }
        assert_eq!(w.burn, 0);
        run(&mut w);
        assert_eq!((w.ash_wood, w.slots[1].clone()), (400, Some(Stack::new("ntm:powder_ash__wood", 1))), "2400: one ash, 400 left");
        // Coal ash does not stack on the wood's: its level waits.
        w.slots[0] = Some(Stack::new("minecraft:coal", 2));
        w.burn = 0;
        run(&mut w);
        w.burn = 0;
        run(&mut w);
        assert_eq!((w.ash_coal, w.slots[1].clone()), (3200, Some(Stack::new("ntm:powder_ash__wood", 1))));
    }

    /// `while processAsh`: a coal block's 16 000 make its eight coal ashes the tick it is lit.
    #[test]
    fn the_wood_burner_makes_all_its_ash_at_once() {
        let mut w = WoodBurner::default();
        w.slots[0] = Some(Stack::new("minecraft:coal_block", 1));
        w.sync_in();
        w.tick();
        assert_eq!((w.ash_coal, w.slots[1].clone()), (0, Some(Stack::new("ntm:powder_ash__coal", 8))));
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

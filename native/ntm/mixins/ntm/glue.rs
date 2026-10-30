// The NTM bundle's edges in the game (docs/nativo.md of ntm-port): a
// machine's block entity to and from ntm_core::native's plain data. Its
// state is a JSON string under "ntm" in the block entity's compound.
use horizon_client::containers::{ModBlockEntity, ModContainer};
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, JavaString, Tag};
use ntm_core::server::container_bridge::Stack;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const STATE: &str = "ntm";
/// The menu's data, kept in the compound for the clients' models
/// (`native_block_entity_update_tag` sees only the compound).
const DATA: &str = "data";
/// For blocks with no `facing` property (the bombs): the placer's side.
const FACING: &str = "facing";

/// The machines' block entity type ids: their blocks' ids. Every peer
/// registers the same.
pub(super) fn types(types: &mut Vec<String>) {
    types.extend(ntm_core::native::BLOCKS.iter().map(|b| b.to_string()));
}

/// What the clients see of a machine: its data and, for a block without
/// the property, its facing.
pub(super) fn update_tag(id: &str, nbt: &Compound, tag: &mut Option<Compound>) {
    if !ntm_core::native::handles(id) {
        return;
    }
    let mut t = Compound::new();
    for key in [DATA, FACING] {
        if let Some(v) = nbt.get(&JavaString::from(key)) {
            t.insert(JavaString::from(key), v.clone());
        }
    }
    *tag = Some(t);
}

/// The synced indices of `block`'s machine file, read once.
fn synced(game: &crate::game::Game<'_>, block: &str) -> Vec<usize> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<usize>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(block.to_string())
        .or_insert_with(|| {
            ntm_core::native::machine_file(block)
                .and_then(|f| game.native_data(&f))
                .map(|b| ntm_core::native::synced(&b))
                .unwrap_or_default()
        })
        .clone()
}

/// A new machine block's block entity, with its new state.
pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_some() {
        return;
    }
    if let Some(state) = ntm_core::native::new_state(block, None) {
        let mut nbt = Compound::new();
        nbt.insert(JavaString::from(STATE), Tag::String(JavaString::from(state.as_str())));
        *entity = Some((block.to_string(), nbt));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if ntm_core::native::handles(block) {
        *ticks = true;
    }
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if let Some(c) = ntm_core::native::container(block) {
        *container = Some(ModContainer {
            slots: c.slots,
            faces: c.faces,
            inputs: c.inputs,
            outputs: c.outputs,
            menu: c.menu.map(str::to_string),
        });
    }
}

/// A slot as ntm-core sees it: id, count and damage (an empty slot, or air,
/// is none).
fn stack(s: &Option<ItemStack>) -> Option<Stack> {
    let s = s.as_ref().filter(|s| s.count > 0 && &*s.item != "minecraft:air")?;
    let mut t = Stack::new(&s.item, s.count);
    t.damage = s.damage;
    Some(t)
}

/// A server tick: the state, the slots and the menu's data through
/// ntm-core. A slot ntm-core left as it was keeps the game's stack, with
/// all its components. When a synced value changed, the clients hear.
pub(super) fn tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let block = entity.id.clone();
    let Some(c) = ntm_core::native::container(&block) else { return };
    if entity.items.len() < c.slots {
        return;
    }
    let mut state = match entity.nbt.get(&JavaString::from(STATE)) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    };
    let before: Vec<Option<Stack>> = entity.items.iter().map(stack).collect();
    let mut items = before.clone();
    if !ntm_core::native::tick(&block, &mut state, &mut items, &mut entity.data) {
        return;
    }
    entity.nbt.insert(JavaString::from(STATE), Tag::String(JavaString::from(state.as_str())));
    let sent = match entity.nbt.get(&JavaString::from(DATA)) {
        Some(Tag::IntArray(v)) => Some(v.clone()),
        _ => None,
    };
    if sent.as_deref() != Some(&entity.data[..]) {
        let synced = synced(game, &block);
        if ntm_core::native::sync_changed(sent.as_deref(), &entity.data, &synced) {
            game.native_block_entity_changed(pos);
        }
        entity.nbt.insert(JavaString::from(DATA), Tag::IntArray(entity.data.clone()));
    }
    for (i, (new, old)) in items.iter().zip(&before).enumerate() {
        if new != old {
            entity.items[i] = new.as_ref().map(|s| {
                let mut it = ItemStack::new(&s.id, s.count);
                it.damage = s.damage;
                it
            });
        }
    }
}

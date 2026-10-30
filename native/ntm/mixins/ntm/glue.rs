// The NTM bundle's edges in the game (docs/nativo.md of ntm-port): the
// machines' block entities and the server (ntm_core::server::Server, its
// phases on the native host). A machine's state is a JSON string under
// "ntm" in its block entity's compound; its slots and menu data are the
// block entity's own.
use horizon_client::containers::{ModBlockEntity, ModContainer, ModSlotFilter};
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, JavaString, Tag};
use ntm_core::kv::Kv;
use ntm_core::native::SlotFilter;
use ntm_core::server::container_bridge::{Slots, Stack};
use ntm_core::server::{Server, ServerHost};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const STATE: &str = "ntm";
/// The menu's data, kept in the compound for the clients' models
/// (`native_block_entity_update_tag` sees only the compound).
const DATA: &str = "data";
/// For blocks with no `facing` property (the bombs): the placer's side.
const FACING: &str = "facing";

/// The server of the world in play, and its tick count.
static SERVER: Mutex<Option<(Server, u64)>> = Mutex::new(None);
/// Machine and cable blocks changed since the server's last tick:
/// (dimension, position, old block, new block). Its own lock: the server's
/// writes report here while it holds its own.
static CHANGES: Mutex<Vec<(String, [i32; 3], String, String)>> = Mutex::new(Vec::new());
/// Menu buttons pressed: (player, dimension, position, button).
static BUTTONS: Mutex<Vec<(String, String, [i32; 3], u32)>> = Mutex::new(Vec::new());

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What a machine file (`horizon_machine/<block>.json`, embedded in
/// ntm-core) says: its slot rules and synced indices, read once.
fn file(_game: &crate::game::Game<'_>, block: &str) -> (Vec<SlotFilter>, Vec<usize>) {
    static CACHE: OnceLock<Mutex<HashMap<String, (Vec<SlotFilter>, Vec<usize>)>>> = OnceLock::new();
    let mut cache = lock(CACHE.get_or_init(Default::default));
    cache
        .entry(block.to_string())
        .or_insert_with(|| (ntm_core::native::block_slots(block), ntm_core::native::block_synced(block)))
        .clone()
}

fn is_machine(block: &str) -> bool {
    ntm_core::native::handles(block) || ntm_core::native::machine_block(block)
}

// ------------------------------------------------------------ the world

/// A world opened or closed: a new server for it, or none.
pub(super) fn world(open: bool) {
    *lock(&SERVER) = None;
    lock(&CHANGES).clear();
    lock(&BUTTONS).clear();
    if open {
        eprintln!("ntm native: {} machine blocks", ntm_core::registry::machine_ids().count());
    }
}

/// A block changed on the server: kept when a machine or cable is in it.
pub(super) fn block_changed(level: &crate::game::GameLevel<'_>, dim: &str, pos: [i32; 3], old: horizon_world::registry::BlockStateId, new: horizon_world::registry::BlockStateId) {
    let (old, new) = (level.facts.get(old).name.to_string(), level.facts.get(new).name.to_string());
    let ours = |b: &str| b.starts_with("ntm:") && !ntm_core::native::handles(b);
    if ours(&old) || ours(&new) {
        lock(&CHANGES).push((dim.to_string(), pos, old, new));
    }
}

/// A menu button on a machine's screen.
pub(super) fn menu_button(game: &crate::game::Game<'_>, pos: [i32; 3], id: i32) {
    let player = game.native_actor_uuid().unwrap_or_default();
    if let Ok(id) = u32::try_from(id) {
        lock(&BUTTONS).push((player, game.dim.id().to_string(), pos, id));
    }
}

/// The end of a server tick: the server's phases.
pub(super) fn server_tick(game: &mut crate::game::Game<'_>) {
    let mut guard = lock(&SERVER);
    let changes = std::mem::take(&mut *lock(&CHANGES));
    let buttons = std::mem::take(&mut *lock(&BUTTONS));
    let mut host = Host { game, buttons };
    let (server, tick) = guard.get_or_insert_with(|| (Server::native(&mut host, 0), 0));
    *tick += 1;
    for (dim, pos, old, new) in changes {
        // The whole state of what is there now (its facing).
        let now = host.game.native_loaded_block(&dim, pos).unwrap_or(new);
        server.native_block_changed(&dim, pos, &old, &now);
    }
    server.native_tick(&mut host, *tick);
}

// ------------------------------------------------------------ the host

/// The server's view of the game.
struct Host<'a, 'g> {
    game: &'a mut crate::game::Game<'g>,
    buttons: Vec<(String, String, [i32; 3], u32)>,
}

/// No storage of its own: the state is the block entities'.
impl Kv for Host<'_, '_> {
    fn get(&mut self, _key: &str) -> Option<Vec<u8>> {
        None
    }
    fn set(&mut self, _key: &str, _value: &[u8]) -> Result<(), String> {
        Ok(())
    }
    fn delete(&mut self, _key: &str) {}
}

impl Host<'_, '_> {
    /// The machine block entity at `pos` of `dim`, when it is in the
    /// dimension being ticked.
    fn entity<R>(&mut self, dim: &str, pos: [i32; 3], f: impl FnOnce(&mut ModBlockEntity) -> R) -> Option<R> {
        if dim != self.game.dim.id() {
            return None;
        }
        self.game.native_with_mod_block_entity(pos, f)
    }
}

impl ServerHost for Host<'_, '_> {
    fn get_block(&mut self, dim: &str, p: [i32; 3]) -> Option<String> {
        self.game.native_loaded_block(dim, p)
    }
    fn set_blocks(&mut self, dim: &str, blocks: &[([i32; 3], String)]) -> Result<(), String> {
        self.game.native_set_blocks(dim, blocks)
    }
    fn tell(&mut self, player: &str, text: &str) {
        eprintln!("ntm: {player}: {text}");
    }
    fn log(&mut self, text: &str) {
        eprintln!("ntm: {text}");
    }
    fn machine_slots(&mut self, dim: &str, p: [i32; 3]) -> Option<Slots> {
        self.entity(dim, p, |e| e.items.iter().map(stack).collect())
    }
    fn set_machine_slot(&mut self, dim: &str, p: [i32; 3], slot: u32, new: Option<Stack>) -> bool {
        self.entity(dim, p, |e| {
            let Some(cell) = e.items.get_mut(slot as usize) else { return false };
            *cell = item(new.as_ref(), cell.take());
            true
        })
        .unwrap_or(false)
    }
    fn machine_data(&mut self, dim: &str, p: [i32; 3]) -> Option<Vec<i32>> {
        self.entity(dim, p, |e| e.data.clone())
    }
    fn set_machine_data(&mut self, dim: &str, p: [i32; 3], values: &[i32]) -> bool {
        let Some(block) = self.entity(dim, p, |e| e.id.clone()) else { return false };
        let (_, synced) = file(self.game, &block);
        let changed = self.entity(dim, p, |e| {
            let sent = match e.nbt.get(&JavaString::from(DATA)) {
                Some(Tag::IntArray(v)) => Some(v.clone()),
                _ => None,
            };
            e.data = values.to_vec();
            e.nbt.insert(JavaString::from(DATA), Tag::IntArray(values.to_vec()));
            ntm_core::native::sync_changed(sent.as_deref(), values, &synced)
        });
        if changed == Some(true) {
            self.game.native_block_entity_changed(p);
        }
        changed.is_some()
    }
    fn machine_buttons(&mut self) -> Vec<(String, String, [i32; 3], u32)> {
        std::mem::take(&mut self.buttons)
    }
    fn save_machine(&mut self, dim: &str, p: [i32; 3], json: &str) {
        self.entity(dim, p, |e| {
            e.nbt.insert(JavaString::from(STATE), Tag::String(JavaString::from(json)));
        });
    }
}

// ----------------------------------------------------- block entities

/// The machines' block entity type ids: their blocks' ids. Every peer
/// registers the same.
pub(super) fn types(types: &mut Vec<String>) {
    types.extend(ntm_core::registry::machine_ids().filter(|b| is_machine(b)).map(str::to_string));
}

/// A new machine block's block entity: the L1 furnace with its new state,
/// the others empty (the server keeps their state in it).
pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_some() || !is_machine(block) {
        return;
    }
    let mut nbt = Compound::new();
    if let Some(state) = ntm_core::native::new_state(block, None) {
        nbt.insert(JavaString::from(STATE), Tag::String(JavaString::from(state.as_str())));
    }
    *entity = Some((block.to_string(), nbt));
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if is_machine(block) {
        *ticks = true;
    }
}

pub(super) fn container(game: &crate::game::Game<'_>, block: &str, container: &mut Option<ModContainer>) {
    let c = match ntm_core::native::container(block) {
        Some(c) => c,
        None if ntm_core::native::machine_block(block) => {
            let (slots, _) = file(game, block);
            if slots.is_empty() {
                return;
            }
            ntm_core::native::file_container(&slots)
        }
        None => return,
    };
    *container = Some(ModContainer {
        slots: c.slots,
        faces: c.faces,
        inputs: c.inputs,
        outputs: c.outputs,
        menu: c.menu.map(str::to_string),
    });
}

/// The rules of a machine's slots: what each takes, its limit, the output
/// ones (the same for the menu, clicks and hoppers).
pub(super) fn slot_filters(game: &crate::game::Game<'_>, block: &str, filters: &mut Vec<ModSlotFilter>) {
    let rules = if ntm_core::native::handles(block) {
        ntm_core::native::slot_filters(block)
    } else if ntm_core::native::machine_block(block) {
        file(game, block).0
    } else {
        return;
    };
    filters.extend(rules.into_iter().map(|f| ModSlotFilter { accept: f.accept, reject: f.reject, max: f.max, output: f.output }));
}

/// What the clients see of a machine: its data and, for a block without
/// the property, its facing.
pub(super) fn update_tag(id: &str, nbt: &Compound, tag: &mut Option<Compound>) {
    if !is_machine(id) {
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

/// A slot as ntm-core sees it: id, count and damage (an empty slot, or air,
/// is none).
fn stack(s: &Option<ItemStack>) -> Option<Stack> {
    let s = s.as_ref().filter(|s| s.count > 0 && &*s.item != "minecraft:air")?;
    let mut t = Stack::new(&s.item, s.count);
    t.damage = s.damage;
    Some(t)
}

/// The game's stack for `new`: the old one with its components when it is
/// the same item and damage, else a plain one.
fn item(new: Option<&Stack>, old: Option<ItemStack>) -> Option<ItemStack> {
    let new = new?;
    match old {
        Some(mut o) if &*o.item == new.id && o.damage == new.damage => {
            o.count = new.count;
            Some(o)
        }
        _ => {
            let mut it = ItemStack::new(&new.id, new.count);
            it.damage = new.damage;
            Some(it)
        }
    }
}

/// A block entity's server tick. The L1 furnace runs here (state, slots and
/// data through `ntm_core::native::tick`); any other machine is only
/// adopted by the server from its saved state (the world was opened again)
/// — the server's phases run it at the tick's end.
pub(super) fn tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let block = entity.id.clone();
    let state = match entity.nbt.get(&JavaString::from(STATE)) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    };
    if ntm_core::native::machine_block(&block) {
        if !state.is_empty() {
            let dim = game.dim.id().to_string();
            if let Some((server, _)) = lock(&SERVER).as_mut()
                && !server.native_has(&dim, pos)
            {
                server.native_adopt(&dim, pos, &state);
            }
        }
        return;
    }
    let Some(c) = ntm_core::native::container(&block) else { return };
    if entity.items.len() < c.slots {
        return;
    }
    let mut state = state;
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
        let (_, synced) = file(game, &block);
        if ntm_core::native::sync_changed(sent.as_deref(), &entity.data, &synced) {
            game.native_block_entity_changed(pos);
        }
        entity.nbt.insert(JavaString::from(DATA), Tag::IntArray(entity.data.clone()));
    }
    for (i, (new, old)) in items.iter().zip(&before).enumerate() {
        if new != old {
            entity.items[i] = item(new.as_ref(), entity.items[i].take());
        }
    }
}

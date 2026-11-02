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
use ntm_core::server::native_server::DETONATOR;
use ntm_core::server::{PlayerInfo, Server, ServerHost};
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
    ntm_core::native::machine_block(block)
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
    let ours = |b: &str| b.starts_with("ntm:");
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

/// A player's right click on a cell of a machine (`core` its core): the
/// machine may take it (the turbines' levers). True when it did.
pub(super) fn use_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], core: [i32; 3]) -> bool {
    let mut guard = lock(&SERVER);
    let Some((server, _)) = guard.as_mut() else { return false };
    let dim = game.dim.id().to_string();
    let player = game.native_actor_uuid().unwrap_or_default();
    let mut host = Host { game, buttons: Vec::new() };
    server.native_use(&mut host, &dim, core, pos, &player)
}

/// A player used `stack` on the block at `pos`: the fluid identifier on a
/// duct sets its fluid (`custom_data` `ntm_fluid`). True when taken.
pub(super) fn use_item(game: &mut crate::game::Game<'_>, stack: &ItemStack, pos: [i32; 3]) -> bool {
    if &*stack.item == DETONATOR {
        return detonator(game, Some(pos));
    }
    let fluid = stack.data().and_then(|d| {
        let (_, data) = d.other.iter().find(|(k, _)| k == "minecraft:custom_data")?;
        match data.get("ntm_fluid")? {
            Tag::String(s) => Some(s.to_string_lossy()),
            _ => None,
        }
    });
    let mut guard = lock(&SERVER);
    let Some((server, _)) = guard.as_mut() else { return false };
    server.native_use_item(game.dim.id(), pos, &stack.item, fluid.as_deref())
}

/// A player used `stack` in the air: the detonator sets its bomb off.
pub(super) fn use_item_air(game: &mut crate::game::Game<'_>, stack: &ItemStack) -> bool {
    &*stack.item == DETONATOR && detonator(game, None)
}

/// The detonator used by the acting player, on block `target` or in the
/// air (`Server::native_detonator`); they are told what it did.
fn detonator(game: &mut crate::game::Game<'_>, target: Option<[i32; 3]>) -> bool {
    let Some(player) = game.native_actor_uuid() else { return false };
    let mut guard = lock(&SERVER);
    let Some((server, _)) = guard.as_mut() else { return false };
    let mut host = Host { game, buttons: Vec::new() };
    let Some(said) = server.native_detonator(&mut host, &player, target) else { return false };
    host.tell(&player, &said);
    true
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

/// The world's state outside its blocks (`ntm_core::persist`, the
/// explosions in progress, the bombs' settings): a file per key in the
/// world's folder, `<world>/data/ntm/<key>`, each written whole.
impl Kv for Host<'_, '_> {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        std::fs::read(self.store(key)?).ok()
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        let path = self.store(key).ok_or_else(|| format!("key {key}"))?;
        let dir = path.parent().expect("under the world");
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        // A new file renamed over the old: a crash leaves one or the other.
        let tmp = dir.join(format!("{key}.tmp"));
        std::fs::write(&tmp, value).and_then(|()| std::fs::rename(&tmp, &path)).map_err(|e| e.to_string())
    }
    fn delete(&mut self, key: &str) {
        if let Some(p) = self.store(key) {
            let _ = std::fs::remove_file(p);
        }
    }
}

impl Host<'_, '_> {
    /// The file of storage key `key` (letters, digits, `.`, `_`, `-`); none
    /// without a world.
    fn store(&self, key: &str) -> Option<std::path::PathBuf> {
        let plain = !key.starts_with('.') && key.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b));
        let world = self.game.native_world_dir().filter(|_| plain && !key.is_empty())?;
        Some(world.join("data").join("ntm").join(key))
    }

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
    /// To the log: the server's thread has no helper for a player's chat.
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
    fn redstone_many(&mut self, dim: &str, cells: &[[i32; 3]]) -> Vec<Option<u8>> {
        self.game.native_redstone_many(dim, cells)
    }
    /// The living players, by UUID (API 28).
    fn players(&mut self) -> Option<Vec<PlayerInfo>> {
        let players = self.game.native_players().into_iter().filter(|p| p.alive);
        Some(
            players
                .map(|p| PlayerInfo { id: p.uuid, name: p.name, dim: p.dimension, pos: p.pos, creative: p.creative_or_spectator })
                .collect(),
        )
    }
    fn inventory(&mut self, id: &str) -> Option<Vec<(String, u32)>> {
        Some(self.game.native_player_inventory(id)?.into_iter().map(|c| (c.item, c.count)).collect())
    }
    fn hurt(&mut self, id: &str, amount: f32, kind: &str) {
        if let Err(e) = self.game.native_hurt(id, amount, kind) {
            eprintln!("ntm: hurt {id}: {e}");
        }
    }
    fn effect(&mut self, id: &str, effect: &str, seconds: u32, amplifier: u8) {
        if let Err(e) = self.game.native_effect(id, effect, seconds * 20, amplifier, true) {
            eprintln!("ntm: effect {id}: {e}");
        }
    }
    /// Particles and sounds in the dimension in play (`native_send_particles`,
    /// API 11; `native_play_sound`, API 32).
    fn show(&mut self, dim: &str, show: &ntm_core::blast::visuals::Show) {
        use horizon_client::particles::{ParticleOptions, ParticleType};
        use ntm_core::blast::visuals::Show;
        if dim != self.game.dim.id() {
            return;
        }
        let (what, sent) = match show {
            Show::Particles { id, color, at, spread, speed, count } => {
                let Some(ty) = ParticleType::from_name(id) else { return };
                let mut options = ParticleOptions::simple(ty);
                options.color = *color;
                (id, self.game.with_level(|_, level, _| level.native_send_particles(&options, None, *at, *spread, *speed, *count)))
            }
            Show::Sound { id, source, at, volume, pitch } => {
                let Some(source) = horizon_client::settings::SOUND_SOURCES.iter().position(|s| s == source) else { return };
                (id, self.game.with_level(|_, level, _| level.native_play_sound(id, *at, source, *volume, *pitch)))
            }
            Show::Effect { .. } => return,
        };
        if !sent {
            eprintln!("ntm: {what} refused");
        }
    }
    fn set_player_value(&mut self, id: &str, key: &str, value: i64) {
        if let Err(e) = self.game.native_set_player_value(id, key, value) {
            eprintln!("ntm: value {key} of {id}: {e}");
        }
    }
    fn save_machine(&mut self, dim: &str, p: [i32; 3], json: &str) {
        self.entity(dim, p, |e| {
            e.nbt.insert(JavaString::from(STATE), Tag::String(JavaString::from(json)));
        });
    }
}

// ----------------------------------------------------- block entities

/// A new machine block's block entity, empty: the server keeps the
/// machine's state in it.
pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && is_machine(block) {
        *entity = Some((block.to_string(), Compound::new()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if is_machine(block) {
        *ticks = true;
    }
}

pub(super) fn container(_game: &crate::game::Game<'_>, block: &str, container: &mut Option<ModContainer>) {
    let Some(c) = ntm_core::native::container(block) else { return };
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
pub(super) fn slot_filters(_game: &crate::game::Game<'_>, block: &str, filters: &mut Vec<ModSlotFilter>) {
    let rules = ntm_core::native::slot_filters(block);
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

/// A block entity's server tick: the machine is only adopted by the server
/// from its saved state when the server doesn't know it (the world was
/// opened again); the server's phases run it at the tick's end.
pub(super) fn tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if !is_machine(&entity.id) {
        return;
    }
    let Some(Tag::String(state)) = entity.nbt.get(&JavaString::from(STATE)) else { return };
    let dim = game.dim.id().to_string();
    if let Some((server, _)) = lock(&SERVER).as_mut()
        && !server.native_has(&dim, pos)
    {
        server.native_adopt(&dim, pos, &state.to_string_lossy());
    }
}

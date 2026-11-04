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
static POST_SYNC:Mutex<Vec<(ntm_core::server::CellGuard,ntm_core::server::CellSnapshot)>>=Mutex::new(Vec::new());
static BUTTONS: Mutex<Vec<(String, String, [i32; 3], u32)>> = Mutex::new(Vec::new());

/// Confirmed role writes already installed the model synchronously. Tag only
/// their owner cells, so external breaks (including A→air→A) keep their identity.
static OWNED_BLOCKS:Mutex<Vec<(String,[i32;3],String)>>=Mutex::new(Vec::new());
struct OwnedBlockScope(Vec<(String,[i32;3],String)>);
impl OwnedBlockScope {
    fn new(cells:Vec<(String,[i32;3],String)>)->Self{Self(std::mem::replace(&mut *lock(&OWNED_BLOCKS),cells))}
}
impl Drop for OwnedBlockScope {
    fn drop(&mut self){*lock(&OWNED_BLOCKS)=std::mem::take(&mut self.0);}
}

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
    lock(&POST_SYNC).clear();
    if !open {
        // The close/failure hook follows the final save. These are session caches, not facts of the next world.
        lock(&LEDGERS).clear();
        lock(&TRACKED).clear();
        lock(&AWAY).clear();
        lock(&ENTITIES_READY).clear();
        lock(&CHUNKS_FRESH).clear();
        lock(&ENTITIES_UNLOADED).clear();
    }
    if open {
        eprintln!("ntm native: {} machine blocks", ntm_core::registry::machine_ids().count());
    }
}

/// A flushed world save makes already applied item/block effects durable.
pub(super) fn world_saved(game:&mut crate::game::Game<'_>,wait:bool){
    let changes=std::mem::take(&mut *lock(&CHANGES));
    if wait{ledger_confirm(game);}
    if let Some((server,_))=lock(&SERVER).as_mut(){
        let mut host=Host{game,buttons:Vec::new()};
        for (dim,pos,old,new) in changes {
            let now=ntm_core::server::native_server::native_change_state(&new,host.game.native_loaded_block(&dim,pos));
            server.native_block_changed(&dim,pos,&old,&now);
        }
        // A quit/save between ticks must not forget a removed core's hidden
        // contents. Record tombstones only; do not tick or emit after saving.
        server.native_record_events(&mut host);
        server.native_world_saved(&mut host,wait);
    }
}

/// A battery, a fluid tank and a barrel give a comparator a signal (`hasComparatorInputOverride`).
pub(super) fn has_analog_output(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, output: &mut bool) {
    if ntm_core::native::has_comparator(&level.facts.get(state).name) { *output = true; }
}

/// `getComparatorInputOverride` from the machine's saved state, which the server commits each tick.
pub(super) fn analog_output(level: &crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos, state: horizon_world::registry::BlockStateId, output: &mut i32) {
    if !ntm_core::native::has_comparator(&level.facts.get(state).name) { return; }
    let Some(horizon_client::containers::BlockEntity::Mod(m)) = level.world.block_entities.get(&(pos.x, pos.y, pos.z)) else { return };
    if let Some(Tag::String(json)) = m.nbt.get(&JavaString::from(STATE)) {
        *output = i32::from(ntm_core::native::comparator_signal(&json.to_string_lossy()));
    }
}

/// The comparator signal a machine's saved JSON gives (0 for any block without one).
fn comparator_of(block: &str, json: Option<&JavaString>) -> u8 {
    if !ntm_core::native::has_comparator(block) { return 0; }
    json.map_or(0, |j| ntm_core::native::comparator_signal(&j.to_string_lossy()))
}

/// A block changed on the server: kept when a machine or cable is in it.
pub(super) fn block_changed(level: &crate::game::GameLevel<'_>, dim: &str, pos: [i32; 3], old: horizon_world::registry::BlockStateId, new: horizon_world::registry::BlockStateId) {
    let (old, new) = (level.facts.get(old).name.to_string(), level.facts.get(new).name.to_string());
    if trace_effects() && ["minecraft:oak_fence","minecraft:chest","minecraft:oak_log","minecraft:oak_planks"].contains(&old.as_str()){
        eprintln!("ntm trace block {}",serde_json::json!({"tick":level.world.game_time,"dim":dim,"pos":pos,"old":old,"new":new}));
    }
    let mut owned=lock(&OWNED_BLOCKS);
    if let Some(i)=owned.iter().position(|(d,p,n)|d==dim&&*p==pos&&n==ntm_core::blockstate::id_of(&new)) {owned.remove(i);return;}
    drop(owned);
    // Arbitrary cells can be guarded mining/role-effect destinations too.
    // The core ignores untracked vanilla cells without retaining an epoch.
    if old != new {
        lock(&POST_SYNC).retain(|(g,_)|g.dim!=dim||g.pos!=pos);
        lock(&CHANGES).push((dim.to_string(), pos, old, new));
    }
}

fn trace_effects()->bool{static TRACE:OnceLock<bool>=OnceLock::new();*TRACE.get_or_init(||std::env::var_os("NTM_TRACE_EFFECTS").is_some())}
/// Existing API 22, observed without editing its toBlow list or RNG.
pub(super) fn trace_explosion(game:&crate::game::Game<'_>,dim:&str,center:[f64;3],blocks:&[[i32;3]]){
    if !trace_effects(){return;}
    let targets:Vec<_>=blocks.iter().filter_map(|p|game.native_loaded_block(dim,*p).filter(|b|["minecraft:oak_fence","minecraft:chest","minecraft:oak_log","minecraft:oak_planks"].contains(&ntm_core::blockstate::id_of(b))).map(|b|(*p,b))).collect();
    eprintln!("ntm trace explosion plan {}",serde_json::json!({"tick":game.world.game_time,"dim":dim,"center":center,"to_blow":blocks.len(),"wood_targets":targets}));
}

/// A menu button on a machine's screen.
pub(super) fn menu_button(game:&crate::game::Game<'_>,pos:[i32;3],id:i32,entity:&ModBlockEntity){
    let Some(Tag::String(json))=entity.nbt.get(&JavaString::from(STATE)) else{return};
    let Ok(machine)=serde_json::from_str::<ntm_core::machine::Machine>(&json.to_string_lossy()) else{return};
    let mut server_lock=lock(&SERVER);
    let Some((server,_))=server_lock.as_mut() else{return};
    if let Ok(button)=u32::try_from(id){server.native_menu_button(&game.native_actor_uuid().unwrap_or_default(),game.dim.id(),pos,button,machine.generation);}
}

/// Input callbacks may precede ServerTickEnd. Consume their external facts
/// before capturing a machine generation or retrying any old world effect.
fn reconcile(server:&mut Server,host:&mut Host<'_, '_>){
    for (dim,pos,old,new) in std::mem::take(&mut *lock(&CHANGES)){
        let now=ntm_core::server::native_server::native_change_state(&new,host.game.native_loaded_block(&dim,pos));
        server.native_block_changed(&dim,pos,&old,&now);
    }
    server.native_record_events(host);
}

/// A player's right click on a cell of a machine (`core` its core): the
/// machine may take it (the turbines' levers). True when it did.
pub(super) fn use_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], core: [i32; 3], face: [i32; 3], sneaking: bool) -> bool {
    if crate::game::ntm_armor_table::open(game,pos,sneaking){return true;}
    let mut guard = lock(&SERVER);
    let Some((server, _)) = guard.as_mut() else { return false };
    let dim = game.dim.id().to_string();
    let player = game.native_actor_uuid().unwrap_or_default();
    let held = game.held().cloned();
    let pile_stack = stack(&held);
    let depletion = held.as_ref().map_or(0.0, |h| pile_depletion(h.data().and_then(|d| d.other("minecraft:custom_data"))));
    let mut host = Host { game, buttons: Vec::new() };
    reconcile(server,&mut host);
    if let Some(action) = server.native_pile_use(&mut host, &dim, pos, face, pile_stack.as_ref(), depletion, &player) {
        match action {
            ntm_core::server::PileUse::ConsumeRod => {
                if let Some(h) = host.game.held_mut() { h.count -= 1; if h.count == 0 { *host.game.held_mut() = None; } }
            }
            ntm_core::server::PileUse::WearDrill => {
                if held.as_ref().is_some_and(|h| &*h.item == "ntm:hand_drill") { host.game.damage_held_tool(); }
            }
            ntm_core::server::PileUse::Handled => {}
        }
        return true;
    }
    let game = host.game;
    if let Some(held) = game.held().cloned() {
        if &*held.item == "ntm:mirror_tool" {
            if let Some(target) = server.native_solar_target(&dim, core) {
                let mut data = held.data().and_then(|d| d.other("minecraft:custom_data")).and_then(|t| match t { Tag::Compound(c) => Some(c.clone()), _ => None }).unwrap_or_default();
                for (key,value) in ["posX","posY","posZ"].into_iter().zip(target) { data.insert(JavaString::from(key), Tag::Int(value)); }
                if let Some(h) = game.held_mut() { h.data_mut().set_other("minecraft:custom_data", Some(Tag::Compound(data))); }
                return true;
            }
            if let Some(Tag::Compound(data)) = held.data().and_then(|d| d.other("minecraft:custom_data")) {
                let target = ["posX","posY","posZ"].map(|key| match data.get(&JavaString::from(key)) { Some(Tag::Int(n)) => *n, _ => 0 });
                if let Some(result) = server.native_mirror_link(&dim,core,target) {
                    if let Err(text) = result { eprintln!("ntm: {player}: {text}"); }
                    return true;
                }
            }
        }
        if server.native_stirling_gear(&dim,core) == Some(&*held.item) {
            let mut host=Host{game,buttons:Vec::new()};
            if server.native_stirling_repair(&mut host,&dim,core,&held.item)
                && let Some(h)=host.game.held_mut()
            {
                h.count-=1;
                if h.count==0{*host.game.held_mut()=None;}
            }
            return true;
        }
    }
    let mut host = Host { game, buttons: Vec::new() };
    server.native_use(&mut host, &dim, core, pos, &player)
}

/// A player used `stack` on the block at `pos`: the fluid identifier on a
/// duct sets its fluid (`custom_data` `ntm_fluid`). True when taken.
pub(super) fn use_item(game: &mut crate::game::Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3]) -> bool {
    if matches!(&*stack.item,"ntm:hand_drill"|"ntm:hand_drill_desh") { return use_block(game,pos,pos,face,false); }
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
    if crate::game::ntm_items::use_air(game,stack){return true;}
    &*stack.item == DETONATOR && detonator(game, None)
}

/// The detonator used by the acting player, on block `target` or in the
/// air (`Server::native_detonator`); they are told what it did.
fn detonator(game: &mut crate::game::Game<'_>, target: Option<[i32; 3]>) -> bool {
    let Some(player) = game.native_actor_uuid() else { return false };
    let mut guard = lock(&SERVER);
    let Some((server, _)) = guard.as_mut() else { return false };
    let mut host = Host { game, buttons: Vec::new() };
    eprintln!("ntm: detonator input: player={player} tick={} hand={:?} target={target:?}",host.game.ticks,host.game.hand);
    reconcile(server,&mut host);
    let Some(said) = server.native_detonator(&mut host, &player, target) else { return false };
    if !said.is_empty(){host.tell(&player, &said);}
    true
}

/// Use the engine's registered server command/permission/feedback path.
pub(super) fn command(game:&mut crate::game::Game<'_>,player:&horizon_client::mod_api::NativeServerPlayer,line:&str)->bool {
    if crate::game::ntm_items::command(game,line){return true;}
    if line.split_whitespace().next()==Some("ntmledger"){
        for text in ledger_report(game,&line.split_whitespace().skip(1).collect::<Vec<_>>()).lines(){game.native_feedback(horizon_client::text::Text::literal(text));}
        return true;
    }
    if line.split_whitespace().next()!=Some("ntmbomba"){return false;}
    let who=PlayerInfo{id:game.native_actor_uuid().unwrap_or_else(||player.uuid.to_string()),name:player.name.clone(),dim:player.dimension.clone(),pos:player.position,creative:player.game_mode==1};
    let mut guard=lock(&SERVER);
    let mut host=Host{game,buttons:Vec::new()};
    let (server,_)=guard.get_or_insert_with(||(Server::native(&mut host,0),0));
    reconcile(server,&mut host);
    if let Some(reply)=server.native_bomb_command(&mut host,&who,player.permission,line){
        let text=reply.unwrap_or_else(|e|format!("Erro: {e}"));
        for line in text.lines(){host.game.native_feedback(horizon_client::text::Text::literal(line));}
        return true;
    }
    false
}

// ----------------------------------------------------------------- the ledger
// What machines owe the world is kept in the attachments of the chunk of the cell that owes it (see
// `ntm_core::ledger`): written with the chunk, so it is on disk exactly as the block entity that paid it is.

static LEDGER_TYPE: horizon_world::attachments::AttachmentType<String> = horizon_world::attachments::AttachmentType {
    namespace: "ntm",
    name: "ledger",
    default: String::new,
    codec: Some(horizon_world::attachments::AttachmentCodec {
        encode: |v| Ok([("v".into(), Tag::String(JavaString::from(v.as_str())))].into()),
        decode: |c| match c.get(&"v".into()) {
            Some(Tag::String(s)) => Ok(s.to_string_lossy()),
            _ => Err(horizon_world::attachments::AttachmentError::Decode),
        },
    }),
    copy_on_death: false,
    sync: false,
};
/// Names the effect an item entity belongs to, saved with the entity: how a reopened world recognises it.
static EFFECT_TYPE: horizon_world::attachments::AttachmentType<String> = horizon_world::attachments::AttachmentType {
    namespace: "ntm",
    name: "effect",
    default: String::new,
    codec: Some(horizon_world::attachments::AttachmentCodec {
        encode: |v| Ok([("v".into(), Tag::String(JavaString::from(v.as_str())))].into()),
        decode: |c| match c.get(&"v".into()) {
            Some(Tag::String(s)) => Ok(s.to_string_lossy()),
            _ => Err(horizon_world::attachments::AttachmentError::Decode),
        },
    }),
    copy_on_death: false,
    sync: false,
};

/// Writes of the host's storage and their bytes, for `/ntmledger`: a batch of effects must not cost one write each.
static KV_WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static KV_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

type ChunkKey = (String, i32, i32);
/// The ledgers of the loaded chunks that have one, as the chunks hold them.
static LEDGERS: Mutex<std::collections::BTreeMap<ChunkKey, ntm_core::ledger::ChunkLedger>> = Mutex::new(std::collections::BTreeMap::new());
/// The item entities of each spawned entry (dimension, effect id, stack index), with the chunk each was last seen in.
static TRACKED: Mutex<std::collections::BTreeMap<(String, u64, u32), Vec<(u32, [i32; 2])>>> = Mutex::new(std::collections::BTreeMap::new());
/// Chunks whose saved entities have joined the world.
static ENTITIES_READY: Mutex<std::collections::BTreeSet<ChunkKey>> = Mutex::new(std::collections::BTreeSet::new());
/// Chunks whose entities left the world to be saved since the last tick: an entity that is gone from there is not paid.
/// Chunks that came into the world, with the server ticks since: a chunk with no saved entities never tells
/// that its entities joined, so two ticks after it came they are known to be all there.
static CHUNKS_FRESH: Mutex<std::collections::BTreeMap<ChunkKey, u8>> = Mutex::new(std::collections::BTreeMap::new());
static ENTITIES_UNLOADED: Mutex<std::collections::BTreeSet<ChunkKey>> = Mutex::new(std::collections::BTreeSet::new());

fn chunk_key(dim: &str, chunk: [i32; 2]) -> ChunkKey {
    (dim.to_string(), chunk[0], chunk[1])
}

/// The ledger the chunk's attachments hold. Read from their saved form, so that loading a chunk does not
/// mark it to be saved again.
fn ledger_load(g: &mut crate::game::Game<'_>, chunk: [i32; 2]) -> ntm_core::ledger::ChunkLedger {
    let json = g
        .world
        .chunks
        .get(&(chunk[0], chunk[1]))
        .and_then(|c| c.saved.as_ref())
        .map(|extras| extras.attachments.to_nbt())
        .and_then(|nbt| match nbt.get(&JavaString::from("ntm"))? {
            Tag::Compound(names) => match names.get(&JavaString::from("ledger"))? {
                // The saved form of an attachment: `{value: <codec>}`.
                Tag::Compound(saved) => match saved.get(&JavaString::from("value"))? {
                    Tag::Compound(value) => match value.get(&JavaString::from("v"))? {
                        Tag::String(text) => Some(text.to_string_lossy()),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        });
    json.map(|j| ntm_core::ledger::ChunkLedger::from_json(&j)).unwrap_or_default()
}

/// Writes the ledger into the chunk's attachments (and keeps it for the ticks), on the level in place.
fn ledger_store(g: &mut crate::game::Game<'_>, key: &ChunkKey, ledger: ntm_core::ledger::ChunkLedger) {
    use horizon_client::mod_api::Holder;
    if let Ok(map) = g.attachment_map_quiet(Holder::Chunk([key.1, key.2])) {
        let _ = map.set(&LEDGER_TYPE, if ledger.is_empty() { String::new() } else { ledger.to_json() });
    }
    let mut ledgers = lock(&LEDGERS);
    if ledger.is_empty() {
        ledgers.remove(key);
    } else {
        ledgers.insert(key.clone(), ledger);
    }
}

fn ledger_get(g: &mut crate::game::Game<'_>, key: &ChunkKey) -> ntm_core::ledger::ChunkLedger {
    let cached = lock(&LEDGERS).get(key).cloned();
    cached.unwrap_or_else(|| ledger_load(g, [key.1, key.2]))
}

/// Owes `items` (SNBT stacks) at `pos` for the effect `id`, on the level in place. True when the effect is
/// owed now or was before; false when its chunk is not loaded (the effect is retried).
fn ledger_add_here(g: &mut crate::game::Game<'_>, dim: &str, id: u64, pos: [i32; 3], items: Vec<String>) -> bool {
    if !g.world.loaded(pos[0], pos[2]) {
        return false;
    }
    let key = chunk_key(dim, [pos[0] >> 4, pos[2] >> 4]);
    let mut ledger = ledger_get(g, &key);
    if ledger.apply(id, pos, items) {
        ledger_store(g, &key, ledger);
        // The debt is in the chunk's record; in this same exclusive step it becomes entities, as a vanilla drop
        // does (no tick between them). A chunk whose entities are not in yet keeps it pending for the next tick.
        ledger_chunk(g, &key, &std::collections::BTreeSet::new());
    }
    true
}

fn ledger_add(game: &mut crate::game::Game<'_>, dim: &str, id: u64, pos: [i32; 3], items: Vec<String>) -> bool {
    in_place(game, dim, |g| ledger_add_here(g, dim, id, pos, items)).unwrap_or(false)
}

fn item_snbt(stack: &ItemStack) -> String {
    horizon_world::snbt_text::to_snbt(&horizon_client::save::item_to_nbt(stack))
}

/// Forgets the paid ids the storage no longer names, in every loaded chunk that keeps some.
fn ledger_prune(game: &mut crate::game::Game<'_>, live: &std::collections::BTreeSet<u64>) {
    let keys: Vec<ChunkKey> = lock(&LEDGERS).iter().filter(|(_, l)| !l.applied.is_empty() || !l.gone.is_empty()).map(|(k, _)| k.clone()).collect();
    for key in keys {
        in_place(game, &key.0.clone(), |g| {
            let mut ledger = ledger_get(g, &key);
            if ledger.prune(live) {
                ledger_store(g, &key, ledger);
            }
        });
    }
}

/// The chunk whose position `p` is in.
fn chunk_at(p: &horizon_core::geometry::Vec3) -> [i32; 2] {
    [(p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4]
}

/// The tag a loaded item entity was saved with, if it is one of ours.
fn entity_effect(g: &mut crate::game::Game<'_>, entity: u32) -> Option<String> {
    let uuid = *g.persist.uuids.get(&entity)?;
    g.entity_attachments.get_mut(&uuid)?.get_existing(&EFFECT_TYPE).ok().flatten().filter(|t| !t.is_empty())
}

/// Where the entities of an entry last were seen when they left the world with a chunk that unloaded.
static AWAY: Mutex<std::collections::BTreeMap<(String, u64, u32), Vec<[i32; 2]>>> = Mutex::new(std::collections::BTreeMap::new());

fn chunk_ready(g: &mut crate::game::Game<'_>, dim: &str, c: [i32; 2]) -> bool {
    g.world.loaded(c[0] * 16, c[1] * 16) && lock(&ENTITIES_READY).contains(&chunk_key(dim, c))
}

/// The loaded item entities that carry `tag`.
fn tagged_entities(g: &mut crate::game::Game<'_>, tag: &str) -> Vec<(u32, [i32; 2])> {
    let ids: Vec<(u32, [i32; 2])> = g.dropped.iter().map(|d| (d.id, chunk_at(&d.position))).collect();
    ids.into_iter().filter(|(id, _)| entity_effect(g, *id).as_deref() == Some(tag)).collect()
}

/// Every owed stack of one chunk: spawn what is pending, retire what was paid, find again after a reopen what
/// was spawned, and spawn again what a reopened world proves never reached its files.
fn ledger_chunk(g: &mut crate::game::Game<'_>, key: &ChunkKey, unloaded: &std::collections::BTreeSet<ChunkKey>) {
    if !lock(&ENTITIES_READY).contains(key) || !g.world.loaded(key.1 * 16, key.2 * 16) {
        return;
    }
    let mut ledger = ledger_get(g, key);
    let mut changed = false;
    for entry in ledger.entries.clone() {
        let tkey = (key.0.clone(), entry.id, entry.idx);
        let tag = ntm_core::ledger::entity_tag(entry.id, entry.idx, [key.1, key.2]);
        let mut state = entry.state;
        if state == ntm_core::ledger::State::Spawned {
            let tracked = lock(&TRACKED).get(&tkey).cloned();
            if let Some(list) = tracked {
                let alive: Vec<(u32, [i32; 2])> = list
                    .iter()
                    .filter_map(|(id, _)| g.dropped.iter().find(|d| d.id == *id && d.stack.count > 0).map(|d| (d.id, chunk_at(&d.position))))
                    .collect();
                // Those that are not in the world: put away with a chunk that unloaded, or gone (picked up, merged, burnt).
                let away: Vec<[i32; 2]> = list
                    .iter()
                    .filter(|(id, _)| !alive.iter().any(|(a, _)| a == id))
                    .map(|(_, c)| *c)
                    .filter(|c| unloaded.contains(&chunk_key(&key.0, *c)) || !g.world.loaded(c[0] * 16, c[1] * 16))
                    .collect();
                if alive.is_empty() {
                    lock(&TRACKED).remove(&tkey);
                    if away.is_empty() {
                        ledger.retire(entry.id, entry.idx);
                        changed = true;
                    } else {
                        // Its chunk took it to its files: wait for that chunk, then look for it by its tag.
                        lock(&AWAY).insert(tkey, away);
                    }
                } else {
                    // Merged with others or split on the ground: what they hold is what is owed.
                    let held: u32 = alive.iter().filter_map(|(id, _)| g.dropped.iter().find(|d| d.id == *id)).map(|d| d.stack.count).sum();
                    changed |= ledger.set_live(entry.id, entry.idx, held);
                    if let Some((_, c)) = alive.first() {
                        changed |= ledger.set_last(entry.id, entry.idx, *c);
                    }
                    if !away.is_empty() {
                        lock(&AWAY).insert(tkey.clone(), away);
                    }
                    lock(&TRACKED).insert(tkey, alive);
                }
                continue;
            }
            // Not followed in this session: reopened, or taken to a chunk's files while it was followed.
            let waiting = lock(&AWAY).get(&tkey).cloned();
            if let Some(chunks) = waiting {
                if !chunks.iter().all(|c| chunk_ready(g, &key.0, *c)) {
                    continue;
                }
                lock(&AWAY).remove(&tkey);
            }
            let found = tagged_entities(g, &tag);
            if !found.is_empty() {
                for d in g.dropped.iter_mut().filter(|d| found.iter().any(|(id, _)| *id == d.id)) {
                    d.merge_key = ntm_core::ledger::merge_key([key.1, key.2]);
                }
                // The entities may be older than the record (a crash between their files): what the record says is
                // owed is what the world must hold.
                let owed = if entry.live > 0 { entry.live } else { item_count(&entry.item) };
                let held: u32 = found.iter().filter_map(|(id, _)| g.dropped.iter().find(|d| d.id == *id)).map(|d| d.stack.count).sum();
                if held < owed
                    && let Some(d) = g.dropped.iter_mut().find(|d| found.iter().any(|(id, _)| *id == d.id))
                {
                    d.stack.count += owed - held;
                } else if held > owed {
                    changed |= ledger.set_live(entry.id, entry.idx, held);
                }
                if let Some((_, c)) = found.first() {
                    changed |= ledger.set_last(entry.id, entry.idx, *c);
                }
                lock(&TRACKED).insert(tkey, found);
                continue;
            }
            // Not in the world. It never reached the files only if the chunk it was last in has its entities in.
            let last = entry.last.unwrap_or([key.1, key.2]);
            if !chunk_ready(g, &key.0, last) {
                continue;
            }
            state = ntm_core::ledger::State::Pending;
        }
        if state == ntm_core::ledger::State::Pending {
            // Another run may have put it into a chunk's files already: that one is the stack, not a second.
            let existing = tagged_entities(g, &tag);
            if !existing.is_empty() {
                for d in g.dropped.iter_mut().filter(|d| existing.iter().any(|(id, _)| *id == d.id)) {
                    d.merge_key = ntm_core::ledger::merge_key([key.1, key.2]);
                }
                lock(&TRACKED).insert(tkey, existing);
                ledger.mark_spawned(entry.id, entry.idx);
                changed = true;
                continue;
            }
            let parsed = horizon_world::nbt::parse_snbt(&entry.item).ok().and_then(|(t, _)| horizon_client::save::item_from_nbt(&t));
            let Some(mut stack) = parsed else {
                eprintln!("ntm: invalid owed item NBT {:?}", entry.item);
                continue;
            };
            if entry.live > 0 {
                stack.count = entry.live;
            }
            let at = entry.at.map(|v| f64::from(v) + 0.5);
            g.spawn_item(stack, at, crate::game::dispenser::new_item_motion(), 0);
            let spawned = g.just_spawned_item().map(|d| {
                d.merge_key = ntm_core::ledger::merge_key([key.1, key.2]);
                (d.id, chunk_at(&d.position))
            });
            if let Some((eid, chunk)) = spawned {
                use horizon_client::mod_api::Holder;
                if let Ok(map) = g.attachment_map_quiet(Holder::Entity(eid)) {
                    let _ = map.set(&EFFECT_TYPE, tag);
                }
                lock(&TRACKED).insert(tkey, vec![(eid, chunk)]);
                ledger.set_last(entry.id, entry.idx, chunk);
            }
            ledger.mark_spawned(entry.id, entry.idx);
            changed = true;
        }
    }
    if changed {
        ledger_store(g, key, ledger);
    }
}

fn item_count(item: &str) -> u32 {
    horizon_world::nbt::parse_snbt(item).ok().and_then(|(t, _)| horizon_client::save::item_from_nbt(&t)).map_or(0, |s| s.count)
}

/// What the dropped item entities of the level in place say about the ledger, with the level's dimension:
/// older copies of a stack the ledger saw go are removed (the entity files lag the chunk record after a crash, and the
/// stack has merged or been taken since), a second entity of one tag is the stale one, and the tag of a stack that was
/// released (or that no ledger knows) is cleared, so the item merges again as any other.
fn ledger_sweep(g: &mut crate::game::Game<'_>, dim: &str) {
    let ledgers: std::collections::BTreeMap<ChunkKey, ntm_core::ledger::ChunkLedger> =
        lock(&LEDGERS).iter().filter(|(k, _)| k.0 == dim).map(|(k, v)| (k.clone(), v.clone())).collect();
    let ids: Vec<(u32, [i32; 2])> = g.dropped.iter().map(|d| (d.id, chunk_at(&d.position))).collect();
    let mut seen: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for (id, chunk) in ids {
        if !lock(&ENTITIES_READY).contains(&chunk_key(dim, chunk)) {
            continue;
        }
        let Some(tag) = entity_effect(g, id) else { continue };
        let Some((effect, idx, origin)) = ntm_core::ledger::parse_tag(&tag) else { continue };
        let okey = chunk_key(dim, origin);
        let ledger = ledgers.get(&okey);
        let owed = ledger.is_some_and(|l| l.entries.iter().any(|e| e.id == effect && e.idx == idx));
        let gone = ledger.is_some_and(|l| l.is_gone(effect, idx));
        let remove = if gone && !owed {
            true
        } else if owed {
            // Two entities of one tag: the one this session follows is the stack.
            match seen.get(&tag).copied() {
                Some(first) => {
                    let followed = lock(&TRACKED).get(&(dim.to_string(), effect, idx)).is_some_and(|l| l.iter().any(|(e, _)| *e == id));
                    if followed {
                        if let Some(i) = g.dropped.iter().position(|d| d.id == first) {
                            g.remove_dropped(i);
                        }
                        seen.insert(tag.clone(), id);
                        false
                    } else {
                        true
                    }
                }
                None => {
                    seen.insert(tag.clone(), id);
                    false
                }
            }
        } else {
            // No entry and not gone: released by a confirmed save. When the chunk that owned it is in, the tag is spent.
            if chunk_ready(g, dim, origin) {
                use horizon_client::mod_api::Holder;
                if let Ok(map) = g.attachment_map_quiet(Holder::Entity(id)) {
                    let _ = map.set(&EFFECT_TYPE, String::new());
                }
                if let Some(d) = g.dropped.iter_mut().find(|d| d.id == id) {
                    d.merge_key = 0;
                }
            }
            false
        };
        if remove && let Some(i) = g.dropped.iter().position(|d| d.id == id) {
            g.remove_dropped(i);
        }
    }
}

/// A confirmed world save (`wait`: `entities/` and `region/` are on disk) keeps every item entity the ledger
/// follows: from here the world's own files hold them as they hold any drop, and the ledger lets go. They
/// merge as vanilla's do again.
fn ledger_confirm(game: &mut crate::game::Game<'_>) {
    let keys: Vec<ChunkKey> = lock(&LEDGERS).iter().filter(|(_, l)| l.entries.iter().any(|e| e.state == ntm_core::ledger::State::Spawned)).map(|(k, _)| k.clone()).collect();
    for key in keys {
        in_place(game, &key.0.clone(), |g| {
            let mut ledger = ledger_get(g, &key);
            let mut changed = false;
            for entry in ledger.entries.clone() {
                if entry.state != ntm_core::ledger::State::Spawned {
                    continue;
                }
                let tkey = (key.0.clone(), entry.id, entry.idx);
                let Some(list) = lock(&TRACKED).get(&tkey).cloned() else { continue };
                let alive: Vec<u32> = list.iter().filter(|(id, _)| g.dropped.iter().any(|d| d.id == *id && d.stack.count > 0)).map(|(id, _)| *id).collect();
                // Only entities whose chunk is loaded and ready were part of this save.
                if alive.is_empty() || alive.len() != list.len() {
                    continue;
                }
                for id in &alive {
                    if let Some(d) = g.dropped.iter_mut().find(|d| d.id == *id) {
                        d.merge_key = 0;
                    }
                    use horizon_client::mod_api::Holder;
                    if let Ok(map) = g.attachment_map_quiet(Holder::Entity(*id)) {
                        let _ = map.set(&EFFECT_TYPE, String::new());
                    }
                }
                lock(&TRACKED).remove(&tkey);
                ledger.release(entry.id, entry.idx);
                changed = true;
            }
            if changed {
                ledger_store(g, &key, ledger);
            }
        });
    }
}

/// `/ntmledger`: what the loaded chunks owe, and the entities that carry an owed stack, for proving a save.
fn ledger_report(game: &mut crate::game::Game<'_>, words: &[&str]) -> String {
    if words.first() == Some(&"machine") {
        // `ntmledger machine <x> <y> <z>`: whether the server holds a machine for the cell in the dimension in place.
        let at: Vec<i32> = words[1..].iter().filter_map(|w| w.parse().ok()).collect();
        if let [x, y, z] = at[..] {
            let dim = game.dim.id().to_string();
            let present = lock(&SERVER).as_mut().is_some_and(|(server, _)| server.native_has_machine(&dim, [x, y, z]));
            return format!("ntmledger machine={}", if present { "present" } else { "missing" });
        }
    }
    let count = |item: &str| -> i64 {
        horizon_world::nbt::parse_snbt(item).ok().and_then(|(t, _)| horizon_client::save::item_from_nbt(&t)).map_or(0, |s| i64::from(s.count))
    };
    let (mut pending, mut spawned, mut applied, mut gone, mut owed) = (0, 0, 0, 0, 0);
    let ledgers = lock(&LEDGERS).clone();
    for ledger in ledgers.values() {
        applied += ledger.applied.len();
        gone += ledger.gone.len();
        for entry in &ledger.entries {
            match entry.state {
                ntm_core::ledger::State::Pending => pending += 1,
                ntm_core::ledger::State::Spawned => spawned += 1,
            }
            owed += if entry.live > 0 { i64::from(entry.live) } else { count(&entry.item) };
        }
    }
    let mut dims: std::collections::BTreeSet<String> = ledgers.keys().map(|k| k.0.clone()).collect();
    dims.insert(game.dim.id().to_string());
    let (mut tagged, mut tagged_items, mut keyed) = (0, 0, 0);
    for dim in dims {
        let (t, ti, k) = in_place(game, &dim.clone(), |g| {
            let ids: Vec<u32> = g.dropped.iter().map(|d| d.id).collect();
            let (mut t, mut ti, mut k) = (0, 0, 0);
            for id in ids {
                let keyed_here = g.dropped.iter().find(|d| d.id == id).is_some_and(|d| d.merge_key != 0);
                k += i64::from(keyed_here);
                if entity_effect(g, id).is_some() {
                    t += 1;
                    ti += g.dropped.iter().find(|d| d.id == id).map_or(0, |d| i64::from(d.stack.count));
                }
            }
            (t, ti, k)
        })
        .unwrap_or((0, 0, 0));
        tagged += t;
        tagged_items += ti;
        keyed += k;
    }
    format!("ntmledger chunks={} pending={pending} spawned={spawned} applied={applied} gone={gone} owed_items={owed} tagged={tagged} tagged_items={tagged_items} keyed={keyed} kv_writes={} kv_bytes={}", ledgers.len(), KV_WRITES.load(std::sync::atomic::Ordering::Relaxed), KV_BYTES.load(std::sync::atomic::Ordering::Relaxed))
}

/// `ledger_sweep` in every dimension that has a ledger, and the one in place.
fn ledger_sweep_all(game: &mut crate::game::Game<'_>) {
    let mut dims: std::collections::BTreeSet<String> = lock(&LEDGERS).keys().map(|k| k.0.clone()).collect();
    dims.insert(game.dim.id().to_string());
    for dim in dims {
        in_place(game, &dim.clone(), |g| ledger_sweep(g, &dim));
    }
}

fn ledger_tick(game: &mut crate::game::Game<'_>) {
    {
        let mut fresh = lock(&CHUNKS_FRESH);
        if !fresh.is_empty() {
            let mut ready = lock(&ENTITIES_READY);
            fresh.retain(|key, ticks| {
                *ticks += 1;
                if *ticks >= 2 {
                    ready.insert(key.clone());
                }
                *ticks < 2
            });
        }
    }
    let keys: Vec<ChunkKey> = lock(&LEDGERS).iter().filter(|(_, l)| !l.entries.is_empty()).map(|(k, _)| k.clone()).collect();
    if keys.is_empty() {
        ledger_sweep_all(game);
        return;
    }
    let unloaded = std::mem::take(&mut *lock(&ENTITIES_UNLOADED));
    for key in keys {
        in_place(game, &key.0.clone(), |g| ledger_chunk(g, &key, &unloaded));
    }
    ledger_sweep_all(game);
}

/// A chunk loaded: its ledger, if it has one, is read and kept.
pub(super) fn chunk_loaded(game: &mut crate::game::Game<'_>, dim: &str, chunk: [i32; 2]) {
    if dim_of(dim) != Some(game.dim) {
        return;
    }
    lock(&CHUNKS_FRESH).insert(chunk_key(dim, chunk), 0);
    let ledger = ledger_load(game, chunk);
    if !ledger.is_empty() {
        lock(&LEDGERS).insert(chunk_key(dim, chunk), ledger);
    }
}

pub(super) fn chunk_unloaded(dim: &str, chunk: [i32; 2]) {
    let key = chunk_key(dim, chunk);
    // Its ledger is read again when it loads; what it followed is looked for by tag then.
    if let Some(ledger) = lock(&LEDGERS).remove(&key) {
        for e in &ledger.entries {
            lock(&TRACKED).remove(&(dim.to_string(), e.id, e.idx));
            lock(&AWAY).remove(&(dim.to_string(), e.id, e.idx));
        }
    }
    lock(&ENTITIES_READY).remove(&key);
    lock(&CHUNKS_FRESH).remove(&key);
}

pub(super) fn entities_loaded(game: &mut crate::game::Game<'_>, dim: &str, chunk: [i32; 2]) {
    lock(&ENTITIES_READY).insert(chunk_key(dim, chunk));
    if dim_of(dim) != Some(game.dim) {
        return;
    }
    // Drops a ledger followed come back marked, before any of them can merge.
    let ids: Vec<u32> = game.dropped.iter().filter(|d| chunk_at(&d.position) == chunk).map(|d| d.id).collect();
    for id in ids {
        let Some((_, _, origin)) = entity_effect(game, id).and_then(|t| ntm_core::ledger::parse_tag(&t)) else { continue };
        if let Some(d) = game.dropped.iter_mut().find(|d| d.id == id) {
            d.merge_key = ntm_core::ledger::merge_key(origin);
        }
    }
}

pub(super) fn entities_put_away(dim: &str, chunk: [i32; 2]) {
    let key = chunk_key(dim, chunk);
    lock(&ENTITIES_READY).remove(&key);
    lock(&ENTITIES_UNLOADED).insert(key);
}

/// The end of a server tick: the server's phases.
pub(super) fn server_tick(game: &mut crate::game::Game<'_>) {
    ledger_tick(game);
    let mut retry=Vec::new();
    for (origin,expected) in std::mem::take(&mut *lock(&POST_SYNC)){
        // Any identity change since this committed sync makes it obsolete.
        match cell_snapshot(game,&origin.dim,origin.pos){
            None=>retry.push((origin,expected)),
            Some(actual) if actual==expected=>{in_place(game,&origin.dim,|g|{g.world.modified.insert((origin.pos[0]>>4,origin.pos[2]>>4));g.native_block_entity_changed(origin.pos);});},
            Some(_)=>{},
        }
    }
    lock(&POST_SYNC).extend(retry);
    let mut guard = lock(&SERVER);
    let changes = std::mem::take(&mut *lock(&CHANGES));
    let buttons = std::mem::take(&mut *lock(&BUTTONS));
    let mut host = Host { game, buttons };
    let (server, tick) = guard.get_or_insert_with(|| (Server::native(&mut host, 0), 0));
    *tick += 1;
    for (dim, pos, old, new) in changes {
        // Preserve each break; enrich placement properties only while the
        // recorded new block is still there. Owned role writes were tagged at
        // their synchronous callback, so they cannot masquerade as removals.
        let now = ntm_core::server::native_server::native_change_state(&new,host.game.native_loaded_block(&dim,pos));
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
        KV_WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        KV_BYTES.fetch_add(value.len() as u64, std::sync::atomic::Ordering::Relaxed);
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

    /// The machine block entity at `pos` of `dim`, in whichever level it is.
    /// ServerTickEnd runs with the last ticked level in place, which is not
    /// every machine's: a parked level's block entities are reached in its
    /// world directly, without switching levels (nothing is opened either:
    /// a level that is not open has no machines to reach).
    fn entity<R>(&mut self, dim: &str, pos: [i32; 3], f: impl FnOnce(&mut ModBlockEntity) -> R) -> Option<R> {
        let target = dim_of(dim)?;
        if target == self.game.dim {
            return self.game.native_with_mod_block_entity(pos, f);
        }
        let key = (pos[0], pos[1], pos[2]);
        let world = self.game.levels.parked_world_mut(target)?;
        let Some(horizon_client::containers::BlockEntity::Mod(m)) = world.block_entities.get_mut(&key) else {
            return None;
        };
        if m.id.is_empty() {
            return None;
        }
        let before = m.items.clone();
        let out = f(m);
        let moved = m.items != before;
        // Its chunk is saved next time, as `setChanged` would.
        world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        if moved {
            // The other half of `setChanged`: the comparators reading it are told, in
            // that level (`native_with_mod_block_entity` does this for the one in place).
            // A passing visit: no level is opened and its idleness keeps counting.
            self.game.visit_level(target, |g| g.container_changed(key));
        }
        Some(out)
    }

    /// Whether a player is in `dim`'s level (a level with none has nobody to show or tell).
    fn watched(&self, dim: &str) -> bool {
        self.game.native_players().iter().any(|p| p.dimension == dim)
    }
}

/// The dimension named by `id`.
fn dim_of(id: &str) -> Option<crate::game::Dim> {
    use crate::game::Dim;
    match id {
        "minecraft:overworld" => Some(Dim::Overworld),
        "minecraft:the_nether" => Some(Dim::Nether),
        "minecraft:the_end" => Some(Dim::End),
        _ => None,
    }
}

/// Run `f` with `dim`'s level in place, for what only the level in place can do
/// (spawn an item, explode, tell its players): the engine's passing visit
/// (`visit_level`), which opens nothing, puts the level back also when `f`
/// panics and leaves no trace (particles, idleness). None: `dim` is not open.
fn in_place<R>(game: &mut crate::game::Game<'_>, dim: &str, f: impl FnOnce(&mut crate::game::Game<'_>) -> R) -> Option<R> {
    game.visit_level(dim_of(dim)?, f)
}

/// Snapshot the whole entity, including item components and custom data.
fn cell_snapshot(game:&crate::game::Game<'_>,dim:&str,pos:[i32;3])->Option<ntm_core::server::CellSnapshot>{
    let state=game.native_loaded_block(dim,pos)?;
    let target=dim_of(dim)?;
    let world=if target==game.dim{&game.world}else{game.levels.parked_world(target)?};
    let entity=world.block_entities.get(&(pos[0],pos[1],pos[2]));
    let items=entity.map(|e|e.contents().iter().map(|i|horizon_world::snbt_text::to_snbt(&horizon_client::save::item_to_nbt(i))).collect()).unwrap_or_default();
    let entity=entity.map(|e|horizon_world::snbt_text::to_snbt(&horizon_client::save::encode_block_entity((pos[0],pos[1],pos[2]),e,ntm_core::blockstate::id_of(&state))));
    Some(ntm_core::server::CellSnapshot{state,entity,items})
}

impl Host<'_, '_> {
    fn publish_atomic(&mut self, commits: &[ntm_core::server::MachineCommit], extra:&[ntm_core::server::CellWrite],retained:&[ntm_core::server::RetainedDrop],wear_held:bool) -> bool {
        // This callback owns &mut Game for its entire duration. No event,
        // hopper, chunk unload or block mutation can interleave these passes.
        // Resolve ALL entities and validate ALL expected slots before changing
        // any. Notifications run only after every member has been published.
        // A use callback may run before ServerTickEnd consumes a confirmed
        // external break/replacement. Do not write an old model into that new
        // BE generation, even when its id and empty slots happen to match.
        if lock(&CHANGES).iter().any(|(d,p,old,new)|ntm_core::blockstate::id_of(old)!=ntm_core::blockstate::id_of(new)&&commits.iter().any(|c|c.dim==*d&&c.pos==*p)){return false;}
        let mut prepared=Vec::with_capacity(commits.len());
        let mut seen=std::collections::BTreeSet::new();
        for c in commits {
            if !seen.insert((c.dim.clone(),c.pos)) { return false; }
            let Some((id,valid))=self.entity(&c.dim,c.pos,|e| {
                let observed:Slots=e.items.iter().map(stack).collect();
                let saved=e.nbt.get(&JavaString::from(STATE)).and_then(|tag|match tag{Tag::String(json)=>serde_json::from_str::<ntm_core::machine::Machine>(&json.to_string_lossy()).ok().map(|m|m.generation),_=>None});
                let generation=if c.expected_saved{saved==Some(c.guard.generation)}else{saved.is_none_or(|g|g==c.guard.generation)};
                (e.id.clone(),generation && e.id==c.guard.state && observed==c.expected && e.items.len()==c.desired.len())
            }) else {return false};
            if !valid {return false;}
            let (_,synced)=file(self.game,&id);
            prepared.push(synced);
        }
        // Resolve states, complete BE snapshots and visitability before ANY
        // publication. A queued external A→air→A also invalidates these cells.
        if self.game.remote.is_some() || self.game.world_debug{return false;}
        let mut cells=Vec::new();
        let mut cell_keys=std::collections::BTreeSet::new();
        for w in commits.iter().flat_map(|c|&c.cells).chain(extra){
            let key=(w.guard.dim.clone(),w.guard.pos);
            if !cell_keys.insert(key.clone()) || seen.contains(&key) || w.guard.generation==u64::MAX{return false;}
            if lock(&CHANGES).iter().any(|(d,p,_,_)|d==&key.0&&p==&key.1){return false;}
            if cell_snapshot(self.game,&key.0,key.1).as_ref()!=Some(&w.expected){return false;}
            // A moving piston's carried state lives in Ticks, outside the BE
            // snapshot. Wait for its vanilla finalTick rather than mine a
            // transient, unaccounted carried block or remove its base early.
            if ntm_core::blockstate::id_of(&w.expected.state)=="minecraft:moving_piston"{return false;}
            let Some(dim)=dim_of(&key.0) else{return false};
            if self.game.visit_level(dim,|_|()).is_none(){return false;}
            let mut reader=horizon_client::command::Reader::new(&w.desired);
            let Ok(arg)=horizon_client::command::block::parse(&mut reader,&self.game.ui.registries,false) else{return false};
            if reader.can_read() || arg.nbt.is_some() || arg.tag{return false;}
            let Ok(state)=self.game.pack.assets.registry.resolve(&horizon_world::chunk::BlockState{name:arg.id,properties:arg.properties.into_iter().collect()}) else{return false};
            let Some(old)=self.game.visit_level(dim,|g|g.world.block(w.guard.pos[0],w.guard.pos[1],w.guard.pos[2]))else{return false};
            // Debug levels reject set_raw; preflight every visited level too.
            if self.game.visit_level(dim,|g|g.world_debug).unwrap_or(true){return false;}
            cells.push((w.clone(),dim,old,state));
        }
        if cells.len()>65_536{return false;}
        // The tool the click used is still the one in hand, and it wears: checked before
        // anything is published, applied with the publication.
        if wear_held && !self.game.held().is_some_and(|h|&*h.item=="ntm:hand_drill"){return false;}
        let mut spawn=Vec::new();
        for d in retained {
            if self.game.native_loaded_block(&d.origin.dim,d.origin.pos).is_none(){return false;}
            let Some(dim)=dim_of(&d.origin.dim)else{return false};
            if self.game.visit_level(dim,|_|()).is_none(){return false;}
            let valid=d.items.iter().all(|i|horizon_world::nbt::parse_snbt(i).ok().and_then(|(t,_)|horizon_client::save::item_from_nbt(&t)).is_some());
            if !valid{return false;}
            spawn.push((dim,d.origin.dim.clone(),d.origin.pos,ntm_core::ledger::retained_id(&d.origin.dim,d.origin.pos,d.origin.generation),d.items.clone()));
        }
        let mut notify=Vec::new();
        let mut signals=Vec::new();
        for (c,synced) in commits.iter().zip(prepared) {
            // The preflight guarantees this entity is still accessible; this
            // closure performs the entire update without any fallible operation.
            let changed=self.entity(&c.dim,c.pos,|e| {
                assert_eq!(e.items.iter().map(stack).collect::<Slots>(),c.expected,"exclusive preflight preserves expected slots");
                assert_eq!(e.items.len(),c.desired.len());
                let sent=match e.nbt.get(&JavaString::from(DATA)) {Some(Tag::IntArray(v))=>Some(v.clone()),_=>None};
                let before_signal=comparator_of(&e.id,match e.nbt.get(&JavaString::from(STATE)){Some(Tag::String(j))=>Some(j),_=>None});
                for (cell,new) in e.items.iter_mut().zip(&c.desired) {
                    *cell=item(new.as_ref(),cell.take());
                }
                e.data=c.data.clone();
                e.nbt.insert(JavaString::from(DATA),Tag::IntArray(c.data.clone()));
                e.nbt.insert(JavaString::from(STATE),Tag::String(JavaString::from(c.json.as_str())));
                let after_signal=comparator_of(&e.id,match e.nbt.get(&JavaString::from(STATE)){Some(Tag::String(j))=>Some(j),_=>None});
                (ntm_core::native::sync_changed(sent.as_deref(),&c.data,&synced),before_signal!=after_signal)
            }).expect("atomic preflight retains entity during exclusive Game access");
            if changed.0 {notify.push((c.dim.clone(),c.pos));}
            if changed.1 {signals.push((c.dim.clone(),c.pos));}
        }
        // Publication cannot fail: exclusive Game access, resident/bounded
        // cells and valid states were checked above. No cascade runs here.
        for (w,dim,old,new) in &cells {
            self.game.visit_level(*dim,|g|{
                let pos=w.guard.pos;
                if old==new{return;}
                if g.res.facts().get(*old).name!=g.res.facts().get(*new).name{
                    g.world.block_entities.remove(&(pos[0],pos[1],pos[2]));
                }
                g.with_level(|b,level,ticks|{
                    let changed=b.set_block(level,ticks,horizon_core::position::BlockPos::new(pos[0],pos[1],pos[2]),*new,horizon_world::level::flags::UPDATE_CLIENTS|horizon_world::level::flags::UPDATE_SKIP_ALL_SIDEEFFECTS);
                    assert!(changed,"exclusive loaded-cell preflight guarantees publication");
                });
            }).expect("preflight retains visited level");
        }
        // Retained contents publish in the same exclusive, infallible commit.
        // Owed in the chunk of the consumed cell, in the same step that took them out of it.
        for (dim,dim_name,pos,id,items) in spawn {
            let owed=self.game.visit_level(dim,|g|ledger_add_here(g,&dim_name,id,pos,items)).expect("retained drop level passed preflight");
            assert!(owed,"the consumed cell's chunk is loaded");
        }
        // All BE and cell writes now exist. Notify through vanilla Sim only.
        // Failures after this boundary are diagnostics/sync retries; NEVER a
        // refusal, because rolling back resource accounting would duplicate it.
        for (w,dim,old,new) in &cells {
            self.game.visit_level(*dim,|g|{
                if old==new{return;}
                let p=w.guard.pos;
                if g.res.facts().get(*old).name!=g.res.facts().get(*new).name{g.jukebox_removed(p);}
                let pos=horizon_core::position::BlockPos::new(p[0],p[1],p[2]);
                if w.notify{g.with_level(|b,level,ticks|b.ntm_committed_updates(level,ticks,pos,*old,*new));}
                if w.destroy{
                    g.level_events.push(horizon_world::level::LevelEvent::LevelEvent{id:2001,pos,data:old.0 as i32});
                    g.level_events.push(horizon_world::level::LevelEvent::GameEvent{pos,event:"minecraft:block_destroy",state:Some(*old)});
                }
                if let Err(e)=g.process_level_events(){
                    eprintln!("ntm: committed cell updates {p:?}: {e}; retaining sync retry");
                    if let Some(snapshot)=cell_snapshot(g,&w.guard.dim,p){let mut guard=w.guard.clone();if g.res.facts().get(*old).name!=g.res.facts().get(*new).name{guard.generation=guard.generation.saturating_add(1);}guard.state=snapshot.state.clone();lock(&POST_SYNC).push((guard,snapshot));}
                }
                let landed=g.world.block(p[0],p[1],p[2]);
                let name=g.res.facts().get(*new).name.clone();
                if g.res.facts().get(*old).name!=name && g.res.facts().get(landed).name==name && let Some(entity)=g.new_block_entity(&name){
                    g.world.block_entities.insert((p[0],p[1],p[2]),entity);
                    // A formation's first state goes in with the block entity itself.
                    if let Some(init)=&w.initial && let Some(horizon_client::containers::BlockEntity::Mod(m))=g.world.block_entities.get_mut(&(p[0],p[1],p[2])){
                        m.data=init.data.clone();
                        m.nbt.insert(JavaString::from(DATA),Tag::IntArray(init.data.clone()));
                        m.nbt.insert(JavaString::from(STATE),Tag::String(JavaString::from(init.json.as_str())));
                        g.world.modified.insert((p[0]>>4,p[2]>>4));
                    }
                }
            }).expect("post-publication level remains accessible");
        }
        for (dim,p) in notify {
            if dim_of(&dim)==Some(self.game.dim){self.game.native_block_entity_changed(p);}
            else if self.watched(&dim){in_place(self.game,&dim,|g|g.native_block_entity_changed(p));}
        }
        // The comparators beside a battery, a tank or a barrel whose signal changed read it again,
        // in its own level (a parked one by a passing visit: no level is opened, idleness keeps counting).
        for (dim,p) in signals {
            if let Some(d)=dim_of(&dim){self.game.visit_level(d,|g|g.native_update_output_neighbors(&dim,p));}
        }
        // The hand's part of the same step (checked above, infallible here).
        if wear_held{self.game.damage_held_tool();}
        true
    }
}

impl ServerHost for Host<'_, '_> {
    fn trace_effects(&self)->bool{trace_effects()}
    fn cell_snapshot(&mut self,dim:&str,pos:[i32;3])->Option<ntm_core::server::CellSnapshot>{cell_snapshot(self.game,dim,pos)}
    fn drop_encoded_items(&mut self,dim:&str,pos:[i32;3],items:&[String])->bool{
        if self.game.native_loaded_block(dim,pos).is_none(){return false;}
        let items:Option<Vec<_>>=items.iter().map(|i|horizon_world::nbt::parse_snbt(i).ok().and_then(|(t,_)|horizon_client::save::item_from_nbt(&t))).collect();
        let Some(items)=items else{eprintln!("ntm: invalid retained item NBT at {dim} {pos:?}");return false};
        in_place(self.game,dim,|g|for i in items{g.spawn_item(i,pos.map(|v|f64::from(v)+0.5),crate::game::dispenser::new_item_motion(),0);}).is_some()
    }
    fn drop_items(&mut self,dim:&str,p:[i32;3],items:&[(String,u32)])->bool {
        if self.game.native_loaded_block(dim,p).is_none(){return false;}
        let at=p.map(|v|f64::from(v)+0.5);
        in_place(self.game,dim,|g|{
            for (id,n) in items {g.spawn_item(ItemStack::new(id,*n),at,crate::game::dispenser::new_item_motion(),0);}
        }).is_some()
    }
    fn apply_items(&mut self,id:u64,dim:&str,p:[i32;3],items:&[(String,u32)])->bool{
        let stacks=items.iter().filter(|(_,n)|*n>0).map(|(i,n)|item_snbt(&ItemStack::new(i,*n))).collect();
        ledger_add(self.game,dim,id,p,stacks)
    }
    fn apply_encoded_items(&mut self,id:u64,dim:&str,p:[i32;3],items:&[String])->bool{
        let valid=items.iter().all(|i|horizon_world::nbt::parse_snbt(i).ok().and_then(|(t,_)|horizon_client::save::item_from_nbt(&t)).is_some());
        if !valid{eprintln!("ntm: invalid retained item NBT at {dim} {p:?}");return false;}
        ledger_add(self.game,dim,id,p,items.to_vec())
    }
    fn ledger_prune(&mut self,live:&std::collections::BTreeSet<u64>){ledger_prune(self.game,live);}
    fn cell_generation(&mut self,dim:&str,pos:[i32;3])->Option<u64>{
        self.entity(dim,pos,|e|match e.nbt.get(&JavaString::from(STATE)){
            Some(Tag::String(json))=>serde_json::from_str::<ntm_core::machine::Machine>(&json.to_string_lossy()).ok().map(|m|m.generation),
            _=>None,
        }).flatten()
    }
    fn effect_seen(&mut self,id:u64,dim:&str,pos:[i32;3])->bool{
        in_place(self.game,dim,|g|{
            let key=chunk_key(dim,[pos[0]>>4,pos[2]>>4]);
            g.world.loaded(pos[0],pos[2])&&ledger_get(g,&key).knows(id)
        }).unwrap_or(false)
    }
    fn effect_applied(&mut self,id:u64,cells:&[(String,[i32;3])]){
        for (dim,pos) in cells{
            in_place(self.game,dim,|g|{
                let key=chunk_key(dim,[pos[0]>>4,pos[2]>>4]);
                let mut ledger=ledger_get(g,&key);
                if ledger.note_applied(id){ledger_store(g,&key,ledger);}
            });
        }
    }
    fn pile_depletion(&mut self, stack: &Stack) -> f64 {
        stack.custom.as_ref().and_then(|c| horizon_world::nbt::parse_snbt(c).ok().map(|(tag,_)| tag)).as_ref().map_or(0.0, |t| pile_depletion(Some(t)))
    }
    fn block_resistance(&mut self,dim:&str,p:[i32;3])->Option<f32> {
        self.game.native_loaded_block(dim,p)?;
        let target=dim_of(dim)?;
        let world=if target==self.game.dim {&self.game.world} else {self.game.levels.parked_world(target)?};
        Some(self.game.res.facts().get(world.block(p[0],p[1],p[2])).explosion_resistance)
    }
    fn zirnox_explode(&mut self, dim: &str, at: [f64;3]) -> bool {
        in_place(self.game, dim, |g|{
            if trace_effects(){eprintln!("ntm trace ZIRNOX explosion call {}",serde_json::json!({"tick":g.world.game_time,"dim":dim,"center":at,"power":12}));}
            let applied=g.native_explode(dim, at, 12.0, true, crate::game::native_explosions::NativeBlast::Block);
            if trace_effects(){eprintln!("ntm trace ZIRNOX explosion result {}",serde_json::json!({"tick":g.world.game_time,"applied":applied}));}
            applied
        }).unwrap_or(false)
    }
    fn pile_explode(&mut self, dim: &str, at: [f64;3]) -> bool {
        in_place(self.game, dim, |g| g.native_explode(dim, at, 15.0, true, crate::game::native_explosions::NativeBlast::Block)).unwrap_or(false)
    }
    /// Piles in a level that is open are reached (drops and blasts switch levels
    /// themselves when they happen); in one that is not they freeze.
    fn pile_reachable(&mut self, dim: &str) -> bool {
        dim_of(dim).is_some_and(|d| d == self.game.dim || self.game.levels.parked_world(d).is_some())
    }

    fn solar_sample(&mut self, dim: &str, p: [i32;3]) -> Option<(i32,i32,bool)> {
        self.game.native_loaded_block(dim,p)?;
        use crate::game::Dim;
        let dimension = dim_of(dim)?;
        // ServerTickEnd runs after the last level. Read the requested
        // level, even when another dimension is currently in place.
        let world = if dimension == self.game.dim { &self.game.world }
            else { self.game.levels.parked_world(dimension)? };
        let sky = i32::from(world.light(p[0],p[1],p[2]) >> 4);
        // `canBlockSeeTheSky(y+1)`: light-blocking height, so glass lets
        // sunlight through, leaves and opaque roofs don't.
        let visible = (p[1]+1..world.top_y()).all(|y| self.game.res.facts().get(world.block(p[0],y,p[2])).dampening == 0);
        let darken = match dimension {
            Dim::Nether => 11,
            Dim::End => {
                let sky = self.game.res.end_environment().sample(world.day_time,None,None);
                (15.0 - 15.0 * sky.sky_light_level).round() as i32
            }
            Dim::Overworld => {
                let (rain,thunder) = world.weather.levels(1.0);
                horizon_client::environment::sky_darken(self.game.res.environment(),world.day_time,rain,thunder)
            }
        };
        Some((sky,darken,visible))
    }
    fn get_block(&mut self, dim: &str, p: [i32; 3]) -> Option<String> {
        self.game.native_loaded_block(dim, p)
    }
    fn set_blocks(&mut self, dim: &str, blocks: &[([i32; 3], String)]) -> Result<(), String> {
        self.game.native_set_blocks(dim, blocks)
    }
    fn set_machine_blocks(&mut self,dim:&str,blocks:&[([i32;3],String)],owners:&[(String,[i32;3])])->Result<(),String>{
        let cells=blocks.iter().filter(|(p,_)|owners.iter().any(|(d,q)|d==dim&&q==p)).map(|(p,b)|(dim.to_string(),*p,ntm_core::blockstate::id_of(b).to_string())).collect();
        let _scope=OwnedBlockScope::new(cells);
        self.game.native_set_blocks(dim,blocks)
    }
    /// Send through the native player-specific chat and retain a log trace.
    fn tell(&mut self, player: &str, text: &str) {
        if let Ok(id) = u128::from_str_radix(&player.replace('-', ""),16) {
            self.game.native_tell(id,horizon_client::text::Text::legacy(text));
        }
        eprintln!("ntm: {player}: {text}");
    }
    fn log(&mut self, text: &str) {
        eprintln!("ntm: {text}");
    }
    fn machine_slots(&mut self, dim: &str, p: [i32; 3]) -> Option<Slots> {
        self.entity(dim, p, |e| e.items.iter().map(stack).collect())
    }
    fn machine_data(&mut self, dim: &str, p: [i32; 3]) -> Option<Vec<i32>> {
        self.entity(dim, p, |e| e.data.clone())
    }
    fn commit_machine(&mut self, c: &ntm_core::server::MachineCommit) -> bool {
        self.commit_machines(std::slice::from_ref(c))
    }
    fn commit_machines(&mut self, commits: &[ntm_core::server::MachineCommit]) -> bool {
        let retained:Vec<_>=commits.iter().flat_map(|c|&c.cells).filter(|w|ntm_core::blockstate::id_of(&w.expected.state)!=ntm_core::blockstate::id_of(&w.desired)&&!w.expected.items.is_empty()).map(|w|ntm_core::server::RetainedDrop{origin:w.guard.clone(),items:w.expected.items.clone()}).collect();
        self.publish_atomic(commits,&[],&retained,false)
    }
    fn commit_structural(&mut self,cells:&[ntm_core::server::CellWrite],drops:&[ntm_core::server::RetainedDrop],owners:&[(String,[i32;3])])->bool{
        let own=cells.iter().filter(|c|owners.contains(&c.guard.key())).map(|c|(c.guard.dim.clone(),c.guard.pos,ntm_core::blockstate::id_of(&c.desired).into())).collect();
        let _scope=OwnedBlockScope::new(own);
        self.publish_atomic(&[],cells,drops,false)
    }
    /// A pile's formation: bricks to pile blocks, the block entities with their first JSON and
    /// menu data, and the held drill's wear, all in the one exclusive publication.
    fn commit_formation(&mut self,cells:&[ntm_core::server::CellWrite],wear_held:bool,owners:&[(String,[i32;3])])->bool{
        let own=cells.iter().filter(|c|owners.contains(&c.guard.key())).map(|c|(c.guard.dim.clone(),c.guard.pos,ntm_core::blockstate::id_of(&c.desired).into())).collect();
        let _scope=OwnedBlockScope::new(own);
        self.publish_atomic(&[],cells,&[],wear_held)
    }
    fn machine_buttons(&mut self) -> Vec<(String, String, [i32; 3], u32)> {
        std::mem::take(&mut self.buttons)
    }
    fn redstone_many(&mut self, dim: &str, cells: &[[i32; 3]]) -> Vec<Option<u8>> {
        // A level that is not in place is visited whole by the engine's read, with no
        // trace in the level in place and its idleness kept (`visit_level`).
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
    fn player_dose(&mut self,id:&str,legacy:&ntm_core::radiation::Dose)->Option<ntm_core::radiation::Dose>{
        self.game.native_visit_player(id,|g|crate::game::ntm_items::read_dose(g,legacy)).flatten()
    }
    fn commit_player_dose(&mut self,id:&str,expected:&ntm_core::radiation::Dose,desired:&ntm_core::radiation::Dose)->bool{
        self.game.native_visit_player(id,|g|crate::game::ntm_items::write_dose(g,expected,desired)).unwrap_or(false)
    }
    fn radiation_factor(&mut self,id:&str)->f64{self.game.native_visit_player(id,|g|crate::game::ntm_items::radiation_factor(g)).unwrap_or(1.0)}
    fn inventory_stacks(&mut self,id:&str)->Option<Vec<Stack>> {
        Some(self.game.native_player_inventory(id)?.into_iter().map(|c|Stack {id:c.item,count:c.count,damage:0,custom:c.custom_data}).collect())
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
        if dim_of(dim) != Some(self.game.dim) {
            // Particles and sounds reach only the players of the level in place.
            if self.watched(dim) {
                in_place(self.game, dim, |g| show_in_place(g, show));
            }
            return;
        }
        show_in_place(self.game, show);
    }
    fn set_player_value(&mut self, id: &str, key: &str, value: i64) {
        if let Err(e) = self.game.native_set_player_value(id, key, value) {
            eprintln!("ntm: value {key} of {id}: {e}");
        }
    }

}

/// `show` in the level in place.
fn show_in_place(game: &mut crate::game::Game<'_>, show: &ntm_core::blast::visuals::Show) {
    use horizon_client::particles::{ParticleOptions, ParticleType};
    use ntm_core::blast::visuals::Show;
    let (what, sent) = match show {
        Show::Particles { id, color, at, spread, speed, count } => {
            let Some(ty) = ParticleType::from_name(id) else { return };
            let mut options = ParticleOptions::simple(ty);
            options.color = *color;
            (id, game.with_level(|_, level, _| level.native_send_particles(&options, None, *at, *spread, *speed, *count)))
        }
        Show::Sound { id, source, at, volume, pitch } => {
            let Some(source) = horizon_client::settings::SOUND_SOURCES.iter().position(|s| s == source) else { return };
            (id, game.with_level(|_, level, _| level.native_play_sound(id, *at, source, *volume, *pitch)))
        }
        Show::Effect { .. } => return,
    };
    if !sent {
        eprintln!("ntm: {what} refused");
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
    if block=="ntm:machine_armor_table"{
        *container=Some(ModContainer{slots:14,faces:std::array::from_fn(|_|Vec::new()),inputs:Vec::new(),outputs:Vec::new(),menu:Some("ntm:armor_table".into())});return;
    }
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
    if block=="ntm:machine_armor_table"{filters.extend(crate::game::ntm_armor_table::filters(_game));return;}
    let rules = ntm_core::native::slot_filters(block);
    use horizon_client::containers::SlotPolicy;
    let policy = match block {
        ntm_core::zirnox::BLOCK => Some(SlotPolicy(zirnox_slot_policy)),
        ntm_core::radioisotopes::RTG => Some(SlotPolicy(rtg_slot_policy)),
        ntm_core::radioisotopes::RADGEN => Some(SlotPolicy(radgen_slot_policy)),
        _ => None,
    };
    filters.extend(rules.into_iter().map(|f| ModSlotFilter {
        accept: f.accept, reject: f.reject, max: f.max, output: f.output,
        quick_move: f.quick_move, quick_reverse: f.quick_reverse, policy,
    }));
}

fn zirnox_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::zirnox::BLOCK, slots, slot, item, action, origin)
}
fn rtg_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::radioisotopes::RTG, slots, slot, item, action, origin)
}
fn radgen_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::radioisotopes::RADGEN, slots, slot, item, action, origin)
}
fn machine_slot_policy(block: &str, slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    use horizon_client::containers::{SlotAction, SlotOrigin};
    let slots: Slots = slots.iter().map(stack).collect();
    let Some(item) = stack(&Some(item.clone())) else { return false; };
    ntm_core::native::slot_allows(block, &slots, slot, &item,
        match action { SlotAction::Insert => ntm_core::native::SlotAction::Insert,
            SlotAction::Extract => ntm_core::native::SlotAction::Extract },
        match origin { SlotOrigin::Manual => ntm_core::native::SlotOrigin::Manual,
            SlotOrigin::Automation => ntm_core::native::SlotOrigin::Automation })
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
pub(super) fn stack(s: &Option<ItemStack>) -> Option<Stack> {
    let s = s.as_ref().filter(|s| s.count > 0 && &*s.item != "minecraft:air")?;
    let mut t = Stack::new(&s.item, s.count);
    t.damage = s.damage;
    if ntm_core::pile::Rod::of(&s.item,0.0).is_some() || ntm_core::zirnox::fuel(&s.item).is_some() || ntm_core::radioisotopes::pellet(&s.item).is_some() {
        t.custom = s.data().and_then(|d|d.other("minecraft:custom_data")).map(horizon_world::snbt_text::to_snbt);
    }
    Some(t)
}

/// The game's stack for `new`: the old one with its components when it is
/// the same item and damage, else a plain one.
fn item(new: Option<&Stack>, old: Option<ItemStack>) -> Option<ItemStack> {
    let new = new?;
    let mut result = match old {
        Some(mut o) if *o.item == new.id && o.damage == new.damage => {
            o.count = new.count;
            Some(o)
        }
        _ => {
            let mut it = ItemStack::new(&new.id, new.count);
            it.damage = new.damage;
            Some(it)
        }
    };
    if let Some(it) = result.as_mut() && let Some(custom) = &new.custom && let Ok((tag,_)) = horizon_world::nbt::parse_snbt(custom) {
        it.data_mut().set_other("minecraft:custom_data",Some(tag));
    }
    result
}

/// A block entity's server tick: the machine is only adopted by the server
/// from its saved state when the server doesn't know it (the world was
/// opened again); the server's phases run it at the tick's end.
pub(super) fn tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if !is_machine(&entity.id) {
        return;
    }
    let dim = game.dim.id().to_string();
    if let Some((server, _)) = lock(&SERVER).as_mut()
        && !server.native_has(&dim, pos)
    {
        if let Some(Tag::String(state))=entity.nbt.get(&JavaString::from(STATE)) {
            server.native_adopt(&dim,pos,&state.to_string_lossy());
        }else if let Some(state)=game.native_loaded_block(&dim,pos){
            // A just-placed BE may have been saved before its first NTM tick.
            // Its real slots remain authoritative; adopt the empty model.
            server.native_block_changed(&dim,pos,"minecraft:air",&state);
        }
    }
}

/// The Java rod stores a double; accept the NBT numeric forms as getDouble does.
pub(super) fn pile_depletion(tag: Option<&Tag>) -> f64 {
    let Some(Tag::Compound(c)) = tag else { return 0.0 };
    match c.get(&JavaString::from("depletion")) {
        Some(Tag::Double(n)) => f64::from_bits(*n), Some(Tag::Float(n)) => f64::from(f32::from_bits(*n)),
        Some(Tag::Int(n)) => f64::from(*n), Some(Tag::Long(n)) => *n as f64,
        _ => 0.0,
    }
}

#[cfg(test)]
mod pile_tests {
    use super::*;
    #[test]
    fn native_pile_rod_depletion_survives_slot_conversion() {
        let mut data=Compound::new();
        data.insert(JavaString::from("depletion"),Tag::Double(12.5_f64.to_bits()));
        let mut rod=ItemStack::new("ntm:pile_rod__nu",1);
        rod.data_mut().set_other("minecraft:custom_data",Some(Tag::Compound(data)));
        let saved=stack(&Some(rod)).unwrap();
        let restored=item(Some(&saved),None).unwrap();
        assert_eq!(pile_depletion(restored.data().and_then(|d|d.other("minecraft:custom_data"))),12.5);
        // A transformed/ejected item is a new plain item, without depletion.
        let product=Stack::new("ntm:pile_rod__pu239",1);
        let plain=item(Some(&product),Some(restored)).unwrap();
        assert_eq!(pile_depletion(plain.data().and_then(|d|d.other("minecraft:custom_data"))),0.0);
    }
}

#[cfg(test)]
mod radioisotope_tests {
    use super::*;
    #[test]
    fn native_pellet_long_and_other_components_survive_a_move_and_decay() {
        let mut pellet=ItemStack::new("ntm:pellet_rtg_radium",1);
        let (tag,_)=horizon_world::nbt::parse_snbt("{PELLET_DEPLETION:11519999999l,nested:{name:'unchanged'}}").unwrap();
        pellet.data_mut().set_other("minecraft:custom_data",Some(tag));
        let s=stack(&Some(pellet.clone())).unwrap();
        assert_eq!(ntm_core::radioisotopes::life(&s),11_519_999_999);
        let moved=item(Some(&s),None).unwrap();
        assert_eq!(stack(&Some(moved)),Some(s));
        let product=Stack::new("ntm:pellet_rtg_depleted__lead",1);
        let spent=item(Some(&product),Some(pellet)).unwrap();
        assert!(spent.data().and_then(|d|d.other("minecraft:custom_data")).is_none());
    }
}

#[cfg(test)]
mod zirnox_tests {
    use super::*;
    #[test]
    fn native_zirnox_life_and_other_custom_fields_follow_the_rod() {
        let mut rod=ItemStack::new("ntm:rod_zirnox__uranium_fuel",1);
        let (tag,_)=horizon_world::nbt::parse_snbt("{life:1234,nested:{name:'unchanged'}}").unwrap();
        rod.data_mut().set_other("minecraft:custom_data",Some(tag));
        let s=stack(&Some(rod.clone())).unwrap();
        assert_eq!(ntm_core::zirnox::life(&s),1234);
        let moved=item(Some(&s),None).unwrap();
        let moved=stack(&Some(moved)).unwrap();
        assert_eq!(moved,s);
        let product=Stack::new("ntm:rod_zirnox_uranium_fuel_depleted",1);
        let spent=item(Some(&product),Some(rod)).unwrap();
        assert!(spent.data().and_then(|d|d.other("minecraft:custom_data")).is_none());
    }
}

#[cfg(test)]
mod world_commit_tests {
    use super::*;
    use horizon_core::position::{BlockPos,ChunkPos};
    use horizon_world::{registry::BlockStateId,level::{Behaviours,LevelAccess,LevelEvent,Ticks,flags}};
    use horizon_client::{world::World,containers::BlockEntity,chunk::Chunk};
    use std::{cell::RefCell,sync::Arc};
    struct Resident {
        world:World,events:Vec<LevelEvent>,neighbors:RefCell<Vec<BlockPos>>,retained:Vec<String>,time:i64,
    }
    impl Resident {
        fn new(air:BlockStateId)->Self{
            let mut world=World::new(air,0);
            for x in -1..=1{for z in -1..=1{world.chunks.insert((x,z),Arc::new(Chunk::empty(x,z)));}}
            Self{world,events:Vec::new(),neighbors:RefCell::new(Vec::new()),retained:Vec::new(),time:0}
        }
        fn retain(&mut self,p:BlockPos){
            if let Some(be)=self.world.block_entities.remove(&(p.x,p.y,p.z)){
                self.retained.extend(be.contents().iter().map(|i|horizon_world::snbt_text::to_snbt(&horizon_client::save::item_to_nbt(i))));
            }
        }
    }
    impl LevelAccess for Resident {
        fn block_state(&self,p:BlockPos)->BlockStateId{self.world.block(p.x,p.y,p.z)}
        fn set_raw(&mut self,p:BlockPos,s:BlockStateId)->Option<BlockStateId>{let old=self.block_state(p);self.world.set(p.x,p.y,p.z,s).ok()?;Some(old)}
        fn is_loaded(&self,p:BlockPos)->bool{self.world.loaded(p.x,p.z)}
        fn random(&mut self)->&mut horizon_core::random::LegacyRandom{&mut self.world.random}
        fn game_time(&self)->i64{self.time}
        fn emit(&mut self,e:LevelEvent){self.events.push(e);}
        fn observe_neighbor_changed(&self,p:BlockPos,_:BlockStateId,_:horizon_world::level::BlockId,_:bool){self.neighbors.borrow_mut().push(p);}
    }
    #[test]
    fn atomic_world_publication_matches_vanilla_storage_light_heightmaps_be_and_updates(){
        let root=std::path::PathBuf::from(std::env::var_os("HORIZON_RESOURCES").expect("HORIZON_RESOURCES required"));
        let registry=horizon_world::level::data::load_registry(&root).unwrap();
        let b=Behaviours::load(&registry,&root).unwrap();
        let id=|name:&str,props:&[(&str,&str)]|registry.resolve(&horizon_world::chunk::BlockState{name:format!("minecraft:{name}"),properties:props.iter().map(|(k,v)|(k.to_string(),v.to_string())).collect()}).unwrap();
        let at=BlockPos::new(1,100,1);
        for name in ["chest","redstone_block","stone"]{
            let old=id(name,&[]);let air=id("air",&[]);
            let mut vanilla=Resident::new(air);let mut committed=Resident::new(air);
            let mut vt=Ticks::new(0);let mut ct=Ticks::new(0);
            for ticks in [&mut vt,&mut ct]{for x in -1..=1{for z in -1..=1{ticks.add_empty_chunk(ChunkPos::new(x,z)).unwrap();}}}
            for level in [&mut vanilla,&mut committed]{
                level.world.set(at.x,at.y,at.z,old).unwrap();
                level.world.set(2,100,1,id("observer",&[("facing","west"),("powered","false")])).unwrap();
                level.world.set(1,100,2,id("redstone_lamp",&[("lit","true")])).unwrap();
                let mut jewel=ItemStack::new("minecraft:diamond",7);
                jewel.data_mut().set_other("minecraft:custom_name",Some(Tag::String("Tesouro".into())));
                if name=="chest"{let mut slots=vec![None;27];slots[0]=Some(jewel);level.world.block_entities.insert((1,100,1),BlockEntity::Chest(slots));}
                level.retain(at);
            }
            assert!(b.set_block(&mut vanilla,&mut vt,at,air,flags::UPDATE_ALL));
            assert!(b.set_block(&mut committed,&mut ct,at,air,flags::UPDATE_CLIENTS|flags::UPDATE_SKIP_ALL_SIDEEFFECTS));
            b.ntm_committed_updates(&mut committed,&mut ct,at,old,air);
            assert_eq!(committed.retained,vanilla.retained);
            if name=="chest"{
                assert_eq!(committed.retained.len(),1);
                let (tag,_)=horizon_world::nbt::parse_snbt(&committed.retained[0]).unwrap();
                let jewel=horizon_client::save::item_from_nbt(&tag).unwrap();
                assert_eq!(jewel.count,7);assert!(jewel.data().unwrap().other("minecraft:custom_name").is_some());
            }
            assert!(!committed.world.block_entities.contains_key(&(1,100,1)));
            let mut pulse=false;
            for _ in 0..8{
                for level in [&mut vanilla,&mut committed]{level.time+=1;}
                b.tick_scheduled(&mut vanilla,&mut vt);b.tick_scheduled(&mut committed,&mut ct);
                for x in 0..=3{for z in 0..=3{
                    assert_eq!(committed.world.world_surface(x,z,|s|b.is_air(s)),vanilla.world.world_surface(x,z,|s|b.is_air(s)));
                    assert_eq!(committed.world.motion_blocking(x,z,|s|!b.is_air(s)),vanilla.world.motion_blocking(x,z,|s|!b.is_air(s)));
                    for y in 99..=101{assert_eq!(committed.world.block(x,y,z),vanilla.world.block(x,y,z));assert_eq!(committed.world.light(x,y,z),vanilla.world.light(x,y,z));}
                }}
                assert_eq!(committed.world.dirty_light,vanilla.world.dirty_light);
                assert_eq!(committed.world.light_edits,vanilla.world.light_edits);
                assert_eq!(committed.events,vanilla.events);
                assert_eq!(*committed.neighbors.borrow(),*vanilla.neighbors.borrow());
                pulse|=committed.world.block(2,100,1)==id("observer",&[("facing","west"),("powered","true")]);
            }
            assert!(pulse,"observer notified for {name}");
            assert!(!committed.events.iter().any(|e|matches!(e,LevelEvent::DropResources{..})));
            assert_eq!(committed.world.block(1,100,2),id("redstone_lamp",&[("lit","false")]));
        }
    }
}

/// Legacy migration seed; all subsequent native dose lives in the player file.
pub(super) fn item_dose(id:&str)->Option<ntm_core::radiation::Dose> {lock(&SERVER).as_ref().map(|(s,_)|s.player_dose(id))}

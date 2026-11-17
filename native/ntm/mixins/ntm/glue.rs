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
        lock(&OWING).clear();
        lock(&ARRIVED).clear();
        lock(&RAD_CHUNKS).clear();
        lock(&GEIGERS).clear();
        KV_ITEMS.store(0, std::sync::atomic::Ordering::Relaxed);
        lock(&LOADED_AT).clear();
    }
    // The next world's storage is read from its own folder.
    *lock(&STORE) = None;
    if open {
        eprintln!("ntm native: {} machine blocks", ntm_core::registry::machine_ids().count());
    }
}

/// The block changes recorded since the server last took them, and the records loaded since, given to the server (it
/// then takes them as the end of a tick does, `Server::native_settle`).
fn feed_changes(server:&mut Server,host:&mut Host<'_,'_>){
    let changes=std::mem::take(&mut *lock(&CHANGES));
    arrived(server);
    for (dim,pos,old,new) in changes {
        let now=ntm_core::server::native_server::native_change_state(&new,host.game.native_loaded_block(&dim,pos));
        server.native_block_changed(&dim,pos,&old,&now);
    }
}

/// A save is about to take the chunks (`world_saving`, a local extension of the engine): a block changed since the
/// last tick (a command, a frozen tick) has its machine's contents out and what it owes in its chunk's record first,
/// as a vanilla container's items are out the instant its block goes (N1 of the fresh review).
pub(super) fn world_saving(game:&mut crate::game::Game<'_>,_wait:bool){
    horizon_client::spans::time("ntm_world_saving", || {
        if let Some((server,_))=lock(&SERVER).as_mut(){
            let mut host=Host{game,buttons:Vec::new()};
            feed_changes(server,&mut host);
            server.native_world_saving(&mut host);
        }
    });
}

/// The world's save: the storage is written with it, as the state of this instant (vanilla's `level.dat` and saved
/// data are, while the regions may still be queued).
pub(super) fn world_saved(game:&mut crate::game::Game<'_>,wait:bool){
    horizon_client::spans::time("ntm_world_saved", || world_saved_timed(game, wait));
}
fn world_saved_timed(game:&mut crate::game::Game<'_>,wait:bool){
    if let Some((server,_))=lock(&SERVER).as_mut(){
        let mut host=Host{game,buttons:Vec::new()};
        // Everything the world did up to this save is in its chunks (`world_saving`): no receipt is pending past it,
        // and nothing is given to a record past it. Only a save that waited wrote them before the storage.
        server.native_world_saved(&mut host,wait);
        KV_ITEMS.store(server.native_kept_items() as u64,std::sync::atomic::Ordering::Relaxed);
        if server.native_kept_items()>0{eprintln!("ntm: the storage keeps {} removals/batches of an earlier version",server.native_kept_items());}
    }
    store_commit(game,wait);
    // `NTM_FAILPOINT=kv_committed` (tests): the process dies right after the storage of a save that does not wait is
    // written, its records still queued (with `HORIZON_WRITER_HOLD`, none of them is written). Off by default.
    if !wait&&std::env::var("NTM_FAILPOINT").is_ok_and(|v|v=="kv_committed"){
        eprintln!("NTM_FAILPOINT kv_committed");
        std::process::abort();
    }
}

/// The entries the storage written by the last save keeps that can hold items (`/ntmledger kv_items`): zero but for an
/// earlier version's.
static KV_ITEMS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A battery, a fluid tank and a barrel give a comparator a signal (`hasComparatorInputOverride`).
pub(super) fn has_analog_output(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, output: &mut bool) {
    let name = &level.facts.get(state).name;
    if ntm_core::native::has_comparator(name) || &**name == GEIGER { *output = true; }
}

/// `getComparatorInputOverride` from the machine's saved state, which the server commits each tick.
pub(super) fn analog_output(level: &crate::game::GameLevel<'_>, dim: &str, pos: horizon_core::position::BlockPos, state: horizon_world::registry::BlockStateId, output: &mut i32) {
    // GeigerCounter.getComparatorInputOverride: the field where it is, read when asked.
    if &*level.facts.get(state).name == GEIGER {
        *output = ntm_core::geiger_block::comparator(field_at(dim, [pos.x, pos.y, pos.z]));
        return;
    }
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
    if old != new {
        field_block_added(dim, pos, &new);
        matter_added(dim, pos, &new, level.world.game_time as i64);
    }
    // A Geiger counter block taken away: a new one starts from 0 (a new TileEntityGeiger).
    if old == GEIGER && new != GEIGER {
        lock(&GEIGERS).remove(&(dim.to_string(), pos));
    }
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
    // GeigerCounter.onBlockActivated: not sneaking, the techBoop and printGeigerData; sneaking, nothing (false).
    if !sneaking && game.native_loaded_block(game.dim.id(), pos).is_some_and(|b| ntm_core::blockstate::id_of(&b) == GEIGER) {
        crate::game::ntm_items::geiger_report(game);
        return true;
    }
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
            // The rod left the hand in the loader's own transfer (`commit_player_transfer`).
            ntm_core::server::PileUse::ConsumeRod => {}
            ntm_core::server::PileUse::WearDrill => {
                if held.as_ref().is_some_and(|h| &*h.item == "ntm:hand_drill") { host.game.damage_held_tool(); }
            }
            ntm_core::server::PileUse::Handled => {}
        }
        return true;
    }
    // A fuel rod into an RBMK fuel channel, moved with the hand in one transfer (T01).
    if server.native_rbmk_use(&mut host, &dim, pos, core, &player) {
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
    if crate::game::ntm_items::use_on_block(game, stack, pos, face) { return true; }
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

/// The radiation server's reading for a player (`radBuf`, milliRAD/s).
pub(super) fn rad_buffer(uuid: &str) -> i64 {
    lock(&SERVER).as_ref().map_or(0, |(server, _)| server.rad_buf(uuid))
}

/// `ContaminationUtil.contaminate` of a player: the amount joins `radEnv`.
pub(super) fn contaminate(uuid: &str, millirad: i64) {
    if let Some((server, _)) = lock(&SERVER).as_mut() {
        server.note_contamination(uuid, millirad);
    }
}

/// The world's radiation by chunk at a position, as `getRadiation` reads it (0 before the server exists).
pub(super) fn chunk_rad(dim: &str, pos: [f64; 3]) -> f32 {
    let chunk = [(pos[0].floor() as i32) >> 4, (pos[2].floor() as i32) >> 4];
    lock(&SERVER).as_ref().map_or(0.0, |(server, _)| server.native_chunk_radiation(dim, chunk))
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
    if line.split_whitespace().next()==Some("ntmtest"){return test_hook(game,player,line);}
    if line.split_whitespace().next()==Some("ntmrad"){
        let mut guard=lock(&SERVER);
        let mut host=Host{game,buttons:Vec::new()};
        let (server,_)=guard.get_or_insert_with(||(Server::native(&mut host,0),0));
        reconcile(server,&mut host);
        arrived(server);
        if let Some(reply)=server.native_radiation_command(&player.dimension,player.position,line){
            match reply{
                Ok(text)=>host.game.native_feedback(horizon_client::text::Text::literal(text)),
                Err(text)=>host.game.native_feedback(horizon_client::text::Text::literal(format!("\u{a7}c{text}"))),
            }
        }
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

/// The in-game proofs' hooks, only with `NTM_TEST_HOOKS=1` (as the save's failpoints): `ntmtest eject <x> <y> <z> <ax>
/// <ay> <az>` puts the rod of the fuel channel at x y z out as an item entity at ax ay az through its reactor's next step
/// (the entity receipt path); `ntmtest take <x> <y> <z> cursor|drop` moves it to the acting player's cursor or out of
/// their hand as a thrown drop, in one transfer. Without the variable the command does not exist.
fn test_hook(game:&mut crate::game::Game<'_>,player:&horizon_client::mod_api::NativeServerPlayer,line:&str)->bool {
    if std::env::var_os("NTM_TEST_HOOKS").is_none(){return false;}
    let w:Vec<&str>=line.split_whitespace().skip(1).collect();
    let n=|i:usize|w.get(i).and_then(|v|v.parse::<f64>().ok());
    let dim=game.dim.id().to_string();
    let me=game.native_actor_uuid().unwrap_or_else(||player.uuid.to_string());
    let mut guard=lock(&SERVER);
    let mut host=Host{game,buttons:Vec::new()};
    let Some((server,_))=guard.as_mut() else{return true};
    reconcile(server,&mut host);
    // `ntmtest height <x> <z>`: the column's motion-blocking height and the blocks at it and the two under it.
    if w.first()==Some(&"height") && let (Some(x),Some(z))=(n(1),n(2)){
        let h=ntm_core::server::ServerHost::surface_heights(&mut host,&dim,&[(x as i32,z as i32)])[0];
        let around=h.map(|h|(h-2..=h).map(|y|host.game.native_loaded_block(&dim,[x as i32,y,z as i32]).unwrap_or_default()).collect::<Vec<_>>().join(",")).unwrap_or_default();
        host.game.native_feedback(horizon_client::text::Text::literal(format!("ntmtest ok=true height={h:?} below2..at={around}")));
        return true;
    }
    // `ntmtest rad <cx> <cz> [r]`: the world's radiation map around a chunk, one `x,z:raw:next:loaded` per chunk the
    // map holds, the next update writes or that is loaded (`raw` the map's value as its float bits, `none` when not in
    // it; `next` what the next update gives it; `loaded` 1 when a save writes it), and the ticks since the last update.
    if w.first()==Some(&"rad") && let (Some(x),Some(z))=(n(1),n(2)){
        let (timer,chunks)=server.native_chunk_radiation_probe(&dim,[x as i32,z as i32],n(3).unwrap_or(0.0) as i32);
        // Only the chunks the map holds or the next update writes: a wide radius stays a short reply.
        let list:Vec<String>=chunks.iter().filter(|(_,raw,next,loaded)|raw.is_some()||*next!=0.0||*loaded).map(|(c,raw,next,loaded)|format!("{},{}:{}:{}:{}",c[0],c[1],raw.map_or("none".to_string(),|r|r.to_bits().to_string()),next.to_bits(),u8::from(*loaded))).collect();
        let reply=format!("ntmtest ok=true timer={timer} rad={}",list.join(";"));
        host.game.native_feedback(horizon_client::text::Text::literal(reply));
        return true;
    }
    // `ntmtest dose`: the acting player's dose as the server keeps it (milliRAD, every tick, where the client's value
    // comes once a second) and their last `radBuf`.
    if w.first()==Some(&"dose"){
        let reply=format!("ntmtest ok=true dose={} buf={}",server.player_dose(&me).millirad,server.rad_buf(&me));
        host.game.native_feedback(horizon_client::text::Text::literal(reply));
        return true;
    }
    let at=match (n(1),n(2),n(3)){(Some(x),Some(y),Some(z))=>Some([x as i32,y as i32,z as i32]),_=>None};
    let result:Result<(),&str>=match (w.first().copied(),at){
        (Some("eject"),Some(p))=>match (n(4),n(5),n(6)){(Some(x),Some(y),Some(z))=>if server.native_test_eject(&dim,p,[x,y,z]){Ok(())}else{Err("no rod to put out")},_=>Err("usage")},
        // The player's state swapped in, as a player's own action has it (the snapshot of a transfer needs it).
        (Some("take"),Some(p))=>host.game.native_visit_player(&me,|g|{
            let mut host=Host{game:g,buttons:Vec::new()};
            server.native_test_take(&mut host,&dim,p,&me,w.get(4)==Some(&"cursor"))
        }).unwrap_or(Err("no such player")),
        _=>Err("usage"),
    };
    let reply=match result{Ok(())=>"ntmtest ok=true".to_string(),Err(e)=>format!("ntmtest ok=false reason={e}")};
    host.game.native_feedback(horizon_client::text::Text::literal(reply));
    true
}

// ------------------------------------------------------------- the Geiger counter block
// `TileEntityGeiger` keeps nothing: its counters live here while its chunk is loaded, from 0 each time it is made.

const GEIGER: &str = "ntm:geiger";

/// Each Geiger counter block's `timer` and `ticker`, by (dimension, position).
static GEIGERS: Mutex<std::collections::BTreeMap<(String, [i32; 3]), ntm_core::geiger_block::GeigerBlock>> =
    Mutex::new(std::collections::BTreeMap::new());

/// `getRadiation` at a block: the world's radiation by chunk (0 before the server exists, or while it is busy).
fn field_at(dim: &str, pos: [i32; 3]) -> f32 {
    SERVER.try_lock().ok().and_then(|g| g.as_ref().map(|(server, _)| server.native_chunk_radiation(dim, [pos[0] >> 4, pos[2] >> 4]))).unwrap_or(0.0)
}

/// `TileEntityGeiger.updateEntity`: the reading every 10 ticks with the neighbours told, a click every 5 at the
/// block's corner (`playSoundEffect(xCoord, yCoord, zCoord, ..)`).
fn geiger_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    let dim = game.dim.id().to_string();
    let step = {
        let mut geigers = lock(&GEIGERS);
        let geiger = geigers.entry((dim.clone(), pos)).or_default();
        geiger.tick(|| field_at(&dim, pos), |n| (crate::game::ntm_items::random_unit(game) * n as f32) as u32 % n.max(1))
    };
    if step.notify {
        game.native_update_output_neighbors(&dim, pos);
    }
    if let Some(r) = step.click {
        crate::game::ntm_items::play_at(game, &format!("hbm:item.geiger{r}"), [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])]);
    }
}

/// A chunk's Geiger counter blocks leave with it: made again from 0 when it loads.
fn geigers_unloaded(dim: &str, chunk: [i32; 2]) {
    lock(&GEIGERS).retain(|(d, p), _| !(d == dim && p[0] >> 4 == chunk[0] && p[2] >> 4 == chunk[1]));
}

// ------------------------------------------------------------- the field's source blocks
// Slice 4d (ntm_core::field_sources): blocks that give the world's radiation by chunk on their ticks. The first tick of a
// block is scheduled from its `onBlockAdded` (the block change), each tick schedules the next, as the Java's do.

/// The `BlockHazard` types whose `rad` a block of theirs added has set: a field of the Java's one `Block` of the type,
/// kept for the whole run of the game (not cleared with the world, as the JVM keeps it).
static ARMED: Mutex<std::collections::BTreeSet<String>> = Mutex::new(std::collections::BTreeSet::new());

/// First ticks to schedule, (dimension, position, block): the block change has no tick queue at hand; the server tick
/// schedules them, in the same game tick.
static FIELD_SCHEDULE: Mutex<Vec<(String, [i32; 3], String)>> = Mutex::new(Vec::new());

pub(super) fn level_dim_id(d: horizon_world::level::Dimension) -> &'static str {
    match d {
        horizon_world::level::Dimension::Overworld => "minecraft:overworld",
        horizon_world::level::Dimension::Nether => "minecraft:the_nether",
        horizon_world::level::Dimension::End => "minecraft:the_end",
    }
}

/// A source block added (`onBlockAdded`): a hazard type armed; its first tick in `tickRate`.
fn field_block_added(dim: &str, pos: [i32; 3], new: &str) {
    use ntm_core::field_sources::{block_source, Kind};
    let Some(source) = block_source(new) else { return };
    let id = ntm_core::blockstate::id_of(new).to_string();
    if source.kind == Kind::Hazard {
        lock(&ARMED).insert(id.clone());
    }
    lock(&FIELD_SCHEDULE).push((dim.to_string(), pos, id));
}

/// The first ticks queued since: scheduled in the level in place (another level's are its own game's).
fn schedule_field_ticks(game: &mut crate::game::Game<'_>) {
    let queued = std::mem::take(&mut *lock(&FIELD_SCHEDULE));
    let matter = std::mem::take(&mut *lock(&MATTER_SCHEDULE));
    if queued.is_empty() && matter.is_empty() {
        return;
    }
    let dim = game.dim.id().to_string();
    game.with_level(|_, level, ticks| {
        for (_, pos, block) in queued.iter().filter(|q| q.0 == dim) {
            level.native_schedule_block_tick(ticks, *pos, block, ntm_core::field_sources::TICK_RATE);
        }
        for (_, pos, block, delay) in matter.iter().filter(|q| q.0 == dim) {
            level.native_schedule_block_tick(ticks, *pos, block, *delay as i32);
        }
    });
}

/// `DigammaMatter.onBlockAdded`'s first ticks, (dimension, position, block, delay), as FIELD_SCHEDULE's.
static MATTER_SCHEDULE: Mutex<Vec<(String, [i32; 3], String, u32)>> = Mutex::new(Vec::new());

/// A digamma matter block added: its tick 10 to 49 ticks on (`10 + rand.nextInt(40)`, a static `Random`; the mods
/// have none: a hash of the cell and the game time).
fn matter_added(dim: &str, pos: [i32; 3], new: &str, time: i64) {
    let id = ntm_core::blockstate::id_of(new);
    if ntm_core::digamma::matter_meta(id).is_none() {
        return;
    }
    let seed = ntm_core::hash::fnv64(&[pos[0].to_le_bytes(), pos[1].to_le_bytes(), pos[2].to_le_bytes(), (time as i32).to_le_bytes()].concat());
    let delay = ntm_core::digamma::matter_delay(|n| (ntm_core::hash::mix(seed) % n as u64) as i32);
    lock(&MATTER_SCHEDULE).push((dim.to_string(), pos, id.to_string(), delay));
}

/// `DigammaMatter.updateTick`: the block goes, and below metadata 7 each cell within two steps that holds no matter
/// takes the next (`digamma::matter_spread`), with `setBlock`'s flag 3. A cell whose chunk is not loaded is left.
fn matter_tick(level: &mut crate::game::GameLevel<'_>, ticks: &mut horizon_world::level::Ticks, pos: horizon_core::position::BlockPos, meta: u8) {
    let Some(air) = level.registry.default_state("minecraft:air") else { return };
    let behaviours = level.behaviours;
    behaviours.set_block(level, ticks, pos, air, 3);
    let cells = ntm_core::digamma::matter_spread(meta, |o| {
        let state = level.world.block(pos.x + o[0], pos.y + o[1], pos.z + o[2]);
        ntm_core::digamma::matter_meta(&level.facts.get(state).name).is_some()
    });
    for (o, next) in cells {
        let at = horizon_core::position::BlockPos::new(pos.x + o[0], pos.y + o[1], pos.z + o[2]);
        if !level.world.loaded(at.x, at.z) {
            continue;
        }
        if let Some(state) = level.registry.default_state(&ntm_core::digamma::matter_id(next)) {
            behaviours.set_block(level, ticks, at, state, 3);
        }
    }
}

/// `DigammaFlame.canPlaceBlockAt` (`World.doesBlockHaveSolidTopSurface(x, y - 1, z)`): a ground whose top is solid,
/// read as an opaque full cube (`isSideSolid`'s rule for a plain block).
pub(super) fn flame_ground(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, ground: horizon_world::registry::BlockStateId, may: &mut Option<bool>) {
    if &*level.facts.get(state).name == ntm_core::digamma::FLAME_BLOCK {
        *may = Some(level.facts.get(ground).solid_render);
    }
}

/// `incrementRad` at a block, into the server's field (its chunk loaded).
fn field_increment(dim: &str, pos: [i32; 3], rad: f32) {
    match SERVER.try_lock() {
        Ok(mut guard) => {
            if let Some((server, _)) = guard.as_mut() {
                arrived(server);
                server.native_increment_radiation(dim, pos, rad);
            }
        }
        Err(_) => eprintln!("ntm: the field's source at {pos:?} found the server busy: {rad} RAD/s not given"),
    }
}

/// A source block's tick (`updateTick`): its `rad` given and its next tick scheduled; a hazard type not armed does
/// neither (`rad` 0: the chain ends). `random` is a random tick (`BlockOre`'s), the others are scheduled ones.
fn field_tick(level: &mut crate::game::GameLevel<'_>, ticks: &mut horizon_world::level::Ticks, dim: &str, pos: [i32; 3], name: &str, random: bool) {
    use ntm_core::field_sources::{block_source, Kind, TICK_RATE};
    let Some(source) = block_source(name) else { return };
    if random && source.kind != Kind::Ore {
        return;
    }
    if source.kind == Kind::Hazard && !lock(&ARMED).contains(name) {
        return;
    }
    field_increment(dim, pos, source.rad);
    level.native_schedule_block_tick(ticks, pos, name, TICK_RATE);
}

/// The `tick_block` hook: a scheduled tick of a source block. Its own (the template's: a falling block's fall) runs
/// after it, as `BlockHazardFalling` calls `super.updateTick`.
pub(super) fn field_scheduled_tick(level: &mut crate::game::GameLevel<'_>, ticks: &mut horizon_world::level::Ticks, dimension: horizon_world::level::Dimension, pos: horizon_core::position::BlockPos, state: horizon_world::registry::BlockStateId) {
    if !level.native_authoritative {
        return;
    }
    let name = level.facts.get(state).name.to_string();
    if let Some(meta) = ntm_core::digamma::matter_meta(&name) {
        matter_tick(level, ticks, pos, meta);
        return;
    }
    field_tick(level, ticks, level_dim_id(dimension), [pos.x, pos.y, pos.z], &name, false);
}

/// The random tick of a source block (`BlockOre`'s) and of sellafield (`BlockSellafield.updateTick:56`, its `rad`
/// before it decays).
pub(super) fn field_random_tick(level: &mut crate::game::GameLevel<'_>, ticks: &mut horizon_world::level::Ticks, dim: &str, pos: [i32; 3], name: &str) {
    if let Some(rad) = ntm_core::field_sources::sellafield_rad(name) {
        field_increment(dim, pos, rad);
        return;
    }
    field_tick(level, ticks, dim, pos, name, true);
}

// ----------------------------------------------------------------- the ledger
// What a chunk's record keeps of the machines' effects (see `ntm_core::ledger`): the stacks an effect owes while it
// cannot drop them yet, written with the block entity that paid them; and what records of earlier versions held.

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
/// The stack an item entity of an earlier version belonged to (`ntm_core::ledger::entity_tag`): read to settle that
/// version's records once.
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

/// The host's storage of the world open, in memory until the world's save writes it (`store_commit`).
struct Store {
    dir: std::path::PathBuf,
    keys: std::collections::BTreeMap<String, Vec<u8>>,
    /// A key changed since the last write.
    changed: bool,
    /// The files of the earlier form, one per key and written every tick: removed once the one file holds them.
    legacy: Vec<std::path::PathBuf>,
}
static STORE: Mutex<Option<Store>> = Mutex::new(None);
/// The one file of the storage, and how it starts.
const STORE_FILE: &str = "state";
const STORE_MAGIC: &[u8] = b"ntm-store 1\n";

/// The storage in `dir`: its one file, else (a world of an earlier version) the file of each key.
fn store_read(dir: &std::path::Path) -> Store {
    let mut store = Store { dir: dir.to_path_buf(), keys: std::collections::BTreeMap::new(), changed: false, legacy: Vec::new() };
    let files: Vec<std::path::PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    let one = dir.join(STORE_FILE);
    let others = files.into_iter().filter(|p| *p != one && p.extension().is_none_or(|x| x != "tmp"));
    match std::fs::read(&one) {
        Ok(bytes) => {
            store.keys = store_decode(&bytes).unwrap_or_else(|| {
                eprintln!("ntm: storage {} unreadable", one.display());
                Default::default()
            });
            // Left from a write that died before removing them: the one file already holds what they held.
            store.legacy = others.collect();
        }
        Err(_) => {
            for path in others {
                if let (Some(name), Ok(value)) = (path.file_name().and_then(|n| n.to_str()).map(str::to_string), std::fs::read(&path)) {
                    store.keys.insert(name, value);
                    store.legacy.push(path);
                }
            }
            store.changed = !store.keys.is_empty();
        }
    }
    store
}

fn store_encode(keys: &std::collections::BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut out = STORE_MAGIC.to_vec();
    for (k, v) in keys {
        out.extend_from_slice(&(k.len() as u32).to_le_bytes());
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(&(v.len() as u32).to_le_bytes());
        out.extend_from_slice(v);
    }
    out
}

fn store_decode(bytes: &[u8]) -> Option<std::collections::BTreeMap<String, Vec<u8>>> {
    let mut rest = bytes.strip_prefix(STORE_MAGIC)?;
    let mut keys = std::collections::BTreeMap::new();
    let take = |rest: &mut &[u8]| -> Option<Vec<u8>> {
        let n = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
        let v = rest.get(4..4 + n)?.to_vec();
        *rest = &rest[4 + n..];
        Some(v)
    };
    while !rest.is_empty() {
        let k = String::from_utf8(take(&mut rest)?).ok()?;
        let v = take(&mut rest)?;
        keys.insert(k, v);
    }
    Some(keys)
}

fn with_store<R>(dir: &std::path::Path, f: impl FnOnce(&mut Store) -> R) -> R {
    let mut guard = lock(&STORE);
    if guard.as_ref().is_none_or(|s| s.dir != dir) {
        *guard = Some(store_read(dir));
    }
    f(guard.as_mut().expect("just read"))
}

/// The storage as it stands now, written whole: a new file renamed over the old, so a death leaves one or the other.
/// A save that waits syncs it (file and folder) after the regions; one that does not leaves it to the page cache, as
/// vanilla leaves its `level.dat`.
fn store_commit(game: &mut crate::game::Game<'_>, wait: bool) {
    horizon_client::spans::time("ntm_kv_commit", || {
        let mut guard = lock(&STORE);
        let Some(store) = guard.as_mut() else { return };
        if !store.changed && store.legacy.is_empty() {
            return;
        }
        let started = std::time::Instant::now();
        let bytes = store_encode(&store.keys);
        let (tmp, file) = (store.dir.join(format!("{STORE_FILE}.tmp")), store.dir.join(STORE_FILE));
        let written = (|| {
            use std::io::Write;
            std::fs::create_dir_all(&store.dir)?;
            let mut out = std::fs::File::create(&tmp)?;
            out.write_all(&bytes)?;
            if wait {
                out.sync_all()?;
            }
            std::fs::rename(&tmp, &file)?;
            if wait {
                std::fs::File::open(&store.dir)?.sync_all()?;
            }
            Ok::<(), std::io::Error>(())
        })();
        match written {
            Ok(()) => {
                store.changed = false;
                for path in std::mem::take(&mut store.legacy) {
                    let _ = std::fs::remove_file(path);
                }
            }
            Err(e) => eprintln!("ntm: storage {}: {e}", file.display()),
        }
        if std::env::var_os("NTM_TRACE_PHASES").is_some() {
            eprintln!("ntm: storage written at tick {}: {} bytes in {:.1} ms (wait={wait})", game.ticks, bytes.len(), started.elapsed().as_secs_f64() * 1000.0);
        }
    });
}

type ChunkKey = (String, i32, i32);
/// The session's copy of the ledgers of the loaded chunks that were looked at (an empty one says the chunk has
/// none). The chunk's attachments are the authority; this copy is never behind them.
static LEDGERS: Mutex<std::collections::BTreeMap<ChunkKey, ntm_core::ledger::ChunkLedger>> = Mutex::new(std::collections::BTreeMap::new());
/// Loaded chunks whose ledger owes stacks, or holds an earlier version's to settle: looked at every server tick.
static OWING: Mutex<std::collections::BTreeSet<ChunkKey>> = Mutex::new(std::collections::BTreeSet::new());
/// When each loaded chunk came in (in game ticks): its saved entities have joined the world two ticks after.
static LOADED_AT: Mutex<std::collections::BTreeMap<ChunkKey, u64>> = Mutex::new(std::collections::BTreeMap::new());

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

/// Writes the ledger into the attachments of its loaded chunk, for the chunk's next save, and keeps it for the ticks.
fn ledger_store(g: &mut crate::game::Game<'_>, key: &ChunkKey, ledger: ntm_core::ledger::ChunkLedger) {
    use horizon_client::mod_api::Holder;
    if !g.world.loaded(key.1 * 16, key.2 * 16) {
        return;
    }
    let value = if ledger.is_empty() { String::new() } else { ledger.to_json() };
    if g.attachment_map_quiet(Holder::Chunk([key.1, key.2])).is_ok_and(|map| map.set(&LEDGER_TYPE, value).is_ok()) {
        g.world.modified.insert((key.1, key.2));
    }
    if ledger.owed.is_empty() {
        lock(&OWING).remove(key);
    } else {
        lock(&OWING).insert(key.clone());
    }
    lock(&LEDGERS).insert(key.clone(), ledger);
}

fn ledger_get(g: &mut crate::game::Game<'_>, key: &ChunkKey) -> ntm_core::ledger::ChunkLedger {
    let cached = lock(&LEDGERS).get(key).cloned();
    cached.unwrap_or_else(|| ledger_load(g, [key.1, key.2]))
}

/// Whether the chunk's ledger knows the effect `id`, without copying it. A chunk read once is kept, even with none.
fn ledger_knows(g: &mut crate::game::Game<'_>, key: &ChunkKey, id: u64) -> bool {
    if let Some(known) = lock(&LEDGERS).get(key).map(|l| l.knows(id)) {
        return known;
    }
    let ledger = ledger_load(g, [key.1, key.2]);
    let known = ledger.knows(id);
    lock(&LEDGERS).insert(key.clone(), ledger);
    known
}

/// Drops `items` (SNBT stacks) at `pos` for the effect `id`, on the level in place, in the same step as the machines
/// whose step put them out (`owners`) paid them. When the chunk of `pos` is not loaded, the stacks are owed in the
/// record of a loaded owner's chunk, as a dropper keeps its stock, and dropped once `pos` is in. A known id (owed, or
/// applied by an earlier version) adds nothing. False when neither can take them (the effect is retried).
fn ledger_add_here(g: &mut crate::game::Game<'_>, dim: &str, id: u64, pos: [i32; 3], items: Vec<String>, owners: &[(String, [i32; 3])]) -> bool {
    horizon_client::spans::time("ntm_ledger_add", || {
        let here = [pos[0] >> 4, pos[2] >> 4];
        let mut chunks: Vec<[i32; 2]> = vec![here];
        for (d, p) in owners {
            let c = [p[0] >> 4, p[2] >> 4];
            if d == dim && !chunks.contains(&c) {
                chunks.push(c);
            }
        }
        let loaded: Vec<[i32; 2]> = chunks.iter().copied().filter(|c| g.world.loaded(c[0] * 16, c[1] * 16)).collect();
        if loaded.iter().any(|c| ledger_knows(g, &chunk_key(dim, *c), id)) {
            return true;
        }
        if loaded.contains(&here) {
            spawn_items(g, pos, &items);
            return true;
        }
        let Some(owner) = loaded.first().copied() else { return false };
        let key = chunk_key(dim, owner);
        let mut ledger = ledger_get(g, &key);
        ledger.owe(id, pos, items);
        ledger_store(g, &key, ledger);
        true
    })
}

/// `ledger_add_here` for the entities of the effect `id` (`WorldEffect::Entities`): spawned now when the chunk of `pos`
/// is loaded, else owed in a loaded owner's record and spawned once `pos` is in; a known id adds nothing.
fn ledger_add_entities_here(g: &mut crate::game::Game<'_>, dim: &str, id: u64, pos: [i32; 3], entities: Vec<(ntm_core::ledger::OwedEntity, String)>, owners: &[(String, [i32; 3])]) -> bool {
    let here = [pos[0] >> 4, pos[2] >> 4];
    let mut chunks: Vec<[i32; 2]> = vec![here];
    for (d, p) in owners {
        let c = [p[0] >> 4, p[2] >> 4];
        if d == dim && !chunks.contains(&c) {
            chunks.push(c);
        }
    }
    let loaded: Vec<[i32; 2]> = chunks.iter().copied().filter(|c| g.world.loaded(c[0] * 16, c[1] * 16)).collect();
    if loaded.iter().any(|c| ledger_knows(g, &chunk_key(dim, *c), id)) {
        return true;
    }
    if loaded.contains(&here) {
        spawn_entities(g, dim, &entities);
        return true;
    }
    let Some(owner) = loaded.first().copied() else { return false };
    let key = chunk_key(dim, owner);
    let mut ledger = ledger_get(g, &key);
    ledger.owe_entities(id, pos, entities);
    ledger_store(g, &key, ledger);
    true
}

/// The entities as `/summon` puts them in the level in place. One a mod keeps out counts as put out (the effect
/// is spent, as a dropped stack a hopper takes at once).
fn spawn_entities(g: &mut crate::game::Game<'_>, dim: &str, entities: &[(ntm_core::ledger::OwedEntity, String)]) {
    for (e, nbt) in entities {
        match horizon_world::nbt::parse_snbt(nbt).ok() {
            Some((Tag::Compound(c), _)) => {
                if g.native_summon(dim, &e.kind, e.pos, c).is_none() {
                    eprintln!("ntm: owed entity {} at {:?} was not spawned", e.kind, e.pos);
                }
            }
            _ => eprintln!("ntm: invalid owed entity NBT {nbt:?}"),
        }
    }
}

fn ledger_add(game: &mut crate::game::Game<'_>, dim: &str, id: u64, pos: [i32; 3], items: Vec<String>, owners: &[(String, [i32; 3])]) -> bool {
    in_place(game, dim, |g| ledger_add_here(g, dim, id, pos, items, owners)).unwrap_or(false)
}

/// The stacks (SNBT) as vanilla drops at the cell `pos`.
fn spawn_items(g: &mut crate::game::Game<'_>, pos: [i32; 3], items: &[String]) {
    for item in items {
        match horizon_world::nbt::parse_snbt(item).ok().and_then(|(t, _)| horizon_client::save::item_from_nbt(&t)) {
            Some(stack) => { g.spawn_item_own(stack, pos.map(|v| f64::from(v) + 0.5), 0); }
            None => eprintln!("ntm: invalid owed item NBT {item:?}"),
        }
    }
}

fn item_snbt(stack: &ItemStack) -> String {
    horizon_world::snbt_text::to_snbt(&horizon_client::save::item_to_nbt(stack))
}

/// Forgets the applied ids the storage no longer names, in every chunk kept that holds some.
fn ledger_prune(game: &mut crate::game::Game<'_>, live: &std::collections::BTreeSet<u64>) {
    let mut changed: Vec<(ChunkKey, ntm_core::ledger::ChunkLedger)> = Vec::new();
    for (key, ledger) in lock(&LEDGERS).iter_mut() {
        if !ledger.applied.is_empty() && ledger.prune(live) {
            changed.push((key.clone(), ledger.clone()));
        }
    }
    for (key, ledger) in changed {
        in_place(game, &key.0.clone(), |g| ledger_store(g, &key, ledger));
    }
}

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
    let ledgers = lock(&LEDGERS);
    let chunks = ledgers.values().filter(|l| !l.is_empty()).count();
    let applied: usize = ledgers.values().map(|l| l.applied.len()).sum();
    let pending: usize = ledgers.values().map(|l| l.owed.len()).sum();
    // What the loaded chunks' records owe (removals and batches), and what the storage the last save wrote keeps that
    // can hold items: zero in a world of this version (`kv_items`).
    let obligations: usize = ledgers.values().map(|l| l.removals.len() + l.batches.len()).sum();
    // What the loaded chunks' records took of an earlier version's storage (`migrated: X`).
    let migrated: usize = ledgers.values().map(|l| l.migrated.len()).sum();
    format!("ntmledger chunks={chunks} applied={applied} pending={pending} obligations={obligations} migrated={migrated} kv_items={} kv_writes={} kv_bytes={}", KV_ITEMS.load(std::sync::atomic::Ordering::Relaxed), KV_WRITES.load(std::sync::atomic::Ordering::Relaxed), KV_BYTES.load(std::sync::atomic::Ordering::Relaxed))
}

/// The stack name a loaded item entity of an earlier version carries.
fn entity_effect(g: &mut crate::game::Game<'_>, entity: u32) -> Option<String> {
    let uuid = *g.persist.uuids.get(&entity)?;
    g.entity_attachments.get_mut(&uuid)?.get_existing(&EFFECT_TYPE).ok().flatten().filter(|t| !t.is_empty())
}

/// The stack name a saved item entity of an earlier version carries.
fn saved_effect(entity: &Tag) -> Option<String> {
    let Ok(Some(data)) = horizon_world::attachments::attachment_compound(entity.get(horizon_world::attachments::ATTACHMENTS_KEY)) else {
        return None;
    };
    horizon_world::attachments::Attachments::from_nbt(data.clone()).get_existing(&EFFECT_TYPE).ok().flatten().filter(|t| !t.is_empty())
}

/// An earlier version's record of the chunk `key` (on the level in place) settled, once every loaded chunk that may hold
/// its stacks has its saved entities in: a folded stack's copies are removed from the world and from the entity files
/// of its unloaded holders, and a spawned stack no entity shows (loaded, or in an unloaded holder's file) is owed again
/// with what it held. False while a holder's entities are not in yet.
fn settle_legacy(g: &mut crate::game::Game<'_>, key: &ChunkKey, ledger: &mut ntm_core::ledger::ChunkLedger) -> bool {
    let now = g.ticks;
    let ready = |g: &crate::game::Game<'_>, c: [i32; 2]| -> bool {
        !g.world.loaded(c[0] * 16, c[1] * 16) || lock(&LOADED_AT).get(&chunk_key(&key.0, c)).is_some_and(|t| now >= t + 2)
    };
    let mut holders: Vec<[i32; 2]> = ledger.owed.iter().filter(|e| !e.pending()).flat_map(|e| e.holders()).collect();
    holders.extend(ledger.gone.values().flatten().copied());
    holders.push([key.1, key.2]);
    if !holders.iter().all(|c| ready(g, *c)) {
        return false;
    }
    let ids: Vec<u32> = g.dropped.iter().map(|d| d.id).collect();
    let loaded_tags: Vec<(u32, String)> = ids.into_iter().filter_map(|id| entity_effect(g, id).map(|t| (id, t))).collect();
    let in_file = |g: &crate::game::Game<'_>, c: [i32; 2], tag: &str| -> bool {
        !g.world.loaded(c[0] * 16, c[1] * 16) && g.save.load_entities(c[0], c[1]).iter().any(|e| saved_effect(e).as_deref() == Some(tag))
    };
    let copies = ledger.legacy(|e| {
        let tag = e.tag();
        loaded_tags.iter().any(|(_, t)| *t == tag) || e.holders().into_iter().any(|c| in_file(g, c, &tag))
    });
    for ((id, idx), holders) in copies {
        let tag = ntm_core::ledger::entity_tag(id, idx, [key.1, key.2]);
        for (eid, _) in loaded_tags.iter().filter(|(_, t)| *t == tag) {
            if let Some(i) = g.dropped.iter().position(|d| d.id == *eid) {
                g.remove_dropped(i);
            }
        }
        for c in holders.into_iter().chain([[key.1, key.2]]) {
            if !g.world.loaded(c[0] * 16, c[1] * 16) {
                let entities = g.save.load_entities(c[0], c[1]);
                let kept: Vec<Tag> = entities.iter().filter(|e| saved_effect(e).as_deref() != Some(tag.as_str())).cloned().collect();
                if kept.len() != entities.len() && let Err(e) = g.save.save_entities_later(vec![((c[0], c[1]), kept)]) {
                    eprintln!("ntm: settling an earlier record's copies in {c:?}: {e}");
                }
            }
        }
    }
    true
}

/// The chunks that owe stacks drop those whose cell is loaded now; an earlier version's record is settled first.
fn ledger_tick(game: &mut crate::game::Game<'_>) {
    let owing: Vec<ChunkKey> = lock(&OWING).iter().cloned().collect();
    for key in owing {
        in_place(game, &key.0.clone(), |g| {
            if !g.world.loaded(key.1 * 16, key.2 * 16) {
                lock(&OWING).remove(&key);
                return;
            }
            let mut ledger = ledger_get(g, &key);
            if ledger.has_legacy() && !settle_legacy(g, &key, &mut ledger) {
                return;
            }
            let drops = ledger.take_owed(|e| g.world.loaded(e.at[0], e.at[2]));
            for (at, item) in drops {
                spawn_items(g, at, &[item]);
            }
            let entities = ledger.take_owed_entities(|e| g.world.loaded(e.at[0], e.at[2]));
            spawn_entities(g, &key.0, &entities);
            ledger_store(g, &key, ledger);
        });
    }
}

/// What a chunk owes (`ntm_core::server::obligations`) put in its ledger, written with its record. False when it is not
/// loaded.
fn store_obligations(game: &mut crate::game::Game<'_>, dim: &str, chunk: [i32; 2], removals: &[ntm_core::persist::Removal], batches: &[ntm_core::server::CommittedEffects], migrated: &std::collections::BTreeSet<ntm_core::persist::Obligation>) -> bool {
    in_place(game, dim, |g| {
        if !g.world.loaded(chunk[0] * 16, chunk[1] * 16) {
            return false;
        }
        let key = chunk_key(dim, chunk);
        let mut ledger = ledger_get(g, &key);
        ledger.removals = removals.to_vec();
        ledger.batches = batches.to_vec();
        ledger.migrated = migrated.clone();
        ledger_store(g, &key, ledger);
        true
    })
    .unwrap_or(false)
}

/// What the records of chunks loaded owe (removals and batches) and took of an earlier version's storage, for the
/// server to take before their block entities are adopted.
type Arrived = (String, [i32; 2], Vec<ntm_core::persist::Removal>, Vec<ntm_core::server::CommittedEffects>, std::collections::BTreeSet<ntm_core::persist::Obligation>);
static ARRIVED: Mutex<Vec<Arrived>> = Mutex::new(Vec::new());

/// Chunks loaded (true) and unloaded (false) since the server last took them, in their order, for the world's
/// radiation by chunk (`Server::native_chunk_radiation_loaded`/`_unloaded`).
static RAD_CHUNKS: Mutex<Vec<(String, [i32; 2], bool)>> = Mutex::new(Vec::new());

/// What the chunks loaded since owe, and the chunks loaded and unloaded since, given to the server.
fn arrived(server: &mut Server) {
    for (dim, chunk, loaded) in std::mem::take(&mut *lock(&RAD_CHUNKS)) {
        if loaded {
            server.native_chunk_radiation_loaded(&dim, chunk);
        } else {
            server.native_chunk_radiation_unloaded(&dim, chunk);
        }
    }
    for (dim, chunk, removals, batches, migrated) in std::mem::take(&mut *lock(&ARRIVED)) {
        server.native_obligations_loaded(&dim, chunk, removals, batches, migrated);
    }
}

/// A chunk loaded: its ledger, if it has one, is read and kept.
pub(super) fn chunk_loaded(game: &mut crate::game::Game<'_>, dim: &str, chunk: [i32; 2]) {
    if dim_of(dim) != Some(game.dim) {
        return;
    }
    lock(&RAD_CHUNKS).push((dim.to_string(), chunk, true));
    let key = chunk_key(dim, chunk);
    lock(&LOADED_AT).insert(key.clone(), game.ticks);
    let ledger = ledger_load(game, chunk);
    if !ledger.owed.is_empty() || ledger.has_legacy() {
        lock(&OWING).insert(key.clone());
    }
    if !ledger.removals.is_empty() || !ledger.batches.is_empty() || !ledger.migrated.is_empty() {
        lock(&ARRIVED).push((dim.to_string(), chunk, ledger.removals.clone(), ledger.batches.clone(), ledger.migrated.clone()));
    }
    if !ledger.is_empty() {
        lock(&LEDGERS).insert(key, ledger);
    }
}

/// A chunk leaves, still in place (the engine's unload order, a local extension): the block changes not yet taken are,
/// a removed machine's contents out into the entities that leave with it, and what each chunk owes is in its record
/// before that record is written (R7-2).
pub(super) fn chunk_unloaded(game: &mut crate::game::Game<'_>, dim: &str, chunk: [i32; 2]) {
    // Before the server takes the block changes: the chunk's value is saved as it leaves.
    lock(&RAD_CHUNKS).push((dim.to_string(), chunk, false));
    geigers_unloaded(dim, chunk);
    if let Ok(mut guard) = SERVER.try_lock() && let Some((server, _)) = guard.as_mut() {
        let mut host = Host { game, buttons: Vec::new() };
        feed_changes(server, &mut host);
        server.native_chunk_unloading(&mut host);
    }
    let key = chunk_key(dim, chunk);
    lock(&OWING).remove(&key);
    lock(&LOADED_AT).remove(&key);
    lock(&LEDGERS).remove(&key);
}

/// The end of a server tick: the server's phases.
pub(super) fn server_tick(game: &mut crate::game::Game<'_>) {
    schedule_field_ticks(game);
    horizon_client::spans::time("ntm_ledger_tick", || ledger_tick(game));
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
    {
    let mut guard = lock(&SERVER);
    let changes = std::mem::take(&mut *lock(&CHANGES));
    let buttons = std::mem::take(&mut *lock(&BUTTONS));
    let mut host = Host { game: &mut *game, buttons };
    let (server, tick) = guard.get_or_insert_with(|| (Server::native(&mut host, 0), 0));
    *tick += 1;
    // Everything the region writer was given is on the disk: the last save that did not wait is confirmed.
    if host.game.save.writes_finished().unwrap_or(false) {
        server.native_writes_finished();
    }
    arrived(server);
    for (dim, pos, old, new) in changes {
        // Preserve each break; enrich placement properties only while the
        // recorded new block is still there. Owned role writes were tagged at
        // their synchronous callback, so they cannot masquerade as removals.
        let now = ntm_core::server::native_server::native_change_state(&new,host.game.native_loaded_block(&dim,pos));
        server.native_block_changed(&dim, pos, &old, &now);
    }
    horizon_client::spans::time("ntm_native_tick", || server.native_tick(&mut host, *tick));
    }
}

// ------------------------------------------------------------ the host

/// The server's view of the game.
struct Host<'a, 'g> {
    game: &'a mut crate::game::Game<'g>,
    buttons: Vec<(String, String, [i32; 3], u32)>,
}

/// The world's state outside its blocks (`ntm_core::persist`, the explosions in progress, the bombs' settings): kept in
/// memory while the world plays and written with its save, all keys in one file, `<world>/data/ntm/state`
/// (`store_commit`). A death outside a save brings it back as that save left it, with the regions and entities.
impl Kv for Host<'_, '_> {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        let dir = self.store_dir(key)?;
        with_store(&dir, |s| s.keys.get(key).cloned())
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        let dir = self.store_dir(key).ok_or_else(|| format!("key {key}"))?;
        KV_WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        KV_BYTES.fetch_add(value.len() as u64, std::sync::atomic::Ordering::Relaxed);
        with_store(&dir, |s| {
            if s.keys.get(key).is_none_or(|v| v != value) {
                s.keys.insert(key.to_string(), value.to_vec());
                s.changed = true;
            }
        });
        Ok(())
    }
    fn delete(&mut self, key: &str) {
        if let Some(dir) = self.store_dir(key) {
            with_store(&dir, |s| s.changed |= s.keys.remove(key).is_some());
        }
    }
}

impl Host<'_, '_> {
    /// The storage folder of the world open, for key `key` (letters, digits, `.`, `_`, `-`); none without a world.
    fn store_dir(&self, key: &str) -> Option<std::path::PathBuf> {
        let plain = !key.starts_with('.') && key.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b));
        let world = self.game.native_world_dir().filter(|_| plain && !key.is_empty())?;
        Some(world.join("data").join("ntm"))
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
pub(super) fn in_place<R>(game: &mut crate::game::Game<'_>, dim: &str, f: impl FnOnce(&mut crate::game::Game<'_>) -> R) -> Option<R> {
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

/// The contents of the cells machine commits consume, retained in the same publication.
fn consumed_contents(commits:&[ntm_core::server::MachineCommit])->Vec<ntm_core::server::RetainedDrop>{
    commits.iter().flat_map(|c|&c.cells).filter(|w|ntm_core::blockstate::id_of(&w.expected.state)!=ntm_core::blockstate::id_of(&w.desired)&&!w.expected.items.is_empty()).map(|w|ntm_core::server::RetainedDrop{origin:w.guard.clone(),items:w.expected.items.clone()}).collect()
}

impl Host<'_, '_> {
    fn publish_atomic(&mut self, commits: &[ntm_core::server::MachineCommit], extra:&[ntm_core::server::CellWrite],retained:&[ntm_core::server::RetainedDrop],wear_held:bool) -> bool {
        self.publish_atomic_with(commits,extra,retained,wear_held,None,&[])
    }
    /// `player`: a transfer's player side (`commit_player_transfer`), published by the engine as a whole or not at all,
    /// after every check of the machines' side and before their publication, which cannot fail.
    /// `templates`: machine slots that take a copy of a player's whole stack (`SlotCopy`), by dimension, cell and slot.
    fn publish_atomic_with(&mut self, commits: &[ntm_core::server::MachineCommit], extra:&[ntm_core::server::CellWrite],retained:&[ntm_core::server::RetainedDrop],wear_held:bool,player:Option<&mut dyn FnMut(&mut crate::game::Game<'_>)->bool>,templates:&[(String,[i32;3],usize,ItemStack)]) -> bool {
        horizon_client::spans::time("ntm_publish_atomic", || self.publish_atomic_timed(commits, extra, retained, wear_held, player, templates))
    }
    fn publish_atomic_timed(&mut self, commits: &[ntm_core::server::MachineCommit], extra:&[ntm_core::server::CellWrite],retained:&[ntm_core::server::RetainedDrop],wear_held:bool,player:Option<&mut dyn FnMut(&mut crate::game::Game<'_>)->bool>,templates:&[(String,[i32;3],usize,ItemStack)]) -> bool {
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
        // A transfer's player side: compared whole and published by the engine, or refused with nothing changed. The
        // machines' side passed every check above and nothing below can fail, so both land in this callback or neither.
        if let Some(player)=player && !player(self.game){return false;}
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
                let before_audio=match e.nbt.get(&JavaString::from(STATE)){Some(Tag::String(j))=>ntm_core::sounds::snapshot(&j.to_string_lossy()),_=>Vec::new()};
                for (i,(cell,new)) in e.items.iter_mut().zip(&c.desired).enumerate() {
                    // A copy of a player's stack keeps its every component (`ItemStack.copy()`).
                    let template=templates.iter().find(|t|t.0==c.dim&&t.1==c.pos&&t.2==i).map(|t|t.3.clone());
                    let old=cell.take();
                    *cell=item(new.as_ref(),template.or(old));
                }
                e.data=c.data.clone();
                e.nbt.insert(JavaString::from(DATA),Tag::IntArray(c.data.clone()));
                e.nbt.insert(JavaString::from(STATE),Tag::String(JavaString::from(c.json.as_str())));
                let after_signal=comparator_of(&e.id,match e.nbt.get(&JavaString::from(STATE)){Some(Tag::String(j))=>Some(j),_=>None});
                (ntm_core::native::sync_changed(sent.as_deref(),&c.data,&synced)||before_audio!=ntm_core::sounds::snapshot(&c.json),before_signal!=after_signal)
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
            let owed=self.game.visit_level(dim,|g|ledger_add_here(g,&dim_name,id,pos,items,&[])).expect("retained drop level passed preflight");
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
    fn trace_phases(&self)->bool{static TRACE:OnceLock<bool>=OnceLock::new();*TRACE.get_or_init(||std::env::var_os("NTM_TRACE_PHASES").is_some())}
    /// Items and retained contents are dropped once per effect id, which the ledger of their chunk keeps.
    fn replays_by_id(&self)->bool{true}
    fn cell_snapshot(&mut self,dim:&str,pos:[i32;3])->Option<ntm_core::server::CellSnapshot>{cell_snapshot(self.game,dim,pos)}
    fn drop_encoded_items(&mut self,dim:&str,pos:[i32;3],items:&[String])->bool{
        if self.game.native_loaded_block(dim,pos).is_none(){return false;}
        let items:Option<Vec<_>>=items.iter().map(|i|horizon_world::nbt::parse_snbt(i).ok().and_then(|(t,_)|horizon_client::save::item_from_nbt(&t))).collect();
        let Some(items)=items else{eprintln!("ntm: invalid retained item NBT at {dim} {pos:?}");return false};
        in_place(self.game,dim,|g|for i in items{g.spawn_item_own(i,pos.map(|v|f64::from(v)+0.5),0);}).is_some()
    }
    fn drop_items(&mut self,dim:&str,p:[i32;3],items:&[(String,u32)])->bool {
        if self.game.native_loaded_block(dim,p).is_none(){return false;}
        let at=p.map(|v|f64::from(v)+0.5);
        in_place(self.game,dim,|g|{
            for (id,n) in items {g.spawn_item_own(ItemStack::new(id,*n),at,0);}
        }).is_some()
    }
    fn apply_items(&mut self,id:u64,dim:&str,p:[i32;3],items:&[(String,u32)],owners:&[(String,[i32;3])])->bool{
        let stacks=items.iter().filter(|(_,n)|*n>0).map(|(i,n)|item_snbt(&ItemStack::new(i,*n))).collect();
        ledger_add(self.game,dim,id,p,stacks,owners)
    }
    fn apply_encoded_items(&mut self,id:u64,dim:&str,p:[i32;3],items:&[String],owners:&[(String,[i32;3])])->bool{
        let valid=items.iter().all(|i|horizon_world::nbt::parse_snbt(i).ok().and_then(|(t,_)|horizon_client::save::item_from_nbt(&t)).is_some());
        if !valid{eprintln!("ntm: invalid retained item NBT at {dim} {p:?}");return false;}
        ledger_add(self.game,dim,id,p,items.to_vec(),owners)
    }
    /// Each entity's compound must read and its position be finite before anything is spawned or owed.
    fn apply_entities(&mut self,id:u64,dim:&str,p:[i32;3],entities:&[ntm_core::server::EntitySpawn],owners:&[(String,[i32;3])])->bool{
        // An entity the engine cannot summon is refused, never spent: the effect waits, its owner frozen (P2-2).
        let summonable=|k:&str|self.game.ui.registries.summonable.iter().any(|s|s==k);
        let valid=entities.iter().all(|e|summonable(&e.kind)&&e.pos.iter().all(|v|v.is_finite())&&matches!(horizon_world::nbt::parse_snbt(&e.nbt),Ok((Tag::Compound(_),_))));
        if !valid{eprintln!("ntm: entity effect {id:016x} at {dim} {p:?} refused: a kind the engine cannot summon or an invalid compound; it waits");return false;}
        let owed=entities.iter().map(|e|(ntm_core::ledger::OwedEntity{kind:e.kind.clone(),pos:e.pos},e.nbt.clone())).collect();
        in_place(self.game,dim,|g|ledger_add_entities_here(g,dim,id,p,owed,owners)).unwrap_or(false)
    }
    fn ledger_prune(&mut self,live:&std::collections::BTreeSet<u64>){ledger_prune(self.game,live);}
    fn store_obligations(&mut self,dim:&str,chunk:[i32;2],removals:&[ntm_core::persist::Removal],batches:&[ntm_core::server::CommittedEffects],migrated:&std::collections::BTreeSet<ntm_core::persist::Obligation>)->bool{
        store_obligations(self.game,dim,chunk,removals,batches,migrated)
    }
    fn home_migrated(&mut self,dim:&str,chunk:[i32;2])->Option<std::collections::BTreeSet<ntm_core::persist::Obligation>>{
        in_place(self.game,dim,|g|{
            if !g.world.loaded(chunk[0]*16,chunk[1]*16){return None;}
            Some(ledger_get(g,&chunk_key(dim,chunk)).migrated)
        }).flatten()
    }
    fn home_loaded(&mut self,dim:&str,chunk:[i32;2])->bool{in_place(self.game,dim,|g|g.world.loaded(chunk[0]*16,chunk[1]*16)).unwrap_or(false)}
    fn cell_generation(&mut self,dim:&str,pos:[i32;3])->Option<u64>{
        self.entity(dim,pos,|e|match e.nbt.get(&JavaString::from(STATE)){
            Some(Tag::String(json))=>serde_json::from_str::<ntm_core::machine::Machine>(&json.to_string_lossy()).ok().map(|m|m.generation),
            _=>None,
        }).flatten()
    }
    fn effect_seen(&mut self,id:u64,dim:&str,pos:[i32;3])->bool{
        in_place(self.game,dim,|g|{
            let key=chunk_key(dim,[pos[0]>>4,pos[2]>>4]);
            g.world.loaded(pos[0],pos[2])&&ledger_knows(g,&key,id)
        }).unwrap_or(false)
    }
    fn pile_depletion(&mut self, stack: &Stack) -> f64 {
        stack.custom.as_ref().and_then(|c| horizon_world::nbt::parse_snbt(c).ok().map(|(tag,_)| tag)).as_ref().map_or(0.0, |t| pile_depletion(Some(t)))
    }
    fn fel_opaque(&mut self,dim:&str,p:[i32;3])->Option<bool> {
        self.game.native_loaded_block(dim,p)?;
        let target=dim_of(dim)?;
        let world=if target==self.game.dim {&self.game.world} else {self.game.levels.parked_world(target)?};
        Some(self.game.res.facts().get(world.block(p[0],p[1],p[2])).can_occlude)
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
                let sky = self.game.res.end_environment().sample(&horizon_client::environment::Sample{day_time:world.day_time,..Default::default()});
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
        self.publish_atomic(commits,&[],&consumed_contents(commits),false)
    }
    /// The acting player's stacks (`native_actor_uuid`), each with its whole SNBT, as a transfer compares them.
    fn player_slots(&mut self,player:&str)->Option<ntm_core::server::PlayerSlots>{
        use ntm_core::server::{HeldStack,PlayerSlot,PlayerSlots};
        if self.game.native_actor_uuid().as_deref()!=Some(player){return None;}
        let s=crate::game::ntm_items::transfer_snapshot(self.game)?;
        let held=|i:&Option<ItemStack>|stack(i).zip(i.as_ref()).map(|(stack,i)|HeldStack{stack,encoded:item_snbt(i)});
        let hand=match self.game.hand{crate::game::Hand::Main=>PlayerSlot::Inventory(self.game.selected),crate::game::Hand::Off=>PlayerSlot::Offhand};
        Some(PlayerSlots{hand:Some(hand),inventory:s.inventory.iter().map(held).collect(),offhand:held(&s.offhand),carried:held(&s.carried),creative:self.game.creative})
    }
    /// A transfer between the acting player and machines: the player's edited stacks compared by their whole SNBT and
    /// the machines' slots and generations, then both published in one exclusive step (`publish_atomic_with`).
    fn commit_player_transfer(&mut self,t:&ntm_core::server::PlayerTransfer,commits:&[ntm_core::server::MachineCommit])->bool{
        use crate::game::native_player_transactions::{NativeDrop,NativeDropMode};
        use ntm_core::server::{PlayerSlot,TransferDrop};
        if self.game.native_actor_uuid().as_deref()!=Some(t.player.as_str()){return false;}
        let slots:std::collections::BTreeSet<_>=t.edits.iter().map(|e|format!("{:?}",e.slot)).collect();
        if slots.len()!=t.edits.len(){return false;}
        // No cursor edit until T05 (`PlayerTransfer::publishable`): one published without an open menu loses the stack.
        if !t.publishable(){return false;}
        let Some(expected)=crate::game::ntm_items::transfer_snapshot(self.game) else{return false};
        let mut desired=expected.clone();
        for e in &t.edits{
            let cell=match e.slot{
                PlayerSlot::Inventory(i)=>match desired.inventory.get_mut(i){Some(c)=>c,None=>return false},
                PlayerSlot::Offhand=>&mut desired.offhand,
                PlayerSlot::Carried=>&mut desired.carried,
            };
            if cell.as_ref().filter(|s|s.count>0&&&*s.item!="minecraft:air").map(item_snbt)!=e.expected{return false;}
            *cell=item(e.desired.as_ref(),cell.take());
        }
        let mut drops=Vec::new();
        for (s,mode) in &t.drops{
            let Some(stack)=item(Some(s),None) else{return false};
            drops.push(NativeDrop::new(stack,match mode{TransferDrop::Thrown=>NativeDropMode::Thrown,TransferDrop::World{delay}=>NativeDropMode::World{delay:*delay}}));
        }
        // The machine slots that take a copy of a player's stack: the stack as the player held it, before the edits.
        let mut templates=Vec::new();
        for c in &t.copies{
            let from=match c.from{
                PlayerSlot::Inventory(i)=>expected.inventory.get(i).cloned().flatten(),
                PlayerSlot::Offhand=>expected.offhand.clone(),
                PlayerSlot::Carried=>expected.carried.clone(),
            };
            let Some(from)=from else{return false};
            let into=commits.iter().find(|m|m.dim==c.dim&&m.pos==c.pos).and_then(|m|m.desired.get(c.slot)).cloned().flatten();
            if into.is_none_or(|s|s.id!=*from.item||s.damage!=from.damage){return false;}
            templates.push((c.dim.clone(),c.pos,c.slot,from));
        }
        let mut player=|g:&mut crate::game::Game<'_>|crate::game::ntm_items::transfer_commit(g,&expected,desired.clone(),drops.clone());
        self.publish_atomic_with(commits,&[],&consumed_contents(commits),false,Some(&mut player),&templates)
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
    fn player_digamma(&mut self,id:&str,legacy:&ntm_core::radiation::Dose)->Option<f32>{
        self.game.native_visit_player(id,|g|crate::game::ntm_items::read_digamma(g,legacy)).flatten()
    }
    fn commit_player_digamma(&mut self,id:&str,legacy:&ntm_core::radiation::Dose,expected:f32,desired:f32)->bool{
        self.game.native_visit_player(id,|g|crate::game::ntm_items::write_digamma(g,legacy,expected,desired)).unwrap_or(false)
    }
    fn sync_digamma(&mut self,id:&str,legacy:&ntm_core::radiation::Dose){
        let _=self.game.native_visit_player(id,|g|crate::game::ntm_items::sync_digamma(g,legacy));
    }
    fn max_health(&mut self,id:&str)->Option<f64>{self.game.native_visit_player(id,|g|crate::game::ntm_items::max_health(g)).flatten()}
    fn worn_armor(&mut self,id:&str)->Option<[Option<String>;4]>{self.game.native_visit_player(id,|g|crate::game::ntm_items::worn_armor(g))}
    fn radiation_factor(&mut self,id:&str)->f64{self.game.native_visit_player(id,|g|crate::game::ntm_items::radiation_factor(g)).unwrap_or(1.0)}
    fn player_irradiation(&mut self,id:&str,legacy:&ntm_core::radiation::Dose)->Option<ntm_core::zirnox::StoredPotion>{
        self.game.native_visit_player(id,|g|crate::game::ntm_items::read_irradiation(g,legacy)).flatten()
    }
    fn commit_player_irradiation(&mut self,id:&str,legacy:&ntm_core::radiation::Dose,expected:ntm_core::zirnox::StoredPotion,desired:ntm_core::zirnox::StoredPotion)->bool{
        self.game.native_visit_player(id,|g|crate::game::ntm_items::write_irradiation(g,legacy,expected,desired)).unwrap_or(false)
    }
    /// `Entity.moveEntity` takes no step "onGround && isSneaking()" for a player, nor while `ridingEntity` is set
    /// (Onda 1, slice 5d, D3). The one place the NTM reads these, and the engine's API (NativePlayerInfo) does not
    /// carry them: engine fields read directly, at the pin of compatibility.json (`eb5d3eed58`):
    /// - a LAN guest: `lan::guest::Guest::{on_ground, sneaking, vehicle}` (crates/horizon-client/src/lan/guest.rs);
    /// - the player in place: `Player::ground.on_ground`, `Game::keys.held(Action::Sneak)` and `Game::riding`.
    ///
    /// A pin that renames or drops them breaks this build, never silently. Asked of the engine as API for later
    /// (docs/onda-1-protecao.md, slice 5d).
    fn step_blocked(&mut self,id:&str)->bool{
        let Ok(uuid)=u128::from_str_radix(&id.replace('-',""),16) else{return false};
        let game=&*self.game;
        if let Some(g)=game.lan.iter().flat_map(|h|h.hub.guests.iter()).find(|g|g.placed&&g.uuid==uuid){
            let guest:&horizon_client::lan::guest::Guest=g;
            return guest.on_ground&&guest.sneaking||guest.vehicle.is_some();
        }
        if game.level.player_uuid.map(horizon_client::lan::playerdata::ints_uuid)==Some(uuid){
            let on_ground:bool=game.player.as_ref().is_some_and(|p|p.ground.on_ground);
            let riding:&Option<u32>=&game.riding;
            return on_ground&&game.keys.held(horizon_client::actions::Action::Sneak)||riding.is_some();
        }
        false
    }
    /// A teleport takes no step (`setPositionAndUpdate` skips `Entity.moveEntity`; slice 5d, review P2-1). Read here,
    /// with `step_blocked`, from engine fields at the same pin:
    /// - a LAN guest: `Guest::teleport_counter` (the id of the last position the server set), `awaiting_teleport` and
    ///   `landing` (moves ignored until the client takes the new position);
    /// - the player in place: `Player::steps` (`MovementDistances::{movement, flight}`, `Entity.moveDist` and
    ///   `flyDist`), which grow by 0.6 x what the physics moved them: a horizontal move longer than that is a teleport,
    ///   also in a tick of walking (`walking::teleported_by_distance`; a fall moves no step distance either way, and
    ///   rides are blocked).
    ///
    /// Remembers each player's last mark; asked once a tick for each player.
    fn teleported(&mut self,id:&str)->bool{
        static SEEN:OnceLock<Mutex<HashMap<u128,(u64,f64,f64)>>>=OnceLock::new();
        let Ok(uuid)=u128::from_str_radix(&id.replace('-',""),16) else{return false};
        let game=&*self.game;
        let mut seen=lock(SEEN.get_or_init(Default::default));
        if let Some(g)=game.lan.iter().flat_map(|h|h.hub.guests.iter()).find(|g|g.placed&&g.uuid==uuid){
            let guest:&horizon_client::lan::guest::Guest=g;
            let counter:i32=guest.teleport_counter;
            let pending=guest.awaiting_teleport.is_some()||guest.landing;
            let last=seen.insert(uuid,(counter as u32 as u64,0.0,0.0));
            return pending||last.is_some_and(|(c,..)|c!=counter as u32 as u64);
        }
        if game.level.player_uuid.map(horizon_client::lan::playerdata::ints_uuid)==Some(uuid)
            && let Some(p)=game.player.as_ref()
        {
            let steps:horizon_world::movement_emission::MovementDistances=p.steps;
            let walked=steps.movement+steps.flight;
            let (x,z)=(p.position.x,p.position.z);
            let last=seen.insert(uuid,(u64::from(walked.to_bits()),x,z));
            return last.is_some_and(|(w,lx,lz)|ntm_core::walking::teleported_by_distance(((x-lx).powi(2)+(z-lz).powi(2)).sqrt(),walked-f32::from_bits(w as u32)));
        }
        false
    }
    fn effect_state(&mut self,id:&str,effect:&str)->Option<Option<(u32,u8)>>{
        self.game.native_visit_player(id,|g|crate::game::ntm_items::effect_state(g,effect))
    }
    fn remove_effect(&mut self,id:&str,effect:&str){
        self.game.native_visit_player(id,|g|crate::game::ntm_items::remove_effect(g,effect));
    }
    fn inventory_stacks(&mut self,id:&str)->Option<Vec<Stack>> {
        Some(self.game.native_player_inventory(id)?.into_iter().map(|c|Stack {id:c.item,count:c.count,damage:0,custom:c.custom_data}).collect())
    }
    fn inventory(&mut self, id: &str) -> Option<Vec<(String, u32)>> {
        Some(self.game.native_player_inventory(id)?.into_iter().map(|c| (c.item, c.count)).collect())
    }
    fn fel_fire(&mut self, id: &str, ticks: u32) {
        let _ = self.game.native_visit_player(id, |g| {
            if let Some(p) = g.player.as_mut() { p.set_remaining_fire_ticks(p.fire_ticks.max(ticks as i32)); }
        });
    }
    fn hurt(&mut self, id: &str, amount: f32, kind: &str) {
        if let Err(e) = self.game.native_hurt(id, amount, kind) {
            eprintln!("ntm: hurt {id}: {e}");
        }
    }
    fn effect(&mut self, id: &str, effect: &str, ticks: u32, amplifier: u8) {
        // The test runs count the sickness rolls by band from this line (tools/test-game/native_items.py).
        static TRACE: OnceLock<bool> = OnceLock::new();
        if *TRACE.get_or_init(|| std::env::var_os("NTM_TRACE_ITEMS").is_some()) {
            eprintln!("ntm trace effect {}", serde_json::json!({"tick": self.game.world.game_time, "player": id, "effect": effect, "ticks": ticks, "amplifier": amplifier}));
        }
        if let Err(e) = self.game.native_effect(id, effect, ticks, amplifier, true) {
            eprintln!("ntm: effect {id}: {e}");
        }
    }
    fn world_seed(&mut self) -> i64 {
        self.game.level.seed
    }
    /// Particles and sounds in the dimension in play (`native_send_particles`,
    /// API 11; `native_play_sound`, API 32).
    /// The motion-blocking heights of the level being ticked (`native_surface_heights`, API 127): another level's
    /// columns, or a refused read, are not loaded. The engine's map leaves the NTM's blocks out (an engine ticket):
    /// where the world-surface map, which counts them, is above it, the highest NTM block that blocks motion between
    /// the two is the top (`chunk_radiation::height_with_ntm`, the bridge of 4c).
    fn surface_heights(&mut self, dim: &str, columns: &[(i32, i32)]) -> Vec<Option<i32>> {
        use super::native_world_edit::NativeHeightmap;
        let none = || vec![None; columns.len()];
        let Ok(motion) = self.game.native_surface_heights(dim, columns, NativeHeightmap::MotionBlocking) else { return none() };
        let Ok(surface) = self.game.native_surface_heights(dim, columns, NativeHeightmap::WorldSurface) else { return none() };
        columns
            .iter()
            .zip(motion.iter().zip(&surface))
            .map(|(&(x, z), (m, s))| match (*m, *s) {
                (Some(m), Some(s)) if s > m => Some(ntm_core::chunk_radiation::height_with_ntm(m, s, |y| self.game.native_loaded_block(dim, [x, y, z]))),
                (m, _) => m,
            })
            .collect()
    }
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
            // The NTM's own particles (the radiation fog's quads): a mod particle type, by its namespaced name.
            let mut options = match ParticleType::from_name(id) {
                Some(ty) => ParticleOptions::simple(ty),
                None if id.starts_with("ntm:") => {
                    let mut options = ParticleOptions::simple(ParticleType::Smoke);
                    options.mod_particle = Some(id.to_string());
                    options
                }
                None => return,
            };
            options.color = *color;
            (id, game.with_level(|_, level, _| level.native_send_particles(&options, None, *at, *spread, *speed, *count)))
        }
        Show::Sound { id, source, at, volume, pitch } => {
            let Some(source) = horizon_client::settings::SOUND_SOURCES.iter().position(|s| s == source) else { return };
            (id, game.with_level(|_, level, _| level.native_play_sound(id, *at, source, *volume, *pitch)))
        }
        Show::NuclearSound { at } => {
            let data=serde_json::to_vec(&ntm_core::sounds::NuclearCue{dimension:format!("{:?}",game.dim),at:*at}).expect("finite explosion position");
            for player in game.native_players().into_iter().filter(|p|dim_of(&p.dimension)==Some(game.dim)).collect::<Vec<_>>() {
                if let Ok(uuid)=u128::from_str_radix(&player.uuid.replace('-',""),16)
                    && let Err(error)=game.native_net_send_to_player(uuid,"ntm:nuclear_sound",&data) {
                    eprintln!("ntm nuclear audio: {error}");
                }
            }
            return;
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
    if entity.is_none() && (is_machine(block) || block == GEIGER) {
        *entity = Some((block.to_string(), Compound::new()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if is_machine(block) || block == GEIGER {
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
        _ if ntm_core::native::is_brick_furnace(block) => Some(SlotPolicy(brick_slot_policy)),
        ntm_core::keyforge::BLOCK => Some(SlotPolicy(keyforge_slot_policy)),
        ntm_core::funnel::BLOCK => Some(SlotPolicy(funnel_slot_policy)),
        ntm_core::radiolysis::BLOCK => Some(SlotPolicy(radiolysis_slot_policy)),
        ntm_core::exposure::BLOCK => Some(SlotPolicy(exposure_slot_policy)),
        ntm_core::fel::BLOCK => Some(SlotPolicy(fel_slot_policy)),
        ntm_core::annihilator::BLOCK => Some(SlotPolicy(annihilator_slot_policy)),
        ntm_core::cyclotron::BLOCK => Some(SlotPolicy(cyclotron_slot_policy)),
        ntm_core::furnaces::IRON => Some(SlotPolicy(iron_furnace_slot_policy)),
        ntm_core::furnaces::STEEL => Some(SlotPolicy(steel_furnace_slot_policy)),
        ntm_core::furnaces::COMBINATION => Some(SlotPolicy(combination_furnace_slot_policy)),
        ntm_core::welders::ARC => Some(SlotPolicy(arc_welder_slot_policy)),
        ntm_core::welders::SOLDER => Some(SlotPolicy(soldering_slot_policy)),
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
fn keyforge_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::keyforge::BLOCK, slots, slot, item, action, origin)
}
fn funnel_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::funnel::BLOCK, slots, slot, item, action, origin)
}
fn annihilator_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::annihilator::BLOCK, slots, slot, item, action, origin)
}
fn cyclotron_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::cyclotron::BLOCK, slots, slot, item, action, origin)
}
fn fel_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::fel::BLOCK, slots, slot, item, action, origin)
}
fn exposure_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::exposure::BLOCK, slots, slot, item, action, origin)
}
fn radiolysis_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::radiolysis::BLOCK, slots, slot, item, action, origin)
}
fn arc_welder_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::welders::ARC, slots, slot, item, action, origin)
}
fn soldering_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::welders::SOLDER, slots, slot, item, action, origin)
}
fn combination_furnace_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::furnaces::COMBINATION, slots, slot, item, action, origin)
}
fn steel_furnace_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::furnaces::STEEL, slots, slot, item, action, origin)
}
fn iron_furnace_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy(ntm_core::furnaces::IRON, slots, slot, item, action, origin)
}
fn brick_slot_policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
    action: horizon_client::containers::SlotAction, origin: horizon_client::containers::SlotOrigin) -> bool {
    machine_slot_policy("ntm:machine_furnace_brick_off", slots, slot, item, action, origin)
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
    // Keep the update tag compact: the server already computed these loops
    // from transient processing flags before committing the TE snapshot.
    let audio = match nbt.get(&JavaString::from(STATE)) {
        Some(Tag::String(json))=>ntm_core::sounds::snapshot(&json.to_string_lossy()),
        _=>Vec::new(),
    };
    t.insert(JavaString::from("ntm_audio"),Tag::String(JavaString::from(serde_json::to_string(&audio).expect("finite audio state").as_str())));
    *tag = Some(t);
}

/// A slot as ntm-core sees it: id, count and damage (an empty slot, or air,
/// is none).
pub(super) fn stack(s: &Option<ItemStack>) -> Option<Stack> {
    let s = s.as_ref().filter(|s| s.count > 0 && &*s.item != "minecraft:air")?;
    let mut t = Stack::new(&s.item, s.count);
    t.damage = s.damage;
    // The items whose custom data a machine reads: rods and pellets, the fluid identifier (its type), the broken item (its product), and the keys and locks (their pins).
    if ntm_core::pile::Rod::of(&s.item,0.0).is_some() || ntm_core::zirnox::fuel(&s.item).is_some() || ntm_core::radioisotopes::pellet(&s.item).is_some() || ntm_core::rbmk::persist::rod_of(&s.item).is_some()
        || &*s.item == ntm_core::server::native_server::IDENTIFIER || &*s.item == ntm_core::broken::ID
        || ntm_core::keyforge::is_pin_item(&s.item) {
        t.custom = s.data().and_then(|d|d.other("minecraft:custom_data")).map(horizon_world::snbt_text::to_snbt);
    } else if let Some(custom) = s.data().and_then(|d|d.other("minecraft:custom_data")).map(horizon_world::snbt_text::to_snbt)
        // Any item may carry the contagion (`ntmContagion`) the radiolysis chamber sterilizes.
        && custom.contains(ntm_core::radiolysis::CONTAGION) {
        t.custom = Some(custom);
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
    if let Some(it) = result.as_mut() && let Some(custom) = &new.custom && let Ok((mut tag,_)) = horizon_world::nbt::parse_snbt(custom) {
        if ntm_core::rbmk::persist::rod_of(&new.id).is_some() {
            rod_doubles(&mut tag);
        }
        it.data_mut().set_other("minecraft:custom_data",Some(tag));
    } else if let Some(it) = result.as_mut() && new.custom.is_none()
        && it.data().and_then(|d|d.other("minecraft:custom_data")).map(horizon_world::snbt_text::to_snbt).is_some_and(|c| c.contains(ntm_core::radiolysis::CONTAGION)) {
        // The logic took the contagion off the item's only custom data: it has none left.
        it.data_mut().set_other("minecraft:custom_data",None);
    }
    result
}

/// The engine's SNBT, as Java's, reads the bare words of a non-finite double (`NaNd`, `Infinityd`) as text: an RBMK
/// rod's four doubles come back as the doubles they were written as (P2-1), their bits exact (a NaN canonical, as Java's
/// `writeDouble` keeps it).
fn rod_doubles(tag: &mut Tag) {
    let Tag::Compound(c) = tag else { return };
    for key in ["yield", "xenon", "core", "hull"] {
        let k = JavaString::from(key);
        let v = match c.get(&k) {
            Some(Tag::String(s)) => match s.to_string_lossy().as_ref() {
                "NaNd" | "NaND" => f64::NAN,
                "Infinityd" | "InfinityD" => f64::INFINITY,
                "-Infinityd" | "-InfinityD" => f64::NEG_INFINITY,
                _ => continue,
            },
            _ => continue,
        };
        c.insert(k, Tag::Double(v.to_bits()));
    }
}

/// A block entity's server tick: the machine is only adopted by the server
/// from its saved state when the server doesn't know it (the world was
/// opened again); the server's phases run it at the tick's end.
pub(super) fn tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    // An inert machine besides (as the decontaminator): the server knows it, the glue ticks it.
    if entity.id == GEIGER {
        geiger_tick(game, pos);
    }
    if !is_machine(&entity.id) {
        return;
    }
    let dim = game.dim.id().to_string();
    if let Some((server, _)) = lock(&SERVER).as_mut()
        && { arrived(server); !server.native_has(&dim, pos) }
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

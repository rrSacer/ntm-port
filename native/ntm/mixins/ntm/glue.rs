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

/// A flushed world save makes already applied item/block effects durable.
pub(super) fn world_saved(game:&mut crate::game::Game<'_>,wait:bool){
    if let Some((server,_))=lock(&SERVER).as_mut(){
        let mut host=Host{game,buttons:Vec::new()};
        server.native_world_saved(&mut host,wait);
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
pub(super) fn use_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], core: [i32; 3], face: [i32; 3]) -> bool {
    let mut guard = lock(&SERVER);
    let Some((server, _)) = guard.as_mut() else { return false };
    let dim = game.dim.id().to_string();
    let player = game.native_actor_uuid().unwrap_or_default();
    let held = game.held().cloned();
    let pile_stack = stack(&held);
    let depletion = held.as_ref().map_or(0.0, |h| pile_depletion(h.data().and_then(|d| d.other("minecraft:custom_data"))));
    let mut host = Host { game, buttons: Vec::new() };
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
            if let Some(h) = game.held_mut() { h.count -= 1; if h.count == 0 { *game.held_mut() = None; } }
            server.native_stirling_repair(&dim,core,&held.item);
            return true;
        }
    }
    let mut host = Host { game, buttons: Vec::new() };
    server.native_use(&mut host, &dim, core, pos, &player)
}

/// A player used `stack` on the block at `pos`: the fluid identifier on a
/// duct sets its fluid (`custom_data` `ntm_fluid`). True when taken.
pub(super) fn use_item(game: &mut crate::game::Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3]) -> bool {
    if matches!(&*stack.item,"ntm:hand_drill"|"ntm:hand_drill_desh") { return use_block(game,pos,pos,face); }
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
        let world = self.game.levels.parked_world_mut(target)?;
        let Some(horizon_client::containers::BlockEntity::Mod(m)) = world.block_entities.get_mut(&(pos[0], pos[1], pos[2])) else {
            return None;
        };
        if m.id.is_empty() {
            return None;
        }
        let out = f(m);
        // Its chunk is saved next time, as `setChanged` would.
        world.modified.insert((pos[0] >> 4, pos[2] >> 4));
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

impl ServerHost for Host<'_, '_> {
    fn drop_items(&mut self,dim:&str,p:[i32;3],items:&[(String,u32)])->bool {
        let at=p.map(|v|f64::from(v)+0.5);
        in_place(self.game,dim,|g|{
            for (id,n) in items {g.spawn_item(ItemStack::new(id,*n),at,crate::game::dispenser::new_item_motion(),0);}
        }).is_some()
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
        in_place(self.game, dim, |g| g.native_explode(dim, at, 12.0, true, crate::game::native_explosions::NativeBlast::Block)).unwrap_or(false)
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
        // This callback owns &mut Game for its entire duration. No event,
        // hopper, chunk unload or block mutation can interleave these passes.
        // Resolve ALL entities and validate ALL expected slots before changing
        // any. Notifications run only after every member has been published.
        let mut prepared=Vec::with_capacity(commits.len());
        let mut seen=std::collections::BTreeSet::new();
        for c in commits {
            if !seen.insert((c.dim.clone(),c.pos)) { return false; }
            let Some((id,valid))=self.entity(&c.dim,c.pos,|e| {
                let observed:Slots=e.items.iter().map(stack).collect();
                (e.id.clone(),observed==c.expected && e.items.len()==c.desired.len())
            }) else {return false};
            if !valid {return false;}
            let (_,synced)=file(self.game,&id);
            prepared.push(synced);
        }
        let mut notify=Vec::new();
        for (c,synced) in commits.iter().zip(prepared) {
            // The preflight guarantees this entity is still accessible; this
            // closure performs the entire update without any fallible operation.
            let changed=self.entity(&c.dim,c.pos,|e| {
                assert_eq!(e.items.iter().map(stack).collect::<Slots>(),c.expected,"exclusive preflight preserves expected slots");
                assert_eq!(e.items.len(),c.desired.len());
                let sent=match e.nbt.get(&JavaString::from(DATA)) {Some(Tag::IntArray(v))=>Some(v.clone()),_=>None};
                for (cell,new) in e.items.iter_mut().zip(&c.desired) {
                    *cell=item(new.as_ref(),cell.take());
                }
                e.data=c.data.clone();
                e.nbt.insert(JavaString::from(DATA),Tag::IntArray(c.data.clone()));
                e.nbt.insert(JavaString::from(STATE),Tag::String(JavaString::from(c.json.as_str())));
                ntm_core::native::sync_changed(sent.as_deref(),&c.data,&synced)
            }).expect("atomic preflight retains entity during exclusive Game access");
            if changed {notify.push((c.dim.clone(),c.pos));}
        }
        for (dim,p) in notify {
            if dim_of(&dim)==Some(self.game.dim){self.game.native_block_entity_changed(p);}
            else if self.watched(&dim){in_place(self.game,&dim,|g|g.native_block_entity_changed(p));}
        }
        true
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
pub(super) fn stack(s: &Option<ItemStack>) -> Option<Stack> {
    let s = s.as_ref().filter(|s| s.count > 0 && &*s.item != "minecraft:air")?;
    let mut t = Stack::new(&s.item, s.count);
    t.damage = s.damage;
    if ntm_core::pile::Rod::of(&s.item,0.0).is_some() || ntm_core::zirnox::fuel(&s.item).is_some() {
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
    let Some(Tag::String(state)) = entity.nbt.get(&JavaString::from(STATE)) else { return };
    let dim = game.dim.id().to_string();
    if let Some((server, _)) = lock(&SERVER).as_mut()
        && !server.native_has(&dim, pos)
    {
        server.native_adopt(&dim, pos, &state.to_string_lossy());
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

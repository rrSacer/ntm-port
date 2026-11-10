//! The server as the native bundle runs it (docs/nativo.md): machines,
//! energy, fluid networks, foundry, bombs, over a [`ServerHost`] the bundle's
//! glue implements. Each machine's state is its block entity's
//! (`native_server.rs`); the world keeps the rest ([`persist`]).

use crate::{
    Pos,
    blockstate::{self, BlockState},
    energy::{self, EnergyNode, Topology},
    kv::Kv,
    machine::{Kind, Machine},
    persist::WorldState,
    registry,
};
use std::collections::{BTreeMap, BTreeSet};

/// What the engine needs from the game. Its [`Kv`] keeps the world's state
/// outside the machines ([`crate::persist`]).
pub trait ServerHost: Kv {
    /// NativeCarried includes custom data already; older hosts keep their
    /// plain inventory contract through this default adapter.
    fn inventory_stacks(&mut self,id:&str)->Option<Vec<container_bridge::Stack>> {self.inventory(id).map(|v|v.into_iter().map(|(id,n)|container_bridge::Stack::new(&id,n)).collect())}
    /// `inventory_stacks` without the components only an RBMK rod's tag flag reads (`Stack::tagged`), cheap enough for
    /// every tick: what decides a rod's heat is its custom data.
    fn carried_stacks(&mut self,id:&str)->Option<Vec<container_bridge::Stack>> {self.inventory_stacks(id)}

    fn block_resistance(&mut self,dim:&str,p:Pos)->Option<f32> {self.get_block(dim,p).map(|b|crate::blast::vanilla::resistance(&b).max(0.))}

    /// Native explosion (power 12, fire and block destruction) in its own level.
    fn zirnox_explode(&mut self, _dim: &str, _at: [f64;3]) -> bool { false }
    fn get_block(&mut self, dim: &str, p: Pos) -> Option<String>;
    fn set_blocks(&mut self, dim: &str, blocks: &[(Pos, String)]) -> Result<(), String>;
    /// Role changes already represented by the confirmed model. Hosts may
    /// suppress their synchronous notifications for these owner cells only.
    fn set_machine_blocks(&mut self,dim:&str,blocks:&[(Pos,String)],_owners:&[(String,Pos)])->Result<(),String>{self.set_blocks(dim,blocks)}
    fn tell(&mut self, player: &str, text: &str);
    fn log(&mut self, text: &str);
    /// Read-only diagnostic logging; never changes simulation or RNG.
    fn trace_effects(&self)->bool{false}
    /// Whether a slow tick logs how long each of its phases took (`Server::native_tick`).
    fn trace_phases(&self)->bool{false}
    /// Saved sky light, weather/time subtraction, and visibility at y+1.
    /// Pile rod ejection deliberately removes depletion, as the Java does.
    fn pile_drop(&mut self, dim: &str, p: Pos, item: &str) -> bool { self.drop_items(dim,p,&[(item.into(),1)]) }
    /// Read the original double `depletion` from an item's custom data.
    fn pile_depletion(&mut self, _stack: &container_bridge::Stack) -> f64 { 0. }
    /// Native Level.explode(null, ..., 15, true, BLOCK).
    /// False when the explosion could not be applied (other dimension, unloaded chunk).
    fn pile_explode(&mut self, _dim: &str, _at: [f64;3]) -> bool { true }
    /// Whether the host can reach `dim` (it is open): its pile rods can be
    /// ejected and exploded there, through whatever level switching the host
    /// needs. False freezes its piles rather than lose their rods.
    fn pile_reachable(&mut self, _dim: &str) -> bool { true }
    fn solar_sample(&mut self, _dim: &str, _p: Pos) -> Option<(i32, i32, bool)> { None }
    /// The slots of the machine's block entity at `p`.
    fn machine_slots(&mut self, _dim: &str, _p: Pos) -> Option<container_bridge::Slots> {
        None
    }
    /// Atomically compare all observed slots, then publish slots, menu data and
    /// JSON together. Refusal MUST publish nothing. Unsupported hosts freeze.
    fn commit_machine(&mut self, _commit: &MachineCommit) -> bool { false }
    /// Same guarantee across a coupled group (pile loaders and their core).
    fn commit_machines(&mut self, commits: &[MachineCommit]) -> bool {
        commits.is_empty() || commits.len() == 1 && self.commit_machine(&commits[0])
    }
    /// Full BE snapshot and lossless item NBT for cells consumed by a machine.
    fn cell_snapshot(&mut self, dim:&str, pos:Pos)->Option<CellSnapshot>{
        self.get_block(dim,pos).map(|state|CellSnapshot{state,entity:None,items:Vec::new()})
    }
    /// Lossless retained BE contents, released only after the world/BE commit.
    fn drop_encoded_items(&mut self,_dim:&str,_pos:Pos,_items:&[String])->bool{false}
    /// Atomic structural publication plus retained, lossless old-BE drops.
    /// Unsupported hosts refuse; there is no partial setter fallback.
    fn commit_structural(&mut self,_cells:&[CellWrite],_drops:&[RetainedDrop],_owners:&[(String,Pos)])->bool{false}
    /// Puts the items of the effect `id` into the world exactly once: the host's ledger remembers the id, so
    /// applying the same effect again (a replay after a crash) adds nothing. `owners` are the machines whose step
    /// put them out, whose records hold what the step consumed: the id belongs with them. Hosts without a ledger
    /// just drop.
    fn apply_items(&mut self,_id:u64,dim:&str,pos:Pos,items:&[(String,u32)],_owners:&[(String,Pos)])->bool{self.drop_items(dim,pos,items)}
    /// Whether `apply_items`, `apply_encoded_items` and the retained drops of a structural commit are kept under the
    /// effect's id in the record of the chunk that receives them, written with that chunk: a replay finds the id in
    /// any record that holds the items and adds nothing, as a block effect finds its own. Then items need no journal
    /// write before the next effect; only an explosion does.
    fn replays_by_id(&self)->bool{false}
    /// Puts the entities of the effect `id` into the world exactly once, as `apply_items` does its stacks: spawned when
    /// the chunk of `pos` is loaded, else owed in the record of a loaded owner's chunk and spawned once it loads; an id
    /// the ledger knows adds nothing. Hosts without entities refuse (the effect waits, nothing is lost).
    fn apply_entities(&mut self,_id:u64,_dim:&str,_pos:Pos,_entities:&[EntitySpawn],_owners:&[(String,Pos)])->bool{false}
    /// Entities a machine puts out outside a step: inside one they become a [`WorldEffect::Entities`]. Hosts without
    /// entities refuse.
    fn spawn_entities(&mut self,_dim:&str,_pos:Pos,_entities:&[EntitySpawn])->bool{false}
    /// `apply_items` for lossless item NBT.
    fn apply_encoded_items(&mut self,_id:u64,dim:&str,pos:Pos,items:&[String],_owners:&[(String,Pos)])->bool{self.drop_encoded_items(dim,pos,items)}
    /// The generation of the machine the cell's block entity was saved with (None: no such block entity).
    fn cell_generation(&mut self,_dim:&str,_pos:Pos)->Option<u64>{None}
    /// Whether the chunk of `pos` already holds the effect `id` (it was saved after the effect was applied).
    fn effect_seen(&mut self,_id:u64,_dim:&str,_pos:Pos)->bool{false}
    /// The effect `id` was applied to `cells`: each cell's chunk records it with the blocks it just changed.
    fn effect_applied(&mut self,_id:u64,_cells:&[(String,Pos)]){}
    /// The ids of every effect the storage still names: a ledger may forget the retired ones not listed.
    fn ledger_prune(&mut self,_live:&std::collections::BTreeSet<u64>){}
    /// Puts what the chunk `chunk` of `dim` owes the world (`server::obligations`) in its record, in place of what it
    /// held, with the earlier version's ones it took (`migrated`): written with the chunk's block entities at its next
    /// save. False when the chunk is not loaded.
    fn store_obligations(&mut self,_dim:&str,_chunk:[i32;2],_removals:&[crate::persist::Removal],_batches:&[CommittedEffects],_migrated:&BTreeSet<crate::persist::Obligation>)->bool{true}
    /// The earlier version's removals and batches the record of the loaded chunk `chunk` of `dim` took (`migrated`),
    /// as the record holds them now; None when the chunk is not loaded.
    fn home_migrated(&mut self,_dim:&str,_chunk:[i32;2])->Option<BTreeSet<crate::persist::Obligation>>{Some(BTreeSet::new())}
    /// Whether the chunk `chunk` of `dim` is loaded: what it owes is played only then, as a block entity ticks.
    fn home_loaded(&mut self,_dim:&str,_chunk:[i32;2])->bool{true}
    /// A pile's formation: the cells' new block entities with their first state,
    /// and the held drill's wear when `wear_held`, published in one step. Refusal
    /// changes nothing, the tool included. Defaults to the structural commit,
    /// which wears nothing.
    fn commit_formation(&mut self,cells:&[CellWrite],wear_held:bool,owners:&[(String,Pos)])->bool{
        !wear_held && self.commit_structural(cells,&[],owners)
    }
    fn machine_data(&mut self, _dim: &str, _p: Pos) -> Option<Vec<i32>> {
        None
    }
    /// Machine menu buttons pressed: player, dimension, position, button.
    fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
        Vec::new()
    }
    /// The redstone signal each of `cells` of `dim` receives (0..=15; none
    /// unloaded, or a host that can't read it: then a bomb reads its `lit`).
    fn redstone_many(&mut self, _dim: &str, cells: &[Pos]) -> Vec<Option<u8>> {
        vec![None; cells.len()]
    }
    /// Items a broken machine held outside its slots (the slots the game
    /// spills itself), dropped at `p`.
    fn drop_items(&mut self, _dim: &str, _p: Pos, _items: &[(String, u32)]) -> bool { false }
    /// The players in the world (None: the host can't tell, and
    /// [`Server::set_players`] says).
    fn players(&mut self) -> Option<Vec<PlayerInfo>> {
        None
    }
    /// What player `id` carries: (item, count).
    fn inventory(&mut self, _id: &str) -> Option<Vec<(String, u32)>> {
        None
    }
    /// Persistent player-file dose; old hosts retain the world-state adapter.
    fn player_dose(&mut self, _id:&str, legacy:&crate::radiation::Dose)->Option<crate::radiation::Dose>{Some(legacy.clone())}
    fn commit_player_dose(&mut self,_id:&str,_expected:&crate::radiation::Dose,_desired:&crate::radiation::Dose)->bool{true}
    fn radiation_factor(&mut self,_id:&str)->f64{1.0}
    /// Damage to player `id` of a registered type (`minecraft:magic`); the
    /// host skips creative and spectator players.
    fn hurt(&mut self, _id: &str, _amount: f32, _kind: &str) {}
    /// A status effect on player `id` (`poison`).
    fn effect(&mut self, _id: &str, _effect: &str, _seconds: u32, _amplifier: u8) {}
    /// `HazardTypeHot.onUpdate`: `if(!target.isWet()) target.setFire(seconds)`, player `id` burning for at least
    /// `seconds` (less fire protection) unless in water or rain.
    fn set_fire_unless_wet(&mut self, _id: &str, _seconds: i32) {}
    /// Particles or a sound in `dim` (`crate::blast::visuals::Show`, not
    /// its effects).
    fn show(&mut self, _dim: &str, _show: &crate::blast::visuals::Show) {}
    /// A named number player `id`'s client reads (the Geiger's).
    fn set_player_value(&mut self, _id: &str, _key: &str, _value: i64) {}
    /// The stacks of the player acting in this callback (`player` their UUID), as a transfer reads and compares them.
    /// None: another player, or a host without transfers.
    fn player_slots(&mut self, _player: &str) -> Option<PlayerSlots> {
        None
    }
    /// One indivisible transfer between a player and machines (`transfer`): every expected stack of the player and
    /// every expected slot and generation of `commits` compared first, then the player's stacks, the machines' slots,
    /// data and JSON, and the player's drops published together. Refusal MUST publish nothing on either side.
    /// Unsupported hosts refuse: there is no fallback that debits one side after the other.
    fn commit_player_transfer(&mut self, _transfer: &PlayerTransfer, _commits: &[MachineCommit]) -> bool {
        false
    }
    /// An RBMK lid off its column (`RBMKBase.onScrew`): the column's commit and the lid owed at `drop.origin` (in its
    /// chunk's record, or dropped there) published together, and the acting player's held `tool` worn in the same
    /// publication, refused as a whole if it is no longer in hand. Unsupported hosts refuse.
    fn commit_lid_removal(&mut self, _commit: &MachineCommit, _drop: &RetainedDrop, _tool: &str) -> bool {
        false
    }
    /// The height RBMK columns are placed with from now on (`dialColumnHeight`, 2..16 cells): standing ones keep theirs.
    fn set_column_height(&mut self, _height: i32) {}
}

/// A stack a player holds: what the logic reads of it, and the whole stack (SNBT, every component) the host compares
/// before a transfer publishes anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldStack {
    pub stack: container_bridge::Stack,
    pub encoded: String,
}

/// Where a player's stack is: an inventory slot (the engine's index, the hotbar first), the off hand or the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerSlot {
    Inventory(usize),
    Offhand,
    Carried,
}

/// The stacks of the acting player a transfer may touch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerSlots {
    /// The stack that acts: the selected hotbar slot, or the off hand.
    pub hand: Option<PlayerSlot>,
    pub inventory: Vec<Option<HeldStack>>,
    pub offhand: Option<HeldStack>,
    pub carried: Option<HeldStack>,
    pub creative: bool,
}

impl PlayerSlots {
    pub fn get(&self, slot: PlayerSlot) -> Option<&HeldStack> {
        match slot {
            PlayerSlot::Inventory(i) => self.inventory.get(i).and_then(Option::as_ref),
            PlayerSlot::Offhand => self.offhand.as_ref(),
            PlayerSlot::Carried => self.carried.as_ref(),
        }
    }
    /// The stack in the hand that acts.
    pub fn held(&self) -> Option<&HeldStack> {
        self.get(self.hand?)
    }
    /// The edit that takes `n` from the stack in `slot`, the rest keeping every component (`stackSize -= n`; an
    /// emptied stack leaves the slot). None when the slot holds fewer.
    pub fn take(&self, slot: PlayerSlot, n: u32) -> Option<SlotEdit> {
        let held = self.get(slot)?;
        let left = held.stack.count.checked_sub(n)?;
        let desired = (left > 0).then(|| container_bridge::Stack { count: left, ..held.stack.clone() });
        Some(SlotEdit { slot, expected: Some(held.encoded.clone()), desired })
    }
}

/// One stack of a transfer's player side: as it was read (`expected`, the host's lossless SNBT; None for an empty
/// slot) and as the transfer leaves it. A desired stack of the same item and damage as the expected one keeps the
/// expected stack's other components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotEdit {
    pub slot: PlayerSlot,
    pub expected: Option<String>,
    pub desired: Option<container_bridge::Stack>,
}

/// How a transfer puts a stack the player cannot hold into the world, each as Java does it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDrop {
    /// `dropPlayerItemWithRandomChoice`: from the eyes along the view.
    Thrown,
    /// `new EntityItem(world, x, y + eyeHeight, z, stack)` with the pickup delay given.
    World { delay: u32 },
}

/// A machine slot a transfer fills with a copy of a player's stack (`ItemStack.copy()`): the stack as it was read in
/// the player's slot `from`, every component carried over, at the count the commit's desired stack gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotCopy {
    pub dim: String,
    pub pos: Pos,
    pub slot: usize,
    pub from: PlayerSlot,
}

/// The player's side of a transfer with machines ([`ServerHost::commit_player_transfer`]). The machines' side is a
/// list of [`MachineCommit`]s, compared and published with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerTransfer {
    pub player: String,
    pub edits: Vec<SlotEdit>,
    pub drops: Vec<(container_bridge::Stack, TransferDrop)>,
    /// The machine slots that take a player's whole stack (a rod put in by hand).
    pub copies: Vec<SlotCopy>,
}

impl PlayerTransfer {
    /// Whether a host may publish it: never with a cursor edit, until T05 brings the acting player's open menu as its
    /// context. A cursor exists only in an open menu, and none is reliably known here: the server's own screen is
    /// always none, a client's creative screen clears its cursor as it closes, and a LAN guest's screen is the host's.
    /// A cursor edit published without one loses the stack (found by the T01 proof of the review's pending C).
    pub fn publishable(&self) -> bool {
        !self.edits.iter().any(|e| e.slot == PlayerSlot::Carried)
    }
}

/// One indivisible machine update. `expected` is the inventory read before
/// simulation, not a cached approximation. Hosts validate before any mutation.
#[derive(Clone, Debug)]
pub struct MachineCommit {
    pub cells: Vec<CellWrite>,
    pub guard:CellGuard,
    /// A previously confirmed BE must still carry this generation tag.
    pub expected_saved:bool,
    pub dim: String,
    pub pos: Pos,
    pub expected: container_bridge::Slots,
    pub desired: container_bridge::Slots,
    pub data: Vec<i32>,
    pub json: String,
}

/// A loaded cell, including its complete block entity, not just its slots.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CellSnapshot {
    pub state: String,
    pub entity: Option<String>,
    pub items: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetainedDrop {
    pub origin:CellGuard,
    pub items:Vec<String>,
    /// The drop's own name in the ledger, for one an origin may owe more than once in a generation (an RBMK lid);
    /// none: named by its origin (`ledger::retained_id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id:Option<u64>,
}
/// Consumed world cells publish in the same atomic operation as the machine.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CellWrite {
    pub notify:bool,
    pub guard: CellGuard,
    pub expected: CellSnapshot,
    pub desired: String,
    /// Excavator destroyBlock(false) also emits destruction/game events.
    pub destroy: bool,
    /// What the block entity this write creates holds from the start: the
    /// machine's first JSON and menu data, published with the cell.
    #[serde(default)]
    pub initial: Option<InitialState>,
}
/// A machine's state at its block entity's creation (`CellWrite::initial`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InitialState {
    pub json: String,
    pub data: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    /// The player's UUID, hyphenated: the key of their dose.
    pub id: String,
    pub name: String,
    pub dim: String,
    pub pos: [f64; 3],
    pub creative: bool,
}

/// Ticks between checks of whether a machine's chunk is loaded.
pub const LOAD_CHECK: u64 = 20;

#[derive(Clone, Debug)]
enum BlockEvent {
    Placed(String, Pos, String),
    Broken(String, Pos, String),
}

type Key = (String, Pos);

#[derive(Clone, Debug, Default)]
struct Ready {
    world: WorldState,
    topo: BTreeMap<String, Topology>,
    topo_dirty: bool,
    /// Chunk loaded, as last checked.
    loaded: BTreeMap<Key, bool>,
    sound_marks: BTreeMap<Key, crate::sounds::Mark>,
    frozen: BTreeSet<Key>,
    facts_pending:bool,
    facts_dirty:bool,
    /// Machines whose state came from the journal, not from a block entity of an older chunk: their slots are the
    /// model's, the block entity is brought to them.
    restored:BTreeSet<Key>,
    /// Machines an older chunk still shows although a receipt consumed them: the block goes, its contents were released.
    ghosts:Vec<(Key,generation::CellGuard)>,
    broken:BTreeMap<Key,String>,
    pending_buttons:Vec<generation::Guarded<(String,String,Pos,u32)>>,
    detonations:BTreeMap<Key,generation::Guarded<(String,container_bridge::Slots)>>,
    native_item_claims:BTreeMap<String,(u64,String,Option<Pos>)>,
    /// The state last written or seen, per machine.
    shown: BTreeMap<Key, String>,
    /// Machines and cables whose block was missing at the last check.
    suspect: BTreeMap<Key, u64>,
    /// Machines whose block this session has seen in the world: only those
    /// drop their contents when they vanish.
    seen: BTreeSet<Key>,
    /// The machines' slots and data as last written (`container_bridge`).
    bridge: container_bridge::Bridge,
    /// The state each machine last kept with its block entity.
    native_saved: BTreeMap<Key, String>,
    /// Cells whose neighbours may hold cables not yet known (a machine
    /// adopted, a cable found): the cables are found from the blocks, a
    /// few reads a tick.
    native_scan: std::collections::VecDeque<Key>,
    native_scanned: BTreeSet<Key>,
    /// The redstone signal batteries and bombs received this tick.
    native_signal: BTreeMap<Key, u8>,
    zirnox_radiation_rate: BTreeMap<String,u32>,
    /// RBMK columns whose meltdown is due and already logged (once per column and state; T16 runs it).
    rbmk_melting: BTreeSet<Key>,
    /// Test hooks only (`Server::native_test_eject`): fuel channels asked to put their rod out as an item entity at a
    /// place, taken by their reactor's next accepted step.
    rbmk_ejects: BTreeMap<Key, [f64; 3]>,
    /// The RBMK dials as `RBMKDials` caches them (`rbmk_columns`): created when the server starts.
    rbmk_dials: crate::rbmk::dials::DialState,
    /// Cells above a column's cell that changed: an orphaned dummy there goes (`rbmk_columns`).
    rbmk_orphans: Vec<(String, Pos)>,
    /// Cells whose dummy went: the column's cells under it go down to its core, which breaks (`rbmk_columns`).
    rbmk_breaks: Vec<(String, Pos)>,
    /// The stamps of the world's state as last kept ([`crate::persist::WorldState::stamps`]); None: write it next time.
    kept_rev: Option<[u64; 10]>,
    /// The stamp of the RBMK layers as last kept (`persist::RBMK_KEY`); None: write them next time.
    kept_rbmk: Option<u64>,
    /// What was last kept, without the clock: debug builds check every skipped write against it.
    #[cfg(debug_assertions)]
    kept: Vec<u8>,
}

pub struct Server {
    /// Bombs and explosions (`server/bomb_server.rs`, docs/bombas.md).
    blasts: crate::blast::Runtime,
    /// Fluid networks (`server/fluid_server.rs`, docs/fluidos.md).
    fluidnet: fluid_server::FluidNet,
    tick: u64,
    players: Vec<PlayerInfo>,
    /// What each player's client was last told: dose and rate.
    radiation_shown: BTreeMap<String, (i64, i64)>,
    /// `HbmLivingProps.radEnv`: what point `contaminate` calls added to each
    /// player since the last window, in milliRAD. The radiation tick folds it
    /// into the reading it publishes (`radBuf`) and starts a new window.
    radiation_env: BTreeMap<String, i64>,
    queued: Vec<(BlockEvent,u64)>,
    ready: Ready,
    /// What each chunk's record was last given of what it owes (a digest; `server::obligations`).
    homes: BTreeMap<obligations::Home, u64>,
    /// The earlier version's removals and batches whose chunk's record the server has not read yet: none of them is
    /// played before (`server::obligations`).
    unread: BTreeSet<obligations::Home>,
    /// What each chunk's record must say it took of an earlier version's storage (`ChunkLedger::migrated`).
    markers: BTreeMap<obligations::Home, BTreeSet<crate::persist::Obligation>>,
    /// Of those, the ones given to a loaded chunk's record: a save writes them with it.
    marked: BTreeSet<crate::persist::Obligation>,
    /// The earlier version's storage as the world was opened: a record's marker it does not name is of no use.
    legacy_read: BTreeSet<crate::persist::Obligation>,
    /// The marked ones as a save took its chunks (`native_world_saving`): a waited save wrote them before its storage.
    captured: Option<BTreeSet<crate::persist::Obligation>>,
    /// The receipts and survivors a save took with its chunks (`native_world_saving`), and those of the last save that
    /// did not wait, confirmed once the writer has put everything it queued on the disk (`native_writes_finished`).
    taken: Option<transaction::Taken>,
    awaiting: Option<transaction::Taken>,
    /// Drawn when the server starts: the names of the batches it puts out are its own, whatever an older storage or
    /// a newer chunk record names (a storage set back by a death names no batch twice).
    nonce: u64,
}

impl Server {
    fn ready(&mut self) -> Option<&mut Ready> {
        Some(&mut self.ready)
    }

    /// The online players (the bombs' viewers and doses).
    pub fn set_players(&mut self, players: Vec<PlayerInfo>) {
        self.players = players;
    }

    /// What a broken machine held outside the game's slots, dropped.
    fn drop_contents(&mut self, host: &mut impl ServerHost, dim: &str, pos: Pos, m: &Machine) {
        let key = (dim.to_string(), pos);
        let items = if self.slots_gone(&key) {
            container_bridge::held_contents(&m.data)
        } else {
            m.logic().contents()
        };
        let items: Vec<(String, u32)> = items.into_iter().filter(|i| i.1 > 0).collect();
        if !items.is_empty() {
            host.drop_items(dim, pos, &items);
        }
        // `RBMKBase.breakBlock`: the lid at the top as the dial reads now (the core, if that cell is not loaded).
        if let crate::machine::Data::Rbmk(c) = &m.data
            && let Some(lid) = c.lid.item()
        {
            let top = self.rbmk_lid_drop_at(pos);
            if !host.drop_items(dim, top, &[(lid.into(), 1)]) {
                host.drop_items(dim, pos, &[(lid.into(), 1)]);
            }
        }
    }

    fn block_event(&mut self, host: &mut impl ServerHost, e: BlockEvent) {
        // Pile writes install their runtime state synchronously. Their delayed
        // native notifications must not discard the newly assembled channels.
        let (dim,pos,state,placed)=match &e { BlockEvent::Placed(d,p,s)=>(d,p,s,true),BlockEvent::Broken(d,p,s)=>(d,p,s,false) };
        if let Some(m)=self.ready.world.machines.get(&(dim.clone(),*pos)) && matches!(m.data,crate::machine::Data::Zirnox(_)) && (placed && m.block==blockstate::id_of(state) || !placed && m.block!=blockstate::id_of(state)) {return;}
        let mut dismantle=None;
        if let Some(m)=self.ready.world.machines.get(&(dim.clone(),*pos)) && let crate::machine::Data::Pile(p)=&m.data {
            if placed && m.block==blockstate::id_of(state) || !placed && m.block!=blockstate::id_of(state) {return;}
            // Java breakBlock: breaking the core or any member dismantles the
            // whole pile at once, even if part of it sits in an unloaded chunk.
            if !placed {
                let core=match &**p { crate::pile::Pile::Core(_)=>Some(*pos), crate::pile::Pile::Member(Some(c))=>Some(*c), _=>None };
                dismantle=core.and_then(|c|self.ready.world.machines.get(&(dim.clone(),c))).and_then(|cm|match &cm.data {
                    crate::machine::Data::Pile(cp)=>match &**cp { crate::pile::Pile::Core(Some(c))=>Some(c.clone()), _=>None },
                    _=>None });
            }
        }
        if let Some(c)=dismantle {
            let done=self.pile_destroy(host,dim,&c);
            // Out of reach (frozen dimension): keep the core, so the tick's
            // sweep ejects its rods once the dimension can be entered.
            if !done { return; }
        }

        if let BlockEvent::Placed(dim, pos, state) = &e
            && registry::is_bomb(blockstate::id_of(state))
        {
            self.bomb_placed((dim.clone(), *pos));
        }
        let Some(r) = self.ready() else { return };
        match e {
            BlockEvent::Placed(dim, pos, state) => {
                let id = blockstate::id_of(&state);
                let key = (dim, pos);
                if let Some(m) = machine_from_state(&state) {
                    r.world.machines.insert(key.clone(), m);
                    r.loaded.insert(key.clone(), true);
                    r.seen.insert(key.clone());
                    r.shown.insert(key, state.clone());
                    r.topo_dirty = true;
                } else if registry::is_cable(id) {
                    r.world.cables.insert(key);
                    r.topo_dirty = true;
                }
            }
            BlockEvent::Broken(dim, pos, _state) => {
                let key = (dim.clone(), pos);
                if let Some(m) = r.world.machines.remove(&key) {
                    r.loaded.remove(&key);
                    r.shown.remove(&key);
                    r.seen.remove(&key);
                    r.topo_dirty = true;
                    host.log(&format!("machine {} broken at {pos:?}", m.block));
                    self.drop_contents(host, &dim, pos, &m);
                } else if r.world.cables.remove(&key) {
                    r.topo_dirty = true;
                }
            }
        }
    }

    /// Machines, networks and looks: one tick of the world.
    fn simulate(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        let mut removed: Vec<(Key, Machine, bool)> = Vec::new();
        let Some(r) = self.ready() else { return };

        // Loaded chunks, and what the world shows, a machine every 20 ticks.
        for (i, (key, m)) in r.world.machines.iter().enumerate() {
            if r.frozen.contains(key) {continue;}
            let known = r.loaded.get(key).copied();
            if known.is_some() && !(tick + i as u64).is_multiple_of(LOAD_CHECK) {
                continue;
            }
            let actual = host.get_block(&key.0, key.1);
            r.loaded.insert(key.clone(), actual.is_some());
            let Some(actual) = actual else { continue };
            let id = blockstate::id_of(&actual);
            if id == m.block {
                r.shown.insert(key.clone(), actual);
                r.suspect.remove(key);
                r.seen.insert(key.clone());
            } else if r.suspect.get(key).is_some_and(|t| tick >= t + LOAD_CHECK) {
                removed.push((key.clone(), m.clone(), r.seen.contains(key)));
            } else {
                r.suspect.entry(key.clone()).or_insert(tick);
                r.loaded.insert(key.clone(), false);
            }
        }
        // Cables, the same way (they hold nothing).
        let mut gone_cables = Vec::new();
        for (i, key) in r.world.cables.iter().enumerate() {
            if !(tick + i as u64).is_multiple_of(LOAD_CHECK) {
                continue;
            }
            let Some(actual) = host.get_block(&key.0, key.1) else {
                continue;
            };
            if registry::is_cable(blockstate::id_of(&actual)) {
                r.suspect.remove(key);
            } else if r.suspect.get(key).is_some_and(|t| tick >= t + LOAD_CHECK) {
                gone_cables.push(key.clone());
            } else {
                r.suspect.entry(key.clone()).or_insert(tick);
            }
        }
        for key in gone_cables {
            r.world.cables.remove(&key);
            r.suspect.remove(&key);
            r.topo_dirty = true;
        }
        for (key, m, _) in &removed {
            r.world.machines.remove(key);
            r.loaded.remove(key);
            r.shown.remove(key);
            r.suspect.remove(key);
            host.log(&format!("machine {} at {:?} is gone from the world", m.block, key.1));
            r.topo_dirty = true;
        }

        // Energy: a machine may be in several networks (one per separate
        // port): they move energy one after the other.
        if r.topo_dirty {
            r.topo = build_topology(&r.world);
            r.topo_dirty = false;
        }
        for (dim, t) in &r.topo {
            for net in &t.networks {
                let mut group: Vec<&mut dyn EnergyNode> = r
                    .world
                    .machines
                    .iter_mut()
                    .filter(|(k, _)| k.0 == *dim && net.binary_search(&k.1).is_ok())
                    .filter(|(k, _)| r.loaded.get(*k) == Some(&true))
                    .filter_map(|(_, m)| m.logic_mut().energy())
                    .collect();
                energy::distribute(&mut group);
            }
        }

        // Looks: what changed, written only where the machine still is.
        let mut writes: BTreeMap<String, Vec<(Pos, String)>> = BTreeMap::new();
        let mut recheck = Vec::new();
        for (key, m) in &r.world.machines {
            if r.loaded.get(key) != Some(&true) {
                continue;
            }
            let Some(want) = desired_state(m) else { continue };
            if r.shown.get(key).is_some_and(|s| same_look(s, &want)) {
                continue;
            }
            match host.get_block(&key.0, key.1) {
                Some(actual) if blockstate::id_of(&actual) == m.block => {
                    writes.entry(key.0.clone()).or_default().push((key.1, want.clone()));
                    r.shown.insert(key.clone(), want);
                }
                _ => recheck.push(key.clone()),
            }
        }
        for key in recheck {
            r.loaded.remove(&key);
            r.shown.remove(&key);
        }
        for (dim, blocks) in writes {
            for chunk in blocks.chunks(4096) {
                if let Err(e) = host.set_blocks(&dim, chunk) {
                    host.log(&format!("set-blocks: {e}"));
                }
            }
        }

        for (key, m, seen) in removed {
            if seen {
                self.drop_contents(host, &key.0, key.1, &m);
            } else if !m.logic().contents().is_empty() {
                host.log(&format!(
                    "machine {} at {:?} was never seen in this world: its contents are not dropped",
                    m.block, key.1
                ));
            }
        }
    }
}

fn build_topology(w: &WorldState) -> BTreeMap<String, Topology> {
    type Ports = BTreeMap<Pos, Vec<crate::energy::Port>>;
    type PortCells = BTreeMap<(Pos,usize),Pos>;
    type Shape = (BTreeSet<Pos>, Ports, PortCells);
    let mut dims: BTreeMap<&str, Shape> = BTreeMap::new();
    for (d, p) in &w.cables {
        dims.entry(d).or_default().0.insert(*p);
    }
    for ((d, p), m) in &w.machines {
        let Some(spec) = registry::spec(&m.block) else { continue };
        let ports = registry::ports(spec, m.facing.as_deref());
        if m.kind() != Kind::Inert && !ports.is_empty() {
            dims.entry(d).or_default().1.insert(*p, ports);
            for (i,cell) in container_bridge::energy_port_cells(&m.block,*p,m.facing.as_deref()).into_iter().enumerate() {
                dims.entry(d).or_default().2.insert((*p,i),cell);
            }
            // A multiblock's bottom cells carry its power, as HBM's proxies.
            let cells = container_bridge::proxy_cells(&m.block, *p, m.facing.as_deref());
            dims.entry(d).or_default().0.extend(cells);
        }
    }
    dims.into_iter()
        .map(|(d, (c, m, at))| (d.to_string(), Topology::build_at(&c, &m, &at)))
        .collect()
}

/// A new machine for a placed (or found) machine block.
pub fn machine_from_state(state: &str) -> Option<Machine> {
    let s = BlockState::parse(state)?;
    let spec = registry::spec(&s.id)?;
    let facing = spec.facing.and_then(|f| s.get(f)).map(str::to_string);
    Some(Machine::new(spec.kind, &s.id, facing))
}

/// The state that shows `m` now.
pub fn desired_state(m: &Machine) -> Option<String> {
    let spec = registry::spec(&m.block)?;
    let mut s = BlockState::new(&m.block);
    if let (Some(prop), Some(f)) = (spec.facing, &m.facing) {
        s = s.with(prop, f);
    }
    if let Some((prop, max)) = spec.level {
        s = s.with(prop, m.logic().level(max));
    }
    Some(s.to_string())
}

/// Whether `actual` (as the world reports it, every property) already
/// shows `want` (only the properties the logic sets).
fn same_look(actual: &str, want: &str) -> bool {
    // Sem os dois `parse` (~3 k fuel cada, por máquina carregada a cada tick).
    blockstate::shows(actual, want)
}

mod bomb_server;
mod excavator_server;
mod oil_server;
mod solar_server;
pub mod container_bridge;
mod fluid_server;
mod foundry_server;
pub mod native_server;
mod radiation_server;

#[cfg(test)]
mod tests;


mod pile_server;
pub use pile_server::PileUse;

mod zirnox_server;
mod rbmk_server;
pub mod rbmk_columns;
mod transaction;
pub mod obligations;
mod generation;
pub use generation::CellGuard;
pub use transaction::{CommittedEffects,EntitySpawn,Survivor,WorldEffect,PreparedRetention};

#[cfg(test)]
mod generation_tests;

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
    fn get_block(&mut self, dim: &str, p: Pos) -> Option<String>;
    fn set_blocks(&mut self, dim: &str, blocks: &[(Pos, String)]) -> Result<(), String>;
    fn tell(&mut self, player: &str, text: &str);
    fn log(&mut self, text: &str);
    /// Saved sky light, weather/time subtraction, and visibility at y+1.
    fn solar_sample(&mut self, _dim: &str, _p: Pos) -> Option<(i32, i32, bool)> { None }
    /// The slots of the machine's block entity at `p`.
    fn machine_slots(&mut self, _dim: &str, _p: Pos) -> Option<container_bridge::Slots> {
        None
    }
    fn set_machine_slot(&mut self, _dim: &str, _p: Pos, _slot: u32, _stack: Option<container_bridge::Stack>) -> bool {
        false
    }
    /// Its menu's data (the screen's values, then `Slotted::save_state`).
    fn set_machine_data(&mut self, _dim: &str, _p: Pos, _values: &[i32]) -> bool {
        false
    }
    fn machine_data(&mut self, _dim: &str, _p: Pos) -> Option<Vec<i32>> {
        None
    }
    /// Machine menu buttons pressed: player, dimension, position, button.
    fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
        Vec::new()
    }
    /// The machine's whole state (JSON), kept with its block entity.
    fn save_machine(&mut self, _dim: &str, _p: Pos, _json: &str) {}
    /// The redstone signal each of `cells` of `dim` receives (0..=15; none
    /// unloaded, or a host that can't read it: then a bomb reads its `lit`).
    fn redstone_many(&mut self, _dim: &str, cells: &[Pos]) -> Vec<Option<u8>> {
        vec![None; cells.len()]
    }
    /// Items a broken machine held outside its slots (the slots the game
    /// spills itself), dropped at `p`.
    fn drop_items(&mut self, _dim: &str, _p: Pos, _items: &[(String, u32)]) {}
    /// The players in the world (None: the host can't tell, and
    /// [`Server::set_players`] says).
    fn players(&mut self) -> Option<Vec<PlayerInfo>> {
        None
    }
    /// What player `id` carries: (item, count).
    fn inventory(&mut self, _id: &str) -> Option<Vec<(String, u32)>> {
        None
    }
    /// Damage to player `id` of a registered type (`minecraft:magic`); the
    /// host skips creative and spectator players.
    fn hurt(&mut self, _id: &str, _amount: f32, _kind: &str) {}
    /// A status effect on player `id` (`poison`).
    fn effect(&mut self, _id: &str, _effect: &str, _seconds: u32, _amplifier: u8) {}
    /// Particles or a sound in `dim` (`crate::blast::visuals::Show`, not
    /// its effects).
    fn show(&mut self, _dim: &str, _show: &crate::blast::visuals::Show) {}
    /// A named number player `id`'s client reads (the Geiger's).
    fn set_player_value(&mut self, _id: &str, _key: &str, _value: i64) {}
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

#[derive(Debug, Default)]
struct Ready {
    world: WorldState,
    topo: BTreeMap<String, Topology>,
    topo_dirty: bool,
    /// Chunk loaded, as last checked.
    loaded: BTreeMap<Key, bool>,
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
    /// The world's state as last kept ([`crate::persist`]).
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
    queued: Vec<BlockEvent>,
    ready: Ready,
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
    }

    fn block_event(&mut self, host: &mut impl ServerHost, e: BlockEvent) {
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

        // Machine ticks.
        for (key, m) in r.world.machines.iter_mut() {
            if r.loaded.get(key) == Some(&true) {
                m.logic_mut().tick();
            }
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
    let mut dims: BTreeMap<&str, (BTreeSet<Pos>, Ports)> = BTreeMap::new();
    for (d, p) in &w.cables {
        dims.entry(d).or_default().0.insert(*p);
    }
    for ((d, p), m) in &w.machines {
        let Some(spec) = registry::spec(&m.block) else { continue };
        let ports = registry::ports(spec, m.facing.as_deref());
        if m.kind() != Kind::Inert && !ports.is_empty() {
            dims.entry(d).or_default().1.insert(*p, ports);
            // A multiblock's bottom cells carry its power, as HBM's proxies.
            let cells = container_bridge::proxy_cells(&m.block, *p, m.facing.as_deref());
            dims.entry(d).or_default().0.extend(cells);
        }
    }
    dims.into_iter()
        .map(|(d, (c, m))| (d.to_string(), Topology::build(&c, &m)))
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


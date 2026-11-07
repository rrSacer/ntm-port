//! What a machine's removal or step still owes the world, kept where vanilla keeps a block entity's contents: in the
//! record of its chunk.
//!
//! A removal waiting for its group (a core broken while a member's chunk is out) holds the machine with what it held;
//! a batch of effects in flight (rods ejected, members to turn back to bricks) holds items and the models it consumed.
//! Each has a home: the chunk of the removed machine, or that of the batch's first owner (the pile's core, the machine
//! of a single step), whose block entity recorded the consumption. The server gives each chunk's record what it owes
//! in the very tick it changes (`ServerHost::store_obligations`), so the record written by any save or unload holds it
//! with the block entities it follows; the host's storage holds none. A death between two chunks' records leaves the
//! world as vanilla leaves a double chest or a hopper on a chunk border: each chunk as its own record says.
//!
//! What a chunk owes is played only while it is loaded (`ServerHost::home_loaded`), as a block entity ticks only
//! then; when it comes back its record says what it still owes (`Server::native_obligations_loaded`).

use super::*;
use crate::persist::{Obligation, Removal};

/// A chunk: its dimension and coordinates.
pub type Home = (String, i32, i32);

pub fn home_of(key: &Key) -> Home {
    (key.0.clone(), key.1[0] >> 4, key.1[2] >> 4)
}

impl Removal {
    /// The chunk of the removed machine.
    pub fn home(&self) -> Home {
        home_of(&self.key)
    }
}

impl CommittedEffects {
    /// The chunk of the batch's first owner, whose block entity recorded what the batch puts out.
    pub fn home(&self) -> Option<Home> {
        self.owners.first().map(home_of)
    }
}

/// The digest of what a record is given: written again only when it changes.
fn digest(removals: &[Removal], batches: &[CommittedEffects]) -> u64 {
    let r = serde_json::to_vec(removals).unwrap_or_default();
    let b = serde_json::to_vec(batches).unwrap_or_default();
    crate::ledger::fnv(&[&r, &b])
}

impl Server {
    /// `keys` with the home first: the pile's core (it records the rods its channels give up), else as they are.
    pub(super) fn owners_home_first(&self, mut keys: Vec<Key>, before: &BTreeMap<Key, Machine>) -> Vec<Key> {
        let core = keys.iter().position(|k| {
            before.get(k).or_else(|| self.ready.world.machines.get(k)).is_some_and(|m| {
                matches!(&m.data, crate::machine::Data::Pile(p) if matches!(&**p, crate::pile::Pile::Core(_)))
            })
        });
        if let Some(i) = core {
            let k = keys.remove(i);
            keys.insert(0, k);
        }
        keys
    }

    /// Gives each chunk's record what it owes now, where that changed since it was last given.
    pub(super) fn sync_obligations(&mut self, host: &mut impl ServerHost) {
        let mut by: BTreeMap<Home, (Vec<Removal>, Vec<CommittedEffects>)> = BTreeMap::new();
        for r in self.ready.world.removals.iter() {
            by.entry(r.home()).or_default().0.push(r.clone());
        }
        for b in self.ready.world.machine_effects.iter() {
            if let Some(h) = b.home() {
                by.entry(h).or_default().1.push(b.clone());
            }
        }
        let homes: BTreeSet<Home> = by.keys().cloned().chain(self.homes.keys().cloned()).collect();
        for home in homes {
            let (removals, batches) = by.remove(&home).unwrap_or_default();
            let d = digest(&removals, &batches);
            let empty = removals.is_empty() && batches.is_empty();
            if self.homes.get(&home) == Some(&d) || empty && !self.homes.contains_key(&home) {
                continue;
            }
            if !host.store_obligations(&home.0, [home.1, home.2], &removals, &batches) {
                continue;
            }
            if empty {
                self.homes.remove(&home);
            } else {
                self.homes.insert(home, d);
            }
            self.in_home.extend(removals.iter().map(Removal::obligation));
            self.in_home.extend(batches.iter().map(|b| Obligation::Batch(b.serial)));
        }
    }

    /// The record of the chunk `chunk` of `dim`, loaded, owes `removals` and `batches`: those the server does not
    /// hold are played again from it. A world opened again finds them only there.
    pub fn native_obligations_loaded(&mut self, dim: &str, chunk: [i32; 2], removals: Vec<Removal>, batches: Vec<CommittedEffects>) {
        let home: Home = (dim.to_string(), chunk[0], chunk[1]);
        let d = digest(&removals, &batches);
        for r in removals {
            let id = r.obligation();
            self.in_home.insert(id.clone());
            if self.ready.world.removals.iter().any(|x| x.obligation() == id) {
                continue;
            }
            // An earlier version's storage may have brought the same one: the chunk's record is the authority.
            self.ready.broken.insert(r.key.clone(), r.machine.block.clone());
            if r.engine_slots {
                self.ready.bridge.last.insert(r.key.clone(), Vec::new());
            }
            let past = r.machine.generation.saturating_add(1);
            if self.ready.world.generations.get(&r.key).is_none_or(|g| *g < past) {
                self.ready.world.generations.insert(r.key.clone(), past);
            }
            self.ready.world.machines.entry(r.key.clone()).or_insert_with(|| r.machine.clone());
            self.ready.world.removals.push(r);
        }
        for mut b in batches {
            self.in_home.insert(Obligation::Batch(b.serial));
            if let Some(i) = self.ready.world.machine_effects.iter().position(|x| x.serial == b.serial) {
                // Held already: the server's copy is the record's, played since only while the chunk was loaded.
                // One an earlier version's storage brought gives way to the record's.
                if self.ready.world.legacy.contains(&Obligation::Batch(b.serial)) {
                    b.replayed = true;
                    self.ready.world.machine_effects[i] = b;
                }
                continue;
            }
            // Its effects reach other chunks, whose records may be older or newer than this one.
            b.replayed = true;
            self.ready.world.machine_effects.push_back(b);
        }
        self.homes.insert(home, d);
    }

    /// What the record of the chunk `chunk` of `dim` must hold before it leaves: the block changes not yet seen, then
    /// what each chunk owes.
    pub fn native_chunk_unloading(&mut self, host: &mut impl ServerHost) {
        self.native_record_events(host);
        self.sync_obligations(host);
    }

    /// The entries the host's storage would keep that can hold items or decide what a chunk holds: zero in a world of
    /// this version, at every save.
    pub fn native_kept_items(&self) -> usize {
        self.ready.world.kept_items()
    }

    /// An earlier version's removals and batches that their chunk holds and a waited save wrote: the storage need
    /// not keep them any longer.
    pub(super) fn retire_legacy(&mut self) {
        if self.ready.world.legacy.is_empty() {
            return;
        }
        let held: BTreeSet<Obligation> = self
            .ready
            .world
            .removals
            .iter()
            .map(Removal::obligation)
            .chain(self.ready.world.machine_effects.iter().map(|b| Obligation::Batch(b.serial)))
            .collect();
        let in_home = &self.in_home;
        let keep: BTreeSet<Obligation> =
            self.ready.world.legacy.iter().filter(|o| held.contains(o) && !in_home.contains(o)).cloned().collect();
        if keep != *self.ready.world.legacy {
            *self.ready.world.legacy = keep;
        }
    }

    /// A name for a batch no other batch has, in this session or a stored one: the server's nonce and its count.
    pub(super) fn batch_serial(&mut self) -> u64 {
        *self.ready.world.effect_serial += 1;
        let n = *self.ready.world.effect_serial;
        crate::ledger::fnv(&[b"batch", &self.nonce.to_le_bytes(), &n.to_le_bytes()]).max(1)
    }
}

/// A number no earlier session drew: the clock, the process and the address space, through a randomly keyed hash.
pub(super) fn draw_nonce() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.write_u32(std::process::id());
    h.write_usize(&h as *const _ as usize);
    h.finish()
}

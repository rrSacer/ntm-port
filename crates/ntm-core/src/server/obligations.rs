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
//!
//! An earlier version's storage held removals and batches itself. Each moves to its chunk's record once: none is played
//! before that record is read (`ServerHost::home_migrated`). A record that took it already (`migrated: X`, kept with
//! the record) says what is left of it, and the storage's copy adds nothing; otherwise the server takes it up and the
//! record says so from then on. The storage keeps the copy, as it read it, until the record that took it is on the disk:
//! a waited save wrote it before the storage (`native_world_saving`, `native_world_saved`), or it was read back from the
//! disk with the marker. A save that does not wait retires none.

use super::*;
use crate::persist::{Legacy, Obligation, Removal};

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
    /// The home of an earlier version's batch: its first owner's chunk, or that of the first cell it read or consumed.
    fn legacy_home(&self) -> Option<Home> {
        self.home()
            .or_else(|| self.origins.first().map(|g| home_of(&g.key())))
            .or_else(|| self.tombstones.first().map(|(k, _)| home_of(k)))
    }
}

/// The chunks whose records an earlier version's removals and batches wait for. A batch with no cell at all has none:
/// it stays in the storage, unplayed (no version puts one out).
pub(super) fn legacy_homes(legacy: &Legacy) -> BTreeSet<Home> {
    legacy.removals.iter().map(Removal::home).chain(legacy.batches.iter().filter_map(|(_, b)| b.legacy_home())).collect()
}

/// The digest of what a record is given: written again only when it changes.
fn digest(removals: &[Removal], batches: &[CommittedEffects], migrated: &BTreeSet<Obligation>) -> u64 {
    let r = serde_json::to_vec(removals).unwrap_or_default();
    let b = serde_json::to_vec(batches).unwrap_or_default();
    let m = serde_json::to_vec(migrated).unwrap_or_default();
    crate::ledger::fnv(&[&r, &b, &m])
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

    /// Gives each chunk's record what it owes now, where that changed since it was last given. A record not read yet
    /// for an earlier version's storage is left alone: what it took is not known.
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
        let homes: BTreeSet<Home> =
            by.keys().cloned().chain(self.homes.keys().cloned()).chain(self.markers.keys().cloned()).collect();
        for home in homes {
            if self.unread.contains(&home) {
                continue;
            }
            let (removals, batches) = by.remove(&home).unwrap_or_default();
            let migrated = self.markers.get(&home).cloned().unwrap_or_default();
            let d = digest(&removals, &batches, &migrated);
            let empty = removals.is_empty() && batches.is_empty() && migrated.is_empty();
            if self.homes.get(&home) == Some(&d) || empty && !self.homes.contains_key(&home) {
                continue;
            }
            if !host.store_obligations(&home.0, [home.1, home.2], &removals, &batches, &migrated) {
                continue;
            }
            if empty {
                self.homes.remove(&home);
            } else {
                self.homes.insert(home, d);
            }
            self.marked.extend(migrated);
        }
    }

    /// The record of the chunk `chunk` of `dim`, loaded, owes `removals` and `batches`: those the server does not
    /// hold are played again from it. A world opened again finds them only there. A marker of an earlier version's
    /// storage that the storage read with the world no longer names goes with the record's next write.
    pub fn native_obligations_loaded(
        &mut self,
        dim: &str,
        chunk: [i32; 2],
        removals: Vec<Removal>,
        batches: Vec<CommittedEffects>,
        migrated: BTreeSet<Obligation>,
    ) {
        let home: Home = (dim.to_string(), chunk[0], chunk[1]);
        // Even if the first settle pays everything, the loaded record must be written empty. Otherwise its old
        // batch comes back on every reopen (an explosion has no destination id of its own to suppress it).
        if !removals.is_empty() || !batches.is_empty() || !migrated.is_empty() {
            self.homes.entry(home.clone()).or_insert(0);
        }
        for r in removals {
            if self.ready.world.removals.iter().any(|x| x.obligation() == r.obligation()) {
                continue;
            }
            self.take_up_removal(r);
        }
        for mut b in batches {
            if self.ready.world.machine_effects.iter().any(|x| x.serial == b.serial) {
                // Held already: the server's copy is the record's, played since only while the chunk was loaded.
                continue;
            }
            // Its effects reach other chunks, whose records may be older or newer than this one.
            b.replayed = true;
            self.ready.world.machine_effects.push_back(b);
        }
        if self.unread.contains(&home) || self.markers.contains_key(&home) {
            return;
        }
        if migrated.iter().any(|m| !self.legacy_read.contains(m)) {
            // Given again, without them (an empty record is then written too).
            self.homes.insert(home, 0);
        }
    }

    /// A removal the server holds from now on: the machine taken up as broken, with what it held.
    fn take_up_removal(&mut self, r: Removal) {
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

    /// The records of the loaded chunks an earlier version's removals and batches wait for, read once: what a record
    /// took already leaves the storage (that record came from the disk); the rest is taken up now, the record saying
    /// so from its next write on.
    pub(super) fn read_legacy(&mut self, host: &mut impl ServerHost) {
        if self.unread.is_empty() {
            return;
        }
        for home in self.unread.clone() {
            let Some(on_record) = host.home_migrated(&home.0, [home.1, home.2]) else {
                continue;
            };
            self.unread.remove(&home);
            let legacy = &self.ready.world.legacy;
            let removals: Vec<Removal> =
                legacy.removals.iter().filter(|r| r.home() == home && !on_record.contains(&r.obligation())).cloned().collect();
            let batches: Vec<(u64, CommittedEffects)> = legacy
                .batches
                .iter()
                .filter(|(n, b)| b.legacy_home().as_ref() == Some(&home) && !on_record.contains(&Obligation::Batch(*n)))
                .cloned()
                .collect();
            let taken: BTreeSet<Obligation> = legacy.ids().intersection(&on_record).cloned().collect();
            if !taken.is_empty() {
                self.ready.world.legacy.retain(|o| !taken.contains(o));
            }
            let mut markers: BTreeSet<Obligation> = on_record.into_iter().filter(|m| self.legacy_read.contains(m)).collect();
            for r in removals {
                markers.insert(r.obligation());
                self.take_up_removal(r);
            }
            for (name, mut b) in batches {
                markers.insert(Obligation::Batch(name));
                b.serial = name;
                b.replayed = true;
                self.ready.world.machine_effects.push_back(b);
            }
            if !markers.is_empty() {
                self.markers.insert(home.clone(), markers);
            }
            // Given at the next sync, with its markers.
            self.homes.remove(&home);
        }
    }

    /// The block changes not taken yet, as the end of a tick takes them: a removed machine's contents put out and what
    /// each chunk owes in its record. Before a save takes its chunks (`native_world_saving`) and while chunks leaving
    /// are still in place (`native_chunk_unloading`): a vanilla container's items are out the instant its block goes.
    /// No machine runs a step but those whose block went.
    pub fn native_settle(&mut self, host: &mut impl ServerHost) {
        self.native_record_events(host);
        self.read_legacy(host);
        // Heard at every chunk that leaves: with no machine gone and no batch in flight there is nothing to take.
        if !self.ready.facts_pending && (!self.ready.broken.is_empty() || !self.ready.world.machine_effects.is_empty()) {
            self.flush_machine_effects(host);
            if !self.ready.broken.is_empty() {
                self.removal_steps(host);
                self.flush_machine_effects(host);
            }
        }
        self.sync_obligations(host);
    }

    /// A save is about to take the chunks: everything up to now is in them. The earlier version's removals and
    /// batches whose records it takes with their markers are noted: a waited save writes those before the storage.
    pub fn native_world_saving(&mut self, host: &mut impl ServerHost) {
        self.native_settle(host);
        self.save_loaded_chunk_radiation();
        self.taken = Some(self.taking());
        self.taken_annihilator = Some(self.ready.annihilator.clone());
        let legacy = self.ready.world.legacy.ids();
        self.captured = Some(self.marked.intersection(&legacy).cloned().collect());
    }

    /// What the record of the chunk `chunk` of `dim` must hold before it leaves: the block changes not yet seen, then
    /// what each chunk owes.
    pub fn native_chunk_unloading(&mut self, host: &mut impl ServerHost) {
        self.native_settle(host);
    }

    /// The entries the host's storage would keep that can hold items or decide what a chunk holds: zero in a world of
    /// this version, at every save.
    pub fn native_kept_items(&self) -> usize {
        self.ready.world.kept_items()
    }

    /// After a save: when it waited, the records it took with their markers are on the disk, so the storage written
    /// now need not keep the earlier version's removals and batches they took.
    pub(super) fn retire_legacy(&mut self, wait: bool) {
        let Some(captured) = self.captured.take() else {
            return;
        };
        if wait && !captured.is_empty() {
            self.ready.world.legacy.retain(|o| !captured.contains(o));
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

//! What a chunk remembers of the effects that changed it, kept in the record of the chunk of the cell.
//!
//! An effect that puts items into the world (a pile's ejected rods, a consumed block entity's retained
//! contents) waits in the host's separate storage until a world save confirms it, and a crash in between
//! replays it. The items are dropped as vanilla drops in the very step that takes them out of the machine,
//! and the effect's id goes into the ledger of the chunk the machine stands in: that record is written
//! whole, so the block entity that paid and the id are always on disk together. A replay that finds the id
//! adds nothing; one that does not finds the machine as it was before the effect, too.
//!
//! The host writes that record as vanilla writes a region, with the next save, ahead of its entities; earlier
//! only when a chunk that may hold the items (where they lie, a stack or a hopper that took some) unloads, and
//! then followed by those chunks' records and entities. No entity file or other record on disk holds the items
//! before the record that names their effect, and a death keeps them where vanilla's world keeps them. A save
//! that does not wait writes the player's file at once, ahead of its regions, as vanilla's does.
//!
//! An id stays until the host's storage no longer names the effect, because only then can no replay
//! bring it back.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// FNV-1a over 64 bits: the ids are derived, never drawn.
pub fn fnv(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.iter().chain(&[0xffu8]) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The id of the `done`-th effect of the batch named `serial`.
pub fn effect_id(serial: u64, done: u32) -> u64 {
    fnv(&[b"effect", &serial.to_le_bytes(), &done.to_le_bytes()])
}

/// The id of the contents retained from the cell `(dim, pos)` as it stood in `generation`: a cell is
/// consumed once per generation, so this names that one retention for ever.
pub fn retained_id(dim: &str, pos: [i32; 3], generation: u64) -> u64 {
    let mut p = Vec::new();
    for v in pos {
        p.extend_from_slice(&v.to_le_bytes());
    }
    fnv(&[b"retained", dim.as_bytes(), &p, &generation.to_le_bytes()])
}

/// A stack a record of an earlier version still owes: that ledger kept the items themselves until they
/// were spawned, and followed them after. Read only to settle it once (`ChunkLedger::take_owed`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Owed {
    #[serde(with = "hex")]
    pub id: u64,
    /// Where the items appear (a cell of the chunk).
    pub at: [i32; 3],
    /// The item stack as SNBT.
    pub item: String,
    /// `Pending`: not in the world yet. Any other: its entities are in the world's files.
    #[serde(default)]
    pub state: String,
    /// How many items it held then (0: as `item` says).
    #[serde(default)]
    pub live: u32,
}

impl Owed {
    /// Whether its items were never put into the world.
    pub fn pending(&self) -> bool {
        self.state.is_empty() || self.state == "Pending"
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkLedger {
    /// Ids of the effects applied to this chunk, kept until the host's storage forgets the effect.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty", with = "hex_set")]
    pub applied: BTreeSet<u64>,
    /// The stacks of a record of an earlier version; never written again. Other fields of those records
    /// (the folded stacks, where entities were seen) are ignored.
    #[serde(default, rename = "entries", skip_serializing)]
    pub owed: Vec<Owed>,
}

impl ChunkLedger {
    pub fn is_empty(&self) -> bool {
        self.applied.is_empty() && self.owed.is_empty()
    }
    pub fn knows(&self, id: u64) -> bool {
        self.applied.contains(&id) || self.owed.iter().any(|e| e.id == id)
    }
    /// The effect `id` was applied to this chunk. True when new.
    pub fn note_applied(&mut self, id: u64) -> bool {
        self.applied.insert(id)
    }
    /// The stacks of an earlier version's record whose items never reached the world, with the cell they
    /// appear at; every effect it named is applied from here. Empty when there were none.
    pub fn take_owed(&mut self) -> Vec<([i32; 3], String)> {
        let mut out = Vec::new();
        for e in std::mem::take(&mut self.owed) {
            self.applied.insert(e.id);
            if e.pending() {
                out.push((e.at, e.item));
            }
        }
        out
    }
    /// Forgets the ids the host's storage no longer names. Returns whether any went.
    pub fn prune(&mut self, live: &BTreeSet<u64>) -> bool {
        let before = self.applied.len();
        self.applied.retain(|id| live.contains(id));
        self.applied.len() != before
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    pub fn from_json(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }
}

mod hex {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let text = String::deserialize(d)?;
        u64::from_str_radix(&text, 16).map_err(serde::de::Error::custom)
    }
}

mod hex_set {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeSet;
    pub fn serialize<S: Serializer>(v: &BTreeSet<u64>, s: S) -> Result<S::Ok, S::Error> {
        v.iter().map(|v| format!("{v:016x}")).collect::<Vec<_>>().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeSet<u64>, D::Error> {
        Vec::<String>::deserialize(d)?
            .into_iter()
            .map(|t| u64::from_str_radix(&t, 16).map_err(serde::de::Error::custom))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_effect_applied_twice_is_known_once() {
        let mut ledger = ChunkLedger::default();
        let id = effect_id(7, 0);
        assert!(!ledger.knows(id));
        assert!(ledger.note_applied(id));
        // A replay after a crash finds it.
        assert!(!ledger.note_applied(id));
        assert!(ledger.knows(id));
    }

    #[test]
    fn identical_effects_of_one_batch_have_their_own_ids_and_a_source_has_one() {
        let ids: BTreeSet<u64> = (0..50u32).map(|n| effect_id(1, n)).chain((0..50u32).map(|n| effect_id(2, n))).collect();
        assert_eq!(ids.len(), 100);
        assert_eq!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:overworld", [3, 64, -4], 5));
        assert_ne!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:overworld", [3, 64, -4], 6));
        assert_ne!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:the_nether", [3, 64, -4], 5));
    }

    #[test]
    fn the_ledger_round_trips_and_forgets_only_what_storage_no_longer_names() {
        let mut ledger = ChunkLedger::default();
        let (a, b) = (effect_id(1, 0), effect_id(1, 1));
        ledger.note_applied(a);
        ledger.note_applied(b);
        let back = ChunkLedger::from_json(&ledger.to_json());
        assert_eq!(back, ledger);
        let mut pruned = back;
        assert!(pruned.prune(&BTreeSet::from([b])));
        assert_eq!(pruned.applied, BTreeSet::from([b]));
        assert!(!pruned.prune(&BTreeSet::from([b])));
        assert_eq!(ChunkLedger::from_json("not json"), ChunkLedger::default());
        assert_eq!(ChunkLedger::default().to_json(), "{}");
    }
}

//! The ledger of what machines owe the world, kept in the record of the chunk of the cell that owes it.
//!
//! An effect that puts items into the world (a pile's ejected rods, a consumed block entity's retained
//! contents) used to wait in the host's separate storage until a world save was confirmed, so a crash
//! between the two files could drop it twice or never. The items now move, in the very step that takes
//! them out of the machine, into the ledger of the chunk the machine stands in. That chunk's record is
//! written whole, so the block entity and the debt are always written together.
//!
//! Each entry has an `id` that never changes: the id of the effect that created it. Applying an effect
//! twice, as a replay after a crash does, finds the id and adds nothing. Spawning an entry's items
//! makes it `Spawned`; it leaves the ledger when its item entities are gone (folded into another stack of
//! the ledger, picked up, burnt), in the same step as the world sees that. The id stays in `applied` until
//! the host's storage no longer names the effect, because only then can no replay bring it back.
//!
//! The entities live in the entity files of whatever chunks they reach, which are written apart from the
//! ledger's record. The host writes the record first: every chunk its entities were seen in (`seen`) is named
//! there before any entity file can hold them, so a reopened world knows every file to look in before it
//! decides that a stack never reached one.

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// Owed, not yet in the world.
    #[default]
    Pending,
    /// Its item entities were created; they are the debt now.
    Spawned,
}

/// One stack an effect owes. An effect with several stacks has several entries, so each stack's fate
/// (still a pending debt, a living item entity, paid) is its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(with = "hex")]
    pub id: u64,
    /// The stack's place in its effect.
    pub idx: u32,
    /// Where the items appear (a cell of the chunk).
    pub at: [i32; 3],
    /// The item stack as SNBT, lossless.
    pub item: String,
    #[serde(default)]
    pub state: State,
    /// How many items its entities hold now, once they have merged with others or split (0: as `item` says).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub live: u32,
    /// The chunk its entities were last seen in (None: still in the chunk of `at`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<[i32; 2]>,
    /// Every other chunk than the one of `at` its entities were seen in: the entity files that may hold them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen: Vec<[i32; 2]>,
}

impl Entry {
    /// The chunk of `at`, whose ledger owns the entry.
    pub fn origin(&self) -> [i32; 2] {
        [self.at[0] >> 4, self.at[2] >> 4]
    }
    /// The chunks whose entity files may hold the stack: its own, and every one its entities were seen in.
    pub fn holders(&self) -> Vec<[i32; 2]> {
        let mut out = vec![self.origin()];
        for c in self.seen.iter().chain(&self.last) {
            if !out.contains(c) {
                out.push(*c);
            }
        }
        out
    }
    /// Its entities are in `chunk`. True when the ledger did not name that chunk yet.
    pub fn see(&mut self, chunk: [i32; 2]) -> bool {
        if chunk == self.origin() || self.seen.contains(&chunk) {
            return false;
        }
        self.seen.push(chunk);
        true
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// The entry an entity tag names, and the chunk whose ledger owns it.
pub fn parse_tag(tag: &str) -> Option<(u64, u32, [i32; 2])> {
    let (stack, chunk) = tag.split_once('@')?;
    let (id, idx) = stack.split_once(':')?;
    let (cx, cz) = chunk.split_once(',')?;
    Some((u64::from_str_radix(id, 16).ok()?, idx.parse().ok()?, [cx.parse().ok()?, cz.parse().ok()?]))
}

/// The name an item entity carries for its entry: `<id>:<idx>@<chunk x>,<chunk z>`.
pub fn entity_tag(id: u64, idx: u32, chunk: [i32; 2]) -> String {
    format!("{id:016x}:{idx}@{},{}", chunk[0], chunk[1])
}

/// What the entities of a chunk's ledger carry as `ItemEntity.merge_key` while it follows them: items of
/// one ledger fold into each other (one record keeps the count), never into other items.
pub fn merge_key(chunk: [i32; 2]) -> u64 {
    fnv(&[b"merge", &chunk[0].to_le_bytes(), &chunk[1].to_le_bytes()]) | 1
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkLedger {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<Entry>,
    /// Ids whose effect was applied and is paid, kept until the host's storage forgets the effect.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty", with = "hex_set")]
    pub applied: BTreeSet<u64>,
    /// Stacks whose entities are gone (picked up, merged away, burnt) and whose effect is still named by the
    /// host's storage: an entity file older than this record may still show them, and they must not come back.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty", with = "stack_set")]
    pub gone: BTreeSet<(u64, u32)>,
}

impl ChunkLedger {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.applied.is_empty() && self.gone.is_empty()
    }
    pub fn knows(&self, id: u64) -> bool {
        self.applied.contains(&id) || self.entries.iter().any(|e| e.id == id)
    }
    /// Owes `items` at `at` for the effect `id`; false (nothing added) when the effect is already here,
    /// pending, spawned or paid.
    pub fn apply(&mut self, id: u64, at: [i32; 3], items: Vec<String>) -> bool {
        if self.knows(id) {
            return false;
        }
        if items.is_empty() {
            self.applied.insert(id);
        }
        for (idx, item) in items.into_iter().enumerate() {
            self.entries.push(Entry { id, idx: idx as u32, at, item, state: State::Pending, live: 0, last: None, seen: Vec::new() });
        }
        true
    }
    /// The effect `id` changed blocks of this chunk, saved in the very record that holds the blocks. True when new.
    pub fn note_applied(&mut self, id: u64) -> bool {
        self.applied.insert(id)
    }
    pub fn mark_spawned(&mut self, id: u64, idx: u32) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id && e.idx == idx) {
            e.state = State::Spawned;
        }
    }
    /// The entry's items are `held` now (merged or split on the ground). True when that changed it.
    pub fn set_live(&mut self, id: u64, idx: u32, held: u32) -> bool {
        match self.entries.iter_mut().find(|e| e.id == id && e.idx == idx) {
            Some(e) if e.live != held => {
                e.live = held;
                true
            }
            _ => false,
        }
    }
    /// The entry's entities were last seen in `chunk`. True when that changed it.
    pub fn set_last(&mut self, id: u64, idx: u32, chunk: [i32; 2]) -> bool {
        match self.entries.iter_mut().find(|e| e.id == id && e.idx == idx) {
            Some(e) if e.last != Some(chunk) => {
                e.last = Some(chunk);
                true
            }
            _ => false,
        }
    }
    /// The effect was applied and every stack of it is paid.
    pub fn is_paid(&self, id: u64) -> bool {
        self.applied.contains(&id) && !self.entries.iter().any(|e| e.id == id)
    }
    /// The stack folded into another stack of this ledger, which holds its items now; when it was the effect's
    /// last, the id moves to `applied`. An entity file older than the record may still show it: it is `gone`.
    pub fn retire(&mut self, id: u64, idx: u32) {
        let Some(i) = self.entries.iter().position(|e| e.id == id && e.idx == idx) else {
            return;
        };
        self.entries.remove(i);
        self.gone.insert((id, idx));
        if !self.entries.iter().any(|e| e.id == id) {
            self.applied.insert(id);
        }
    }
    /// The ledger stops following the stack: its entities were saved with the world (confirmed), or taken out of
    /// it (picked up, burnt). Either way an entity file that still shows one is the world's own business from here,
    /// as for any drop: unlike `retire`, they are not `gone`.
    pub fn release(&mut self, id: u64, idx: u32) {
        let Some(i) = self.entries.iter().position(|e| e.id == id && e.idx == idx) else {
            return;
        };
        self.entries.remove(i);
        if !self.entries.iter().any(|e| e.id == id) {
            self.applied.insert(id);
        }
    }
    pub fn is_gone(&self, id: u64, idx: u32) -> bool {
        self.gone.contains(&(id, idx))
    }
    /// Forgets the paid ids the host's storage no longer names. Returns whether any went.
    pub fn prune(&mut self, live: &BTreeSet<u64>) -> bool {
        let before = self.applied.len();
        let gone = self.gone.len();
        self.applied.retain(|id| live.contains(id));
        self.gone.retain(|(id, _)| live.contains(id));
        self.applied.len() != before || self.gone.len() != gone
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    pub fn from_json(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }
}

mod hex {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:016x}"))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let text = String::deserialize(d)?;
        u64::from_str_radix(&text, 16).map_err(serde::de::Error::custom)
    }
}

mod stack_set {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeSet;
    pub fn serialize<S: Serializer>(v: &BTreeSet<(u64, u32)>, s: S) -> Result<S::Ok, S::Error> {
        v.iter().map(|(id, idx)| format!("{id:016x}:{idx}")).collect::<Vec<_>>().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeSet<(u64, u32)>, D::Error> {
        Vec::<String>::deserialize(d)?
            .into_iter()
            .map(|t| {
                let (id, idx) = t.split_once(':').ok_or_else(|| serde::de::Error::custom("stack"))?;
                Ok((u64::from_str_radix(id, 16).map_err(serde::de::Error::custom)?, idx.parse().map_err(serde::de::Error::custom)?))
            })
            .collect()
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
    fn an_effect_applied_twice_is_owed_once() {
        let mut ledger = ChunkLedger::default();
        let id = effect_id(7, 0);
        assert!(ledger.apply(id, [1, 64, 2], vec!["a".into(), "b".into()]));
        // A replay after a crash finds it pending, then spawned, then paid.
        assert!(!ledger.apply(id, [1, 64, 2], vec!["a".into(), "b".into()]));
        ledger.mark_spawned(id, 0);
        ledger.mark_spawned(id, 1);
        assert!(!ledger.apply(id, [1, 64, 2], vec!["x".into()]));
        ledger.retire(id, 0);
        assert!(!ledger.applied.contains(&id), "one stack is still owed");
        assert!(!ledger.apply(id, [1, 64, 2], vec!["x".into()]));
        ledger.retire(id, 1);
        assert!(ledger.entries.is_empty() && ledger.applied.contains(&id));
        assert!(!ledger.apply(id, [1, 64, 2], vec!["x".into()]));
        assert!(ledger.entries.is_empty(), "nothing came back");
    }

    #[test]
    fn what_merged_on_the_ground_is_what_is_owed() {
        let mut ledger = ChunkLedger::default();
        let (a, b) = (effect_id(3, 0), effect_id(3, 1));
        ledger.apply(a, [0, 0, 0], vec!["x".into()]);
        ledger.apply(b, [0, 0, 0], vec!["x".into()]);
        ledger.mark_spawned(a, 0);
        ledger.mark_spawned(b, 0);
        // `a` took `b`'s item: one step, one record.
        assert!(ledger.set_live(a, 0, 2));
        assert!(!ledger.set_live(a, 0, 2));
        ledger.retire(b, 0);
        let back = ChunkLedger::from_json(&ledger.to_json());
        assert_eq!(back.entries[0].live, 2);
        assert!(back.is_paid(b) && !back.is_paid(a));
        assert_eq!(parse_tag(&entity_tag(a, 0, [-3, 7])), Some((a, 0, [-3, 7])));
        assert!(back.is_gone(b, 0) && !back.is_gone(a, 0));
        // Released: followed no more, and not gone.
        let mut released = back.clone();
        released.release(a, 0);
        assert!(released.entries.is_empty() && released.applied.contains(&a) && !released.is_gone(a, 0));
    }

    #[test]
    fn the_chunks_a_stack_was_seen_in_are_named_once_and_survive_the_record() {
        let mut ledger = ChunkLedger::default();
        let id = effect_id(4, 0);
        ledger.apply(id, [-1, 64, 17], vec!["x".into()]);
        let e = &mut ledger.entries[0];
        assert_eq!(e.origin(), [-1, 1]);
        assert!(!e.see([-1, 1]), "its own chunk is always a holder");
        assert!(e.see([0, 1]) && !e.see([0, 1]) && e.see([0, 2]));
        e.last = Some([0, 2]);
        assert_eq!(e.holders(), vec![[-1, 1], [0, 1], [0, 2]]);
        let back = ChunkLedger::from_json(&ledger.to_json());
        assert_eq!(back.entries[0].seen, vec![[0, 1], [0, 2]]);
        // A record written before `seen` existed reads as one seen nowhere else.
        let old = ChunkLedger::from_json(&ledger.to_json().replace(",\"seen\":[[0,1],[0,2]]", ""));
        assert!(old.entries[0].seen.is_empty() && old.entries[0].last == Some([0, 2]));
    }

    #[test]
    fn identical_effects_of_one_batch_have_their_own_ids_and_a_source_has_one() {
        let ids: BTreeSet<u64> = (0..50u32).map(|n| effect_id(1, n)).chain((0..50u32).map(|n| effect_id(2, n))).collect();
        assert_eq!(ids.len(), 100);
        assert_eq!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:overworld", [3, 64, -4], 5));
        assert_ne!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:overworld", [3, 64, -4], 6));
        assert_ne!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:the_nether", [3, 64, -4], 5));
        assert_ne!(entity_tag(1, 0, [0, 0]), entity_tag(1, 1, [0, 0]));
        assert_ne!(merge_key([0, 0]), merge_key([0, 1]));
        assert_ne!(merge_key([0, 0]), 0);
    }

    #[test]
    fn the_ledger_round_trips_and_forgets_only_what_storage_no_longer_names() {
        let mut ledger = ChunkLedger::default();
        let (a, b, c) = (effect_id(1, 0), effect_id(1, 1), effect_id(1, 2));
        ledger.apply(a, [0, 0, 0], vec!["a".into()]);
        ledger.apply(b, [0, 0, 0], vec!["b".into()]);
        ledger.apply(c, [0, 0, 0], vec![]);
        ledger.mark_spawned(a, 0);
        ledger.retire(b, 0);
        let back = ChunkLedger::from_json(&ledger.to_json());
        assert_eq!(back, ledger);
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].state, State::Spawned);
        let mut pruned = back;
        assert!(pruned.prune(&BTreeSet::from([b])));
        assert_eq!(pruned.applied, BTreeSet::from([b]));
        assert!(!pruned.prune(&BTreeSet::from([b])));
        assert_eq!(ChunkLedger::from_json("not json"), ChunkLedger::default());
    }
}

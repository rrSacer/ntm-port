//! What a chunk's record keeps of the machines' effects: the stacks an effect owes the world while it cannot drop them
//! yet, as a dropper keeps its stock; the removals and batches of effects whose home this chunk is
//! (`server::obligations`); and what records of earlier versions held, read once to settle it.
//!
//! The host's storage is written with the world's save, so after a death the machines, their effects in flight and the
//! world all come back as the last save left them, as vanilla's do. An effect drops its items in the very step that
//! takes them out of the machine. One whose drop has to wait (the chunk it drops into is not loaded) keeps its stacks
//! here, in the record of the chunk that paid them, written with the block entity: they leave the record when they are
//! dropped, in the same step. A replay that finds the effect's id here (owed, or applied in an earlier version) adds
//! nothing.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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

/// The name an item entity of an earlier version carried for its stack: `<id>:<idx>@<chunk x>,<chunk z>`.
pub fn entity_tag(id: u64, idx: u32, chunk: [i32; 2]) -> String {
    format!("{id:016x}:{idx}@{},{}", chunk[0], chunk[1])
}

/// One stack an effect owes. `Spawned` stacks (with where they were seen) only come from records of the version that
/// followed its drops; they are settled once (`ChunkLedger::legacy`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owed {
    #[serde(with = "hex")]
    pub id: u64,
    /// The stack's place in its effect.
    #[serde(default)]
    pub idx: u32,
    /// Where the items appear (a cell).
    pub at: [i32; 3],
    /// The item stack as SNBT, lossless.
    pub item: String,
    /// `Pending`: not in the world yet. `Spawned`: an earlier version's stack whose entities were put out.
    #[serde(default = "pending", skip_serializing_if = "is_pending")]
    pub state: String,
    /// How many items its entities held then (0: as `item` says).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub live: u32,
    /// The chunks its entities were last and ever seen in (earlier versions only). Written back as long as the stack is
    /// not settled: a record saved before its settling still says where to look.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<[i32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen: Vec<[i32; 2]>,
}

fn pending() -> String {
    "Pending".into()
}
fn is_pending(s: &String) -> bool {
    s == "Pending"
}
fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Owed {
    /// Whether its items were never put into the world.
    pub fn pending(&self) -> bool {
        self.state.is_empty() || is_pending(&self.state)
    }
    /// The chunk of `at`.
    pub fn origin(&self) -> [i32; 2] {
        [self.at[0] >> 4, self.at[2] >> 4]
    }
    /// The chunks whose entity files may hold the stack of an earlier version: its own, and every one it was seen in.
    pub fn holders(&self) -> Vec<[i32; 2]> {
        let mut out = vec![self.origin()];
        for c in self.seen.iter().chain(&self.last) {
            if !out.contains(c) {
                out.push(*c);
            }
        }
        out
    }
    /// The name its entities carried (earlier versions).
    pub fn tag(&self) -> String {
        entity_tag(self.id, self.idx, self.origin())
    }
}

/// A folded stack of an earlier version (its effect and place in it) and the chunks it was seen in.
pub type Folded = ((u64, u32), Vec<[i32; 2]>);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChunkLedger {
    /// The stacks owed by effects whose drop waits.
    #[serde(default, rename = "entries", skip_serializing_if = "Vec::is_empty")]
    pub owed: Vec<Owed>,
    /// Ids of effects applied here, by earlier versions (and by this one to drop what was owed): a replay that finds
    /// one adds nothing. Kept until the host's storage forgets the effect.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty", with = "hex_set")]
    pub applied: BTreeSet<u64>,
    /// An earlier version's stacks that folded into another, with the chunks they were seen in: an entity file of one
    /// of those still showing them shows a copy (`ChunkLedger::legacy`). Written back until settled.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty", with = "stack_set")]
    pub gone: BTreeMap<(u64, u32), Vec<[i32; 2]>>,
    /// Machines of this chunk removed whose group has not taken them yet, with what they held.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removals: Vec<crate::persist::Removal>,
    /// Batches of effects of machines of this chunk still in flight.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batches: Vec<crate::server::CommittedEffects>,
}

impl ChunkLedger {
    pub fn is_empty(&self) -> bool {
        self.owed.is_empty() && self.applied.is_empty() && self.gone.is_empty() && self.removals.is_empty() && self.batches.is_empty()
    }
    pub fn knows(&self, id: u64) -> bool {
        self.applied.contains(&id) || self.owed.iter().any(|e| e.id == id)
    }
    /// The effect `id` owes `items` at `at` until they can be dropped. False when it is already known here.
    pub fn owe(&mut self, id: u64, at: [i32; 3], items: Vec<String>) -> bool {
        if self.knows(id) {
            return false;
        }
        for (idx, item) in items.into_iter().enumerate() {
            self.owed.push(Owed { id, idx: idx as u32, at, item, state: pending(), live: 0, last: None, seen: Vec::new() });
        }
        true
    }
    /// The owed stacks that `ready` says can be dropped now, taken out with their cell; their effects are applied.
    pub fn take_owed(&mut self, mut ready: impl FnMut(&Owed) -> bool) -> Vec<([i32; 3], String)> {
        let mut out = Vec::new();
        let mut kept = Vec::new();
        for e in std::mem::take(&mut self.owed) {
            if e.pending() && ready(&e) {
                self.applied.insert(e.id);
                out.push((e.at, e.item));
            } else {
                kept.push(e);
            }
        }
        self.owed = kept;
        out
    }
    /// Whether the record holds stacks of an earlier version still to settle (`legacy`).
    pub fn has_legacy(&self) -> bool {
        !self.gone.is_empty() || self.owed.iter().any(|e| !e.pending())
    }
    /// An earlier version's record settled once: the folded stacks' markers are returned (the host removes the copies
    /// their holders' files still show: the stack folded into another), and each spawned stack whose entities no file
    /// shows (`shown`) is owed again with what it held, to be dropped as any owed stack. Afterwards the record holds only
    /// owed stacks and applied ids.
    pub fn legacy(&mut self, mut shown: impl FnMut(&Owed) -> bool) -> Vec<Folded> {
        let copies: Vec<Folded> = std::mem::take(&mut self.gone).into_iter().collect();
        let mut kept = Vec::new();
        for mut e in std::mem::take(&mut self.owed) {
            if e.pending() {
                kept.push(e);
            } else if shown(&e) {
                self.applied.insert(e.id);
            } else {
                if e.live > 0 {
                    e.item = with_count(&e.item, e.live);
                }
                e.state = pending();
                e.live = 0;
                e.last = None;
                e.seen.clear();
                kept.push(e);
            }
        }
        self.owed = kept;
        copies
    }
    /// Forgets the applied ids the host's storage no longer names. Returns whether any went. A record still to settle
    /// is left as it is: its ids say what its stacks were (F2 of the fresh review).
    pub fn prune(&mut self, live: &BTreeSet<u64>) -> bool {
        if self.has_legacy() {
            return false;
        }
        let before = self.applied.len();
        self.applied.retain(|id| live.contains(id) || self.owed.iter().any(|e| e.id == *id));
        self.applied.len() != before
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    pub fn from_json(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }
}

/// The SNBT stack `item` with its count set to `count`, read by the SNBT parser (a `count:` or a brace inside a quoted
/// component is text, not structure). A stack that does not read is kept as it is.
fn with_count(item: &str, count: u32) -> String {
    if crate::snbt::fields(item).is_none() {
        return item.to_string();
    }
    crate::snbt::with_field(Some(item), "count", &count.to_string())
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

/// `<id>:<idx>`, then `@x,z;x,z` for the holders when there are any.
mod stack_set {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;
    type Gone = BTreeMap<(u64, u32), Vec<[i32; 2]>>;
    pub fn serialize<S: Serializer>(v: &Gone, s: S) -> Result<S::Ok, S::Error> {
        v.iter()
            .map(|((id, idx), holders)| {
                let at: Vec<String> = holders.iter().map(|c| format!("{},{}", c[0], c[1])).collect();
                if at.is_empty() { format!("{id:016x}:{idx}") } else { format!("{id:016x}:{idx}@{}", at.join(";")) }
            })
            .collect::<Vec<_>>()
            .serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Gone, D::Error> {
        let bad = || serde::de::Error::custom("stack");
        Vec::<String>::deserialize(d)?
            .into_iter()
            .map(|t| {
                let (stack, at) = t.split_once('@').unwrap_or((t.as_str(), ""));
                let (id, idx) = stack.split_once(':').ok_or_else(bad)?;
                let holders = at
                    .split(';')
                    .filter(|c| !c.is_empty())
                    .map(|c| {
                        let (x, z) = c.split_once(',').ok_or_else(bad)?;
                        Ok([x.parse().map_err(|_| bad())?, z.parse().map_err(|_| bad())?])
                    })
                    .collect::<Result<Vec<[i32; 2]>, D::Error>>()?;
                Ok(((u64::from_str_radix(id, 16).map_err(serde::de::Error::custom)?, idx.parse().map_err(serde::de::Error::custom)?), holders))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_owed_effect_is_known_until_dropped_and_after() {
        let mut ledger = ChunkLedger::default();
        let id = effect_id(7, 0);
        assert!(ledger.owe(id, [1, 64, 2], vec!["{id:'ntm:niter',count:3}".into()]));
        assert!(!ledger.owe(id, [1, 64, 2], vec!["{id:'ntm:niter',count:3}".into()]), "a replay adds nothing");
        assert!(ledger.take_owed(|_| false).is_empty());
        assert_eq!(ledger.take_owed(|_| true), vec![([1, 64, 2], "{id:'ntm:niter',count:3}".to_string())]);
        assert!(ledger.knows(id) && ledger.owed.is_empty());
        let back = ChunkLedger::from_json(&ledger.to_json());
        assert_eq!(back, ledger);
    }

    #[test]
    fn identical_effects_of_one_batch_have_their_own_ids_and_a_source_has_one() {
        let ids: BTreeSet<u64> = (0..50u32).map(|n| effect_id(1, n)).chain((0..50u32).map(|n| effect_id(2, n))).collect();
        assert_eq!(ids.len(), 100);
        assert_eq!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:overworld", [3, 64, -4], 5));
        assert_ne!(retained_id("minecraft:overworld", [3, 64, -4], 5), retained_id("minecraft:overworld", [3, 64, -4], 6));
    }

    #[test]
    fn a_count_is_set_at_the_stacks_own_depth() {
        let count = |item: &str| crate::snbt::fields(item).unwrap()["count"].clone();
        let set = with_count("{id:'ntm:niter',count:16}", 64);
        assert_eq!((count(&set), crate::snbt::fields(&set).unwrap()["id"].as_str()), ("64".into(), "\"ntm:niter\""));
        let nested = with_count("{components:{'minecraft:bundle_contents':[{id:'a',count:2}]},count:1,id:'b'}", 9);
        assert_eq!(count(&nested), "9");
        assert!(nested.contains("{count:2,id:\"a\"}"), "{nested}");
        // F3 of the fresh review: a brace and a `count:` inside a quoted component are text.
        let quoted = with_count("{components:{'minecraft:custom_data':{note:'{',n:'count:3'}},count:16,id:'ntm:niter'}", 64);
        assert_eq!(count(&quoted), "64", "{quoted}");
        assert!(quoted.contains("note:\"{\"") && quoted.contains("n:\"count:3\""), "{quoted}");
    }

    #[test]
    fn the_ledger_forgets_only_what_storage_no_longer_names() {
        let mut ledger = ChunkLedger::default();
        let (a, b) = (effect_id(1, 0), effect_id(1, 1));
        ledger.applied.extend([a, b]);
        assert!(ledger.prune(&BTreeSet::from([b])));
        assert_eq!(ledger.applied, BTreeSet::from([b]));
        assert_eq!(ChunkLedger::from_json("not json"), ChunkLedger::default());
        assert_eq!(ChunkLedger::default().to_json(), "{}");
    }
}

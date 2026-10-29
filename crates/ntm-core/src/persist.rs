//! A world's machines in `storage` (docs/fundacao.md, "Persistência").
//!
//! One key per world, `w.<wid>`, holds the whole world: every machine and
//! cable of every dimension. `storage::set` replaces a file without an
//! atomic rename, so a crash mid-write could leave half of it: each save
//! first goes whole to the shared `journal` key, and a tick later to
//! `w.<wid>`. Loading takes, of the two, the copy for this world whose
//! checksum holds and whose generation is newest.
//!
//! Format: one header line `NTM1 <wid> <generation> <fnv64 hex> <len>`, then the
//! JSON. The budget: one hook call may write 1 MiB, and the worst call
//! writes a save three times (a journal of another world moved to its key,
//! the journal, the world's key) plus a mailbox, so a save is at most
//! [`MAX_SAVE`] bytes. The folder holds 4 MiB and 64 keys: [`Worlds`]
//! counts what the saves take, for a warning before it fills.

use crate::{
    Pos,
    hash::fnv64,
    kv::{Kv, keys},
    machine::Machine,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SAVE: usize = 256 << 10;
/// The folder's quota and key count, as the game enforces them.
pub const QUOTA: u64 = 4 << 20;
pub const MAX_KEYS: usize = 64;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineEntry {
    pub d: String,
    pub p: Pos,
    pub m: MachineSer,
}
/// Serde helper so an unknown machine kind (a newer save) is kept as raw
/// JSON rather than failing the load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MachineSer {
    Known(Machine),
    Raw(serde_json::Value),
}
impl Default for MachineSer {
    fn default() -> Self {
        Self::Raw(serde_json::Value::Null)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SaveData {
    #[serde(default)]
    pub radiation: BTreeMap<String, crate::radiation::Dose>,
    /// Item transactions in flight: committed with the machines they move
    /// items of (`crate::tx`).
    #[serde(default)]
    pub txs: Vec<crate::tx::Tx>,
    /// The next transaction id, and this save's salt for score holders (a
    /// lost save starts a new salt: its ids never meet old scores).
    #[serde(default)]
    pub next_tx: u64,
    #[serde(default)]
    pub salt: String,
    /// Transactions that went through, kept until the world surely saved
    /// them ([`crate::tx::Applied`]), and the world's tick count.
    #[serde(default)]
    pub applied: Vec<crate::tx::Applied>,
    #[serde(default)]
    pub ticks: u64,
    /// Score resets waiting for the client (lines), kept until it has them.
    #[serde(default)]
    pub resets: Vec<String>,
    /// Answer cells of finished transactions, watched for a late token until
    /// the world's tick count given (durable: a token whose restore didn't
    /// reach the world before a crash is still put back).
    #[serde(default)]
    pub retired: Vec<(crate::tx::Cell, u64)>,
    /// Bombs: explosion count, radiation zones, detonator links, settings
    /// (`crate::blast::BombWorld`).
    #[serde(default)]
    pub bombs: crate::blast::BombWorld,
    pub v: u32,
    pub machines: Vec<MachineEntry>,
    /// Per dimension, flat `x y z x y z ...`.
    pub cables: BTreeMap<String, Vec<i32>>,
}

/// The live state of one world, by dimension and position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldState {
    pub radiation: BTreeMap<String, crate::radiation::Dose>,
    pub txs: Vec<crate::tx::Tx>,
    pub next_tx: u64,
    pub salt: String,
    pub applied: Vec<crate::tx::Applied>,
    pub ticks: u64,
    pub resets: Vec<String>,
    pub retired: Vec<(crate::tx::Cell, u64)>,
    pub machines: BTreeMap<(String, Pos), Machine>,
    pub cables: BTreeSet<(String, Pos)>,
    pub bombs: crate::blast::BombWorld,
    /// Entries a newer version wrote: kept as they were.
    pub unknown: Vec<MachineEntry>,
}

impl WorldState {
    pub fn to_save(&self) -> SaveData {
        let mut machines: Vec<MachineEntry> = self
            .machines
            .iter()
            .map(|((d, p), m)| MachineEntry {
                d: d.clone(),
                p: *p,
                m: MachineSer::Known(m.clone()),
            })
            .collect();
        machines.extend(self.unknown.iter().cloned());
        let mut cables: BTreeMap<String, Vec<i32>> = BTreeMap::new();
        for (d, p) in &self.cables {
            cables.entry(d.clone()).or_default().extend_from_slice(p);
        }
        SaveData {
            radiation: self.radiation.clone(),
            txs: self.txs.clone(),
            next_tx: self.next_tx,
            salt: self.salt.clone(),
            applied: self.applied.clone(),
            ticks: self.ticks,
            resets: self.resets.clone(),
            retired: self.retired.clone(),
            bombs: self.bombs.clone(),
            v: 1,
            machines,
            cables,
        }
    }

    pub fn from_save(s: SaveData) -> Self {
        let mut w = WorldState {
            radiation: s.radiation,
            txs: s.txs,
            next_tx: s.next_tx,
            salt: s.salt,
            applied: s.applied,
            ticks: s.ticks,
            resets: s.resets,
            retired: s.retired,
            bombs: s.bombs,
            ..WorldState::default()
        };
        for e in s.machines {
            match e.m {
                MachineSer::Known(m) => {
                    w.machines.insert((e.d, e.p), m);
                }
                MachineSer::Raw(_) => w.unknown.push(e),
            }
        }
        for (d, flat) in s.cables {
            for p in flat.as_chunks::<3>().0 {
                w.cables.insert((d.clone(), [p[0], p[1], p[2]]));
            }
        }
        w
    }
}

pub fn encode(wid: &str, generation: u64, data: &SaveData) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(data).map_err(|e| e.to_string())?;
    let mut out = format!(
        "NTM1 {wid} {generation} {:016x} {}\n",
        fnv64(&json),
        json.len()
    )
    .into_bytes();
    out.extend_from_slice(&json);
    if out.len() > MAX_SAVE {
        return Err(format!(
            "o save do mundo tem {} KiB, acima do limite de {} KiB",
            out.len() >> 10,
            MAX_SAVE >> 10
        ));
    }
    Ok(out)
}

/// `(wid, generation, data)` when the bytes are a whole, unchanged save.
pub fn decode(bytes: &[u8]) -> Option<(String, u64, SaveData)> {
    let nl = bytes.iter().position(|b| *b == b'\n')?;
    let header = std::str::from_utf8(&bytes[..nl]).ok()?;
    let mut f = header.split(' ');
    if f.next()? != "NTM1" {
        return None;
    }
    let wid = f.next()?.to_string();
    let generation: u64 = f.next()?.parse().ok()?;
    let sum = u64::from_str_radix(f.next()?, 16).ok()?;
    let len: usize = f.next()?.parse().ok()?;
    let json = &bytes[nl + 1..];
    if json.len() != len || fnv64(json) != sum {
        return None;
    }
    Some((wid, generation, serde_json::from_slice(json).ok()?))
}

/// The newest whole copy of world `wid`: its key or the journal.
pub fn load(kv: &mut impl Kv, wid: &str) -> Option<(u64, SaveData)> {
    [kv.get(&keys::world(wid)), kv.get(keys::JOURNAL)]
        .into_iter()
        .flatten()
        .filter_map(|b| decode(&b))
        .filter(|(w, _, _)| w == wid)
        .max_by_key(|(_, g, _)| *g)
        .map(|(_, g, d)| (g, d))
}

/// A save in two steps, one per tick: the journal, then the world's key.
#[derive(Debug, Default)]
pub struct Saver {
    pending: Option<(String, Vec<u8>)>,
}

impl Saver {
    /// Step one: the whole save to the journal; step two waits for
    /// [`Saver::finish`].
    pub fn begin(&mut self, kv: &mut impl Kv, wid: &str, bytes: Vec<u8>) -> Result<(), String> {
        // The journal may hold another world's newest save (its second step
        // never ran: the world was left, or the game stopped): move it to
        // that world's key before it is overwritten.
        if let Some(old) = kv.get(keys::JOURNAL)
            && let Some((other, generation, _)) = decode(&old)
            && other != wid
        {
            let key_gen = kv
                .get(&keys::world(&other))
                .and_then(|b| decode(&b))
                .map(|(_, g, _)| g);
            if key_gen.is_none_or(|g| g < generation) {
                kv.set(&keys::world(&other), &old)?;
            }
        }
        kv.set(keys::JOURNAL, &bytes)?;
        self.pending = Some((wid.to_string(), bytes));
        Ok(())
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    /// Bytes of the save waiting for its second step.
    pub fn pending_len(&self) -> usize {
        self.pending.as_ref().map_or(0, |(_, b)| b.len())
    }
    /// Step two, at a later call (a call may write 1 MiB in all).
    pub fn finish(&mut self, kv: &mut impl Kv) -> Result<(), String> {
        let Some((wid, bytes)) = self.pending.take() else {
            return Ok(());
        };
        kv.set(&keys::world(&wid), &bytes)
    }
    /// Both steps now (a world being left: there may be no later tick).
    pub fn now(&mut self, kv: &mut impl Kv, wid: &str, bytes: Vec<u8>) -> Result<(), String> {
        self.begin(kv, wid, bytes)?;
        self.finish(kv)
    }
}

/// The `worlds` key: what this installation has saved.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Worlds {
    /// Bumped at every server mod start: entropy, and a clock for `seen`.
    pub boot: u64,
    pub worlds: BTreeMap<String, WorldInfo>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldInfo {
    pub seen: u64,
    pub machines: usize,
    /// Bytes of its last save.
    #[serde(default)]
    pub bytes: u64,
}

impl Worlds {
    /// Bytes and keys the folder holds by this count: every world's save,
    /// the journal (as large as the largest), the mailboxes at their bound,
    /// the fixed keys.
    pub fn usage(&self) -> (u64, usize) {
        let saves: u64 = self.worlds.values().map(|w| w.bytes).sum();
        let largest = self.worlds.values().map(|w| w.bytes).max().unwrap_or(0);
        let mailboxes = 2 * crate::proto::MAX_ENVELOPE as u64;
        (saves + largest + mailboxes, self.worlds.len() + 5)
    }
    /// Over 80% of the bytes or the keys.
    pub fn nearly_full(&self) -> bool {
        let (bytes, n) = self.usage();
        bytes * 5 > QUOTA * 4 || n * 5 > MAX_KEYS * 4
    }
}

impl Worlds {
    pub fn load(kv: &mut impl Kv) -> Self {
        kv.get(keys::WORLDS)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn store(&self, kv: &mut impl Kv) -> Result<(), String> {
        kv.set(
            keys::WORLDS,
            &serde_json::to_vec(self).map_err(|e| e.to_string())?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kv::MemKv, machine::Kind};

    fn world() -> WorldState {
        let mut w = WorldState::default();
        w.machines.insert(
            ("minecraft:overworld".into(), [1, 64, 2]),
            Machine::new(Kind::Battery, "ntm:machine_battery", Some("north".into())),
        );
        w.cables.insert(("minecraft:overworld".into(), [2, 64, 2]));
        w.radiation.insert(
            "player-uuid".into(),
            crate::radiation::Dose { millirad: 123_456 },
        );
        w
    }

    #[test]
    fn a_journal_of_another_world_is_moved_to_its_key_first() {
        let mut kv = MemKv::default();
        let w = world();
        let mut saver = Saver::default();
        // World A's newest save is in the journal only (left before step 2).
        saver.begin(&mut kv, "aaaa", encode("aaaa", 7, &w.to_save()).unwrap()).unwrap();
        let mut saver = Saver::default();
        saver.begin(&mut kv, "bbbb", encode("bbbb", 1, &WorldState::default().to_save()).unwrap()).unwrap();
        let (g, d) = load(&mut kv, "aaaa").expect("A survives B's save");
        assert_eq!((g, WorldState::from_save(d)), (7, w));
        assert!(kv.written < (3 * MAX_SAVE) as u64 + 4096);
    }

    #[test]
    fn round_trip_and_torn_write() {
        let mut kv = MemKv::default();
        let w = world();
        let mut saver = Saver::default();
        saver
            .begin(&mut kv, "0123", encode("0123", 1, &w.to_save()).unwrap())
            .unwrap();
        // Crash between the steps: the journal alone loads.
        let (g, d) = load(&mut kv, "0123").unwrap();
        assert_eq!((g, WorldState::from_save(d)), (1, w.clone()));
        saver.finish(&mut kv).unwrap();
        // A torn second save of the world key: the journal (same generation 2) wins.
        let mut w2 = w.clone();
        w2.cables.clear();
        let b = encode("0123", 2, &w2.to_save()).unwrap();
        saver.begin(&mut kv, "0123", b.clone()).unwrap();
        kv.map.insert("w.0123".into(), b[..b.len() - 3].to_vec());
        let (g, d) = load(&mut kv, "0123").unwrap();
        assert_eq!((g, WorldState::from_save(d)), (2, w2));
        // Another world's journal never loads here.
        assert!(load(&mut kv, "9999").is_none());
    }

    #[test]
    fn unknown_kinds_survive() {
        let json = r#"{"v":1,"machines":[{"d":"x","p":[0,0,0],"m":{"data":{"k":"Reactor","heat":5}}}],"cables":{}}"#;
        let s: SaveData = serde_json::from_str(json).unwrap();
        let w = WorldState::from_save(s);
        assert!(w.machines.is_empty());
        assert!(
            w.radiation.is_empty(),
            "old saves without radiation still load"
        );
        assert_eq!(w.unknown.len(), 1);
        let back = serde_json::to_string(&w.to_save()).unwrap();
        assert!(back.contains("Reactor"));
    }
}

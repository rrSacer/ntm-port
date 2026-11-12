//! The world the server runs (docs/nativo.md). Each machine's state is its
//! block entity's and the cables are found again from the blocks, so the
//! world keeps only the rest: bombs set off and not yet exploded, the
//! detonators' links, the players' doses, the ticks played (the balefires'
//! clock). That goes to the host's [`Kv`] under [`KEY`] whenever it
//! changes, and every [`TICKS_EVERY`] ticks for the clock alone.
//!
//! What a machine's removal or step still owes the world (a removal waiting for its group, a batch of effects in
//! flight) is not kept here: it lives in the record of its chunk (`server::obligations`), written with the block
//! entities it follows, so a save never pairs it with records it does not belong to. Only an earlier version's storage
//! still holds some (`WorldState::legacy`), kept as it read them until a waited save wrote their chunk's record with
//! them, or that record was read back with them (`server::obligations`).

use crate::{Pos, kv::Kv, machine::Machine};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The key of the world's state in the host's storage.
pub const KEY: &str = "world";
/// The key of the RBMK layers' state (`WorldState::rbmk`), in the same storage and written with it at every save, apart
/// from the rest: a reactor that steps rewrites only this one between saves (P2-3 of the T01 review).
pub const RBMK_KEY: &str = "rbmk";
/// The sickness rolls' random (`radiation::SicknessRng`), under its own key: a tick with someone sick rewrites only it.
pub const SICKNESS_KEY: &str = "sickness";
/// The storage key of the world's radiation by chunk as the chunks' saves hold it (`chunk_radiation::Saved`): under its
/// own key, so a save rewrites it alone when only it changed.
pub const CHUNK_RADIATION_KEY: &str = "chunk_radiation";
/// Ticks between keeps of the clock alone: a crash sets it back at most this.
pub const TICKS_EVERY: u64 = 600;

/// Stamps of changes, unique in the process: a part changed since it was kept when its stamp differs.
static STAMPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn stamp() -> u64 {
    STAMPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A kept part of the world with the stamp of its last change. Reading it is free; every mutable access stamps it
/// anew, so no path that changes it can be left out of the next keep. A copy keeps the stamp of what it copied: one
/// put back in place still differs from what was kept after it changed.
#[derive(Clone, Debug)]
pub struct Tracked<T> {
    value: T,
    stamp: u64,
}

impl<T> Tracked<T> {
    pub fn new(value: T) -> Self {
        Tracked { value, stamp: stamp() }
    }
    /// The stamp of the part's last change.
    pub fn stamp(this: &Self) -> u64 {
        this.stamp
    }
}

/// A new part, as `std::mem::take` leaves in place of the one it takes: a change.
impl<T: Default> Default for Tracked<T> {
    fn default() -> Self {
        Tracked::new(T::default())
    }
}

/// The same content, whatever the stamps.
impl<T: PartialEq> PartialEq for Tracked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> std::ops::Deref for Tracked<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> std::ops::DerefMut for Tracked<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.stamp = stamp();
        &mut self.value
    }
}

impl<'a, T> IntoIterator for &'a Tracked<T>
where
    &'a T: IntoIterator,
{
    type Item = <&'a T as IntoIterator>::Item;
    type IntoIter = <&'a T as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        (&self.value).into_iter()
    }
}

impl<T> From<T> for Tracked<T> {
    fn from(value: T) -> Self {
        Tracked::new(value)
    }
}

/// A confirmed removal may outlive its BE. Remember which inventory the
/// engine already owns/drops, so reopening cannot emit those slots again.
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct Removal {
    pub key:(String,Pos),
    pub machine:Machine,
    pub engine_slots:bool,
}

/// What the world keeps outside its blocks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub radiation: BTreeMap<String, crate::radiation::Dose>,
    #[serde(default)]
    pub bombs: crate::blast::BombWorld,
    #[serde(default)]
    pub zirnox: crate::zirnox::Remnants,
    #[serde(default)]
    pub ticks: u64,
    /// Batches of effects named so far (`CommittedEffects::serial`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub effect_serial: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removals:Vec<Removal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generations:Vec<((String,Pos),u64)>,
    #[serde(default, skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub machine_effects:std::collections::VecDeque<crate::server::CommittedEffects>,
    #[serde(default, skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub applied_machine_effects:std::collections::VecDeque<crate::server::CommittedEffects>,
    #[serde(default, skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub prepared_retentions:std::collections::VecDeque<crate::server::PreparedRetention>,
    /// What machines became in steps whose block entities may reach the disk in different chunk records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub survivors:Vec<crate::server::Survivor>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// A removal or a batch of effects, by what names it for good.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Obligation {
    Removal((String, Pos), u64),
    Batch(u64),
}

impl Removal {
    pub fn obligation(&self) -> Obligation {
        Obligation::Removal(self.key.clone(), self.machine.generation)
    }
}

/// An earlier version's removals and batches, as its storage held them. They are written back as they were read until
/// they leave (`server::obligations`): what the world did with them since is in its chunks' records, not here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Legacy {
    pub removals: Vec<Removal>,
    /// Each with the name it goes by: its own, or, for one an earlier version did not name, one derived from it, the
    /// same at every read.
    pub batches: Vec<(u64, crate::server::CommittedEffects)>,
}

impl Legacy {
    fn read(removals: Vec<Removal>, batches: std::collections::VecDeque<crate::server::CommittedEffects>) -> Self {
        let named = |b: crate::server::CommittedEffects| {
            let name = if b.serial != 0 {
                b.serial
            } else {
                crate::ledger::fnv(&[b"legacy batch", &serde_json::to_vec(&b).unwrap_or_default()]).max(1)
            };
            (name, b)
        };
        Legacy { removals, batches: batches.into_iter().map(named).collect() }
    }
    pub fn is_empty(&self) -> bool {
        self.removals.is_empty() && self.batches.is_empty()
    }
    pub fn len(&self) -> usize {
        self.removals.len() + self.batches.len()
    }
    pub fn ids(&self) -> BTreeSet<Obligation> {
        self.removals.iter().map(Removal::obligation).chain(self.batches.iter().map(|(n, _)| Obligation::Batch(*n))).collect()
    }
    /// Keeps the ones `keep` names.
    pub fn retain(&mut self, keep: impl Fn(&Obligation) -> bool) {
        self.removals.retain(|r| keep(&r.obligation()));
        self.batches.retain(|(n, _)| keep(&Obligation::Batch(*n)));
    }
}

/// The kept parts are [`Tracked`]: whether they changed since the last keep is known without serializing them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldState {
    /// Ticks played in this world.
    pub ticks: u64,
    pub effect_serial: Tracked<u64>,
    pub removals:Tracked<Vec<Removal>>,
    pub generations:Tracked<BTreeMap<(String,Pos),u64>>,
    pub machine_effects:Tracked<std::collections::VecDeque<crate::server::CommittedEffects>>,
    pub applied_machine_effects:Tracked<std::collections::VecDeque<crate::server::CommittedEffects>>,
    pub prepared_retentions:Tracked<std::collections::VecDeque<crate::server::PreparedRetention>>,
    /// The newest journaled state of each machine (the one of the highest revision), until a save is confirmed.
    pub survivors:Tracked<BTreeMap<(String,Pos),crate::server::Survivor>>,
    pub radiation: Tracked<BTreeMap<String, crate::radiation::Dose>>,
    pub machines: BTreeMap<(String, Pos), Machine>,
    pub cables: BTreeSet<(String, Pos)>,
    pub bombs: Tracked<crate::blast::BombWorld>,
    pub zirnox: Tracked<crate::zirnox::Remnants>,
    /// The sickness rolls' `java.util.Random` (radiation::SicknessRng): seeded from the world's seed the first time
    /// someone is sick, so a world nobody was sick in never writes it.
    pub sickness: Tracked<Option<crate::radiation::SicknessRng>>,
    /// What each chunk's save holds of the world's radiation (`hfr_simple_radiation`, `CHUNK_RADIATION_KEY`).
    pub chunk_radiation: Tracked<crate::chunk_radiation::Saved>,
    /// Each RBMK layer's simulation state, by dimension and base Y. It holds no item: a crash between the regions and
    /// this storage sets it back to the save before while the columns are as the regions left them (docs).
    pub rbmk: Tracked<BTreeMap<(String, i32), crate::rbmk::persist::Layer>>,
    /// The removals and batches an earlier version's storage held, still kept here as it held them: until a waited
    /// save wrote their chunk's record with them before this storage, or that record was read back with them.
    pub legacy: Tracked<Legacy>,
}

impl WorldState {
    /// The world as the host's storage kept it (empty when it holds none,
    /// or none that reads).
    pub fn load(kv: &mut impl Kv) -> Self {
        let kept: Kept = kv.get(KEY).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        // Removals and batches are an earlier version's: the server takes them up once their chunk's record says it
        // does not hold them already (`server::obligations`).
        let mut batches=kept.applied_machine_effects;
        batches.extend(kept.machine_effects);
        WorldState {
            legacy:Legacy::read(kept.removals,batches).into(),
            prepared_retentions:kept.prepared_retentions.into(),
            survivors:Tracked::new(kept.survivors.into_iter().map(|s|((s.dim.clone(),s.pos),s)).collect()),
            generations:Tracked::new(kept.generations.into_iter().collect()),
            radiation: kept.radiation.into(),
            bombs: kept.bombs.into(),
            zirnox: kept.zirnox.into(),
            sickness: kv
                .get(SICKNESS_KEY)
                .and_then(|b| serde_json::from_slice::<crate::radiation::SicknessRng>(&b).ok())
                .into(),
            chunk_radiation: kv
                .get(CHUNK_RADIATION_KEY)
                .and_then(|b| serde_json::from_slice::<crate::chunk_radiation::Saved>(&b).ok())
                .unwrap_or_default()
                .into(),
            rbmk: Tracked::new(
                kv.get(RBMK_KEY)
                    .and_then(|b| serde_json::from_slice::<Vec<((String, i32), crate::rbmk::persist::Layer)>>(&b).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(_, l)| l.schema == crate::rbmk::persist::SCHEMA)
                    .collect(),
            ),
            ticks: kept.ticks,
            effect_serial: kept.effect_serial.into(),
            ..WorldState::default()
        }
    }

    /// The stamps of the kept parts: equal to those of the last keep, the parts are as kept. Of removals and batches
    /// only an earlier version's are (`legacy`): the others are kept with their chunks.
    pub fn stamps(&self) -> [u64; 10] {
        [
            Tracked::stamp(&self.effect_serial),
            Tracked::stamp(&self.legacy),
            Tracked::stamp(&self.generations),
            0,
            0,
            Tracked::stamp(&self.prepared_retentions),
            0,
            Tracked::stamp(&self.radiation),
            Tracked::stamp(&self.bombs),
            Tracked::stamp(&self.zirnox),
        ]
    }

    /// The entries the storage would keep that can hold items or decide what a chunk holds: removals, batches and
    /// prepared retentions. Zero in a world of this version at every save (an earlier version's, until they leave).
    pub fn kept_items(&self) -> usize {
        self.legacy.len() + self.prepared_retentions.len()
    }

    /// The stamp of the sickness random's last change (`SICKNESS_KEY`).
    pub fn sickness_stamp(&self) -> u64 {
        Tracked::stamp(&self.sickness)
    }

    /// What [`WorldState::load`] reads back of the sickness random (`SICKNESS_KEY`): a few bytes, whatever the world.
    pub fn kept_sickness(&self) -> Vec<u8> {
        serde_json::to_vec(&*self.sickness).unwrap_or_default()
    }

    /// The stamp of the chunks' saved radiation's last change (`CHUNK_RADIATION_KEY`).
    pub fn chunk_radiation_stamp(&self) -> u64 {
        Tracked::stamp(&self.chunk_radiation)
    }

    /// What [`WorldState::load`] reads back of the chunks' saved radiation (`CHUNK_RADIATION_KEY`).
    pub fn kept_chunk_radiation(&self) -> Vec<u8> {
        serde_json::to_vec(&*self.chunk_radiation).unwrap_or_default()
    }

    /// The stamp of the RBMK layers' last change (`RBMK_KEY`).
    pub fn rbmk_stamp(&self) -> u64 {
        Tracked::stamp(&self.rbmk)
    }

    /// What [`WorldState::load`] reads back of the RBMK layers (`RBMK_KEY`): as big as the reactors, whatever the world.
    pub fn kept_rbmk(&self) -> Vec<u8> {
        serde_json::to_vec(&self.rbmk.iter().collect::<Vec<_>>()).unwrap_or_default()
    }

    /// What [`WorldState::load`] reads back, the clock at `ticks`.
    pub fn kept(&self, ticks: u64) -> Vec<u8> {
        // Borrowed, not copied: this is the size of every journal write.
        #[derive(Serialize)]
        struct KeptRef<'a> {
            #[serde(skip_serializing_if = "BTreeMap::is_empty")]
            radiation: &'a BTreeMap<String, crate::radiation::Dose>,
            bombs: &'a crate::blast::BombWorld,
            zirnox: &'a crate::zirnox::Remnants,
            ticks: u64,
            #[serde(skip_serializing_if = "is_zero")]
            effect_serial: &'a u64,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            removals: Vec<&'a Removal>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            generations: Vec<(&'a (String, Pos), &'a u64)>,
            #[serde(skip_serializing_if = "std::collections::VecDeque::is_empty")]
            machine_effects: std::collections::VecDeque<&'a crate::server::CommittedEffects>,
            #[serde(skip_serializing_if = "std::collections::VecDeque::is_empty")]
            prepared_retentions: &'a std::collections::VecDeque<crate::server::PreparedRetention>,
        }
        let kept = KeptRef {
            radiation: &self.radiation,
            bombs: &self.bombs,
            zirnox: &self.zirnox,
            ticks,
            effect_serial: &self.effect_serial,
            // Receipts and survivors are the session's: every save confirms them before the storage is written.
            removals: self.legacy.removals.iter().collect(),
            generations: self.generations.iter().collect(),
            machine_effects: self.legacy.batches.iter().map(|(_, b)| b).collect(),
            prepared_retentions: &self.prepared_retentions,
        };
        serde_json::to_vec(&kept).unwrap_or_default()
    }
}

#[cfg(test)]
impl WorldState {
    /// The storage as an earlier version wrote it: removals, batches, receipts and survivors with the rest.
    pub fn kept_as_an_earlier_version(&self, ticks: u64) -> Vec<u8> {
        let kept = Kept {
            radiation: (*self.radiation).clone(),
            bombs: (*self.bombs).clone(),
            zirnox: (*self.zirnox).clone(),
            ticks,
            effect_serial: *self.effect_serial,
            removals: (*self.removals).clone(),
            generations: self.generations.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            machine_effects: (*self.machine_effects).clone(),
            applied_machine_effects: (*self.applied_machine_effects).clone(),
            prepared_retentions: (*self.prepared_retentions).clone(),
            survivors: self.survivors.values().cloned().collect(),
        };
        serde_json::to_vec(&kept).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::MemKv;

    #[test]
    fn the_kept_state_reads_back() {
        let mut kv = MemKv::default();
        assert_eq!(WorldState::load(&mut kv), WorldState::default());
        let mut w = WorldState::default();
        w.radiation.insert("p".into(), crate::radiation::Dose { millirad: 7 });
        w.bombs.next = 3;
        w.ticks = 9;
        w.machines.insert(("d".into(), [0, 0, 0]), Machine::new(crate::machine::Kind::Battery, "ntm:machine_battery", None));
        kv.set(KEY, &w.kept(w.ticks)).unwrap();
        let back = WorldState::load(&mut kv);
        assert_eq!((back.radiation, back.bombs.next, back.ticks, back.machines.len()), (w.radiation, 3, 9, 0));
        kv.set(KEY, b"not json").unwrap();
        assert_eq!(WorldState::load(&mut kv), WorldState::default());
    }

    /// A world saved before slice 4e kept its bombs' zones (the port's lingering areas, gone: the Java has none): it
    /// loads with everything else it kept, and its next save has no zones.
    #[test]
    fn a_world_kept_with_zones_loads_without_them() {
        let mut kv = MemKv::default();
        let mut w = WorldState::default();
        w.radiation.insert("p".into(), crate::radiation::Dose { millirad: 7 });
        w.bombs.next = 3;
        w.bombs.links.insert("p".into(), ("ntm:nuke_boy".into(), [1, 70, 1]));
        w.ticks = 9;
        let mut old: serde_json::Value = serde_json::from_slice(&w.kept(w.ticks)).unwrap();
        old["bombs"]["zones"] = serde_json::json!([{"dim": "minecraft:overworld", "center": [0, 70, 0], "radius": 25, "peak": 2500, "t0": 0, "half_life": 12000}]);
        kv.set(KEY, &serde_json::to_vec(&old).unwrap()).unwrap();
        let back = WorldState::load(&mut kv);
        assert_eq!((&back.radiation, back.bombs.next, &back.bombs.links, back.ticks), (&w.radiation, 3, &w.bombs.links, 9));
        let again: serde_json::Value = serde_json::from_slice(&back.kept(back.ticks)).unwrap();
        assert!(again["bombs"].get("zones").is_none(), "{again}");
    }
}

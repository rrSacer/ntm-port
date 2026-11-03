//! The world the server runs (docs/nativo.md). Each machine's state is its
//! block entity's and the cables are found again from the blocks, so the
//! world keeps only the rest: bombs set off and not yet exploded, their
//! zones, the detonators' links, the players' doses, the ticks played (the
//! zones' clock). That goes to the host's [`Kv`] under [`KEY`] whenever it
//! changes, and every [`TICKS_EVERY`] ticks for the clock alone.

use crate::{Pos, kv::Kv, machine::Machine};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The key of the world's state in the host's storage.
pub const KEY: &str = "world";
/// Ticks between keeps of the clock alone: a crash sets it back at most this.
pub const TICKS_EVERY: u64 = 600;

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removals:Vec<Removal>,
    #[serde(default, skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub machine_effects:std::collections::VecDeque<crate::server::CommittedEffects>,
    #[serde(default, skip_serializing_if = "std::collections::VecDeque::is_empty")]
    pub applied_machine_effects:std::collections::VecDeque<crate::server::CommittedEffects>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldState {
    /// Ticks played in this world.
    pub ticks: u64,
    pub removals:Vec<Removal>,
    pub machine_effects:std::collections::VecDeque<crate::server::CommittedEffects>,
    pub applied_machine_effects:std::collections::VecDeque<crate::server::CommittedEffects>,
    pub radiation: BTreeMap<String, crate::radiation::Dose>,
    pub machines: BTreeMap<(String, Pos), Machine>,
    pub cables: BTreeSet<(String, Pos)>,
    pub bombs: crate::blast::BombWorld,
    pub zirnox: crate::zirnox::Remnants,
}

impl WorldState {
    /// The world as the host's storage kept it (empty when it holds none,
    /// or none that reads).
    pub fn load(kv: &mut impl Kv) -> Self {
        let kept: Kept = kv.get(KEY).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut machine_effects=kept.applied_machine_effects;
        machine_effects.extend(kept.machine_effects);
        let machines=kept.removals.iter().map(|r|(r.key.clone(),r.machine.clone())).collect();
        WorldState {
            machines,
            removals:kept.removals,
            radiation: kept.radiation,
            bombs: kept.bombs,
            zirnox: kept.zirnox,
            ticks: kept.ticks,
            machine_effects,
            ..WorldState::default()
        }
    }

    /// What [`WorldState::load`] reads back, the clock at `ticks`.
    pub fn kept(&self, ticks: u64) -> Vec<u8> {
        let kept = Kept {
            radiation: self.radiation.clone(),
            bombs: self.bombs.clone(),
            zirnox: self.zirnox.clone(),
            ticks,
            removals:self.removals.clone(),
            machine_effects:self.machine_effects.clone(),
            applied_machine_effects:self.applied_machine_effects.clone(),
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
}

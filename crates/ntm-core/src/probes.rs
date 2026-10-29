//! Probes that move no items (docs/fundacao.md, "Sondas"): the world-id
//! column and the radiation sample. Items move only through transactions
//! ([`crate::tx`]), whose truth is in the world's scoreboard.
//!
//! - A probe is stored **before** its lines are published, and its lines
//!   are published only once the store succeeded, so a crash never leaves
//!   tokens in the world nobody knows about.
//! - Stores alternate between two keys, each with a generation and a
//!   checksum: a torn write leaves the other one whole.
//! - Only cells still showing a token are put back; a late probe is only
//!   put back, never read for its answer.

use crate::{Pos, hash::fnv64, kv::Kv, token::Cell};
use serde::{Deserialize, Serialize};

/// The two storage keys the probes alternate between.
pub const KEYS: [&str; 2] = ["probes.0", "probes.1"];
/// The single key of earlier versions: read once, then removed.
const OLD_KEY: &str = "probes";
/// Ticks a probe is active once the client acked its lines: its outcome is
/// reported at the end of it. Counted from the ack, not from the send, so
/// a client that ticks slower than the server (a loaded PC) isn't late.
pub const ACTIVE_TICKS: u64 = 100;
/// Ticks a probe waits for the client to ack its lines before it is
/// reported failed (a client that froze, or went).
pub const UNACKED_TICKS: u64 = 1200;
/// Ticks a late probe's cells are still watched, to put tokens back.
pub const LATE_TICKS: u64 = 12_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Purpose {
    /// The column that spells the world's id ([`crate::worldid`]).
    WorldId,
    /// The radiation module's inventory sample.
    Radiation { player: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub id: u32,
    /// The world it belongs to; `None` for a world-id probe (the column
    /// itself spells its world).
    pub wid: Option<String>,
    pub dim: String,
    pub cells: Vec<Cell>,
    pub purpose: Purpose,
    /// What the client runs.
    pub lines: Vec<String>,
    /// The client session it went to.
    pub sent_to: Option<u64>,
    /// Admitted, waiting for its store to succeed before it is published.
    #[serde(skip)]
    pub admitted: bool,
    /// The lines were sent by their owner (radiation).
    pub external: bool,
    /// The outbox seq of the message with its last lines (0: not all sent).
    /// Meaningful for the session it went to only.
    #[serde(skip)]
    pub seq: u64,
    /// The client acked `seq`: `deadline` counts from then.
    #[serde(skip)]
    pub acked: bool,
    pub deadline: u64,
    /// The outcome was reported; the cells are only put back now.
    pub late: bool,
    pub late_until: u64,
}

impl Probe {
    pub fn new(id: u32, wid: Option<String>, dim: &str, cells: Vec<Cell>, purpose: Purpose) -> Self {
        Self {
            id,
            wid,
            dim: dim.into(),
            cells,
            purpose,
            lines: Vec::new(),
            sent_to: None,
            admitted: false,
            external: false,
            seq: 0,
            acked: false,
            deadline: 0,
            late: false,
            late_until: 0,
        }
    }

    pub fn sent(&self) -> bool {
        self.sent_to.is_some() || self.external
    }

    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        self.cells.iter().map(|c| c.pos)
    }
}

/// Whether a probe needing `pos` in `dim` may start beside `others` (the
/// probes of the same world or session): never on a cell another uses.
pub fn admit(others: &[&Probe], dim: &str, pos: &[Pos]) -> bool {
    !others
        .iter()
        .any(|o| o.dim == dim && o.positions().any(|p| pos.contains(&p)))
}

/// What the store keeps between calls: the next slot and generation.
#[derive(Debug, Default)]
pub struct Store {
    generation: u64,
    last: Vec<u8>,
}

impl Store {
    /// Writes `probes` if they changed, to the slot the last write didn't
    /// use. Only `Ok` means they are durable.
    pub fn store(&mut self, kv: &mut impl Kv, probes: &[Probe]) -> Result<(), String> {
        let json = serde_json::to_vec(probes).map_err(|e| e.to_string())?;
        if json == self.last {
            return Ok(());
        }
        // The slot that doesn't hold the newest whole write; the generation
        // and slot move on only once the write went through, so a failed one
        // (maybe torn) is retried on the same slot and never on the other.
        let next = self.generation + 1;
        let mut out = format!("NTMP2 {next} {:016x} {}\n", fnv64(&json), json.len()).into_bytes();
        out.extend_from_slice(&json);
        kv.set(KEYS[(next % 2) as usize], &out)?;
        self.generation = next;
        self.last = json;
        Ok(())
    }

    /// The newest whole write a previous run left, and a store that goes
    /// on from its generation.
    pub fn load(kv: &mut impl Kv) -> (Vec<Probe>, Self) {
        let mut best: Option<(u64, Vec<Probe>)> = None;
        for key in KEYS {
            if let Some((g, p)) = kv.get(key).and_then(|b| decode(&b))
                && best.as_ref().is_none_or(|(bg, _)| g > *bg)
            {
                best = Some((g, p));
            }
        }
        if best.is_none()
            && let Some(old) = kv.get(OLD_KEY)
            && let Some(p) = decode_v1(&old)
        {
            best = Some((0, p));
        }
        kv.delete(OLD_KEY);
        let (generation, probes) = best.unwrap_or_default();
        (
            probes,
            Self {
                generation,
                last: Vec::new(),
            },
        )
    }
}

fn decode(bytes: &[u8]) -> Option<(u64, Vec<Probe>)> {
    let nl = bytes.iter().position(|b| *b == b'\n')?;
    let header = std::str::from_utf8(&bytes[..nl]).ok()?;
    let f: Vec<&str> = header.split(' ').collect();
    let json = &bytes[nl + 1..];
    if f.len() != 4
        || f[0] != "NTMP2"
        || u64::from_str_radix(f[2], 16).ok() != Some(fnv64(json))
        || f[3].parse::<usize>().ok() != Some(json.len())
    {
        return None;
    }
    Some((f[1].parse().ok()?, serde_json::from_slice(json).ok()?))
}

/// Earlier versions' format; item probes in it are dropped (their items
/// were never moved by the new transactions).
fn decode_v1(bytes: &[u8]) -> Option<Vec<Probe>> {
    let nl = bytes.iter().position(|b| *b == b'\n')?;
    let json: Vec<serde_json::Value> = serde_json::from_slice(&bytes[nl + 1..]).ok()?;
    Some(
        json.into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kv::MemKv, token::Restore};

    fn probe(dim: &str) -> Probe {
        Probe::new(
            1,
            Some("w".into()),
            dim,
            vec![Cell {
                pos: [0, 0, 0],
                restore: Restore::State("minecraft:bedrock".into()),
                seen: None,
            }],
            Purpose::WorldId,
        )
    }

    #[test]
    fn admission() {
        let p = probe("d");
        assert!(admit(&[], "d", &[[0, 0, 0]]));
        assert!(!admit(&[&p], "d", &[[0, 0, 0]]));
        assert!(admit(&[&p], "e", &[[0, 0, 0]]));
    }

    /// A store that tears its next `n` writes (half written, then an
    /// error), as an interrupted `storage.set` would.
    struct Tearing {
        kv: MemKv,
        tear: usize,
    }
    impl Kv for Tearing {
        fn get(&mut self, key: &str) -> Option<Vec<u8>> {
            self.kv.get(key)
        }
        fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
            if self.tear > 0 {
                self.tear -= 1;
                self.kv.set(key, &value[..value.len() / 2])?;
                return Err("torn".into());
            }
            self.kv.set(key, value)
        }
        fn delete(&mut self, key: &str) {
            self.kv.delete(key)
        }
    }

    /// Third check, finding (d): two failed writes in a row never destroy
    /// both slots: the retry goes to the same slot, the newest whole one
    /// stays readable throughout.
    #[test]
    fn failed_writes_retry_the_same_slot() {
        let mut kv = Tearing { kv: MemKv::default(), tear: 0 };
        let mut s = Store::default();
        let one = vec![probe("a")];
        let two = vec![probe("a"), probe("b")];
        let three = vec![probe("c")];
        s.store(&mut kv, &one).unwrap();
        s.store(&mut kv, &two).unwrap();
        kv.tear = 2;
        assert!(s.store(&mut kv, &three).is_err());
        assert_eq!(Store::load(&mut kv).0, two, "after one torn write");
        assert!(s.store(&mut kv, &three).is_err());
        assert_eq!(Store::load(&mut kv).0, two, "after two torn writes");
        s.store(&mut kv, &three).unwrap();
        assert_eq!(Store::load(&mut kv).0, three);
    }

    #[test]
    fn a_torn_write_leaves_the_other_slot() {
        let mut kv = MemKv::default();
        let mut s = Store::default();
        let one = vec![probe("a")];
        let two = vec![probe("a"), probe("b")];
        s.store(&mut kv, &one).unwrap();
        s.store(&mut kv, &two).unwrap();
        assert_eq!(Store::load(&mut kv).0, two);
        // The newest write tears: the one before it loads.
        let key = KEYS[(s.generation % 2) as usize];
        let b = kv.map[key].clone();
        kv.map.insert(key.into(), b[..b.len() - 3].to_vec());
        assert_eq!(Store::load(&mut kv).0, one);
        // A store going on from the loaded generation never overwrites the
        // newest whole slot first.
        let (_, mut s2) = Store::load(&mut kv);
        s2.store(&mut kv, &[]).unwrap();
        assert_eq!(Store::load(&mut kv).0, Vec::<Probe>::new());
    }
}

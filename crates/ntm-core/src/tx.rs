//! Item transactions (docs/fundacao.md, "Transações").
//!
//! Every move of items between a machine and the player is a transaction
//! whose **truth lives in the world's own scoreboard** (objective
//! `ntm_tx`, holder `#t<salt><id>`), which vanilla saves together with
//! the player's inventory:
//!
//! - **Act lines** move the items in ONE command that records its own
//!   result (`execute store result … run clear`, `execute store success …
//!   run give`), guarded by the score so they run at most once: sending them
//!   again, late, cut between ticks or after a restart is harmless.
//! - **Query lines** only read the score and write a token at an answer
//!   cell the server reserved (bottom of the world, under the player), each
//!   `setblock` conditioned on the cell still holding what the server saw
//!   there. Queries can be repeated freely.
//!
//! The server keeps the transaction in the world's save (the same commit
//! as the machine it belongs to), publishes its lines only after that save
//! succeeded, re-sends act + query until an answer comes, applies the
//! answer, saves, and only then resets the score.

use crate::{Pos, token};
use serde::{Deserialize, Serialize};

pub const OBJECTIVE: &str = "ntm_tx";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    /// One `item` from the main hand into the machine at the tx's place;
    /// `tagged`: read the stamp-wear bucket too.
    Take { item: String, tagged: bool },
    /// Items to the player.
    Give { item: String, count: u32 },
    /// Items summoned at the tx's place (a broken machine's contents).
    Drop { item: String, count: u32 },
}

/// A cell the server borrowed for answers: put back when its token is read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub dim: String,
    pub pos: Pos,
    pub orig: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    pub id: u64,
    pub dim: String,
    pub pos: Pos,
    pub kind: Kind,
    /// The score holder, fixed when the transaction is made: a later change
    /// of the naming never makes an old one look forgotten.
    #[serde(default)]
    pub name: String,
    /// The machine as it was right before the action that made this
    /// transaction: what it goes back to if the world forgets it (a crash),
    /// undoing whatever came of it too (smelted, pressed, taken out).
    #[serde(default)]
    pub before: Option<Box<crate::machine::Machine>>,
    /// Answer cells written by queries (the newest last), for the answer and
    /// to put back.
    #[serde(default)]
    pub cells: Vec<Cell>,
    /// Runtime: the tick its lines last left (never across a restart).
    #[serde(skip)]
    pub sent_at: Option<u64>,
    /// Runtime: publications so far (the wait between them doubles).
    #[serde(skip)]
    pub sends: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Done; for a tagged take, the wear bucket read.
    Ok(u8),
    Failed,
}

impl Tx {
    /// The score holder: `name`, or for a new one `#t<salt><id in hex>`
    /// (the salt is always 6 characters).
    pub fn holder(&self, salt: &str) -> String {
        if self.name.is_empty() {
            format!("#t{salt}{:x}", self.id)
        } else {
            self.name.clone()
        }
    }

    /// The stamp-wear bucket's holder: the main one's with `#u` for `#t`, a
    /// prefix no main holder has, so no bucket is ever another
    /// transaction's score (`#t…1b` would be transaction `0x1b`'s).
    pub fn bucket_holder(&self, salt: &str) -> String {
        let h = self.holder(salt);
        match h.strip_prefix("#t") {
            Some(rest) => format!("#u{rest}"),
            None => format!("#u{}", h.trim_start_matches('#')),
        }
    }
    fn tagged(&self) -> bool {
        matches!(self.kind, Kind::Take { tagged: true, .. })
    }
    /// Token levels of an untagged answer.
    fn ok_level(&self) -> u8 {
        1 + (self.id % 4) as u8
    }
    fn fail_level(&self) -> u8 {
        5 + (self.id % 4) as u8
    }

    /// The answer a token level at one of this tx's cells means.
    pub fn read(&self, level: u8) -> Option<Outcome> {
        if self.tagged() {
            return match level {
                1..=7 => Some(Outcome::Ok(level - 1)),
                8 => Some(Outcome::Failed),
                _ => None,
            };
        }
        if level == self.ok_level() {
            Some(Outcome::Ok(0))
        } else if level == self.fail_level() {
            Some(Outcome::Failed)
        } else {
            None
        }
    }

    /// The lines that move the items, at most once (any order of delivery,
    /// any number of repeats).
    pub fn act_lines(&self, salt: &str) -> Vec<String> {
        let h = self.holder(salt);
        let o = OBJECTIVE;
        // Every value this mod stores is -1 or more: "unset" in few chars
        // (a quiet line is cut at 256).
        let unset = format!("execute unless score {h} {o} matches -9..");
        let mut out = vec![format!("scoreboard objectives add {o} dummy")];
        match &self.kind {
            Kind::Take { item, tagged: false } => {
                out.push(format!("{unset} run scoreboard players set {h} {o} 0"));
                out.push(format!(
                    "execute if score {h} {o} matches 0 if items entity @s weapon.mainhand {item} \
                     run execute store result score {h} {o} run clear @s {item} 1"
                ));
                out.push(format!(
                    "execute if score {h} {o} matches 0 run scoreboard players set {h} {o} -1"
                ));
            }
            Kind::Take { item, tagged: true } => {
                let hb = self.bucket_holder(salt);
                out.push(format!("{unset} run scoreboard players set {h} {o} 0"));
                let pred = |b: u8| match b {
                    0 => format!("{item}[!minecraft:custom_data]"),
                    b => format!(
                        "{item}[minecraft:custom_data~{{{}:{b}}}]",
                        crate::press::WEAR_KEY
                    ),
                };
                for b in 0..=crate::press::WEAR_BUCKETS {
                    out.push(format!(
                        "execute if score {h} {o} matches 0 if items entity @s weapon.mainhand {} \
                         run scoreboard players set {hb} {o} {b}",
                        pred(b)
                    ));
                }
                // The bucket the hand showed: a stamp of that wear, taken from
                // wherever it is now (the hand may have changed between ticks).
                for b in 0..=crate::press::WEAR_BUCKETS {
                    out.push(format!(
                        "execute if score {h} {o} matches 0 if score {hb} {o} matches {b} \
                         run execute store result score {h} {o} run clear @s {} 1",
                        pred(b)
                    ));
                }
                out.push(format!(
                    "execute if score {h} {o} matches 0 run scoreboard players set {h} {o} -1"
                ));
            }
            Kind::Give { item, count } => out.push(format!(
                "{unset} run execute store success score {h} {o} run give @s {item} {count}"
            )),
            Kind::Drop { item, count } => {
                let [x, y, z] = self.pos;
                out.push(format!(
                    "execute in {} unless score {h} {o} matches -9.. run execute store success \
                     score {h} {o} run summon item {x}.5 {y}.5 {z}.5 {}",
                    self.dim,
                    item_nbt(item, *count)
                ));
            }
        }
        out
    }

    /// Whether every act line fits a quiet line (a drop far out with a long
    /// item may not: it goes to the player as a give instead).
    pub fn fits(&self, salt: &str) -> bool {
        self.act_lines(salt).iter().all(|l| l.len() <= token::MAX_LINE)
    }

    /// The lines that answer at `cell`: they only read.
    pub fn query_lines(&self, salt: &str, cell: &Cell) -> Vec<String> {
        let h = self.holder(salt);
        let o = OBJECTIVE;
        let [x, y, z] = cell.pos;
        let put = |level: u8| {
            format!(
                "if block {x} {y} {z} {} run setblock {x} {y} {z} {}",
                cell.orig,
                token::token_state(level)
            )
        };
        let at = format!("execute in {} if score {h} {o}", cell.dim);
        match &self.kind {
            Kind::Take { tagged: true, .. } => {
                let hb = self.bucket_holder(salt);
                let mut out: Vec<String> = (0..=crate::press::WEAR_BUCKETS)
                    .map(|b| {
                        format!("{at} matches 1 if score {hb} {o} matches {b} {}", put(b + 1))
                    })
                    .collect();
                out.push(format!("{at} matches ..-1 {}", put(8)));
                out
            }
            Kind::Take { .. } => vec![
                format!("{at} matches 1.. {}", put(self.ok_level())),
                format!("{at} matches ..-1 {}", put(self.fail_level())),
            ],
            Kind::Give { .. } | Kind::Drop { .. } => vec![
                format!("{at} matches 1.. {}", put(self.ok_level())),
                format!("{at} matches 0 {}", put(self.fail_level())),
            ],
        }
    }

    /// The score's clean-up, once the outcome is saved.
    pub fn reset_lines(&self, salt: &str) -> Vec<String> {
        let h = self.holder(salt);
        let mut out = vec![format!("scoreboard players reset {h} {OBJECTIVE}")];
        if self.tagged() {
            out.push(format!(
                "scoreboard players reset {} {OBJECTIVE}",
                self.bucket_holder(salt)
            ));
        }
        out
    }
}

/// A transaction that went through, kept for a while: if the world comes
/// back without its score (a crash rolled the save back past it), the
/// mod's side of it is undone, so machine and inventory agree again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub tx: Tx,
    /// The world's tick count (`WorldState::ticks`) when it went through.
    pub at: u64,
}

/// What a verification read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remembered {
    Yes,
    No,
}

impl Applied {
    pub const YES: u8 = 1;
    pub const NO: u8 = 2;

    /// Whether the world still has the transaction's score.
    pub fn verify_lines(&self, salt: &str, cell: &Cell) -> Vec<String> {
        let h = self.tx.holder(salt);
        let o = OBJECTIVE;
        let [x, y, z] = cell.pos;
        let put = |level: u8| {
            format!(
                "if block {x} {y} {z} {} run setblock {x} {y} {z} {}",
                cell.orig,
                token::token_state(level)
            )
        };
        vec![
            format!("execute in {} if score {h} {o} matches ..2147483647 {}", cell.dim, put(Self::YES)),
            format!("execute in {} unless score {h} {o} matches ..2147483647 {}", cell.dim, put(Self::NO)),
        ]
    }

    pub fn read(level: u8) -> Option<Remembered> {
        match level {
            Self::YES => Some(Remembered::Yes),
            Self::NO => Some(Remembered::No),
            _ => None,
        }
    }
}

/// `{Item:{id:"…",count:N[,components:{…}]}}` for `summon item`, from an
/// item as `give` writes it (`id` or `id[ns:component=value]`).
pub fn item_nbt(item: &str, count: u32) -> String {
    let (id, components) = match item.split_once('[') {
        Some((id, rest)) => match rest.trim_end_matches(']').split_once('=') {
            Some((k, v)) => (id, format!(",components:{{\"{k}\":{v}}}")),
            None => (id, String::new()),
        },
        None => (item, String::new()),
    };
    format!("{{Item:{{id:\"{id}\",count:{count}{components}}}}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(kind: Kind) -> Tx {
        Tx {
            id: 6,
            dim: "minecraft:overworld".into(),
            pos: [1, 64, 2],
            kind,
            name: String::new(),
            before: None,
            cells: Vec::new(),
            sent_at: None,
            sends: 0,
        }
    }

    #[test]
    fn lines_record_their_own_result_and_answers_are_conditional() {
        let t = tx(Kind::Take {
            item: "minecraft:coal".into(),
            tagged: false,
        });
        let act = t.act_lines("ab");
        assert_eq!(
            act[2],
            "execute if score #tab6 ntm_tx matches 0 if items entity @s weapon.mainhand minecraft:coal \
             run execute store result score #tab6 ntm_tx run clear @s minecraft:coal 1"
        );
        let cell = Cell {
            dim: "minecraft:overworld".into(),
            pos: [0, -64, 0],
            orig: "minecraft:bedrock".into(),
        };
        let q = t.query_lines("ab", &cell);
        assert_eq!(
            q[0],
            "execute in minecraft:overworld if score #tab6 ntm_tx matches 1.. if block 0 -64 0 minecraft:bedrock \
             run setblock 0 -64 0 ntm:logic_token[level=3]"
        );
        assert_eq!(t.read(3), Some(Outcome::Ok(0)));
        assert_eq!(t.read(7), Some(Outcome::Failed));
        assert_eq!(t.read(1), None);
        let g = tx(Kind::Give {
            item: "minecraft:iron_ingot".into(),
            count: 64,
        });
        assert!(g.act_lines("ab")[1].ends_with("run execute store success score #tab6 ntm_tx run give @s minecraft:iron_ingot 64"));
        let d = tx(Kind::Drop {
            item: "a:b[minecraft:custom_data={w:3}]".into(),
            count: 1,
        });
        assert!(d.act_lines("ab")[1].starts_with("execute in minecraft:overworld unless score #tab6 ntm_tx matches -9.."));
        assert!(d.act_lines("ab")[1].ends_with(
            "summon item 1.5 64.5 2.5 {Item:{id:\"a:b\",count:1,components:{\"minecraft:custom_data\":{w:3}}}}"
        ));
        let s = tx(Kind::Take {
            item: "ntm:stamp_iron_plate".into(),
            tagged: true,
        });
        assert_eq!(s.act_lines("ab").len(), 1 + 1 + 7 + 7 + 1);
        assert!(s.act_lines("ab")[2].contains("#uab6 ntm_tx"), "{}", s.act_lines("ab")[2]);
        assert_eq!(s.read(3), Some(Outcome::Ok(2)));
        assert_eq!(s.reset_lines("ab").len(), 2);
    }

    /// No holder of any transaction is any other's, main or bucket, over
    /// every id up to 0x3000 and a few salts (the review's `#t…1b` case:
    /// transaction 1's bucket against transaction 0x1b).
    #[test]
    fn holders_never_collide() {
        let mut seen = std::collections::HashSet::new();
        for salt in ["000000", "d01aab", "ffffff"] {
            for id in 0..0x3000u64 {
                let t = Tx { id, ..tx(Kind::Take { item: "a:b".into(), tagged: true }) };
                assert!(seen.insert((salt, t.holder(salt))), "{salt} {id} main");
                assert!(seen.insert((salt, t.bucket_holder(salt))), "{salt} {id} bucket");
            }
        }
        // A stored name keeps its bucket apart too.
        let named = Tx { name: "#td01aab1".into(), ..tx(Kind::Give { item: "a:b".into(), count: 1 }) };
        assert_eq!(named.bucket_holder("x"), "#ud01aab1");
    }

    /// The game cuts a quiet line at 256 characters: the longest lines any
    /// transaction makes fit, with the longest ids, places and states.
    #[test]
    fn every_line_fits_the_quiet_limit() {
        let long_item = format!("ntm:{}", "a".repeat(36));
        let stamp = format!("{long_item}[minecraft:custom_data={{ntm_wear:6}}]");
        let kinds = [
            Kind::Take { item: long_item.clone(), tagged: false },
            Kind::Take { item: long_item.clone(), tagged: true },
            Kind::Give { item: stamp.clone(), count: 6400 },
            Kind::Drop { item: stamp, count: 64 },
        ];
        let cell = Cell {
            dim: "minecraft:the_nether".into(),
            pos: [-29_999_999, -64, -29_999_999],
            // The longest state a cell may hold (`answer_cell`: 40 at most).
            orig: format!("minecraft:{}", "d".repeat(30)),
        };
        for kind in kinds {
            let drop = matches!(kind, Kind::Drop { .. });
            let t = Tx {
                id: u64::from(u32::MAX),
                dim: "minecraft:the_nether".into(),
                pos: [-29_999_999, 319, -29_999_999],
                kind,
                name: String::new(),
                before: None,
                cells: vec![],
                sent_at: None,
                sends: 0,
            };
            if drop {
                // Far out with a tagged long item: too long, a give instead.
                assert!(!t.fits("ffffff"));
                // A plain item far out fits; a worn stamp goes as a give
                // (the server's `new_tx` turns it).
                let plain = Tx {
                    kind: Kind::Drop {
                        item: "ntm:machine_electric_furnace_off".into(),
                        count: 64,
                    },
                    ..t.clone()
                };
                assert!(plain.fits("ffffff"), "{:?}", plain.act_lines("ffffff"));
                continue;
            }
            let a = Applied { tx: t.clone(), at: 0 };
            let lines = t
                .act_lines("ffffff")
                .into_iter()
                .chain(t.query_lines("ffffff", &cell))
                .chain(t.reset_lines("ffffff"))
                .chain(a.verify_lines("ffffff", &cell));
            for l in lines {
                assert!(l.len() <= token::MAX_LINE, "{} chars: {l}", l.len());
            }
        }
    }
}

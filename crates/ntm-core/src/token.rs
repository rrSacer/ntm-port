//! "The server asks, the client runs vanilla commands, the world answers"
//! (docs/fundacao.md, "Comandos vanilla com confirmação").
//!
//! The server mod can't run commands nor read their results; the client mod
//! can run them (`command.run-quiet`) but gets no result either. So each
//! request carries conditional `setblock`s of the token block
//! (`ntm:logic_token[level=N]`) at positions the server chose, and the
//! server reads those blocks back with `world.get-block`: a probe. When
//! every cell shows a token the server puts back what was there and acts
//! on the levels it read. Keeping track of probes (persisted, late answers,
//! one per cell) is [`crate::probes`]; this module builds the lines.

use crate::{Pos, blockstate, registry::TOKEN};
use serde::{Deserialize, Serialize};

/// What a cell held before the probe, put back when it ends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Restore {
    /// This exact state (a world-id cell).
    State(String),
    /// The machine at this position, as it looks then.
    Machine,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub pos: Pos,
    pub restore: Restore,
    /// The token level read, once a token showed.
    pub seen: Option<u8>,
}

/// The level a token state carries, if `state` is a token.
pub fn token_level(state: &str) -> Option<u8> {
    if blockstate::id_of(state) != TOKEN {
        return None;
    }
    let s = blockstate::BlockState::parse(state)?;
    s.get("level").and_then(|l| l.parse().ok())
}

pub fn token_state(level: u8) -> String {
    format!("{TOKEN}[level={level}]")
}

/// The (ok, fail) level pairs of one-cell probes: two probes waiting on
/// one cell (a late one and a new one) never share a pair
/// ([`crate::probes`] picks a free one).
pub const PAIRS: [(u8, u8); 4] = [(1, 5), (2, 6), (3, 7), (4, 8)];

/// A level by probe id, for probes that sign their own completion cell
/// (radiation's last cell): not a yes/no pair.
pub fn ok_level(id: u32) -> u8 {
    1 + (id % 4) as u8
}

fn xyz(p: Pos) -> String {
    format!("{} {} {}", p[0], p[1], p[2])
}

/// `execute in <dim> run <line>`: the client runs a line where its player
/// is, so every line names the dimension its positions are in.
pub fn in_dim(dim: &str, lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|l| format!("execute in {dim} run {l}"))
        .collect()
}

/// The longest quiet line the game keeps (`horizon_protocol::quiet`): the
/// rest is cut off, so no line this mod sends is longer.
pub const MAX_LINE: usize = 256;

/// Whether `s` is a plain `namespace:path` id, safe inside a command line
/// (and short enough for one).
pub fn valid_id(s: &str) -> bool {
    if s.len() > 40 {
        return false;
    }
    let Some((ns, path)) = s.split_once(':') else {
        return false;
    };
    !ns.is_empty()
        && !path.is_empty()
        && ns.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
        && path
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
}

/// An item as the probes write it: a valid id, optionally with the one
/// component form this mod gives (`[minecraft:custom_data={key:N}]`).
pub fn valid_item(s: &str) -> bool {
    match s.split_once('[') {
        None => valid_id(s),
        Some((id, rest)) => {
            valid_id(id)
                && rest.strip_prefix("minecraft:custom_data={").is_some_and(|r| {
                    r.strip_suffix("}]").is_some_and(|kv| {
                        kv.split_once(':').is_some_and(|(k, v)| {
                            !k.is_empty()
                                && k.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                                && v.parse::<u32>().is_ok()
                        })
                    })
                })
        }
    }
}

/// Take one `item` from the main hand, answered at `pos` (the machine):
/// level `ok` when the hand held it and one was cleared, `fail` otherwise.
pub fn take_held_lines(pos: Pos, item: &str, (ok, fail): (u8, u8)) -> Vec<String> {
    let (at, ok, fail) = (xyz(pos), token_state(ok), token_state(fail));
    vec![
        format!("execute if items entity @s weapon.mainhand {item} run setblock {at} {ok}"),
        format!("execute if block {at} {ok} run clear @s {item} 1"),
        format!("execute unless block {at} {ok} run setblock {at} {fail}"),
    ]
}

/// The level of a tagged take that failed (levels 1..=7 are buckets 0..=6).
pub const TAGGED_FAIL: u8 = 8;

/// Take one `item` from the main hand and read its tag: level `1 + b` when
/// its `minecraft:custom_data` holds `key: b` (b in 1..=buckets), 1 when
/// it holds none of them, [`TAGGED_FAIL`] when the hand doesn't hold the
/// item. The `clear` takes an item of the bucket read, so a stack of other
/// wear elsewhere in the inventory stays.
pub fn take_tagged_lines(pos: Pos, item: &str, key: &str, buckets: u8) -> Vec<String> {
    let at = xyz(pos);
    let buckets = buckets.min(TAGGED_FAIL - 2);
    let tagged = |b: u8| format!("{item}[minecraft:custom_data~{{{key}:{b}}}]");
    // Bucket 0 is an item with no custom data at all: one with data of
    // another kind fails, rather than being taken as fresh and not cleared.
    let mut out = vec![format!(
        "execute if items entity @s weapon.mainhand {item}[!minecraft:custom_data] run setblock {at} {}",
        token_state(1)
    )];
    for b in 1..=buckets {
        out.push(format!(
            "execute if items entity @s weapon.mainhand {} run setblock {at} {}",
            tagged(b),
            token_state(b + 1)
        ));
    }
    for b in 1..=buckets {
        out.push(format!(
            "execute if block {at} {} run clear @s {} 1",
            token_state(b + 1),
            tagged(b)
        ));
    }
    out.push(format!(
        "execute if block {at} {} run clear @s {item}[!minecraft:custom_data] 1",
        token_state(1)
    ));
    out.push(format!(
        "execute unless block {at} {TOKEN} run setblock {at} {}",
        token_state(TAGGED_FAIL)
    ));
    out
}

/// Items for the player (a full inventory drops them), then the token
/// `ack` at `pos`: the server knows the `give` ran.
pub fn give_lines(pos: Pos, item: &str, count: u32, ack: u8) -> Vec<String> {
    let mut lines = Vec::new();
    let mut left = count;
    while left > 0 {
        let n = left.min(64);
        lines.push(format!("give @s {item} {n}"));
        left -= n;
    }
    lines.push(format!("setblock {} {}", xyz(pos), token_state(ack)));
    lines
}

/// Items dropped at a position (a broken machine's contents). An item may
/// carry one component, `id[ns:component=value]`, as `give` writes it.
/// The token `ack` at `pos` says the lines ran.
pub fn drop_lines(pos: Pos, items: &[(String, u32)], ack: u8) -> Vec<String> {
    let at = format!("{}.5 {}.5 {}.5", pos[0], pos[1], pos[2]);
    let mut lines: Vec<String> = items
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(item, n)| {
            let (id, components) = match item.split_once('[') {
                Some((id, rest)) => {
                    let inner = rest.trim_end_matches(']');
                    match inner.split_once('=') {
                        Some((k, v)) => (id, format!(",components:{{\"{k}\":{v}}}")),
                        None => (id, String::new()),
                    }
                }
                None => (item.as_str(), String::new()),
            };
            format!("summon item {at} {{Item:{{id:\"{id}\",count:{n}{components}}}}}")
        })
        .collect();
    lines.push(format!("setblock {} {}", xyz(pos), token_state(ack)));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levels_and_lines() {
        assert_eq!(token_level("ntm:logic_token[level=3]"), Some(3));
        assert_eq!(token_level("ntm:teste[level=3]"), None);
        let l = take_held_lines([1, 2, 3], "minecraft:coal", PAIRS[2]);
        assert_eq!(
            l[0],
            "execute if items entity @s weapon.mainhand minecraft:coal run setblock 1 2 3 ntm:logic_token[level=3]"
        );
        assert_eq!(give_lines([0, 0, 0], "a:b", 130, 1).len(), 4);
        assert!(valid_item("ntm:stamp_iron_plate[minecraft:custom_data={ntm_wear:3}]"));
        assert!(!valid_item("x:stone run say hi"));
        assert!(!valid_item("ntm:a[minecraft:custom_data={k:1} ] x"));
        assert_eq!(in_dim("minecraft:the_nether", vec!["setblock 0 0 0 a:b".into()])[0],
            "execute in minecraft:the_nether run setblock 0 0 0 a:b");
        assert_eq!(
            drop_lines([0, 64, 0], &[("a:b".into(), 2)], 1)[0],
            "summon item 0.5 64.5 0.5 {Item:{id:\"a:b\",count:2}}"
        );
        assert_eq!(
            drop_lines([0, 64, 0], &[("a:b[minecraft:custom_data={w:3}]".into(), 1)], 1)[0],
            "summon item 0.5 64.5 0.5 {Item:{id:\"a:b\",count:1,components:{\"minecraft:custom_data\":{w:3}}}}"
        );
        let t = take_tagged_lines([1, 2, 3], "a:s", "w", 6);
        assert_eq!(t.len(), 1 + 6 + 6 + 2);
        assert_eq!(
            t[2],
            "execute if items entity @s weapon.mainhand a:s[minecraft:custom_data~{w:2}] run setblock 1 2 3 ntm:logic_token[level=3]"
        );
        assert_eq!(t.last().unwrap(), "execute unless block 1 2 3 ntm:logic_token run setblock 1 2 3 ntm:logic_token[level=8]");
    }
}

//! World identity (docs/fundacao.md, "Identidade do mundo").
//!
//! Capability `storage` is one folder per installation, shared by every
//! world, so the save of each world needs a key that belongs to that world.
//! Nothing the server mod reads names the world; the world's own command
//! storage (`data/ntmlogic/command_storage.dat`, inside the save) can hold
//! one, and the client mod's vanilla commands can read and write it. So:
//!
//! 1. The server proposes a fresh id (`DIGITS` octal digits from entropy).
//! 2. The client runs `execute unless data storage ntmlogic:world v run data
//!    merge storage ntmlogic:world {v:1b,d0:..b,...}`: a world that has an id
//!    keeps it, a new one takes the proposal.
//! 3. Then, per digit `i` and value `v`, `execute if data storage
//!    ntmlogic:world {d<i>:<v>b} run setblock <cell i> ntm:logic_token[level=v+1]`:
//!    the id appears as token levels in a column at the bottom of the world
//!    under the player (always loaded, never seen).
//! 4. The server reads the column, puts back what was there, and has the id.
//!
//! The id travels with the save (copying a world copies it), survives a lost
//! `storage` folder, and needs no input from the player. It needs cheats
//! (the client's commands run with the player's rights), as everything of
//! the client channel does.

use crate::Pos;

/// Octal digits of an id: 30 bits.
pub const DIGITS: usize = 10;
pub const STORAGE: &str = "ntmlogic:world";

pub type Wid = String;

/// Fresh digits from 64 bits of entropy.
pub fn fresh(entropy: u64) -> [u8; DIGITS] {
    let mut d = [0u8; DIGITS];
    for (i, slot) in d.iter_mut().enumerate() {
        *slot = ((entropy >> (3 * i)) & 7) as u8;
    }
    d
}

pub fn to_wid(d: &[u8; DIGITS]) -> Wid {
    d.iter().map(|v| char::from(b'0' + v)).collect()
}

/// The lowest y of a dimension (the column's bottom).
pub fn min_y(dim: &str) -> i32 {
    match dim {
        "minecraft:overworld" => -64,
        _ => 0,
    }
}

/// The cells of the column at `x z` in `dim`.
pub fn column(dim: &str, x: i32, z: i32) -> Vec<Pos> {
    let y0 = min_y(dim);
    (0..DIGITS as i32).map(|i| [x, y0 + i, z]).collect()
}

/// Blocks a probe may replace for a tick and put back exactly: no block
/// entity, nothing that reacts to a neighbour changing.
pub fn safe_to_borrow(state: &str) -> bool {
    let id = crate::blockstate::id_of(state);
    let path = id.strip_prefix("minecraft:").unwrap_or("-");
    matches!(
        path,
        "bedrock"
            | "stone"
            | "deepslate"
            | "cobbled_deepslate"
            | "tuff"
            | "granite"
            | "diorite"
            | "andesite"
            | "calcite"
            | "dirt"
            | "gravel"
            | "netherrack"
            | "basalt"
            | "smooth_basalt"
            | "blackstone"
            | "end_stone"
            | "obsidian"
            | "air"
            | "cave_air"
            | "void_air"
            | "lava"
            | "water"
            | "magma_block"
            | "soul_sand"
            | "soul_soil"
    ) || (path.ends_with("_ore") && !path.contains("redstone"))
}

/// The commands of steps 2 and 3 for `cells` (one per digit, with the
/// state the server saw there: a cell is only written while it still holds
/// it, so a block placed meanwhile is never replaced).
pub fn lines(proposal: &[u8; DIGITS], cells: &[(Pos, String)]) -> Vec<String> {
    let merge: Vec<String> = proposal
        .iter()
        .enumerate()
        .map(|(i, v)| format!("d{i}:{v}b"))
        .collect();
    let mut out = vec![format!(
        "execute unless data storage {STORAGE} v run data merge storage {STORAGE} {{v:1b,{}}}",
        merge.join(",")
    )];
    for (i, (p, orig)) in cells.iter().enumerate() {
        let [x, y, z] = *p;
        for v in 0..8u8 {
            out.push(format!(
                "execute if data storage {STORAGE} {{d{i}:{v}b}} if block {x} {y} {z} {orig} \
                 run setblock {x} {y} {z} {}",
                crate::token::token_state(v + 1)
            ));
        }
    }
    out
}

/// The id the column's levels spell (`level - 1` per digit), if whole.
pub fn decode(levels: &[Option<u8>]) -> Option<Wid> {
    if levels.len() != DIGITS {
        return None;
    }
    let mut d = [0u8; DIGITS];
    for (slot, l) in d.iter_mut().zip(levels) {
        let l = (*l)?;
        if !(1..=8).contains(&l) {
            return None;
        }
        *slot = l - 1;
    }
    Some(to_wid(&d))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spell_and_read_back() {
        let d = fresh(0o1234567012);
        assert_eq!(to_wid(&d), "2107654321");
        let levels: Vec<Option<u8>> = d.iter().map(|v| Some(v + 1)).collect();
        assert_eq!(decode(&levels).as_deref(), Some("2107654321"));
        assert_eq!(decode(&levels[..9]), None);
        let cells = column("minecraft:overworld", 5, 7);
        assert_eq!(cells[0], [5, -64, 7]);
        let cells: Vec<(Pos, String)> =
            cells.into_iter().map(|p| (p, "minecraft:bedrock".into())).collect();
        let l = lines(&d, &cells);
        assert_eq!(l.len(), 1 + 80);
        assert!(l[0].starts_with("execute unless data storage ntmlogic:world v run data merge"));
        assert_eq!(
            l[1],
            "execute if data storage ntmlogic:world {d0:0b} if block 5 -64 7 minecraft:bedrock run setblock 5 -64 7 ntm:logic_token[level=1]"
        );
        // Far out, the longest state a cell may hold: every line fits a
        // quiet line once `execute in` is added.
        let far: Vec<(Pos, String)> = column("minecraft:the_nether", -29_999_999, -29_999_999)
            .into_iter()
            .map(|p| (p, "minecraft:deepslate_diamond_ore".to_string()))
            .collect();
        for l in crate::token::in_dim("minecraft:the_nether", lines(&d, &far)) {
            assert!(l.len() <= crate::token::MAX_LINE, "{} chars: {l}", l.len());
        }
        assert!(safe_to_borrow("minecraft:deepslate_diamond_ore"));
        assert!(!safe_to_borrow("minecraft:chest[facing=north]"));
    }
}

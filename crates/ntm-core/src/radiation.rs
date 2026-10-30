//! M1 inventory radiation. Values are NTM gameplay RAD, not physical dosimetry.
//! Sampling and platform integration live in `server/radiation_server.rs`.
use crate::{Pos, token};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::OnceLock};

pub const SAMPLE_TICKS: u64 = 20;
pub const GRACE_TICKS: u64 = 200;
/// Changed player dose is saved at most every five seconds, or at a sickness threshold.
pub const DOSE_SAVE_TICKS: u64 = 100;
pub const DIGITS: usize = 8;
pub const CELLS: usize = DIGITS + 1;
pub const COMMANDS_PER_TICK: usize = 16;
pub const OBJECTIVE: &str = "ntm_rad_m1";
/// The largest rate the wire carries (eight base-8 digits); more reads as it.
pub const MAX_RATE: u32 = (1 << (3 * DIGITS)) - 1;
/// The partial sum is clamped to `MAX_RATE + 1` every this many bits, so the
/// game's 32-bit scores never wrap: between clamps it grows at most 2^4 times
/// plus 15 item counts, below 2^31 for any inventory under 2^26 items.
const CLAMP_EVERY: u32 = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dose {
    pub millirad: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    pub dose_millirad: u64,
    pub rate_millirad_s: u32,
    /// An empty status means a confirmed inventory sample.
    pub status: String,
}
impl Reading {
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "NTM Geiger | {:.3} RAD/s",
                self.rate_millirad_s as f64 / 1000.0
            ),
            format!("Dose: {:.2} RAD", self.dose_millirad as f64 / 1000.0),
        ];
        if !self.status.is_empty() {
            lines.push(self.status.clone());
        }
        lines
    }
    pub fn color(&self) -> u32 {
        match self.dose_millirad {
            0..200_000 => 0x88EE88,
            200_000..400_000 => 0xFFFF55,
            400_000..600_000 => 0xFFAA00,
            _ => 0xFF5555,
        }
    }
}

#[derive(Default, Debug)]
pub struct Runtime {
    pub next_dose_save: u64,
    pub dose_dirty: bool,
    pub next_sample: u64,
    pub commands: VecDeque<String>,
    pub joined: Option<(String, u64)>,
    /// How samples count. Back to [`Mode::Tags`] at every world entry (the
    /// runtime is reset there).
    pub mode: Mode,
    /// Tag samples in a row whose tags were unknown to the game.
    pub tag_misses: u32,
    /// In [`Mode::Ids`]: when one tag sample is tried again.
    pub retry_tags_at: u64,
    /// The way each sample in flight counts, by probe id.
    pub kinds: Vec<(u32, Mode)>,
    /// The tick the last confirmed sample's dose was added: the next one
    /// stands for the ticks since (a sample's round trip makes the cycle
    /// longer than its period, more so on a loaded PC). `None` after a
    /// sample without a rate, the grace or a world entry: no stale rate.
    pub last_dose: Option<u64>,
}

/// The most ticks one confirmed sample stands for (at least two of its
/// mode's periods).
pub const MAX_SAMPLE_GAP: u64 = 200;

/// The ticks of exposure a confirmed sample of `kind` read at `now` stands for.
pub fn exposure_ticks(kind: Mode, last_dose: Option<u64>, now: u64) -> u64 {
    let cap = MAX_SAMPLE_GAP.max(2 * kind.period());
    last_dose.map_or(kind.period(), |t| now.saturating_sub(t)).min(cap)
}

/// How a sample counts the inventory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// By the content mod's tags, one group per bit of the rates: ~1/s.
    #[default]
    Tags,
    /// By item ids, for a game that doesn't know the mod's tags in
    /// commands (Horizon without `horizon-patches/mod-item-tags.diff`).
    Ids,
}

impl Mode {
    /// Ticks between samples, and so the exposure one confirmed sample stands for.
    pub fn period(self) -> u64 {
        match self {
            Mode::Tags => SAMPLE_TICKS,
            Mode::Ids => ids_period(),
        }
    }
}

/// Ticks between id samples: their lines at 16 a tick, and a little more
/// (it grows with the catalog: ~3.5 s for 220 radioactive items).
pub fn ids_period() -> u64 {
    static P: OnceLock<u64> = OnceLock::new();
    *P.get_or_init(|| {
        let cells: [(Pos, String); CELLS] = std::array::from_fn(|_| ([0, 0, 0], String::new()));
        ids_sample_lines(0, "", &cells).len().div_ceil(COMMANDS_PER_TICK) as u64 + IDS_SLACK_TICKS
    })
}

/// Tag samples in a row the game reports unknown tags for before samples
/// switch to ids.
pub const TAG_MISSES: u32 = 3;
/// Ticks an id sample's period has beyond the ticks its lines take.
pub const IDS_SLACK_TICKS: u64 = 6;
/// Ticks in id mode before one tag sample is tried again.
pub const TAG_RETRY_TICKS: u64 = 6000;
/// Where id samples keep each item's weighted count.
pub const IDS_STORAGE: &str = "ntmlogic:rad";

/// What a sample's column says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    Rate(u32),
    /// Every line ran, but the game didn't know a tag the sample named.
    TagsUnknown,
    /// Not (yet) complete: lines not run, no cheats, cells taken.
    Incomplete,
}

#[derive(Debug, Deserialize)]
pub struct Hazard {
    pub id: String,
    pub radiation_millirad_s: u32,
}
#[derive(Deserialize)]
struct Table {
    entries: Vec<Hazard>,
}
/// Unlike identity negotiation, repeated sampling never borrows fluids, air,
/// falling blocks or block entities: restoring a state cannot undo fluid ticks.
pub fn safe_cell(state: &str) -> bool {
    let id = crate::blockstate::id_of(state);
    matches!(
        id,
        "minecraft:bedrock"
            | "minecraft:stone"
            | "minecraft:deepslate"
            | "minecraft:tuff"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:andesite"
            | "minecraft:netherrack"
            | "minecraft:basalt"
            | "minecraft:blackstone"
            | "minecraft:end_stone"
            | "minecraft:obsidian"
    )
}

/// Builds the tables now: parsing the 600 KB hazard table takes ~16M of the
/// 50M fuel a tick may use, so the server mod does it in `init` (2G).
pub fn warm() {
    bits();
    ids_period();
}

/// The radioactive items (rate above zero) of the checked-in table.
pub fn hazards() -> &'static [Hazard] {
    static H: OnceLock<Vec<Hazard>> = OnceLock::new();
    H.get_or_init(|| {
        serde_json::from_str::<Table>(include_str!("../data/hazards.json"))
            .expect("checked-in hazard table")
            .entries
            .into_iter()
            .filter(|h| h.radiation_millirad_s > 0)
            .collect()
    })
}

/// The bits of `radiation_millirad_s` some item has set, lowest first. Bit K
/// has the item tag [`bit_tag`]`(K)`: every item whose rate has bit K set
/// (`content/data/ntm/tags/item/rad/bit_K.json`, emitted from the same table).
pub fn bits() -> &'static [u32] {
    static B: OnceLock<Vec<u32>> = OnceLock::new();
    B.get_or_init(|| {
        let all = hazards().iter().fold(0u32, |a, h| a | h.radiation_millirad_s);
        (0..32).filter(|k| all >> k & 1 == 1).collect()
    })
}

pub fn bit_tag(k: u32) -> String {
    format!("ntm:rad/bit_{k}")
}

/// The exact rate of an inventory, as the sample computes it in the game.
pub fn rate_of<'a>(items: impl IntoIterator<Item = (&'a str, u32)>) -> u64 {
    items
        .into_iter()
        .filter_map(|(id, n)| {
            hazards()
                .iter()
                .find(|h| h.id == id)
                .map(|h| u64::from(n) * u64::from(h.radiation_millirad_s))
        })
        .sum()
}

/// Read-only `clear ... 0` counts all carried items, including offhand.
/// Scores are world-local; every temporary is overwritten on every sample.
///
/// The sum Σ count·rate is exact, with no rate rounded: it is
/// Σ_K 2^K · count(items of bit K), computed by Horner from the highest bit,
/// one doubling per bit and, for each bit some item has, a reset, a `clear`
/// of its tag, a check that it parsed and an add (docs/radiacao.md has the
/// count of lines).
///
/// Eight base-8 digits carry the sum; a ninth, written last, confirms completion.
/// The host sends at most 16 lines per tick, yielding to machines.
/// Each token write requires the exact state borrowed at planning time: a
/// changed cell is left alone, and the incomplete sample cannot apply dose.
pub fn sample_lines(id: u32, dim: &str, cells: &[(Pos, String); CELLS]) -> Vec<String> {
    let o = OBJECTIVE;
    let mut out = header(o);
    out.push(format!("scoreboard players set #two {o} 2"));
    let bits = bits();
    let top = bits.last().copied().unwrap_or(0);
    for k in (0..=top).rev() {
        if k != top {
            out.push(format!("scoreboard players operation #total {o} *= #two {o}"));
        }
        if bits.contains(&k) {
            // A line that doesn't parse (a content mod without the tag)
            // stores nothing: the count stays -1, and the sample is never
            // confirmed, rather than read short.
            out.extend([
                format!("scoreboard players set #count {o} -1"),
                format!(
                    "execute store result score #count {o} run clear @s #{} 0",
                    bit_tag(k)
                ),
                format!("execute if score #count {o} matches ..-1 run scoreboard players set #bad {o} 1"),
                format!("scoreboard players operation #total {o} += #count {o}"),
            ]);
        }
        if k % CLAMP_EVERY == 0 && k != 0 {
            out.push(clamp(o));
        }
    }
    tail(&mut out, id, dim, cells);
    out
}

/// The same sum by item ids, for a game whose commands don't know the
/// mod's tags: ~4 lines an item, one sample every [`ids_period`].
///
/// Each item's count is weighted by its rate in the store itself
/// (`store result storage … int <rate>`, which saturates rather than
/// wraps), read back, kept at most `MAX_RATE + 1` and added. Every item has
/// its own path, all removed first: an id the game doesn't know (an older
/// content mod) leaves its path missing and counts 0, which is right, as
/// no player can carry it; it never reads another item's count.
pub fn ids_sample_lines(id: u32, dim: &str, cells: &[(Pos, String); CELLS]) -> Vec<String> {
    let o = OBJECTIVE;
    let mut out = header(o);
    out.push(format!("data remove storage {IDS_STORAGE} r"));
    for (i, h) in hazards().iter().enumerate() {
        let path = format!("r.i{i}");
        out.extend([
            format!(
                "execute store result storage {IDS_STORAGE} {path} int {} run clear @s {} 0",
                h.radiation_millirad_s, h.id
            ),
            format!("execute store result score #count {o} run data get storage {IDS_STORAGE} {path}"),
            format!(
                "execute if score #count {o} matches {}.. run scoreboard players set #count {o} {}",
                MAX_RATE + 1,
                MAX_RATE + 1
            ),
            format!("scoreboard players operation #total {o} += #count {o}"),
        ]);
        // At most 64 adds of MAX_RATE + 1 between clamps: below 2^31.
        if i % 64 == 63 {
            out.push(clamp(o));
        }
    }
    tail(&mut out, id, dim, cells);
    out
}

fn header(o: &str) -> Vec<String> {
    vec![
        format!("scoreboard objectives add {o} dummy"),
        format!("scoreboard players set #total {o} 0"),
        format!("scoreboard players set #eight {o} 8"),
        format!("scoreboard players set #bad {o} 0"),
    ]
}

/// Creative zero, the ceiling, the eight digits and the completion cell:
/// its level says confirmed (`ok_level`) or unknown tags (`ok_level + 4`).
fn tail(out: &mut Vec<String>, id: u32, dim: &str, cells: &[(Pos, String); CELLS]) {
    let o = OBJECTIVE;
    for mode in ["creative", "spectator"] {
        out.push(format!(
            "execute if entity @s[gamemode={mode}] run scoreboard players set #total {o} 0"
        ));
    }
    // Bound the wire representation even if another mod allows oversized stacks.
    out.push(format!(
        "execute if score #total {o} matches {}.. run scoreboard players set #total {o} {MAX_RATE}",
        MAX_RATE + 1
    ));
    for (p, orig) in &cells[..DIGITS] {
        out.push(format!(
            "scoreboard players operation #digit {o} = #total {o}"
        ));
        out.push(format!(
            "scoreboard players operation #digit {o} %= #eight {o}"
        ));
        for value in 0..8 {
            out.push(format!(
                "execute in {dim} if score #digit {o} matches {value} if block {x} {y} {z} {orig} \
                 run setblock {x} {y} {z} {}",
                token::token_state(value + 1),
                x = p[0],
                y = p[1],
                z = p[2],
            ));
        }
        out.push(format!(
            "scoreboard players operation #total {o} /= #eight {o}"
        ));
    }
    let (p, orig) = &cells[DIGITS];
    for (bad, level) in [("0", token::ok_level(id)), ("1..", token::ok_level(id) + 4)] {
        out.push(format!(
            "execute in {dim} if score #bad {o} matches {bad} if block {x} {y} {z} {orig} run setblock {x} {y} {z} {}",
            token::token_state(level),
            x = p[0],
            y = p[1],
            z = p[2],
        ));
    }
}

/// Keeps a partial sum at most `MAX_RATE + 1`: once there, the whole sum
/// is at least that much, as later steps only double and add.
fn clamp(o: &str) -> String {
    format!(
        "execute if score #total {o} matches {}.. run scoreboard players set #total {o} {}",
        MAX_RATE + 1,
        MAX_RATE + 1
    )
}

/// What a sample's column says.
pub fn read(id: u32, levels: &[Option<u8>]) -> Sample {
    if levels.len() == CELLS && levels[DIGITS] == Some(token::ok_level(id) + 4) {
        return Sample::TagsUnknown;
    }
    decode(id, levels).map_or(Sample::Incomplete, Sample::Rate)
}

pub fn decode(id: u32, levels: &[Option<u8>]) -> Option<u32> {
    if levels.len() != CELLS || levels[DIGITS] != Some(token::ok_level(id)) {
        return None;
    }
    let mut total = 0;
    for digit in levels[..DIGITS].iter().rev() {
        let level = (*digit)?;
        if !(1..=8).contains(&level) {
            return None;
        }
        total = total * 8 + u32::from(level - 1);
    }
    Some(total)
}

/// One confirmed sample represents one second of exposure. No stale rate is
/// integrated while a probe is absent, timed out, or queued behind a machine.
pub fn accumulate(dose: &mut Dose, rate: u32) {
    add_dose(dose, u64::from(rate));
}

/// The one way dose grows (samples, and pulses from other modules), clamped
/// at 2500 RAD.
pub fn add_dose(dose: &mut Dose, millirad: u64) {
    dose.millirad = dose.millirad.saturating_add(millirad).min(MAX_DOSE);
}

pub const MAX_DOSE: u64 = 2_500_000;

/// Deterministic M1 approximation of NTM's random sickness rolls. Vanilla
/// effects expire naturally; never clear unrelated effects from other sources.
pub fn effect_lines(dose: &mut Dose) -> Vec<String> {
    let mut out = Vec::new();
    let rad = dose.millirad;
    if rad >= 1_000_000 {
        out.push("execute unless entity @s[gamemode=creative] unless entity @s[gamemode=spectator] run damage @s 1000 minecraft:generic_kill".into());
        dose.millirad = 0;
        return out;
    }
    let effects: &[(&str, u32)] = match rad {
        800_000.. => &[
            ("nausea", 0),
            ("slowness", 2),
            ("weakness", 2),
            ("poison", 2),
            ("wither", 1),
        ],
        600_000.. => &[
            ("nausea", 0),
            ("slowness", 2),
            ("weakness", 2),
            ("poison", 1),
        ],
        400_000.. => &[("nausea", 0), ("slowness", 0), ("weakness", 1)],
        200_000.. => &[("nausea", 0), ("weakness", 0)],
        _ => &[],
    };
    for (effect, amplifier) in effects {
        out.push(format!(
            "effect give @s minecraft:{effect} 5 {amplifier} true"
        ));
    }
    if rad >= 800_000 {
        out.push("damage @s 1 minecraft:magic".into());
    }
    out.into_iter().map(|line| format!("execute unless entity @s[gamemode=creative] unless entity @s[gamemode=spectator] run {line}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_sample_line_fits_the_quiet_limit() {
        let cells: [(Pos, String); CELLS] = std::array::from_fn(|_| {
            (
                [-29_999_999, 0, -29_999_999],
                format!("minecraft:{}", "d".repeat(30)),
            )
        });
        for l in sample_lines(u32::MAX, "minecraft:the_nether", &cells) {
            assert!(l.len() <= crate::token::MAX_LINE, "{} chars: {l}", l.len());
        }
    }

    use super::*;
    #[test]
    fn all_content_ids_covered_and_known_rates_match() {
        let table: serde_json::Value =
            serde_json::from_str(include_str!("../data/hazards.json")).unwrap();
        let ids: serde_json::Value =
            serde_json::from_str(include_str!("../../../content/ids.json")).unwrap();
        let a: std::collections::BTreeSet<_> = table["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        let b: std::collections::BTreeSet<_> = ids["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        assert_eq!(a, b);
        let radioactive = table["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["radiation_millirad_s"].as_u64().unwrap() > 0)
            .count();
        assert!(radioactive > 200, "the full catalog, not M1's 13");
        assert_eq!(hazards().len(), radioactive);
        let rate = |id: &str| hazards().iter().find(|h| h.id == id).unwrap().radiation_millirad_s;
        assert_eq!(rate("ntm:ingot_uranium"), 350);
        assert_eq!(rate("ntm:powder_thorium"), 300);
    }

    fn content(path: &str) -> serde_json::Value {
        let p = format!("{}/../../content/{path}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}")))
            .unwrap()
    }

    /// The tags the sample clears are exactly the table's bits: each bit_K
    /// lists every item whose rate has bit K, and no other; no other bit_K
    /// file exists. Any drift would count a rate wrong.
    #[test]
    fn bit_tags_on_disk_match_the_table() {
        let dir = format!("{}/../../content/data/ntm/tags/item/rad", env!("CARGO_MANIFEST_DIR"));
        let mut on_disk: Vec<u32> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let n = e.unwrap().file_name().into_string().unwrap();
                n.strip_prefix("bit_")?.strip_suffix(".json")?.parse().ok()
            })
            .collect();
        on_disk.sort();
        assert_eq!(on_disk, bits());
        let buckets: serde_json::Value =
            serde_json::from_str(include_str!("../data/radiation_buckets.json")).unwrap();
        let listed: Vec<(String, u64, u64)> = buckets["bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                (
                    b["tag"].as_str().unwrap().to_string(),
                    b["bit"].as_u64().unwrap(),
                    b["item_count"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(listed.len(), bits().len());
        for &k in bits() {
            let tag = content(&format!("data/ntm/tags/item/rad/bit_{k}.json"));
            assert_eq!(tag["replace"], false);
            let got: std::collections::BTreeSet<&str> = tag["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().expect("direct ids, no nested tags"))
                .collect();
            let want: std::collections::BTreeSet<&str> = hazards()
                .iter()
                .filter(|h| h.radiation_millirad_s >> k & 1 == 1)
                .map(|h| h.id.as_str())
                .collect();
            assert_eq!(got, want, "bit_{k}");
            assert!(listed.contains(&(bit_tag(k), u64::from(k), want.len() as u64)));
        }
        // Rebuilding every rate from its bits gives it back.
        for h in hazards() {
            let back: u32 = bits()
                .iter()
                .filter(|&&k| h.radiation_millirad_s >> k & 1 == 1)
                .map(|k| 1 << k)
                .sum();
            assert_eq!(back, h.radiation_millirad_s, "{}", h.id);
        }
    }

    /// The cadence: a sample's lines leave at 16 a tick, inside the second
    /// between samples, whatever the catalog's size (they grow with the
    /// rates' bits, never with the item count).
    #[test]
    fn a_sample_fits_its_second() {
        let cells: [(Pos, String); CELLS] =
            std::array::from_fn(|i| ([0, -64 + i as i32, 0], "minecraft:bedrock".to_string()));
        let n = sample_lines(1, "minecraft:overworld", &cells).len();
        let ticks = n.div_ceil(COMMANDS_PER_TICK) as u64;
        assert!(ticks < SAMPLE_TICKS, "{n} lines take {ticks} ticks");
        let top = u64::from(*bits().last().unwrap());
        let per_bit = 4 * bits().len() as u64 + top;
        assert_eq!(n as u64, 5 + per_bit + top / u64::from(CLAMP_EVERY) + 3 + 8 * 11 + 2);
        // Ids: ~4 lines an item, inside their own period.
        let ids = ids_sample_lines(1, "minecraft:overworld", &cells);
        let ticks = ids.len().div_ceil(COMMANDS_PER_TICK) as u64;
        assert!(ticks < ids_period(), "{} id lines take {ticks} ticks", ids.len());
        // About one id sample every 4-5 s with energy batch 5's 63 more
        // radioactive items (292). This is the WebAssembly mods' channel,
        // which the native host's inventory read replaces (stage L4).
        assert!(ids_period() <= 100, "about one id sample every 4-5 s: {} ticks", ids_period());
        let h = hazards().len();
        assert_eq!(ids.len(), 5 + 4 * h + h / 64 + 3 + 8 * 11 + 2);
        let cells: [(Pos, String); CELLS] = std::array::from_fn(|_| {
            ([-29_999_999, 0, -29_999_999], format!("minecraft:{}", "d".repeat(30)))
        });
        for l in ids_sample_lines(u32::MAX, "minecraft:the_nether", &cells) {
            assert!(l.len() <= crate::token::MAX_LINE, "{} chars: {l}", l.len());
        }
    }

    #[test]
    fn unknown_tags_read_apart_from_incomplete() {
        let mut levels: Vec<Option<u8>> = vec![Some(1); CELLS];
        levels[DIGITS] = Some(token::ok_level(17));
        assert_eq!(read(17, &levels), Sample::Rate(0));
        levels[DIGITS] = Some(token::ok_level(17) + 4);
        assert_eq!(read(17, &levels), Sample::TagsUnknown);
        levels[DIGITS] = None;
        assert_eq!(read(17, &levels), Sample::Incomplete);
        levels[DIGITS] = Some(token::ok_level(18));
        assert_eq!(read(17, &levels), Sample::Incomplete, "another probe's level");
    }

    #[test]
    fn wire_requires_all_digits_and_completion_token() {
        for rate in [0, 10, 35, 350, 3500, 224000, 9184000, 16777215] {
            let mut n = rate;
            let mut levels = Vec::new();
            for _ in 0..DIGITS {
                levels.push(Some((n % 8 + 1) as u8));
                n /= 8;
            }
            levels.push(Some(token::ok_level(17)));
            assert_eq!(decode(17, &levels), Some(rate));
            levels[DIGITS] = None;
            assert_eq!(decode(17, &levels), None);
        }
    }
    #[test]
    fn accumulation_sickness_and_death() {
        let mut d = Dose::default();
        for _ in 0..10 {
            accumulate(&mut d, 22400);
        }
        assert_eq!(d.millirad, 224000);
        assert!(effect_lines(&mut d).iter().any(|s| s.contains("weakness")));
        accumulate(&mut d, 0);
        assert_eq!(d.millirad, 224000, "no natural player-dose decay in NTM");
        d.millirad = 810000;
        assert!(
            effect_lines(&mut d)
                .iter()
                .any(|s| s.contains("damage @s 1"))
        );
        d.millirad = 1000000;
        assert_eq!(
            effect_lines(&mut d),
            [
                "execute unless entity @s[gamemode=creative] unless entity @s[gamemode=spectator] run damage @s 1000 minecraft:generic_kill"
            ]
        );
        assert_eq!(d.millirad, 0);
    }
}

//! Radiation (docs/radiacao.md): the items' rates, the players' doses and
//! sickness. Values are NTM gameplay RAD, not physical dosimetry. The native
//! host's inventory read and effects bring it to the players (stage L4 of
//! the migration); the bombs' pulses and zones add to the doses already.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Ticks after a player joins before their dose grows.
pub const GRACE_TICKS: u64 = 200;

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

#[derive(Debug, Deserialize)]
pub struct Hazard {
    pub id: String,
    pub radiation_millirad_s: u32,
}
#[derive(Deserialize)]
struct Table {
    entries: Vec<Hazard>,
}

/// Builds the hazard table now (600 KB of JSON), not in a tick.
pub fn warm() {
    bits();
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

/// The exact rate of an inventory (milliRAD/s).
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

/// One second of exposure at `rate` (milliRAD/s).
pub fn accumulate(dose: &mut Dose, rate: u32) {
    add_dose(dose, u64::from(rate));
}

/// The one way dose grows (exposure, and the bombs' pulses), clamped at
/// 2500 RAD.
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

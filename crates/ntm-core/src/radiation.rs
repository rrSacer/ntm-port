//! Radiation (docs/radiacao.md): the items' rates, the players' doses and
//! sickness. Values are NTM gameplay RAD, not physical dosimetry. The
//! server reads each player's inventory once a second
//! (`server/radiation_server.rs`); the nukes' pulses and the chunk field
//! add to the same doses.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Ticks after a player joins before their dose grows.
pub const GRACE_TICKS: u64 = 200;
/// The per-player values the client's Geiger reads: the dose (milliRAD)
/// and the rate where the player stands (milliRAD/s).
pub const KEY_DOSE: &str = "ntm_rad_dose";
pub const KEY_RATE: &str = "ntm_rad_rate";

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

#[derive(Clone, Debug, Deserialize)]
pub struct Hazard {
    pub id: String,
    pub radiation_millirad_s: u32,
    #[serde(default)] pub radiation_rad_s: f32,
    #[serde(default)] pub fuel_target_rad_s: Option<f32>,
    #[serde(default)] pub rtg_target_rad_s: Option<f32>,
    /// An RBMK rod's or pellet's `HazardModifierRBMKRadiation` (and the rods' `HazardModifierRBMKHot`).
    #[serde(default)] pub rbmk: Option<RbmkHazard>,
}

/// `registerRBMK*`'s modifiers: the radiation the item tends to as it depletes (`dep`), whether linearly, whether it
/// is HOT, and `HazardRegistry.xe135` and `nugget` (the pellets' xenon weight).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct RbmkHazard {
    pub target_rad_s: f32,
    pub linear: bool,
    pub hot: bool,
    pub xe135_rad_s: f32,
    pub nugget: f32,
}

/// The RBMK hazard of item `id`.
pub fn rbmk_hazard(id: &str) -> Option<&'static Hazard> {
    all_hazards().iter().find(|h| h.id == id && h.rbmk.is_some())
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
fn all_hazards() -> &'static [Hazard] {
    static H: OnceLock<Vec<Hazard>> = OnceLock::new();
    H.get_or_init(||serde_json::from_str::<Table>(include_str!("../data/hazards.json")).expect("checked-in hazard table").entries)
}
pub fn hazards() -> &'static [Hazard] {
    static H: OnceLock<Vec<Hazard>> = OnceLock::new();
    H.get_or_init(||all_hazards().iter().filter(|h|h.radiation_millirad_s>0).cloned().collect())
}
/// Includes fresh lithium's zero base: its life-dependent target is nonzero.
pub fn fuel_hazard(id:&str)->Option<&'static Hazard> {
    all_hazards().iter().find(|h|h.id==id && h.fuel_target_rad_s.is_some())
}

pub fn rtg_hazard(id: &str) -> Option<&'static Hazard> {
    all_hazards().iter().find(|h| h.id==id && h.rtg_target_rad_s.is_some())
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

/// What radiation sickness does to a player: an effect for some ticks, or
/// damage of a kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Harm {
    Effect { id: &'static str, amplifier: u8, ticks: u32 },
    Hurt { amount: f32, kind: &'static str },
}

/// `ModDamageSource.radiation`, `new DamageSource("radiation").setDamageBypassesArmor()`: the content damage type
/// (decision D2 of slice 5; `tools/content-gen/damage_types.py`), which armor does not stop and Resistance does.
pub const RADIATION_DAMAGE: &str = "ntm:radiation";
/// `HbmPotion.radiation` as a status effect: the flag a player sees while the potion runs.
pub const RADIATION_EFFECT: &str = "ntm:radiation";

/// At 1000 RAD radiation kills (`EntityEffectHandler.handleRadiationEffect:223-226`:
/// `attackEntityFrom(ModDamageSource.radiation, 1000F)`, then the dose is 0, whatever the attack did).
pub fn death(dose: &mut Dose) -> Option<Harm> {
    if dose.millirad < 1_000_000 {
        return None;
    }
    dose.millirad = 0;
    Some(Harm::Hurt { amount: 1000.0, kind: RADIATION_DAMAGE })
}

/// One tick of `EntityEffectHandler.handleRadiationEffect:221-258` for a
/// player between 200 and 1000 RAD: each effect of the dose's band is rolled
/// with `world.rand.nextInt(n) == 0`, in the Java order, every call made
/// whatever the earlier ones gave (`addPotionEffect(new PotionEffect(id,
/// ticks, amplifier))`). Outside the bands nothing is drawn.
pub fn sickness_roll(dose: &Dose, rng: &mut crate::rbmk::jrandom::JavaRandom) -> Vec<Harm> {
    // (bound, effect, ticks, amplifier), as the Java lists them.
    let rolls: &[(i32, &'static str, u32, u8)] = match dose.millirad {
        1_000_000.. => &[],
        800_000.. => &[(300, "nausea", 5 * 30, 0), (300, "slowness", 10 * 20, 2), (300, "weakness", 10 * 20, 2), (500, "poison", 3 * 20, 2), (700, "wither", 3 * 20, 1)],
        600_000.. => &[(300, "nausea", 5 * 30, 0), (300, "slowness", 10 * 20, 2), (300, "weakness", 10 * 20, 2), (500, "poison", 3 * 20, 1)],
        400_000.. => &[(300, "nausea", 5 * 30, 0), (500, "slowness", 5 * 20, 0), (300, "weakness", 5 * 20, 1)],
        200_000.. => &[(300, "nausea", 5 * 20, 0), (500, "weakness", 5 * 20, 0)],
        _ => &[],
    };
    rolls
        .iter()
        .filter(|&&(bound, ..)| rng.next_int(bound) == 0)
        .map(|&(_, id, ticks, amplifier)| Harm::Effect { id, amplifier, ticks })
        .collect()
}

/// The sickness rolls' `java.util.Random` (decision D1 of slice 5, the shape of the RBMK's D3): one per world, seeded
/// from the world's seed the first time someone is sick, by its seed and cached gaussian.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SicknessRng {
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gaussian: Option<u64>,
}

impl SicknessRng {
    pub fn seeded(world_seed: i64) -> Self {
        let mut s = SicknessRng { seed: 0, gaussian: None };
        s.set_rng(&crate::rbmk::jrandom::JavaRandom::new(world_seed));
        s
    }
    pub fn rng(&self) -> crate::rbmk::jrandom::JavaRandom {
        crate::rbmk::jrandom::JavaRandom::from_state(self.seed, self.gaussian.map(f64::from_bits))
    }
    pub fn set_rng(&mut self, rng: &crate::rbmk::jrandom::JavaRandom) {
        let (seed, gaussian) = rng.state();
        self.seed = seed;
        self.gaussian = gaussian.map(f64::to_bits);
    }
}

/// The decontaminator (`BlockDecon`, `TileEntityDecon`).
pub const DECON: &str = "ntm:decon";
/// `HbmLivingProps.incrementRadiation(e, -0.5F)`: per tick and per unit.
pub const DECON_MILLIRAD: u64 = 500;
/// `EntityPlayer.setSize(0.6F, 1.8F)`: the box `getEntitiesWithinAABB` meets.
const PLAYER_HALF_WIDTH: f64 = 0.3;
const PLAYER_HEIGHT: f64 = 1.8;

/// Whether the decontaminator at `block` reaches a player standing at `feet`:
/// `TileEntityDecon.updateEntity` takes the entities within
/// `(x - 0.5, y, z - 0.5) .. (x + 1.5, y + 2, z + 1.5)`, and an entity is within
/// a box when its own box intersects it (strict on every axis).
pub fn decon_reaches(block: crate::Pos, feet: [f64; 3]) -> bool {
    let [x, y, z] = block.map(f64::from);
    let (lo, hi) = ([x - 0.5, y, z - 0.5], [x + 1.5, y + 2.0, z + 1.5]);
    let (plo, phi) = (
        [feet[0] - PLAYER_HALF_WIDTH, feet[1], feet[2] - PLAYER_HALF_WIDTH],
        [feet[0] + PLAYER_HALF_WIDTH, feet[1] + PLAYER_HEIGHT, feet[2] + PLAYER_HALF_WIDTH],
    );
    (0..3).all(|i| plo[i] < hi[i] && phi[i] > lo[i])
}

/// Every block whose decontaminator box could reach a player at `feet`, the
/// cells to look at.
pub fn decon_cells(feet: [f64; 3]) -> Vec<crate::Pos> {
    let [fx, fy, fz] = feet.map(|v| v.floor() as i32);
    let mut out = Vec::new();
    for x in fx - 2..=fx + 1 {
        for y in fy - 2..=fy + 2 {
            for z in fz - 2..=fz + 1 {
                if decon_reaches([x, y, z], feet) {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

/// One tick of `n` decontaminators on a dose: `incrementRadiation(-0.5F)` each,
/// clamped at 0 (`HbmLivingProps.setRadiation`).
pub fn decontaminate(dose: &mut Dose, n: usize) {
    dose.millirad = dose.millirad.saturating_sub(DECON_MILLIRAD.saturating_mul(n as u64));
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
        accumulate(&mut d, 0);
        assert_eq!(d.millirad, 224000, "no natural player-dose decay in NTM");
        assert_eq!(death(&mut d), None);
        d.millirad = 999_999;
        assert_eq!(death(&mut d), None);
        d.millirad = 1000000;
        assert_eq!(death(&mut d), Some(Harm::Hurt { amount: 1000.0, kind: "ntm:radiation" }));
        assert_eq!(d.millirad, 0);
    }

    /// Below 200 RAD and from 1000 nothing is drawn: the random is where it was.
    #[test]
    fn sickness_draws_only_in_its_bands() {
        use crate::rbmk::jrandom::JavaRandom;
        for millirad in [0, 199_999, 1_000_000, 2_500_000] {
            let mut r = JavaRandom::new(7);
            assert!(sickness_roll(&Dose { millirad }, &mut r).is_empty());
            assert_eq!(r.state(), JavaRandom::new(7).state(), "{millirad}");
        }
        // Each band draws once per effect it lists, whatever the draws gave.
        let bands: [(u64, &[i32]); 5] = [
            (200_000, &[300, 500]),
            (400_000, &[300, 500, 300]),
            (600_000, &[300, 300, 300, 500]),
            (800_000, &[300, 300, 300, 500, 700]),
            (999_999, &[300, 300, 300, 500, 700]),
        ];
        for (millirad, bounds) in bands {
            let mut r = JavaRandom::new(7);
            sickness_roll(&Dose { millirad }, &mut r);
            let mut expected = JavaRandom::new(7);
            for &b in bounds {
                expected.next_int(b);
            }
            assert_eq!(r.state(), expected.state(), "{millirad}");
        }
    }

    /// The rolls against the Java's own `handleRadiationEffect`, compiled and run with the same seed over the same
    /// doses (`tools/sickness-oracle`): the same effects on the same ticks, with their lengths and amplifiers.
    #[test]
    fn sickness_rolls_as_the_java_oracle() {
        use crate::rbmk::jrandom::JavaRandom;
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tools/sickness-oracle/fixture.json")).unwrap();
        // The doses tick by tick: each (RAD, ticks) of the schedule held in turn.
        let doses: Vec<u64> = fixture["schedule"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| std::iter::repeat_n(s[0].as_u64().unwrap(), s[1].as_u64().unwrap() as usize))
            .collect();
        for case in fixture["cases"].as_array().unwrap() {
            let mut r = JavaRandom::new(case["seed"].as_i64().unwrap());
            let (mut seen, mut attacks) = (Vec::new(), Vec::new());
            for (tick, &rad) in doses.iter().enumerate() {
                let mut dose = Dose { millirad: rad * 1000 };
                // The Java's chain: from 1000 RAD the attack and nothing drawn, below it the band's rolls.
                if let Some(Harm::Hurt { amount, kind }) = death(&mut dose) {
                    assert_eq!((kind, dose.millirad), (RADIATION_DAMAGE, 0));
                    attacks.push(serde_json::json!([tick, "attack:radiation", amount]));
                    continue;
                }
                for h in sickness_roll(&dose, &mut r) {
                    if let Harm::Effect { id, amplifier, ticks } = h {
                        seen.push(serde_json::json!([tick, id, ticks, amplifier]));
                    }
                }
            }
            let expected: Vec<serde_json::Value> = case["effects"].as_array().unwrap().clone();
            assert!(!expected.is_empty());
            assert_eq!(seen, expected, "seed {}", case["seed"]);
            assert_eq!(&attacks, case["attacks"].as_array().unwrap(), "seed {}", case["seed"]);
        }
    }

    /// TileEntityDecon's box against a 0.6 x 1.8 player.
    #[test]
    fn the_decontaminator_reaches_who_stands_over_it() {
        // Standing on top, centred: y + 1 is the box's middle.
        assert!(decon_reaches([0, 64, 0], [0.5, 65.0, 0.5]));
        // The box reaches 0.5 to the sides: a player 1.79 away still touches it, 1.8 does not (strict).
        assert!(decon_reaches([0, 64, 0], [1.5 + 0.29, 65.0, 0.5]));
        assert!(!decon_reaches([0, 64, 0], [1.5 + 0.3, 65.0, 0.5]));
        assert!(decon_reaches([0, 64, 0], [-0.5 - 0.29, 65.0, 0.5]));
        assert!(!decon_reaches([0, 64, 0], [-0.5 - 0.3, 65.0, 0.5]));
        // Height: from y to y + 2; feet at y + 2 are out, a head inside the block is in.
        assert!(!decon_reaches([0, 64, 0], [0.5, 66.0, 0.5]));
        assert!(decon_reaches([0, 64, 0], [0.5, 65.99, 0.5]));
        assert!(!decon_reaches([0, 64, 0], [0.5, 64.0 - 1.8, 0.5]));
        assert!(decon_reaches([0, 64, 0], [0.5, 64.0 - 1.79, 0.5]));
        // The cells looked at are exactly the ones that reach.
        let feet = [10.2, 70.0, -3.7];
        let cells = decon_cells(feet);
        for x in 5..15 {
            for y in 64..76 {
                for z in -9..2 {
                    assert_eq!(cells.contains(&[x, y, z]), decon_reaches([x, y, z], feet), "{x} {y} {z}");
                }
            }
        }
        // Two decontaminators side by side both reach a player between them.
        assert!(decon_reaches([0, 64, 0], [1.0, 65.0, 0.5]) && decon_reaches([1, 64, 0], [1.0, 65.0, 0.5]));
    }

    #[test]
    fn each_decontaminator_takes_half_a_rad_a_tick() {
        let mut d = Dose { millirad: 1_200 };
        decontaminate(&mut d, 1);
        assert_eq!(d.millirad, 700);
        decontaminate(&mut d, 2);
        assert_eq!(d.millirad, 0);
        decontaminate(&mut d, 1);
        assert_eq!(d.millirad, 0);
    }
}

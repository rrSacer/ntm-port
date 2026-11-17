//! Digamma (`HbmLivingProps.digamma`), the dose the DRX laser, the digamma flame and the digamma items give. A
//! player's is a float from 0 to 10 (`incrementDigamma`, `extprop/HbmLivingProps.java:185-199`), kept in their own
//! storage beside the radiation dose and saved with them (`hfr_digamma`, `:305`/`:332`); a new body starts at 0, as
//! a respawned player's new `HbmLivingProps` does.
//!
//! `setDigamma` (`:134-183`) puts a `maxHealth` modifier on them, `0.5^digamma - 1` with operation 2 (the total
//! times `1 + amount`), caps their health to the new maximum, and kills them at 10 (`ModDamageSource.digamma`, 500,
//! then `setHealth(0)`).
//!
//! Two ways in, with the same gates for a player (`util/ContaminationUtil.java`):
//! - `applyDigammaData` (`:108-129`: the DRX beam, the hazard of digamma items): none in creative, none in their
//!   first 200 ticks, none under `HbmPotion.stability`, none in a full set of FAU or DNS armour (`checkForDigamma`);
//! - `contaminate(.., DIGAMMA, DIGAMMA, ..)` (`:265-300`: the digamma flame): `checkForDigamma`, or
//!   `checkForDigamma2` (robes, stability and iron cladding: never without stability, so `checkForDigamma` holds
//!   whenever it does), then creative, then the 200 ticks.
//!
//! The blocks the DRX beam leaves are here too: `DigammaFlame` (`blocks/bomb/DigammaFlame.java`) and
//! `DigammaMatter` (`blocks/bomb/DigammaMatter.java`), whose metadata 0..7 the port keeps as one block each
//! (`ntm:digamma_matter`, `ntm:digamma_matter__<n>`), as it does every metadata subtype.

/// `incrementDigamma`'s bounds.
pub const MAX: f32 = 10.0;
/// What `TileEntityFEL` gives each living thing in a DRX beam, every tick (`ContaminationUtil.applyDigammaData(entity,
/// 0.1F)`, `:134`).
pub const FEL_DRX: f32 = 0.1;
/// What `DigammaFlame.onEntityCollidedWithBlock` gives, every tick a body touches it (`contaminate(.., 0.05F)`).
pub const FLAME: f32 = 0.05;
/// `ModDamageSource.digamma`: absolute, bypasses armour, allowed in creative.
pub const DAMAGE: &str = "ntm:digamma";
/// `setDigamma`'s blow (`attackEntityFrom(ModDamageSource.digamma, 500F)`).
pub const DEATH_DAMAGE: f32 = 500.0;
/// `digamma_UUID`: the port's id of the `maxHealth` modifier.
pub const MODIFIER: &str = "ntm:digamma";

pub const FLAME_BLOCK: &str = "ntm:fire_digamma";
pub const MATTER_BLOCK: &str = "ntm:digamma_matter";
pub const ASH_BLOCK: &str = "ntm:ash_digamma";
/// `DigammaMatter.updateTick`: from metadata 7 it only goes away.
pub const MATTER_LAST: u8 = 7;

/// `ArmorUtil.checkForDigamma`'s full sets (`util/ArmorUtil.java:157-163`), helmet to boots. Not in the catalog yet:
/// nobody can wear them, and the check holds no one.
pub const FAU_SET: [&str; 4] = ["ntm:fau_helmet", "ntm:fau_plate", "ntm:fau_legs", "ntm:fau_boots"];
pub const DNS_SET: [&str; 4] = ["ntm:dns_helmet", "ntm:dns_plate", "ntm:dns_legs", "ntm:dns_boots"];
/// `HbmPotion.stability`.
pub const STABILITY: &str = "ntm:stability";

/// `incrementDigamma`: the sum in float, held between 0 and 10.
pub fn increment(current: f32, amount: f32) -> f32 {
    (current + amount).clamp(0.0, MAX)
}

/// `setDigamma`'s modifier: `(float) Math.pow(0.5, digamma) - 1F`, applied as operation 2 (`add_multiplied_total`).
pub fn health_modifier(digamma: f32) -> f32 {
    0.5f64.powf(f64::from(digamma)) as f32 - 1.0
}

/// `setDigamma:158`: `getMaxHealth() <= 0 || digamma >= 10`. A player's maximum is 20 times `0.5^digamma` at the
/// most, never 0 below 10.
pub fn kills(digamma: f32, max_health: f64) -> bool {
    max_health <= 0.0 || digamma >= MAX
}

/// `ArmorUtil.checkArmor` over a set: every armour slot (helmet, chest, legs, boots) holds the set's piece.
pub fn wears_set(worn: &[Option<String>; 4], set: &[&str; 4]) -> bool {
    worn.iter().zip(set).all(|(w, s)| w.as_deref() == Some(*s))
}

/// What keeps a player's digamma where it is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gates {
    pub creative: bool,
    /// `ticksExisted < 200`.
    pub joining: bool,
    pub stability: bool,
    /// A full FAU or DNS set.
    pub armor: bool,
}

/// `applyDigammaData` and `contaminate(.., DIGAMMA, DIGAMMA, ..)` hold a player by the same gates (see the module).
pub fn blocked(g: Gates) -> bool {
    g.creative || g.joining || g.stability || g.armor
}

/// `EntityEffectHandler.handleDigamma:343-364`: from 0.01 a player sweats soul sand, in 1 of `max(10 - (int)
/// digamma, 1)` ticks (every tick from 9).
pub fn sweat_chance(digamma: f32) -> Option<i32> {
    (digamma >= 0.01).then(|| (10 - digamma as i32).max(1))
}

/// The id of `DigammaMatter` at `meta`.
pub fn matter_id(meta: u8) -> String {
    if meta == 0 { MATTER_BLOCK.into() } else { format!("{MATTER_BLOCK}__{meta}") }
}

/// The metadata of a `DigammaMatter` id.
pub fn matter_meta(id: &str) -> Option<u8> {
    if id == MATTER_BLOCK {
        return Some(0);
    }
    let meta: u8 = id.strip_prefix(MATTER_BLOCK)?.strip_prefix("__")?.parse().ok()?;
    (1..=MATTER_LAST).contains(&meta).then_some(meta)
}

/// `DigammaMatter.onBlockAdded`: the next tick of a new matter block, 10 to 49 ticks on (`10 + rand.nextInt(40)`).
pub fn matter_delay(roll: impl FnOnce(i32) -> i32) -> u32 {
    10 + roll(40) as u32
}

/// `DigammaMatter.updateTick`: the block goes; below metadata 7 each cell 1 or 2 steps away (`|i|+|j|+|k|` in
/// 1..3, the 6 faces and 12 edges, in the loop's order) that holds no matter takes metadata `meta + 1`. Whatever was
/// there (`setBlock`), unbreakable or not.
pub fn matter_spread(meta: u8, is_matter: impl Fn([i32; 3]) -> bool) -> Vec<([i32; 3], u8)> {
    if meta >= MATTER_LAST {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in -1..=1 {
        for j in -1..=1 {
            for k in -1..=1 {
                let dist = i32::abs(i) + i32::abs(j) + i32::abs(k);
                if dist > 0 && dist < 3 && !is_matter([i, j, k]) {
                    out.push(([i, j, k], meta + 1));
                }
            }
        }
    }
    out
}

/// `MainRegistry.polaroidID` (`main/MainRegistry.java:250-256`), drawn once per launch: the config's
/// `generalOverride` when it is 1 to 18, else 1 to 18 but 4 and 9 (`rand.nextInt(18) + 1` until neither).
pub fn polaroid(general_override: Option<i32>, mut next_int: impl FnMut(i32) -> i32) -> i32 {
    if let Some(o) = general_override.filter(|o| (1..19).contains(o)) {
        return o;
    }
    let mut id = next_int(18) + 1;
    while id == 4 || id == 9 {
        id = next_int(18) + 1;
    }
    id
}

/// `TileEntityFEL:187`: what a DRX beam leaves where it burns, `digamma_matter` only with polaroid 11.
pub fn drx_block(polaroid: i32) -> &'static str {
    if polaroid == 11 { MATTER_BLOCK } else { FLAME_BLOCK }
}

/// `ItemPill.onFoodEaten:81-104`: the pills that set a player's digamma (`setDigamma`, no gate): `xanax` takes 0.5
/// off (not below 0), `fmn` brings it down to 2, `five_htp` to 0. None for any other item.
pub fn pill(item: &str, current: f32) -> Option<f32> {
    match item {
        "ntm:xanax" => Some((current - 0.5).max(0.0)),
        "ntm:fmn" => Some(current.min(2.0)),
        "ntm:five_htp" => Some(0.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dose_sums_in_float_and_stays_between_0_and_10() {
        assert_eq!(increment(0.0, FEL_DRX), 0.1);
        let mut d = 0.0f32;
        for _ in 0..10 {
            d = increment(d, FEL_DRX);
        }
        // Ten float sums of 0.1F pass 1 (1.0000001), as the Java's do.
        assert_eq!(d, 1.000_000_1);
        assert!(d > 1.0);
        assert_eq!(increment(9.95, FEL_DRX), MAX);
        assert_eq!(increment(0.02, -1.0), 0.0);
    }

    #[test]
    fn the_health_modifier_halves_the_maximum_each_point() {
        assert_eq!(health_modifier(0.0), 0.0);
        assert_eq!(health_modifier(1.0), -0.5);
        assert_eq!(health_modifier(2.0), -0.75);
        assert_eq!(health_modifier(0.1), (0.5f64.powf(f64::from(0.1f32)) as f32) - 1.0);
        // 20 times (1 + modifier): a player's maximum at 10 is above 0.
        assert!(20.0 * (1.0 + f64::from(health_modifier(MAX))) > 0.0);
    }

    #[test]
    fn ten_kills_and_so_does_no_maximum() {
        assert!(!kills(9.999, 0.02));
        assert!(kills(MAX, 0.02));
        assert!(kills(0.5, 0.0));
    }

    #[test]
    fn every_gate_holds_a_player() {
        assert!(!blocked(Gates::default()));
        for g in [
            Gates { creative: true, ..Default::default() },
            Gates { joining: true, ..Default::default() },
            Gates { stability: true, ..Default::default() },
            Gates { armor: true, ..Default::default() },
        ] {
            assert!(blocked(g), "{g:?}");
        }
    }

    #[test]
    fn only_a_whole_set_is_armour() {
        let full = FAU_SET.map(|s| Some(s.to_string()));
        assert!(wears_set(&full, &FAU_SET));
        assert!(!wears_set(&full, &DNS_SET));
        let mut missing = full.clone();
        missing[3] = None;
        assert!(!wears_set(&missing, &FAU_SET));
        let mut mixed = full;
        mixed[0] = Some(DNS_SET[0].into());
        assert!(!wears_set(&mixed, &FAU_SET));
    }

    #[test]
    fn sweat_starts_at_a_hundredth_and_every_tick_from_nine() {
        assert_eq!(sweat_chance(0.009), None);
        assert_eq!(sweat_chance(0.01), Some(10));
        assert_eq!(sweat_chance(1.5), Some(9));
        assert_eq!(sweat_chance(8.99), Some(2));
        assert_eq!(sweat_chance(9.0), Some(1));
        assert_eq!(sweat_chance(MAX), Some(1));
    }

    #[test]
    fn matter_ids_round_trip_its_eight_metadata() {
        assert_eq!(matter_id(0), "ntm:digamma_matter");
        assert_eq!(matter_id(7), "ntm:digamma_matter__7");
        for meta in 0..=MATTER_LAST {
            assert_eq!(matter_meta(&matter_id(meta)), Some(meta));
        }
        assert_eq!(matter_meta("ntm:digamma_matter__8"), None);
        assert_eq!(matter_meta("ntm:digamma_matter__0"), None);
        assert_eq!(matter_meta("ntm:fire_digamma"), None);
    }

    #[test]
    fn matter_waits_10_to_49_ticks() {
        assert_eq!(matter_delay(|_| 0), 10);
        assert_eq!(matter_delay(|n| n - 1), 49);
        let mut asked = 0;
        matter_delay(|n| {
            asked = n;
            0
        });
        assert_eq!(asked, 40);
    }

    #[test]
    fn matter_spreads_to_the_18_cells_within_two_steps_until_7() {
        let cells = matter_spread(0, |_| false);
        assert_eq!(cells.len(), 18);
        assert!(cells.iter().all(|(o, m)| *m == 1 && (1..3).contains(&o.iter().map(|v| v.abs()).sum::<i32>())));
        assert_eq!(cells[0].0, [-1, -1, 0]);
        assert_eq!(cells[17].0, [1, 1, 0]);
        assert!(!cells.iter().any(|(o, _)| o == &[1, 1, 1] || o == &[0, 0, 0]));
        // Cells that already hold matter keep it.
        let around = matter_spread(3, |o| o == [0, 1, 0]);
        assert_eq!(around.len(), 17);
        assert!(around.iter().all(|(_, m)| *m == 4));
        assert!(matter_spread(6, |_| false).iter().all(|(_, m)| *m == 7));
        assert!(matter_spread(MATTER_LAST, |_| false).is_empty());
    }

    #[test]
    fn the_polaroid_takes_the_override_or_skips_4_and_9() {
        assert_eq!(polaroid(Some(11), |_| panic!()), 11);
        assert_eq!(polaroid(Some(18), |_| panic!()), 18);
        let mut rolls = vec![3, 8, 10].into_iter();
        assert_eq!(polaroid(Some(19), |n| { assert_eq!(n, 18); rolls.next().unwrap() }), 11);
        let mut rolls = vec![0].into_iter();
        assert_eq!(polaroid(Some(0), |_| rolls.next().unwrap()), 1);
        let mut rolls = vec![17].into_iter();
        assert_eq!(polaroid(None, |_| rolls.next().unwrap()), 18);
        assert_eq!(drx_block(11), MATTER_BLOCK);
        assert_eq!(drx_block(10), FLAME_BLOCK);
        assert_eq!(drx_block(12), FLAME_BLOCK);
    }

    #[test]
    fn the_three_pills_set_the_digamma() {
        assert_eq!(pill("ntm:xanax", 1.2), Some(1.2f32 - 0.5));
        assert_eq!(pill("ntm:xanax", 0.3), Some(0.0));
        assert_eq!(pill("ntm:fmn", 7.5), Some(2.0));
        assert_eq!(pill("ntm:fmn", 1.5), Some(1.5));
        assert_eq!(pill("ntm:five_htp", 9.0), Some(0.0));
        assert_eq!(pill("ntm:siox", 3.0), None);
    }
}

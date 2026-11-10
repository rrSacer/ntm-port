//! An RBMK rod's hazards from its stack's tag: `HazardModifierRBMKRadiation` (the radiation goes from the fresh rod's
//! towards its target as it depletes, plus Xe-135 by its poison) and `HazardModifierRBMKHot` (HOT from its hull's
//! heat). Pellets keep their fresh rate here: their grade and xenon live in metadata the port does not carry yet (T14).
use super::persist;
use crate::server::container_bridge::Stack;

/// `HazardModifierRBMKRadiation.modify` for a rod, in mRAD/s per item: `level + (target − level) · depletion` (float
/// endpoints, double interpolation, float result), `depletion = 1 − e` when linear else `1 − e²`, then
/// `+ xe135 · poisonLevel`. None: not an RBMK rod with a hazard.
pub fn radiation_rate(stack: &Stack) -> Option<u32> {
    // `if(level > 0)` contaminates: a NaN or negative level none (the cast's 0), an infinite one all it can.
    radiation_level(stack).map(|level| (level * 1000.0).round() as u32)
}

/// The rod's radiation level in RAD/s per item, the float `HazardModifierRBMKRadiation.modify` returns.
pub fn radiation_level(stack: &Stack) -> Option<f32> {
    let rod = persist::rod_of(&stack.id)?;
    let h = crate::radiation::rbmk_hazard(&stack.id)?;
    let m = h.rbmk?;
    let mut s = persist::stack_rod(stack);
    let e = s.enrichment(rod);
    let depletion = if m.linear { 1.0 - e } else { 1.0 - e.powi(2) };
    let xenon = s.poison_level(rod);
    let level = (f64::from(h.radiation_rad_s) + f64::from(m.target_rad_s - h.radiation_rad_s) * depletion) as f32;
    // `level += xe135 * xenon`: float times double, back to float.
    Some((f64::from(level) + f64::from(m.xe135_rad_s) * xenon) as f32)
}

/// `HazardTypeRadiation.addHazardInformation` for a `level` (RAD/s per item) and a stack of `count`: nothing below
/// `1e-5`, else the rate as `"" + Math.floor(level * 1000) / 1000` (the product in float, `Double.toString`) and, past
/// one item, the stack's `Math.floor(level * 1000 * count) / 1000`. A NaN level shows `NaNRAD/s`, as in Java.
pub fn radiation_tooltip(level: f32, count: u32) -> Vec<String> {
    if level < 1e-5 {
        return Vec::new();
    }
    let shown = |v: f32| super::tooltip::java_double(f64::from(v).floor() / 1000.0);
    let mut lines = vec!["\u{a7}a[Radioactive]".to_owned(), format!("\u{a7}e{}RAD/s", shown(level * 1000.0))];
    if count > 1 {
        lines.push(format!("\u{a7}eStack: {}RAD/s", shown(level * 1000.0 * count as f32)));
    }
    lines
}

/// `Entity.setFire(seconds)` on a player: `seconds · 20`, less `floor(t · level · 0.15F)` for the highest Fire
/// Protection level among the armour (`EnchantmentProtection.getFireTimeForEntity`, `getLastActiveItems` = the
/// armour inventory), the fire then the longer of what is left and this (`if(fire < j)`).
pub fn fire_ticks(seconds: i32, highest_fire_protection: u32, left: i32) -> i32 {
    let mut j = seconds.saturating_mul(20);
    if highest_fire_protection > 0 {
        j -= (j as f32 * highest_fire_protection as f32 * 0.15f32).floor() as i32;
    }
    left.max(j)
}

/// `HazardModifierRBMKHot.modify`: `(int) min(ceil((hull − 100) / 10), 60)`, negative below 100 °C (nothing then, as
/// `HazardTypeHot` acts only above 0). None: not a HOT rod.
pub fn hot_level(stack: &Stack) -> Option<i32> {
    let rod = persist::rod_of(&stack.id)?;
    if !crate::radiation::rbmk_hazard(&stack.id)?.rbmk?.hot {
        return None;
    }
    let heat = persist::stack_rod(stack).hull_heat(rod);
    Some(((heat - 100.0) / 10.0).ceil().min(60.0) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbmk::rods::RodStack;

    fn rod_stack(id: &str, edit: impl FnOnce(&mut RodStack, &crate::rbmk::rods::RodType)) -> Stack {
        let rod = persist::rod_of(id).unwrap();
        let mut s = RodStack::fresh(rod);
        edit(&mut s, rod);
        Stack { custom: persist::with_rod_stack(None, &s), tagged: true, ..Stack::new(id, 1) }
    }

    #[test]
    fn a_rod_goes_towards_its_target_as_it_depletes_plus_its_xenon() {
        let ueu = "ntm:rbmk_fuel_ueu";
        let h = crate::radiation::rbmk_hazard(ueu).unwrap();
        let m = h.rbmk.unwrap();
        assert!(!m.linear && m.hot);
        // Fresh: the base rate. A bare stack is fresh too (its tag set up on first use).
        assert_eq!(radiation_rate(&rod_stack(ueu, |_, _| {})), Some((h.radiation_rad_s * 1000.0).round() as u32));
        assert_eq!(radiation_rate(&Stack::new(ueu, 1)), radiation_rate(&rod_stack(ueu, |_, _| {})));
        // Half depleted, quadratic: 1 − 0.25 = 0.75 of the way.
        let half = rod_stack(ueu, |s, r| s.set_yield(r, r.yield_ / 2.0));
        let want = (f64::from(h.radiation_rad_s) + f64::from(m.target_rad_s - h.radiation_rad_s) * 0.75) as f32;
        assert_eq!(radiation_rate(&half), Some((want * 1000.0).round() as u32));
        // Poisoned 10 %: + xe135 · 0.1.
        let poisoned = rod_stack(ueu, |s, r| s.set_poison(r, 10.0));
        let want = (f64::from(h.radiation_rad_s) + f64::from(m.xe135_rad_s) * 0.1) as f32;
        assert_eq!(radiation_rate(&poisoned), Some((want * 1000.0).round() as u32));
        // A renamed rod reads a yield of 0: fully depleted, at its target.
        let renamed = Stack { tagged: true, ..Stack::new(ueu, 1) };
        assert_eq!(radiation_rate(&renamed), Some((m.target_rad_s * 1000.0).round() as u32));
        // Linear (Po-210/Be): half depleted is half way.
        let po = "ntm:rbmk_fuel_po210be";
        let p = crate::radiation::rbmk_hazard(po).unwrap();
        assert!(p.rbmk.unwrap().linear);
        let half = rod_stack(po, |s, r| s.set_yield(r, r.yield_ / 2.0));
        let want = (f64::from(p.radiation_rad_s) + f64::from(p.rbmk.unwrap().target_rad_s - p.radiation_rad_s) * 0.5) as f32;
        assert_eq!(radiation_rate(&half), Some((want * 1000.0).round() as u32));
        assert_eq!(radiation_rate(&Stack::new("ntm:rbmk_pellet_ueu", 1)), None, "pellets: T14");
        let nan = rod_stack(ueu, |s, r| s.set_yield(r, f64::NAN));
        assert_eq!(radiation_rate(&nan), Some(0), "NaN > 0 is false: no radiation");
        assert_eq!(radiation_rate(&Stack::new("ntm:rbmk_fuel_test", 1)), None, "THE VOICES registers no hazard");
    }

    #[test]
    fn the_tooltip_truncates_the_float_level_as_java() {
        let ueu = "ntm:rbmk_fuel_ueu";
        // Every depletion state of the UEU rod: the shown rate is the level truncated to 0.001 (never rounded up),
        // formatted as Double.toString of floor(f32(level · 1000)) / 1000.
        for k in 1..1000 {
            let s = rod_stack(ueu, |s, r| s.set_yield(r, r.yield_ * f64::from(k) / 1000.0));
            let level = radiation_level(&s).unwrap();
            let lines = radiation_tooltip(level, 1);
            let want = crate::rbmk::tooltip::java_double(f64::from(level * 1000.0).floor() / 1000.0);
            assert_eq!(lines[1], format!("\u{a7}e{want}RAD/s"), "{k}");
            // Truncated, as Java's floor of the float product (`level * 1000` rounds in float first, as Java's does).
            let product = f64::from(level * 1000.0);
            let shown: f64 = want.parse().unwrap();
            assert!(shown * 1000.0 <= product + 1e-6 && product - shown * 1000.0 < 1.0 + 1e-6, "{k}: {shown} vs {level}");
        }
        // Enrichment 0.998: 6.1896… shows 6.189, not 6.19.
        let s = rod_stack(ueu, |s, r| s.set_yield(r, r.yield_ * 0.998));
        let level = radiation_level(&s).unwrap();
        assert!((6.189..6.19).contains(&level), "{level}");
        assert_eq!(radiation_tooltip(level, 1)[1], "\u{a7}e6.189RAD/s");
        // A spent balefire rod: 1.2e8 RAD/s as Java's float prints it, no u32 ceiling.
        let bf = "ntm:rbmk_fuel_balefire";
        let spent = rod_stack(bf, |s, r| s.set_yield(r, 0.0));
        let level = radiation_level(&spent).unwrap();
        assert_eq!(radiation_tooltip(level, 1)[1], format!("\u{a7}e{}RAD/s", crate::rbmk::tooltip::java_double(f64::from(level * 1000.0).floor() / 1000.0)));
        assert!(radiation_tooltip(level, 1)[1].contains("E8"), "{:?}", radiation_tooltip(level, 1));
        // Shown from 1e-5 up, hidden below; a stack's line past one item.
        assert!(radiation_tooltip(0.000_009, 1).is_empty());
        assert_eq!(radiation_tooltip(0.0002, 1)[1], "\u{a7}e0.0RAD/s");
        assert_eq!(radiation_tooltip(1.5, 3), ["\u{a7}a[Radioactive]", "\u{a7}e1.5RAD/s", "\u{a7}eStack: 4.5RAD/s"]);
        assert_eq!(radiation_tooltip(f32::NAN, 1)[1], "\u{a7}eNaNRAD/s");
    }

    #[test]
    fn fire_protection_takes_the_highest_piece_as_1_7_10() {
        // A rod at 400 °C: level 30, 600 ticks. No protection; one piece of IV; two of IV (the highest, not the sum).
        assert_eq!(fire_ticks(30, 0, -20), 600);
        assert_eq!(fire_ticks(30, 4, -20), 240);
        assert_eq!(fire_ticks(30, 2, -20), 420);
        // The longer of what is left and this.
        assert_eq!(fire_ticks(30, 4, 500), 500);
        assert_eq!(fire_ticks(1, 1, 0), 17);
    }

    #[test]
    fn a_rod_is_hot_from_its_hull() {
        let ueu = "ntm:rbmk_fuel_ueu";
        for (hull, level) in [(20.0, -8), (100.0, 0), (100.5, 1), (110.0, 1), (110.01, 2), (695.0, 60), (5000.0, 60)] {
            assert_eq!(hot_level(&rod_stack(ueu, |s, r| s.set_hull_heat(r, hull))), Some(level), "{hull}");
        }
        assert_eq!(hot_level(&Stack::new("ntm:rbmk_pellet_ueu", 1)), None);
    }
}

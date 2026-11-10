//! An RBMK rod's hazards from its stack's tag: `HazardModifierRBMKRadiation` (the radiation goes from the fresh rod's
//! towards its target as it depletes, plus Xe-135 by its poison) and `HazardModifierRBMKHot` (HOT from its hull's
//! heat). Pellets keep their fresh rate here: their grade and xenon live in metadata the port does not carry yet (T14).
use super::persist;
use crate::server::container_bridge::Stack;

/// `HazardModifierRBMKRadiation.modify` for a rod, in mRAD/s per item: `level + (target − level) · depletion` (float
/// endpoints, double interpolation, float result), `depletion = 1 − e` when linear else `1 − e²`, then
/// `+ xe135 · poisonLevel`. None: not an RBMK rod with a hazard.
pub fn radiation_rate(stack: &Stack) -> Option<u32> {
    let rod = persist::rod_of(&stack.id)?;
    let h = crate::radiation::rbmk_hazard(&stack.id)?;
    let m = h.rbmk?;
    let mut s = persist::stack_rod(stack);
    let e = s.enrichment(rod);
    let depletion = if m.linear { 1.0 - e } else { 1.0 - e.powi(2) };
    let xenon = s.poison_level(rod);
    let level = (f64::from(h.radiation_rad_s) + f64::from(m.target_rad_s - h.radiation_rad_s) * depletion) as f32;
    // `level += xe135 * xenon`: float times double, back to float.
    let level = (f64::from(level) + f64::from(m.xe135_rad_s) * xenon) as f32;
    // `if(level > 0)` contaminates: a NaN or negative level none (the cast's 0), an infinite one all it can.
    Some((level * 1000.0).round() as u32)
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
    fn a_rod_is_hot_from_its_hull() {
        let ueu = "ntm:rbmk_fuel_ueu";
        for (hull, level) in [(20.0, -8), (100.0, 0), (100.5, 1), (110.0, 1), (110.01, 2), (695.0, 60), (5000.0, 60)] {
            assert_eq!(hot_level(&rod_stack(ueu, |s, r| s.set_hull_heat(r, hull))), Some(level), "{hull}");
        }
        assert_eq!(hot_level(&Stack::new("ntm:rbmk_pellet_ueu", 1)), None);
    }
}

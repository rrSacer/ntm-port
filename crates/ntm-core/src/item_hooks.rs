//! Item-family contracts, independent of the native host. The use table is
//! `consumables`; the hazard and gas-mask rules (HazmatRegistry, ArmorUtil)
//! are here.
pub use crate::consumables::{
    Animation, Consume, Cure, Effect, Gate, Give, Hurt, Overflow, UseSpec, use_spec,
};

/// All stack hazards use one endpoint interpolation. Reject non-finite state
/// rather than allowing NaN to erase a hazard. Values outside the life range
/// clamp to their corresponding endpoint.
pub fn interpolate(fresh: f64, spent: f64, remaining: f64, maximum: f64) -> Option<f64> {
    if ![fresh, spent, remaining, maximum]
        .iter()
        .all(|x| x.is_finite())
        || maximum <= 0.0
        || fresh < 0.0
        || spent < 0.0
    {
        return None;
    }
    Some(spent + (fresh - spent) * (remaining / maximum).clamp(0.0, 1.0))
}

/// Shared Java HazardModifier interpolation: the endpoint difference is
/// evaluated in float, interpolation in double, result quantized in float.
/// The caller chooses its life curve/clamping (RTG and ZIRNOX differ).
pub fn hazard_rate(base: f32, target: f32, fraction: f64) -> Option<u32> {
    if !base.is_finite() || !target.is_finite() || !fraction.is_finite() {
        return None;
    }
    let level = (f64::from(base) + f64::from(target - base) * fraction) as f32;
    Some((level * 1000.0).round() as u32)
}

pub fn resistance(id: &str) -> f64 {
    match id {
        "ntm:gas_mask_m65" => 0.095,
        "ntm:hazmat_helmet" => 0.12,
        "ntm:hazmat_plate" => 0.24,
        "ntm:hazmat_legs" => 0.18,
        "ntm:hazmat_boots" => 0.06,
        // HazmatRegistry: red is 1.0 and grey 2.0 times the body fractions
        // (helmet 0.2, chest 0.4, legs 0.3, boots 0.1); yellow is 0.6.
        "ntm:hazmat_helmet_red" => 0.2,
        "ntm:hazmat_plate_red" => 0.4,
        "ntm:hazmat_legs_red" => 0.3,
        "ntm:hazmat_boots_red" => 0.1,
        "ntm:hazmat_helmet_grey" => 0.4,
        "ntm:hazmat_plate_grey" => 0.8,
        "ntm:hazmat_legs_grey" => 0.6,
        "ntm:hazmat_boots_grey" => 0.2,
        _ => 0.0,
    }
}
/// `10^(-HazmatRegistry.getResistance)`: the four pieces, and `+0.2F` while
/// the Rad-X potion is active.
pub fn dose_factor<'a>(equipment: impl IntoIterator<Item = &'a str>, radx: bool) -> f64 {
    let sum = equipment.into_iter().map(resistance).sum::<f64>() + if radx { f64::from(0.2_f32) } else { 0.0 };
    10.0f64.powf(-sum)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hazard {
    Gamma,
    CoarseParticles,
    FineParticles,
    LungGas,
    BlisteringGas,
    Bacteria,
    CarbonMonoxide,
}
/// `ArmorUtil.register`: the hazard classes each filter stops. The mask's own
/// blacklist (`mask_blacklist`) still prevails.
pub fn filter_classes(filter: &str) -> &'static [Hazard] {
    use Hazard::*;
    match filter {
        "ntm:gas_mask_filter" => &[CoarseParticles, FineParticles, LungGas, BlisteringGas, Bacteria],
        "ntm:gas_mask_filter_mono" => &[CoarseParticles, CarbonMonoxide],
        "ntm:gas_mask_filter_combo" => &[CoarseParticles, FineParticles, LungGas, BlisteringGas, Bacteria, CarbonMonoxide],
        "ntm:gas_mask_filter_rag" => &[CoarseParticles],
        "ntm:gas_mask_filter_piss" => &[CoarseParticles, LungGas],
        _ => &[],
    }
}

/// Whether an item is an `ItemFilter` (20000 wear, stack size 1).
pub fn is_filter(id: &str) -> bool {
    !filter_classes(id).is_empty()
}

/// The `IGasMask` helmets of the port and the hazards each one never
/// protects against (`getBlacklist`): `ArmorGasMask` and `ItemModGasmask`
/// refuse blistering gas, while the `ArmorHazmatMask` hoods (yellow, red and
/// grey) have "no restrictions". `None` when the item is no gas mask.
pub fn mask_blacklist(mask: &str) -> Option<&'static [Hazard]> {
    match mask {
        "ntm:gas_mask_m65" | "ntm:attachment_mask" => Some(&[Hazard::BlisteringGas]),
        "ntm:hazmat_helmet" | "ntm:hazmat_helmet_red" | "ntm:hazmat_helmet_grey" => Some(&[]),
        _ => None,
    }
}

/// Whether an item is an `IGasMask` that takes a filter.
pub fn is_gas_mask(id: &str) -> bool {
    mask_blacklist(id).is_some()
}

pub fn filter_protects(mask: &str, filter: &str, hazard: Hazard) -> bool {
    let Some(blacklist) = mask_blacklist(mask) else {
        return false;
    };
    !blacklist.contains(&hazard) && filter_classes(filter).contains(&hazard)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn life_endpoints_and_invalid_state() {
        assert_eq!(interpolate(20., 4., 100., 100.), Some(20.));
        assert_eq!(interpolate(20., 4., 25., 100.), Some(8.));
        assert_eq!(interpolate(20., 4., -1., 100.), Some(4.));
        assert_eq!(interpolate(20., 4., f64::NAN, 100.), None);
    }
    /// ArmorUtil.register, filter by filter.
    #[test]
    fn each_filter_stops_the_classes_the_java_registers() {
        use Hazard::*;
        let m65 = "ntm:gas_mask_m65";
        assert!(filter_protects(m65, "ntm:gas_mask_filter_mono", CarbonMonoxide));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_mono", LungGas));
        assert!(filter_protects(m65, "ntm:gas_mask_filter_combo", CarbonMonoxide));
        assert!(filter_protects(m65, "ntm:gas_mask_filter_combo", Bacteria));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_combo", BlisteringGas), "the M65 blacklists blistering gas");
        assert!(filter_protects(m65, "ntm:gas_mask_filter_rag", CoarseParticles));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_rag", FineParticles));
        assert!(filter_protects(m65, "ntm:gas_mask_filter_piss", LungGas));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_piss", FineParticles));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter", CarbonMonoxide));
        assert!(!filter_protects(m65, "ntm:stone", CoarseParticles));
        assert!(is_filter("ntm:gas_mask_filter_rag") && !is_filter("ntm:gas_mask_m65"));
    }

    /// `ArmorHazmatMask implements IGasMask` with an empty blacklist: the
    /// yellow, red and grey hoods take filters and, unlike the M65 and the
    /// attachment mask, let a filter stop blistering gas.
    #[test]
    fn the_hazmat_hoods_are_gas_masks_without_a_blacklist() {
        use Hazard::*;
        for hood in ["ntm:hazmat_helmet", "ntm:hazmat_helmet_red", "ntm:hazmat_helmet_grey"] {
            assert!(is_gas_mask(hood), "{hood}");
            assert!(filter_protects(hood, "ntm:gas_mask_filter", BlisteringGas), "{hood}");
            assert!(filter_protects(hood, "ntm:gas_mask_filter_combo", BlisteringGas), "{hood}");
            assert!(filter_protects(hood, "ntm:gas_mask_filter_rag", CoarseParticles), "{hood}");
            assert!(!filter_protects(hood, "ntm:gas_mask_filter_rag", LungGas), "{hood}");
        }
        for mask in ["ntm:gas_mask_m65", "ntm:attachment_mask"] {
            assert!(is_gas_mask(mask) && !filter_protects(mask, "ntm:gas_mask_filter", BlisteringGas));
        }
        assert!(!is_gas_mask("ntm:hazmat_plate") && !is_gas_mask("ntm:stone"));
        assert!(!filter_protects("ntm:hazmat_plate", "ntm:gas_mask_filter", CoarseParticles));
    }

    #[test]
    fn java_protection_has_distinct_gamma_and_filter_paths() {
        assert!((dose_factor(["ntm:hazmat_plate"], false) - 0.5754399373371569).abs() < 1e-10);
        assert!((dose_factor(["ntm:gas_mask_m65"], false) - 0.803526).abs() < 1e-5);
        assert!(filter_protects(
            "ntm:gas_mask_m65",
            "ntm:gas_mask_filter",
            Hazard::LungGas
        ));
        assert!(!filter_protects(
            "ntm:gas_mask_m65",
            "ntm:gas_mask_filter",
            Hazard::Gamma
        ));
        assert!(!filter_protects(
            "ntm:gas_mask_m65",
            "ntm:gas_mask_filter",
            Hazard::BlisteringGas
        ));
    }
}

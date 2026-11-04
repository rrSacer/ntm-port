//! Item-family contracts, independent of the native host. The use table is
//! `consumables`; the hazard and gas-mask rules (HazmatRegistry, ArmorUtil)
//! are here.
pub use crate::consumables::{Animation, Consume, Effect, Gate, Give, Overflow, UseSpec, use_spec};

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
        _ => 0.0,
    }
}
pub fn dose_factor<'a>(equipment: impl IntoIterator<Item = &'a str>) -> f64 {
    10.0f64.powf(-equipment.into_iter().map(resistance).sum::<f64>())
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
pub fn filter_protects(mask: &str, filter: &str, hazard: Hazard) -> bool {
    if !matches!(mask, "ntm:gas_mask_m65" | "ntm:attachment_mask")
        || hazard == Hazard::BlisteringGas
    {
        return false;
    }
    match filter {
        "ntm:gas_mask_filter" => matches!(
            hazard,
            Hazard::CoarseParticles | Hazard::FineParticles | Hazard::LungGas | Hazard::Bacteria
        ),
        _ => false,
    }
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
    #[test]
    fn java_protection_has_distinct_gamma_and_filter_paths() {
        assert!((dose_factor(["ntm:hazmat_plate"]) - 0.5754399373371569).abs() < 1e-10);
        assert!((dose_factor(["ntm:gas_mask_m65"]) - 0.803526).abs() < 1e-5);
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

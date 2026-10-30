//! Radiation as a player's client shows it (stage L4, client side): the
//! Geiger box and the radioactive items' tooltip lines. The server keeps
//! each player's numbers with `native_set_player_value`; the client reads them
//! (`KEY_DOSE`, `KEY_RATE`) and asks this module for the lines to draw.
//! Tooltips follow NTM's `HazardTypeRadiation.addHazardInformation`.
use crate::radiation::{self, Reading};
use std::collections::HashMap;
use std::sync::OnceLock;

/// A player's accumulated dose, in milliRAD.
pub const KEY_DOSE: &str = "ntm_rad_dose";
/// What the Geiger counter reads for the player, in milliRAD/s: the inventory's
/// rate plus the zone's, as the server worked it out.
pub const KEY_RATE: &str = "ntm_rad_rate";

/// A radioactive item's rate, in milliRAD/s per item.
pub fn item_rate(id: &str) -> Option<u32> {
    static RATES: OnceLock<HashMap<&'static str, u32>> = OnceLock::new();
    RATES
        .get_or_init(|| {
            radiation::hazards()
                .iter()
                .map(|h| (h.id.as_str(), h.radiation_millirad_s))
                .collect()
        })
        .get(id)
        .copied()
}

/// `1.0`, `0.175`, `22.4`: milli-units as Java prints `floor(x * 1000) / 1000`.
fn rad(millirad: u64) -> String {
    let (whole, frac) = (millirad / 1000, millirad % 1000);
    if frac == 0 {
        return format!("{whole}.0");
    }
    format!("{whole}.{frac:03}")
        .trim_end_matches('0')
        .to_owned()
}

/// The lines (with `§` codes) NTM adds to a radioactive item's tooltip: the
/// trait, the rate, and the stack's total when it holds more than one.
/// Empty for an item that is not radioactive.
pub fn tooltip_lines(id: &str, count: u32) -> Vec<String> {
    let Some(rate) = item_rate(id).filter(|r| *r > 0) else {
        return Vec::new();
    };
    let mut lines = vec![
        "\u{a7}a[Radioactive]".to_owned(),
        format!("\u{a7}e{}RAD/s", rad(u64::from(rate))),
    ];
    if count > 1 {
        lines.push(format!(
            "\u{a7}eStack: {}RAD/s",
            rad(u64::from(rate) * u64::from(count))
        ));
    }
    lines
}

/// The Geiger box's lines and their colour, or None until the server has
/// sent the player a dose.
pub fn geiger(dose: Option<i64>, rate: Option<i64>) -> Option<(Vec<String>, u32)> {
    let dose = dose?;
    let reading = Reading {
        dose_millirad: dose.clamp(0, radiation::MAX_DOSE as i64) as u64,
        rate_millirad_s: rate.unwrap_or(0).clamp(0, i64::from(u32::MAX)) as u32,
        status: String::new(),
    };
    Some((reading.lines(), reading.color()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_print_as_java_prints_them() {
        assert_eq!(rad(175), "0.175");
        assert_eq!(rad(1000), "1.0");
        assert_eq!(rad(22_400), "22.4");
        assert_eq!(rad(350), "0.35");
        assert_eq!(rad(5), "0.005");
        assert_eq!(rad(16_777_215), "16777.215");
    }

    #[test]
    fn a_radioactive_item_lists_its_rate_and_its_stack() {
        let ingot = tooltip_lines("ntm:billet_uranium", 1);
        assert_eq!(ingot, ["\u{a7}a[Radioactive]", "\u{a7}e0.175RAD/s"]);
        let stack = tooltip_lines("ntm:billet_uranium", 64);
        assert_eq!(stack[2], "\u{a7}eStack: 11.2RAD/s");
        assert!(tooltip_lines("ntm:billet_beryllium", 4).is_empty());
        assert!(tooltip_lines("minecraft:stone", 4).is_empty());
    }

    #[test]
    fn the_geiger_waits_for_a_dose_and_colours_it() {
        assert_eq!(geiger(None, Some(10)), None);
        let (lines, color) = geiger(Some(12_340), Some(1_819)).unwrap();
        assert_eq!(lines, ["NTM Geiger | 1.819 RAD/s", "Dose: 12.34 RAD"]);
        assert_eq!(color, 0x88EE88);
        assert_eq!(geiger(Some(500_000), None).unwrap().1, 0xFFAA00);
        assert_eq!(
            geiger(Some(9_999_999), Some(-5)).unwrap().0[1],
            "Dose: 2500.00 RAD"
        );
    }
}

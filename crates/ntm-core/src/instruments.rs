//! Hand instruments (Geiger counter, dosimeter, diagnostic, survey scanner,
//! pipettes) and `ItemHot`, independent of the native host. The numbers are
//! from `ItemGeigerCounter`, `ItemDosimeter`, `ContaminationUtil.print*`,
//! `ItemSurveyScanner`, `ItemPipette` and `ItemHot`.

/// `ContaminationUtil.getPreffixFromRad`, as legacy colour codes.
pub fn rad_prefix(rads: f64) -> &'static str {
    if rads == 0.0 {
        "§a"
    } else if rads < 1.0 {
        "§e"
    } else if rads < 10.0 {
        "§6"
    } else if rads < 100.0 {
        "§c"
    } else if rads < 1000.0 {
        "§4"
    } else {
        "§8"
    }
}

/// A Java `double` printed in a string: the shortest form, with a `.0` when whole.
pub fn java_f64(x: f64) -> String {
    let s = format!("{x}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
}

/// `((int)(x * 10)) / 10D`.
pub fn tenth(x: f64) -> f64 {
    (x * 10.0).trunc() / 10.0
}

/// The player's own contamination colour in `printGeigerData`.
fn dose_prefix(rad: f64) -> &'static str {
    if rad < 200.0 {
        "§a"
    } else if rad < 400.0 {
        "§e"
    } else if rad < 600.0 {
        "§6"
    } else if rad < 800.0 {
        "§c"
    } else if rad < 1000.0 {
        "§4"
    } else {
        "§8"
    }
}

/// What the readings are, in the units the Java prints.
pub struct Readings {
    /// The player's dose, milliRAD.
    pub dose_millirad: u64,
    /// The fallout zones' rate where the player stands, milliRAD/s.
    pub zone_millirad_s: u32,
    /// `radBuf`: the exposure of the last second, milliRAD/s.
    pub buffer_millirad_s: i64,
    /// `HazmatRegistry.getResistance(player)`, the float exponent of `10^(-r)`.
    pub resistance: f32,
}

/// The five lines `printGeigerData` sends.
pub fn geiger_lines(r: &Readings) -> Vec<String> {
    let e_rad = tenth(r.dose_millirad as f64 / 1000.0);
    let rads = tenth(f64::from(r.zone_millirad_s) / 1000.0);
    let env = tenth(r.buffer_millirad_s as f64 / 1000.0);
    // `calculateRadiationMod` is a float, widened here as in the Java.
    let factor = f64::from(crate::item_hooks::radiation_mod(r.resistance));
    let res = ((10000.0 - factor * 10000.0) as i64) as f64 / 100.0;
    let koeff = ((f64::from(r.resistance) * 100.0) as i64) as f64 / 100.0;
    let res_prefix = if koeff > 0.0 { "§f§a" } else { "§f" };
    vec![
        "§6===== ☢ GEIGER COUNTER ☢ =====".into(),
        format!("§eCurrent chunk radiation: {}{} RAD/s", rad_prefix(rads), java_f64(rads)),
        format!("§eTotal environmental radiation: {}{} RAD/s", rad_prefix(env), java_f64(env)),
        format!("§ePlayer contamination: {}{} RAD", dose_prefix(e_rad), java_f64(e_rad)),
        format!("§ePlayer resistance: {res_prefix}{}% ({})", java_f64(res), java_f64(koeff)),
    ]
}

/// The two lines `printDosimeterData` sends: the reading caps at 3.6.
pub fn dosimeter_lines(buffer_millirad_s: i64) -> Vec<String> {
    let mut env = tenth(buffer_millirad_s as f64 / 1000.0);
    let limit = env > 3.6;
    if limit {
        env = 3.6;
    }
    vec![
        "§6===== ☢ DOSIMETER ☢ =====".into(),
        format!("§eTotal environmental radiation: {}{}{} RAD/s", rad_prefix(env), if limit { ">" } else { "" }, java_f64(env)),
    ]
}

/// `printDiagnosticData` with the digamma the port has: none.
pub fn diagnostic_lines(digamma: f64) -> Vec<String> {
    let d = (digamma * 100.0).trunc() / 100.0;
    let half = ((1.0 - 0.5f64.powf(d)) * 10000.0).trunc() / 100.0;
    vec![
        "§5===== Ϝ DIGAMMA DIAGNOSTIC Ϝ =====".into(),
        format!("§dDigamma exposure: §c {} DRX", java_f64(d)),
        format!("§dDigamma influence: §c {}%", java_f64(half)),
        "§dDigamma resistance: §9 N/A".into(),
    ]
}

/// The clicks `ItemGeigerCounter.onUpdate` may pick from for a buffer of `x`
/// RAD/s (0 is silent), on a tick that is a multiple of 5.
pub fn geiger_pool(x: f32) -> Vec<u8> {
    let mut list = Vec::new();
    if x < 1.0 {
        list.push(0);
    }
    if x < 5.0 {
        list.push(0);
    }
    if x < 10.0 {
        list.push(1);
    }
    if x > 5.0 && x < 15.0 {
        list.push(2);
    }
    if x > 10.0 && x < 20.0 {
        list.push(3);
    }
    if x > 15.0 && x < 25.0 {
        list.push(4);
    }
    if x > 20.0 && x < 30.0 {
        list.push(5);
    }
    if x > 25.0 {
        list.push(6);
    }
    list
}

/// The same for `ItemDosimeter.onUpdate`.
pub fn dosimeter_pool(x: f32) -> Vec<u8> {
    let mut list = Vec::new();
    if x < 0.5 {
        list.push(0);
    }
    if x < 1.0 {
        list.push(1);
    }
    if (0.5..2.0).contains(&x) {
        list.push(2);
    }
    // The Java writes `x >= 1 && x >= 2`, which is `x >= 2`.
    if x >= 2.0 {
        list.push(3);
    }
    list
}

/// Which click to play for a buffer and two rolls in `[0, 1)`: one to pick
/// from the pool, one for the `nextInt(n) == 0` idle click. `None` is silence.
pub fn click(pool: fn(f32) -> Vec<u8>, idle_one_in: u32, x: f32, pick: f32, idle: f32) -> Option<u8> {
    if x > 1e-5 {
        let list = pool(x);
        let r = list[((pick * list.len() as f32) as usize).min(list.len() - 1)];
        (r > 0).then_some(r)
    } else if (idle * idle_one_in as f32) as u32 == 0 {
        Some(1)
    } else {
        None
    }
}

/// `ItemPipette`: the fill each kind reaches and the step of its capacity.
pub fn pipette_max(id: &str) -> i32 {
    if id == "ntm:pipette_laboratory" { 50 } else { 1_000 }
}

/// A right click on an empty pipette changes its capacity: laboratory by 1
/// within 1..=50, the others by 50 within 50..=1000; sneaking lowers it.
pub fn pipette_capacity(id: &str, capacity: i32, sneaking: bool) -> i32 {
    if id == "ntm:pipette_laboratory" {
        if sneaking { (capacity - 1).max(1) } else { (capacity + 1).min(50) }
    } else if sneaking {
        (capacity - 50).max(50)
    } else {
        (capacity + 50).min(1_000)
    }
}

/// `ItemHot`: the items that carry a `heat` tag.
pub fn is_hot(id: &str) -> bool {
    matches!(
        id,
        "ntm:ingot_meteorite" | "ntm:ingot_meteorite_forged" | "ntm:blade_meteorite" | "ntm:ingot_steel_dusted" | "ntm:ingot_chainsteel"
    )
}

/// `ItemHot.onUpdate` on a tag with `heat` `h`: one less per tick, and the
/// whole tag is dropped once it is not above zero. `None` drops the tag.
pub fn cool(h: i32) -> Option<i32> {
    (h > 0).then_some(h - 1)
}

/// `ItemSurveyScanner`'s messages for the blocks it found, with their colours,
/// in the order of its `addChatComponentMessage` calls.
pub fn survey_messages(oil: bool, bedrock_oil: bool, coltan: bool, depth: bool, schist: bool, australium: bool) -> Vec<&'static str> {
    let mut out = Vec::new();
    if oil {
        out.push("§0Found OIL!");
    }
    if bedrock_oil {
        out.push("§0Found BEDROCK OIL!");
    }
    if coltan {
        out.push("§6Found COLTAN!");
    }
    if depth {
        out.push("§7Found DEPTH ROCK!");
    }
    if schist {
        out.push("§3Found SCHIST!");
    }
    if australium {
        out.push("§eFound AUSTRALIUM!");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_follow_the_java_thresholds() {
        assert_eq!(rad_prefix(0.0), "§a");
        assert_eq!(rad_prefix(0.5), "§e");
        assert_eq!(rad_prefix(1.0), "§6");
        assert_eq!(rad_prefix(99.9), "§c");
        assert_eq!(rad_prefix(100.0), "§4");
        assert_eq!(rad_prefix(1000.0), "§8");
        assert_eq!(tenth(12.99), 12.9);
    }

    #[test]
    fn the_geiger_report_has_the_five_java_lines() {
        let lines = geiger_lines(&Readings { dose_millirad: 250_000, zone_millirad_s: 1_500, buffer_millirad_s: 12_345, resistance: 0.095 });
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[1], "§eCurrent chunk radiation: §61.5 RAD/s");
        assert_eq!(lines[2], "§eTotal environmental radiation: §c12.3 RAD/s");
        assert_eq!(lines[3], "§ePlayer contamination: §e250.0 RAD");
        // 10^(-0.095) = 0.8035: 19.64% of the dose is stopped, resistance 0.09 (truncated).
        assert_eq!(lines[4], "§ePlayer resistance: §f§a19.64% (0.09)");
        let line = |resistance| geiger_lines(&Readings { dose_millirad: 0, zone_millirad_s: 0, buffer_millirad_s: 0, resistance })[4].clone();
        // Full grey hazmat: 10^-2 as a float is 0.00999999977, 99% stopped.
        assert_eq!(line(2.0), "§ePlayer resistance: §f§a99.0% (2.0)");
        // Full steel: the float sum 0.045000002 gives 9.84% and (int)(4.5000002) = 4.
        assert_eq!(line(0.045), "§ePlayer resistance: §f§a9.84% (0.04)");
        assert_eq!(line(0.0), "§ePlayer resistance: §f0.0% (0.0)");
    }

    #[test]
    fn the_dosimeter_caps_at_three_point_six() {
        assert_eq!(dosimeter_lines(2_000)[1], "§eTotal environmental radiation: §62.0 RAD/s");
        assert!(dosimeter_lines(9_000)[1].contains(">3.6 RAD/s"));
    }

    #[test]
    fn the_diagnostic_prints_zero_digamma_in_the_port() {
        let lines = diagnostic_lines(0.0);
        assert_eq!(lines[1], "§dDigamma exposure: §c 0.0 DRX");
        assert_eq!(lines[2], "§dDigamma influence: §c 0.0%");
    }

    #[test]
    fn click_pools_follow_the_java_lists() {
        assert_eq!(geiger_pool(0.5), [0, 0, 1]);
        assert_eq!(geiger_pool(7.0), [1, 2]);
        assert_eq!(geiger_pool(12.0), [2, 3]);
        assert_eq!(geiger_pool(40.0), [6]);
        assert_eq!(dosimeter_pool(0.2), [0, 1]);
        assert_eq!(dosimeter_pool(1.5), [2]);
        assert_eq!(dosimeter_pool(3.0), [3]);
        // Radiation: a click from the pool; none: the idle one in 50 (100).
        assert_eq!(click(geiger_pool, 50, 7.0, 0.0, 0.5), Some(1));
        assert_eq!(click(geiger_pool, 50, 0.5, 0.1, 0.5), None, "pool entry 0 is silent");
        assert_eq!(click(geiger_pool, 50, 0.0, 0.0, 0.01), Some(1));
        assert_eq!(click(geiger_pool, 50, 0.0, 0.0, 0.5), None);
        assert_eq!(click(dosimeter_pool, 100, 0.0, 0.0, 0.005), Some(1));
    }

    #[test]
    fn the_pipette_capacity_steps() {
        assert_eq!(pipette_max("ntm:pipette"), 1000);
        assert_eq!(pipette_max("ntm:pipette_laboratory"), 50);
        assert_eq!(pipette_capacity("ntm:pipette", 1000, false), 1000);
        assert_eq!(pipette_capacity("ntm:pipette", 1000, true), 950);
        assert_eq!(pipette_capacity("ntm:pipette_boron", 50, true), 50);
        assert_eq!(pipette_capacity("ntm:pipette_laboratory", 50, false), 50);
        assert_eq!(pipette_capacity("ntm:pipette_laboratory", 1, true), 1);
        assert_eq!(pipette_capacity("ntm:pipette_laboratory", 10, true), 9);
    }

    #[test]
    fn hot_items_cool_one_per_tick_and_lose_their_tag() {
        assert!(is_hot("ntm:ingot_chainsteel") && !is_hot("ntm:ingot_steel"));
        assert_eq!(cool(200), Some(199));
        assert_eq!(cool(1), Some(0));
        assert_eq!(cool(0), None);
    }

    #[test]
    fn survey_messages_keep_the_java_order() {
        assert_eq!(survey_messages(true, false, true, false, false, true), ["§0Found OIL!", "§6Found COLTAN!", "§eFound AUSTRALIUM!"]);
        assert!(survey_messages(false, false, false, false, false, false).is_empty());
    }
}

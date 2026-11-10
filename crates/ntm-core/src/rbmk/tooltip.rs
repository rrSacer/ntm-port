//! `ItemRBMKRod.addInformation`: a rod's tooltip, line by line, as the keys the Java resolves (`I18nUtil.resolveKey`)
//! with their arguments, and the legacy colour codes before them (`EnumChatFormatting`). The client translates the
//! keys; the numbers are written here as Java writes them (`Double.toString`, the `(int)` truncations).
use super::rods::{BurnFunc, NType, RodStack, RodType};

/// One tooltip line: `style` (legacy codes) then the key translated with `args` (themselves legacy text), or, with no
/// key, `args[0]` as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub style: &'static str,
    pub key: Option<String>,
    pub args: Vec<String>,
}

const ITALIC: &str = "§o";
const DARK_GREEN: &str = "§2";
const GREEN: &str = "§a";
const YELLOW: &str = "§e";
const GOLD: &str = "§6";
const RED: &str = "§c";
const DARK_RED: &str = "§4";
const DARK_PURPLE: &str = "§5";
const BLUE: &str = "§9";
const WHITE: &str = "§f";

/// `Double.toString`: the shortest digits, as a decimal from 10^-3 to 10^7, else as `d.dddE±n`.
pub fn java_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    if (1e-3..1e7).contains(&v.abs()) {
        let s = format!("{v:?}");
        return if s.contains('.') { s } else { format!("{s}.0") };
    }
    let s = format!("{v:e}");
    let (mantissa, exponent) = s.split_once('e').expect("an exponent");
    let mantissa = if mantissa.contains('.') { mantissa.to_string() } else { format!("{mantissa}.0") };
    format!("{mantissa}E{exponent}")
}

/// `(int) v / d` as Java computes it: the cast truncates toward zero, saturates, and makes NaN 0.
fn trunc_div(v: f64, d: f64) -> String {
    java_double(f64::from(v as i32) / d)
}

/// `EnumBurnFunc.title`.
pub fn func_title(f: BurnFunc) -> String {
    match f {
        BurnFunc::Passive => format!("{DARK_GREEN}SAFE / PASSIVE"),
        BurnFunc::LogTen => format!("{YELLOW}MEDIUM / LOGARITHMIC"),
        BurnFunc::Plateu => format!("{GREEN}SAFE / EULER"),
        BurnFunc::Arch => format!("{RED}DANGEROUS / NEGATIVE-QUADRATIC"),
        BurnFunc::Sigmoid => format!("{GREEN}SAFE / SIGMOID"),
        BurnFunc::SquareRoot => format!("{YELLOW}MEDIUM / SQUARE ROOT"),
        BurnFunc::Linear => format!("{RED}DANGEROUS / LINEAR"),
        BurnFunc::Quadratic => format!("{RED}DANGEROUS / QUADRATIC"),
        BurnFunc::Experimental => format!("{RED}EXPERIMENTAL / SINE SLOPE"),
    }
}

/// `NType.unlocalized`.
fn neutron(n: NType) -> &'static str {
    match n {
        NType::Fast => "trait.rbmk.neutron.fast",
        NType::Slow => "trait.rbmk.neutron.slow",
        NType::Any => "trait.rbmk.neutron.any",
    }
}

/// `getFuncDescription`: the burn function with the flux (`%1$s`) and the reactivity (`%2$s`) filled in, the
/// enrichment's modifier and share appended while the rod is not full.
pub fn func_description(rod: &RodType, s: &mut RodStack) -> String {
    let function = match rod.function {
        BurnFunc::Passive => format!("{RED}{}", java_double(rod.self_rate)),
        BurnFunc::LogTen => "log10(%1$s + 1) * 0.5 * %2$s".into(),
        BurnFunc::Plateu => "(1 - e^(-%1$s / 25)) * %2$s".into(),
        BurnFunc::Arch => "(%1$s - %1$s² / 10000) / 100 * %2$s [0;∞]".into(),
        BurnFunc::Sigmoid => "%2$s / (1 + e^(-(%1$s - 50) / 10))".into(),
        BurnFunc::SquareRoot => "sqrt(%1$s) * %2$s / 10".into(),
        BurnFunc::Linear => "%1$s / 100 * %2$s".into(),
        BurnFunc::Quadratic => "%1$s² / 10000 * %2$s".into(),
        BurnFunc::Experimental => "%1$s * (sin(%1$s) + 1) * %2$s".into(),
    };
    let enrichment = s.enrichment(rod);
    let mut flux = if rod.self_rate > 0.0 { format!("(x{RED} + {}{WHITE})", java_double(rod.self_rate)) } else { "x".into() };
    let mut tail = String::new();
    if enrichment < 1.0 {
        let e = rod.reactivity_mod_by_enrichment(enrichment);
        flux = format!("({flux} * {YELLOW}{}{WHITE})", trunc_div(e * 1000.0, 1000.0));
        tail = format!("{GOLD} ({}%)", trunc_div(e * 1000.0, 10.0));
    }
    // `String.format(Locale.US, function, flux, reactivity)`: only the positional `%1$s` and `%2$s` appear.
    let filled = function.replace("%1$s", &flux).replace("%2$s", &java_double(rod.reactivity));
    filled + &tail
}

/// The rod's tooltip (`addInformation`, before `super`'s lines): the DRX names its traits `trait.rbmx.*`, its neutrons
/// `….x` and its temperatures in `m`.
pub fn lines(rod: &RodType, s: &mut RodStack) -> Vec<Line> {
    let drx = rod.variable == "rbmk_fuel_drx";
    let (t, x, degrees) = if drx { ("trait.rbmx", ".x", "m") } else { ("trait.rbmk", "", "°C") };
    let key = |k: &str| Some(format!("{t}.{k}"));
    let line = |style: &'static str, key: Option<String>, arg: String| Line { style, key, args: vec![arg] };
    let mut out = vec![Line { style: ITALIC, key: None, args: vec![rod.full_name.to_string()] }];
    if s.hull_heat(rod) >= 50.0 || s.core_heat(rod) >= 50.0 {
        out.push(Line { style: GOLD, key: Some("desc.item.wasteCooling".into()), args: Vec::new() });
    }
    if rod.self_rate > 0.0 || rod.function == BurnFunc::Sigmoid {
        out.push(Line { style: RED, key: key("source"), args: Vec::new() });
    }
    let depletion = (rod.yield_ - s.yield_(rod)) / rod.yield_ * 100000.0;
    out.push(line(GREEN, key("depletion"), format!("{}%", trunc_div(depletion, 1000.0))));
    out.push(line(DARK_PURPLE, key("xenon"), format!("{}%", trunc_div(s.poison(rod) * 1000.0, 1000.0))));
    // The neutron types are keys inside a key: the client translates `args` that are keys (`§key:`).
    out.push(line(BLUE, key("splitsWith"), format!("§key:{}{x}", neutron(rod.n_type))));
    out.push(line(BLUE, key("splitsInto"), format!("§key:{}{x}", neutron(rod.r_type))));
    out.push(line(YELLOW, key("fluxFunc"), format!("{WHITE}{}", func_description(rod, s))));
    out.push(line(YELLOW, key("funcType"), func_title(rod.function)));
    out.push(line(YELLOW, key("xenonGen"), format!("{WHITE}x * {}", java_double(rod.x_gen))));
    out.push(line(YELLOW, key("xenonBurn"), format!("{WHITE}x² / {}", java_double(rod.x_burn))));
    out.push(line(GOLD, key("heat"), format!("{}°C", java_double(rod.heat))));
    out.push(line(GOLD, key("diffusion"), format!("{}¹/²", java_double(rod.diffusion))));
    out.push(line(RED, key("skinTemp"), format!("{}{degrees}", trunc_div(s.hull_heat(rod) * 10.0, 10.0))));
    out.push(line(RED, key("coreTemp"), format!("{}{degrees}", trunc_div(s.core_heat(rod) * 10.0, 10.0))));
    out.push(line(DARK_RED, key("melt"), format!("{}{degrees}", java_double(rod.melting_point))));
    out
}

/// `Math.round(double)`: `floor(v + 0.5)`.
fn java_round(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// The bar under the rod's icon (`showDurabilityBar`/`getDurabilityForDisplay`, drawn as Forge 1.7.10's
/// `RenderItem.renderItemOverlayIntoGUI`): with `health = 1 − enrichment`, shown while `health > 0`, its width
/// `round(13 − health·13)` and its colour `(255 − k) << 16 | k << 8` with `k = round(255 − health·255)`. None: no bar.
pub fn durability_bar(rod: &RodType, s: &mut RodStack) -> Option<(i32, u32)> {
    let health = s.durability_for_display(rod);
    // `showDurabilityBar`: `health > 0`, so a NaN shows no bar.
    if health.is_nan() || health <= 0.0 {
        return None;
    }
    let width = java_round(13.0 - health * 13.0);
    let k = java_round(255.0 - health * 255.0);
    Some((width, (((255 - k) << 16) | (k << 8)) as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_is_forges_for_1_minus_the_enrichment() {
        let rod = RodType::by_variable("rbmk_fuel_ueu").unwrap();
        // Fresh (health 0): no bar. A dead rod (health 1): an empty red bar.
        assert_eq!(durability_bar(rod, &mut RodStack::fresh(rod)), None);
        let mut dead = RodStack::fresh(rod);
        dead.set_yield(rod, 0.0);
        assert_eq!(durability_bar(rod, &mut dead), Some((0, 0xff0000)));
        // Half burnt: health 0.5, width round(6.5) = 7, k = round(127.5) = 128.
        let mut half = RodStack::fresh(rod);
        half.set_yield(rod, rod.yield_ / 2.0);
        assert_eq!(durability_bar(rod, &mut half), Some((7, (127 << 16) | (128 << 8))));
        // A renamed rod reads a yield of 0 (its tag has no key): dead, as in Java.
        let mut renamed = RodStack { has_tag: true, ..RodStack::bare() };
        assert_eq!(durability_bar(rod, &mut renamed), Some((0, 0xff0000)));
    }

    #[test]
    fn doubles_read_as_javas_double_to_string() {
        for (v, java) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (0.5, "0.5"),
            (50.0, "50.0"),
            (0.001, "0.001"),
            (0.0001, "1.0E-4"),
            (1234567.0, "1234567.0"),
            (1.0e7, "1.0E7"),
            (2.5e-5, "2.5E-5"),
            (123456789.0, "1.23456789E8"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (0.1 + 0.2, "0.30000000000000004"),
        ] {
            assert_eq!(java_double(v), java, "{v}");
        }
        // `(int)` saturates and makes NaN 0.
        assert_eq!(trunc_div(f64::NAN, 1000.0), "0.0");
        assert_eq!(trunc_div(1e300, 1.0), "2.147483647E9");
        assert_eq!(trunc_div(-12345.9, 1000.0), "-12.345");
    }

    #[test]
    fn a_fresh_rod_reads_as_the_java_tooltip() {
        let rod = RodType::by_variable("rbmk_fuel_ueu").unwrap();
        let mut s = RodStack::bare();
        let l = lines(rod, &mut s);
        // Fresh: no cooling hint (20 °C), no self-ignition (LOG_TEN, no self rate), 0 % depleted.
        assert_eq!(l[0], Line { style: ITALIC, key: None, args: vec![rod.full_name.into()] });
        assert_eq!(l[1].key.as_deref(), Some("trait.rbmk.depletion"));
        assert_eq!(l[1].args, vec!["0.0%".to_string()]);
        assert_eq!(l[2].args, vec!["0.0%".to_string()]);
        assert_eq!(l[3].args, vec!["§key:trait.rbmk.neutron.slow".to_string()]);
        let flux = &l[5];
        assert_eq!(flux.key.as_deref(), Some("trait.rbmk.fluxFunc"));
        assert_eq!(flux.args, vec![format!("§flog10(x + 1) * 0.5 * {}", java_double(rod.reactivity))]);
        assert_eq!(l.last().unwrap().args, vec![format!("{}°C", java_double(rod.melting_point))]);
        assert_eq!(l[l.len() - 3].args, vec!["20.0°C".to_string()]);
        assert_eq!(l.len(), 14);
    }

    #[test]
    fn a_burnt_hot_self_igniting_rod_shows_its_state() {
        let rod = RodType::by_variable("rbmk_fuel_po210be").unwrap();
        let mut s = RodStack::fresh(rod);
        s.set_yield(rod, rod.yield_ * 0.75);
        s.set_poison(rod, 12.34567);
        s.set_hull_heat(rod, 51.27);
        let l = lines(rod, &mut s);
        assert_eq!(l[1].key.as_deref(), Some("desc.item.wasteCooling"));
        assert_eq!(l[2].key.as_deref(), Some("trait.rbmk.source"), "self rate {}", rod.self_rate);
        assert_eq!(l[3].args, vec!["25.0%".to_string()]);
        assert_eq!(l[4].args, vec!["12.345%".to_string()]);
        let e = rod.reactivity_mod_by_enrichment(0.75);
        let description = func_description(rod, &mut s);
        assert!(description.ends_with(&format!("§6 ({}%)", trunc_div(e * 1000.0, 10.0))), "{description}");
        assert!(l.iter().any(|x| x.args == vec!["51.2°C".to_string()]));
        // The DRX: its own keys, `.x` neutrons and `m`.
        let drx = RodType::by_variable("rbmk_fuel_drx").unwrap();
        let l = lines(drx, &mut RodStack::bare());
        assert!(l.iter().skip(1).all(|x| x.key.as_deref().is_none_or(|k| k.starts_with("trait.rbmx.") || k == "desc.item.wasteCooling")));
        assert!(l.iter().any(|x| x.args.iter().any(|a| a.ends_with(".x"))));
        assert!(l.last().unwrap().args[0].ends_with('m'));
    }
}

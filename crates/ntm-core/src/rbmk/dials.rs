//! `RBMKDials`: the 25 game-rule dials, with the Java parsing and clamping.

use std::collections::BTreeMap;

/// Default of a dial, with the Java type it was declared with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DefaultValue {
    Bool(bool),
    Double(f64),
    Int(i32),
}

impl DefaultValue {
    /// `String.valueOf(defValue)`.
    pub fn java_string(self) -> String {
        match self {
            DefaultValue::Bool(b) => b.to_string(),
            DefaultValue::Double(d) => format!("{d:?}"),
            DefaultValue::Int(i) => i.to_string(),
        }
    }
}

/// One `RBMKKeys` entry: Java name, game-rule name and default.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub name: &'static str,
    pub rule: &'static str,
    pub default: DefaultValue,
}

const fn k(name: &'static str, rule: &'static str, default: DefaultValue) -> Key {
    Key {
        name,
        rule,
        default,
    }
}

/// The 25 keys, in `RBMKKeys` order.
pub const KEYS: [Key; 25] = [
    k("KEY_SAVE_DIALS", "dialSaveDials", DefaultValue::Bool(true)),
    k(
        "KEY_PASSIVE_COOLING",
        "dialPassiveCooling",
        DefaultValue::Double(2.5),
    ),
    k(
        "KEY_PASSIVE_COOLING_INNER",
        "dialPassiveCoolingInner",
        DefaultValue::Double(0.1),
    ),
    k(
        "KEY_COLUMN_HEAT_FLOW",
        "dialColumnHeatFlow",
        DefaultValue::Double(0.2),
    ),
    k(
        "KEY_FUEL_DIFFUSION_MOD",
        "dialDiffusionMod",
        DefaultValue::Double(1.0),
    ),
    k(
        "KEY_HEAT_PROVISION",
        "dialHeatProvision",
        DefaultValue::Double(0.2),
    ),
    k(
        "KEY_COLUMN_HEIGHT",
        "dialColumnHeight",
        DefaultValue::Int(4),
    ),
    k(
        "KEY_PERMANENT_SCRAP",
        "dialEnablePermaScrap",
        DefaultValue::Bool(true),
    ),
    k(
        "KEY_BOILER_HEAT_CONSUMPTION",
        "dialBoilerHeatConsumption",
        DefaultValue::Double(0.1),
    ),
    k(
        "KEY_CONTROL_SPEED_MOD",
        "dialControlSpeed",
        DefaultValue::Double(1.0),
    ),
    k(
        "KEY_REACTIVITY_MOD",
        "dialReactivityMod",
        DefaultValue::Double(1.0),
    ),
    k(
        "KEY_OUTGASSER_MOD",
        "dialOutgasserSpeedMod",
        DefaultValue::Double(1.0),
    ),
    k(
        "KEY_SURGE_MOD",
        "dialControlSurgeMod",
        DefaultValue::Double(1.0),
    ),
    k("KEY_FLUX_RANGE", "dialFluxRange", DefaultValue::Int(5)),
    k("KEY_REASIM_RANGE", "dialReasimRange", DefaultValue::Int(10)),
    k(
        "KEY_REASIM_BOILERS",
        "dialReasimBoilers",
        DefaultValue::Bool(false),
    ),
    k(
        "KEY_REASIM_BOILER_SPEED",
        "dialReasimBoilerSpeed",
        DefaultValue::Double(0.05),
    ),
    k(
        "KEY_DISABLE_MELTDOWNS",
        "dialDisableMeltdowns",
        DefaultValue::Bool(false),
    ),
    k(
        "KEY_ENABLE_MELTDOWN_OVERPRESSURE",
        "dialEnableMeltdownOverpressure",
        DefaultValue::Bool(false),
    ),
    k(
        "KEY_MODERATOR_EFFICIENCY",
        "dialModeratorEfficiency",
        DefaultValue::Double(1.0),
    ),
    k(
        "KEY_ABSORBER_EFFICIENCY",
        "dialAbsorberEfficiency",
        DefaultValue::Double(1.0),
    ),
    k(
        "KEY_REFLECTOR_EFFICIENCY",
        "dialReflectorEfficiency",
        DefaultValue::Double(1.0),
    ),
    k(
        "KEY_DISABLE_DEPLETION",
        "dialDisableDepletion",
        DefaultValue::Bool(false),
    ),
    k(
        "KEY_DISABLE_XENON",
        "dialDisableXenon",
        DefaultValue::Bool(false),
    ),
    k(
        "KEY_ABSORBER_HEAT_CONVERSION",
        "dialAbsorberHeatConversion",
        DefaultValue::Double(0.05),
    ),
];

pub fn key(name: &str) -> &'static Key {
    KEYS.iter()
        .find(|k| k.name == name)
        .expect("unknown RBMK dial key")
}

/// The world's string game rules (`GameRules`): a missing rule reads as `""`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct GameRules(BTreeMap<String, String>);

impl GameRules {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn set(&mut self, rule: &str, value: &str) {
        self.0.insert(rule.to_string(), value.to_string());
    }

    pub fn string(&self, rule: &str) -> &str {
        self.0.get(rule).map_or("", String::as_str)
    }

    /// `Boolean.parseBoolean`: only `true`, ignoring case.
    pub fn boolean(&self, rule: &str) -> bool {
        self.string(rule).eq_ignore_ascii_case("true")
    }
}

/// `Double.parseDouble` (decimal forms, `NaN`, `Infinity`, `d`/`f` suffix,
/// surrounding whitespace). Hexadecimal floating literals are not accepted
/// (they read as the default), a gap with no way to type them in a rule.
pub fn parse_double(s: &str) -> Option<f64> {
    let t = s.trim_matches(|c: char| c <= ' ');
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let v = match body {
        "NaN" => return Some(f64::NAN),
        "Infinity" => f64::INFINITY,
        _ => {
            if body.starts_with(['+', '-']) {
                return None;
            }
            let b = body.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(body);
            let ok = !b.is_empty()
                && b.bytes()
                    .all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
                && b.bytes().any(|c| c.is_ascii_digit());
            if !ok {
                return None;
            }
            b.parse::<f64>().ok()?
        }
    };
    Some(if neg { -v } else { v })
}

/// `Integer.parseInt`.
pub fn parse_int(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.strip_prefix('+').unwrap_or(s).parse::<i32>().ok()
}

fn clamp_d(v: f64, lo: f64, hi: f64) -> f64 {
    // MathHelper.clamp_double: NaN passes through.
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn clamp_i(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn double_of(r: &GameRules, key: &Key) -> f64 {
    let DefaultValue::Double(def) = key.default else {
        unreachable!()
    };
    parse_double(r.string(key.rule)).unwrap_or(def)
}

fn int_of(r: &GameRules, key: &Key) -> i32 {
    let DefaultValue::Int(def) = key.default else {
        unreachable!()
    };
    parse_int(r.string(key.rule)).unwrap_or(def)
}

/// `GameRuleHelper.getDoubleMinimum`: `Math.max` (NaN wins).
fn at_least(r: &GameRules, name: &str, min: f64) -> f64 {
    let v = double_of(r, key(name));
    if v.is_nan() {
        v
    } else {
        v.max(min)
    }
}

fn clamped(r: &GameRules, name: &str, lo: f64, hi: f64) -> f64 {
    clamp_d(double_of(r, key(name)), lo, hi)
}

/// The cache `RBMKDials.refresh` builds: the effective value of every dial.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dials {
    pub passive_cooling: f64,
    pub passive_cooling_inner: f64,
    pub column_heat_flow: f64,
    pub fuel_diffusion_mod: f64,
    pub heat_provision: f64,
    /// Already `clamp(rule, 2, 16) - 1`, as `getColumnHeight`.
    pub column_height: i32,
    pub permanent_scrap: bool,
    pub boiler_heat_consumption: f64,
    pub control_speed: f64,
    pub reactivity_mod: f64,
    pub outgasser_mod: f64,
    pub surge_mod: f64,
    pub flux_range: i32,
    pub reasim_range: i32,
    pub reasim_boilers: bool,
    pub reasim_boiler_speed: f64,
    pub meltdowns_disabled: bool,
    pub overpressure: bool,
    pub moderator_efficiency: f64,
    pub absorber_efficiency: f64,
    pub reflector_efficiency: f64,
    pub depletion_disabled: bool,
    pub xenon_disabled: bool,
    pub absorber_heat_conversion: f64,
}

impl Dials {
    /// `RBMKDials.refresh`; `config_528_reasim` is `GeneralConfig.enable528ReasimBoilers`.
    pub fn refresh(r: &GameRules, config_528_reasim: bool) -> Self {
        Dials {
            passive_cooling: at_least(r, "KEY_PASSIVE_COOLING", 0.0),
            passive_cooling_inner: at_least(r, "KEY_PASSIVE_COOLING_INNER", 0.0),
            column_heat_flow: clamped(r, "KEY_COLUMN_HEAT_FLOW", 0.0, 1.0),
            fuel_diffusion_mod: at_least(r, "KEY_FUEL_DIFFUSION_MOD", 0.0),
            heat_provision: clamped(r, "KEY_HEAT_PROVISION", 0.0, 1.0),
            column_height: clamp_i(int_of(r, key("KEY_COLUMN_HEIGHT")), 2, 16) - 1,
            permanent_scrap: r.boolean("dialEnablePermaScrap"),
            boiler_heat_consumption: at_least(r, "KEY_BOILER_HEAT_CONSUMPTION", 0.0),
            control_speed: at_least(r, "KEY_CONTROL_SPEED_MOD", 0.0),
            reactivity_mod: at_least(r, "KEY_REACTIVITY_MOD", 0.0),
            outgasser_mod: at_least(r, "KEY_OUTGASSER_MOD", 0.0),
            surge_mod: at_least(r, "KEY_SURGE_MOD", 0.0),
            flux_range: clamp_i(int_of(r, key("KEY_FLUX_RANGE")), 1, 100),
            reasim_range: clamp_i(int_of(r, key("KEY_REASIM_RANGE")), 1, 100),
            reasim_boilers: r.boolean("dialReasimBoilers") || config_528_reasim,
            reasim_boiler_speed: clamped(r, "KEY_REASIM_BOILER_SPEED", 0.0, 1.0),
            meltdowns_disabled: r.boolean("dialDisableMeltdowns"),
            overpressure: r.boolean("dialEnableMeltdownOverpressure"),
            moderator_efficiency: clamped(r, "KEY_MODERATOR_EFFICIENCY", 0.0, 1.0),
            absorber_efficiency: clamped(r, "KEY_ABSORBER_EFFICIENCY", 0.0, 1.0),
            reflector_efficiency: clamped(r, "KEY_REFLECTOR_EFFICIENCY", 0.0, 1.0),
            depletion_disabled: r.boolean("dialDisableDepletion"),
            xenon_disabled: r.boolean("dialDisableXenon"),
            absorber_heat_conversion: clamped(r, "KEY_ABSORBER_HEAT_CONVERSION", 0.0, 1.0),
        }
    }

    /// `getDepletion`.
    pub fn depletion(&self) -> bool {
        !self.depletion_disabled
    }

    /// `getXenon`.
    pub fn xenon(&self) -> bool {
        !self.xenon_disabled
    }

    /// `getSurgeMod` on the client reads the *passive cooling* rule (a Java
    /// slip kept on purpose: D2).
    pub fn surge_mod_client(r: &GameRules) -> f64 {
        at_least(r, "KEY_PASSIVE_COOLING", 0.0)
    }

    /// The server's cache is not available to the client: `getGameRule`
    /// returns the declared default.
    pub fn default_of(name: &str) -> DefaultValue {
        key(name).default
    }
}

/// The caches of `RBMKDials.gameRules`, one per `World`, by dimension (T04, decision 3A). The rules themselves are
/// the save's: in 1.7.10 a dimension's `World.getGameRules()` is the overworld's (`DerivedWorldInfo`), so every
/// dimension reads and writes the same rules, while each `World` keeps its own cache, built by `refresh(world)`.
/// The caches live in memory only, as Java's static map: the server's start makes the overworld's (`createDials`),
/// any other dimension's is made by its first read.
#[derive(Clone, Debug, Default)]
pub struct DialState {
    caches: BTreeMap<String, Dials>,
    pub config_528_reasim: bool,
}

impl DialState {
    /// `refresh(world)`: that world's cache from the rules as they are now; the other worlds keep theirs.
    pub fn refresh(&mut self, rules: &GameRules, dim: &str) {
        self.caches.insert(dim.to_string(), Dials::refresh(rules, self.config_528_reasim));
    }

    /// `createDials(world, force)`: every world's cache is emptied (`gameRules.put(key, new ArrayList<>())`), this
    /// world's is rebuilt from the rules *before* the defaults are written, so after a reset it keeps the old values
    /// until the next `refresh` (kept: D2).
    pub fn create(&mut self, rules: &mut GameRules, dim: &str, force: bool) {
        self.caches.clear();
        self.refresh(rules, dim);
        if !rules.boolean(KEYS[0].rule) || force {
            for key in &KEYS {
                rules.set(key.rule, &key.default.java_string());
            }
        }
    }

    /// `getGameRule(world, rule)` of the dial `name`: a world without a cache writes the default of the dial asked
    /// into the shared rules (the world of every dimension: the overworld's cache keeps its value until it refreshes)
    /// and refreshes that world (the `isIteration` retry, `RBMKDials:141-147`).
    pub fn read(&mut self, rules: &mut GameRules, dim: &str, name: &str) -> Dials {
        if !self.caches.contains_key(dim) {
            let key = key(name);
            rules.set(key.rule, &key.default.java_string());
            self.refresh(rules, dim);
        }
        self.caches[dim]
    }

    /// What [`DialState::read`] of `name` would give in `dim`, without reading (nothing is written).
    pub fn peek(&self, rules: &GameRules, dim: &str, name: &str) -> Dials {
        match self.caches.get(dim) {
            Some(d) => *d,
            None => {
                let mut r = rules.clone();
                r.set(key(name).rule, &key(name).default.java_string());
                Dials::refresh(&r, self.config_528_reasim)
            }
        }
    }

    pub fn cached(&self, dim: &str) -> Option<&Dials> {
        self.caches.get(dim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3A: one set of rules for the save, one cache per world. The Nether's first read writes the default of the dial
    /// it asks for into the shared rules and builds the Nether's cache from them; the overworld keeps its cache until
    /// it refreshes; `/gamerule` refreshes only the world of who typed it; `createDials` empties every cache.
    #[test]
    fn each_world_keeps_its_own_cache_over_the_shared_rules() {
        let (ow, nether) = ("minecraft:overworld", "minecraft:the_nether");
        let mut rules = GameRules::default();
        let mut d = DialState::default();
        d.create(&mut rules, ow, false);
        rules.set("dialFluxRange", "9");
        rules.set("dialColumnHeatFlow", "0.7");
        d.refresh(&rules, ow);
        assert_eq!((d.cached(ow).unwrap().flux_range, d.cached(ow).unwrap().column_heat_flow), (9, 0.7));
        // The Nether's first read is a fuel channel's burn (`getXenon`): only dialDisableXenon goes back to its default.
        rules.set("dialDisableXenon", "true");
        assert!(d.cached(nether).is_none());
        let peek = d.peek(&rules, nether, "KEY_DISABLE_XENON");
        let n = d.read(&mut rules, nether, "KEY_DISABLE_XENON");
        assert_eq!(peek, n, "peek is the read, without writing");
        assert_eq!((n.flux_range, n.column_heat_flow, n.xenon_disabled), (9, 0.7, false));
        assert_eq!(rules.string("dialDisableXenon"), "false", "written in the save's rules");
        // A second read in the Nether is the cache: no rule is written again.
        rules.set("dialDisableXenon", "true");
        assert!(!d.read(&mut rules, nether, "KEY_REASIM_BOILERS").xenon_disabled);
        assert_eq!(rules.string("dialDisableXenon"), "true");
        // `/gamerule` typed in the Nether refreshes the Nether only.
        rules.set("dialFluxRange", "3");
        d.refresh(&rules, nether);
        assert_eq!((d.cached(nether).unwrap().flux_range, d.cached(ow).unwrap().flux_range), (3, 9));
        assert!(d.cached(nether).unwrap().xenon_disabled && !d.cached(ow).unwrap().xenon_disabled);
        // `createDials` (the server's start) empties every world's cache and rebuilds the overworld's.
        d.create(&mut rules, ow, false);
        assert!(d.cached(nether).is_none());
        assert_eq!(d.cached(ow).unwrap().flux_range, 3);
    }
}

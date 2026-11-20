//! The RBMK columns as `IControlReceiver`s (T06): what their screens send them (`receiveControl`) and what they show
//! back (the TE's network pack, `serialize`), over the codec of [`crate::screens::control`]. T06 carries the automatic
//! control rod's; the other columns' screens come with their tickets (T07 manual control, T08 boiler, T10 console).

use crate::rbmk::persist::{self, AutoControl, RbmkColumn};
use crate::screens::control::{Fields, Refusal, Value};

/// Whether a column of `block` takes screen commands (T06: the automatic control rods).
pub fn takes_commands(block: &str) -> bool {
    persist::is_auto_control(block)
}

/// `TileEntityRBMKControlAuto.receiveControl`: `function` picks the curve (`abs(i) % RBMKColor.values().length`,
/// 5, into `RBMKFunction.values()`, 3: an index of 3 or 4, or a negative one from `abs(Integer.MIN_VALUE)`, throws in
/// the Java and is refused here, D2); without it the four doubles are set, a missing one to 0. Ok(true) when the
/// column changed.
pub fn receive(block: &str, column: &mut RbmkColumn, fields: &Fields) -> Result<bool, Refusal> {
    if !persist::is_auto_control(block) {
        return Err(Refusal::Invalid(format!("{block} takes no command")));
    }
    let before = column.auto.clone().unwrap_or_default();
    let mut next = before.clone();
    if fields.has_key("function") {
        let c = fields.get_integer("function").wrapping_abs() % 5;
        if !(0..3).contains(&c) {
            return Err(Refusal::Invalid(format!("RBMKFunction.values()[{c}]")));
        }
        next.function = c as u8;
    } else {
        next.level_lower = fields.get_double("levelLower");
        next.level_upper = fields.get_double("levelUpper");
        next.heat_lower = fields.get_double("heatLower");
        next.heat_upper = fields.get_double("heatUpper");
    }
    // By the bits: a NaN setting sent again is no change.
    let bits = |a: &AutoControl| (a.level_lower.to_bits(), a.level_upper.to_bits(), a.heat_lower.to_bits(), a.heat_upper.to_bits(), a.function);
    let changed = bits(&next) != bits(&before) || column.auto.is_none();
    column.auto = Some(next);
    Ok(changed)
}

/// What the screen of an automatic control rod reads (`TileEntityRBMKControl.serialize` then
/// `TileEntityRBMKControlAuto.serialize`): the level and its target (0 until T07 moves the rod), the power and whether
/// it has some (0, false: the ReaSim's power is T07's), the four settings and the curve.
pub fn snapshot(block: &str, column: &RbmkColumn) -> Option<Fields> {
    if !persist::is_auto_control(block) {
        return None;
    }
    let a = column.auto.clone().unwrap_or_default();
    let mut f = Fields::default();
    f.set("level", Value::Double(0.0))
        .set("targetLevel", Value::Double(0.0))
        .set("power", Value::Long(0))
        .set("hasPower", Value::Byte(0))
        .set("levelLower", Value::Double(a.level_lower))
        .set("levelUpper", Value::Double(a.level_upper))
        .set("heatLower", Value::Double(a.heat_lower))
        .set("heatUpper", Value::Double(a.heat_upper))
        .set("function", Value::Int(i32::from(a.function)));
    Some(f)
}

/// The settings a snapshot carries (the client's copy of the TE).
pub fn auto_of(fields: &Fields) -> AutoControl {
    AutoControl {
        level_lower: fields.get_double("levelLower"),
        level_upper: fields.get_double("levelUpper"),
        heat_lower: fields.get_double("heatLower"),
        heat_upper: fields.get_double("heatUpper"),
        function: fields.get_integer("function").clamp(0, 2) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTO: &str = "ntm:rbmk_control_auto";

    fn values(f: &mut Fields, v: [f64; 4]) -> &mut Fields {
        f.set("levelUpper", Value::Double(v[0]))
            .set("levelLower", Value::Double(v[1]))
            .set("heatUpper", Value::Double(v[2]))
            .set("heatLower", Value::Double(v[3]))
    }

    #[test]
    fn the_settings_are_taken_as_the_java_takes_them() {
        let mut c = RbmkColumn::of(AUTO);
        assert_eq!(c.auto, Some(AutoControl::default()));
        let mut f = Fields::default();
        values(&mut f, [100.0, 0.0, 9999.0, 20.0]);
        assert_eq!(receive(AUTO, &mut c, &f), Ok(true));
        let a = c.auto.clone().unwrap();
        assert_eq!((a.level_upper, a.level_lower, a.heat_upper, a.heat_lower, a.function), (100.0, 0.0, 9999.0, 20.0, 0));
        // The same again changes nothing; extreme values the Java would store, stored (the screen never sends them).
        assert_eq!(receive(AUTO, &mut c, &f), Ok(false));
        values(&mut f, [f64::NAN, -0.0, f64::INFINITY, -1e308]);
        assert_eq!(receive(AUTO, &mut c, &f), Ok(true));
        let a = c.auto.clone().unwrap();
        assert!(a.level_upper.is_nan() && a.level_lower.to_bits() == (-0.0f64).to_bits() && a.heat_upper.is_infinite());
        // A missing key is 0 (`getDouble`), an int tag reads as its double.
        let mut f = Fields::default();
        f.set("heatUpper", Value::Int(500));
        receive(AUTO, &mut c, &f).unwrap();
        let a = c.auto.clone().unwrap();
        assert_eq!((a.level_upper, a.level_lower, a.heat_upper, a.heat_lower), (0.0, 0.0, 500.0, 0.0));
    }

    #[test]
    fn the_function_index_is_javas_and_its_throws_are_refused() {
        let mut c = RbmkColumn::of(AUTO);
        let f = |i: i32| {
            let mut f = Fields::default();
            f.set("function", Value::Int(i));
            f
        };
        for (i, expect) in [(0, 0), (1, 1), (2, 2), (-2, 2), (5, 0), (7, 2), (-6, 1)] {
            receive(AUTO, &mut c, &f(i)).unwrap();
            assert_eq!(c.auto.as_ref().unwrap().function, expect, "function {i}");
        }
        // The function alone leaves the four settings.
        let before = c.auto.clone();
        for i in [3, 4, 8, -3, i32::MIN] {
            assert!(matches!(receive(AUTO, &mut c, &f(i)), Err(Refusal::Invalid(_))), "function {i}");
            assert_eq!(c.auto, before, "a refusal changes nothing");
        }
        // A double tag is floored (`getInteger`).
        let mut d = Fields::default();
        d.set("function", Value::Double(1.9));
        receive(AUTO, &mut c, &d).unwrap();
        assert_eq!(c.auto.as_ref().unwrap().function, 1);
    }

    #[test]
    fn only_the_automatic_rods_take_commands_and_a_save_from_before_reads_the_defaults() {
        let mut manual = RbmkColumn::of("ntm:rbmk_control");
        assert_eq!(manual.auto, None);
        assert!(matches!(receive("ntm:rbmk_control", &mut manual, &Fields::default()), Err(Refusal::Invalid(_))));
        assert!(snapshot("ntm:rbmk_control", &manual).is_none());
        // A column saved before T06 has no `auto`: its screen shows the TE's defaults, and a command sets it.
        let mut old = RbmkColumn::of(AUTO);
        old.auto = None;
        let json = serde_json::to_string(&old).unwrap();
        assert!(!json.contains("auto"), "{json}");
        let mut back: RbmkColumn = serde_json::from_str(&json).unwrap();
        assert_eq!(auto_of(&snapshot(AUTO, &back).unwrap()), AutoControl::default());
        assert_eq!(receive(AUTO, &mut back, &Fields::default()), Ok(true));
        assert!(back.auto.is_some());
        // The settings read back by their bits.
        let mut c = RbmkColumn::of("ntm:rbmk_control_reasim_auto");
        let mut f = Fields::default();
        values(&mut f, [f64::from_bits(0x4059_0000_0000_0001), 1.0, 2.0, 3.0]);
        receive("ntm:rbmk_control_reasim_auto", &mut c, &f).unwrap();
        let back: RbmkColumn = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.auto.unwrap().level_upper.to_bits(), 0x4059_0000_0000_0001);
    }

    #[test]
    fn the_snapshot_is_the_network_pack() {
        let mut c = RbmkColumn::of(AUTO);
        let mut f = Fields::default();
        values(&mut f, [90.0, 10.0, 800.0, 200.0]);
        receive(AUTO, &mut c, &f).unwrap();
        let s = snapshot(AUTO, &c).unwrap();
        let keys: Vec<&str> = s.0.keys().map(String::as_str).collect();
        assert_eq!(keys, ["function", "hasPower", "heatLower", "heatUpper", "level", "levelLower", "levelUpper", "power", "targetLevel"]);
        assert_eq!(auto_of(&s), c.auto.unwrap());
    }
}

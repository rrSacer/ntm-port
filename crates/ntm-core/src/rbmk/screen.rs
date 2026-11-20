//! The RBMK columns as `IControlReceiver`s (T06): what their screens send them (`receiveControl`) and what they show
//! back (the TE's network pack, `serialize`), over the codec of [`crate::screens::control`]. T06 carried the automatic
//! control rod's, T07 the manual rods' and the level, target and power of all five; the other columns' screens come
//! with their tickets (T08 boiler, T10 console).

use crate::rbmk::persist::{self, AutoControl, ControlState, RbmkColumn};
use crate::screens::control::{Fields, Refusal, Value};

/// Whether a column of `block` takes screen commands (T06: the automatic control rods; T07: the manual ones).
pub fn takes_commands(block: &str) -> bool {
    persist::is_control(block)
}

/// `TileEntityRBMKControlManual.receiveControl`: `level` sets the target (`setTarget`: the target as sent, the start
/// where the rod is now), `color` toggles a group (`abs(i) % 5`; the group the rod is in takes it out). A negative index
/// (`abs(Integer.MIN_VALUE)`) throws in the Java after the level was set; here it is refused whole (D2).
fn receive_manual(block: &str, column: &mut RbmkColumn, fields: &Fields) -> Result<bool, Refusal> {
    let color = fields.has_key("color").then(|| fields.get_integer("color").wrapping_abs() % 5);
    if let Some(c) = color.filter(|c| *c < 0) {
        return Err(Refusal::Invalid(format!("RBMKColor.values()[{c}]")));
    }
    let st = column.control.get_or_insert_with(|| ControlState::of(block));
    let before = st.clone();
    if fields.has_key("level") {
        st.target_level = fields.get_double("level");
        st.starting_level = st.level;
    }
    if let Some(c) = color {
        let c = c as u8;
        st.color = if st.color == Some(c) { None } else { Some(c) };
    }
    let bits = |s: &ControlState| (s.target_level.to_bits(), s.starting_level.to_bits(), s.color);
    Ok(bits(st) != bits(&before))
}

/// `TileEntityRBMKControlAuto.receiveControl`: `function` picks the curve (`abs(i) % RBMKColor.values().length`,
/// 5, into `RBMKFunction.values()`, 3: an index of 3 or 4, or a negative one from `abs(Integer.MIN_VALUE)`, throws in
/// the Java and is refused here, D2); without it the four doubles are set, a missing one to 0. Ok(true) when the
/// column changed.
pub fn receive(block: &str, column: &mut RbmkColumn, fields: &Fields) -> Result<bool, Refusal> {
    if persist::is_manual_control(block) {
        return receive_manual(block, column, fields);
    }
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

/// What a control rod's screen reads (`TileEntityRBMKControl.serialize`: the level, its target, the power and whether
/// it has enough; then `TileEntityRBMKControlManual.serialize`, the start and the group, -1 for none, or
/// `TileEntityRBMKControlAuto.serialize`, the four settings and the curve).
pub fn snapshot(block: &str, column: &RbmkColumn) -> Option<Fields> {
    if !persist::is_control(block) {
        return None;
    }
    let st = column.control.clone().unwrap_or_else(|| ControlState::of(block));
    let mut f = Fields::default();
    f.set("level", Value::Double(st.level))
        .set("targetLevel", Value::Double(st.target_level))
        .set("power", Value::Long(st.power))
        .set("hasPower", Value::Byte(i8::from(st.has_power)));
    if persist::is_manual_control(block) {
        f.set("startingLevel", Value::Double(st.starting_level))
            .set("color", Value::Int(st.color.map_or(-1, i32::from)));
        return Some(f);
    }
    let a = column.auto.clone().unwrap_or_default();
    f.set("levelLower", Value::Double(a.level_lower))
        .set("levelUpper", Value::Double(a.level_upper))
        .set("heatLower", Value::Double(a.heat_lower))
        .set("heatUpper", Value::Double(a.heat_upper))
        .set("function", Value::Int(i32::from(a.function)));
    Some(f)
}

/// `EnumUtil.grabEnumSafely`: `abs(i % n)`, never out of the values.
fn grab_safely(i: i32, n: i32) -> u8 {
    (i % n).unsigned_abs() as u8
}

/// `ICopiable.getSettings` of a control rod (T07): a manual rod's group (none: an empty compound), an automatic rod's
/// four settings and curve. What a settings tool copies.
pub fn settings(block: &str, column: &RbmkColumn) -> Option<Fields> {
    let mut f = Fields::default();
    if persist::is_manual_control(block) {
        if let Some(c) = column.control.as_ref().and_then(|c| c.color) {
            f.set("color", Value::Int(i32::from(c)));
        }
        return Some(f);
    }
    if !persist::is_auto_control(block) {
        return None;
    }
    let a = column.auto.clone().unwrap_or_default();
    f.set("levelLower", Value::Double(a.level_lower))
        .set("levelUpper", Value::Double(a.level_upper))
        .set("heatLower", Value::Double(a.heat_lower))
        .set("heatUpper", Value::Double(a.heat_upper))
        .set("function", Value::Int(i32::from(a.function)));
    Some(f)
}

/// `ICopiable.pasteSettings`: a manual rod takes the group (`grabEnumSafely`), or none without one; an automatic rod
/// takes each setting the compound has. False for a column that has no settings.
pub fn paste_settings(block: &str, column: &mut RbmkColumn, fields: &Fields) -> bool {
    if persist::is_manual_control(block) {
        let st = column.control.get_or_insert_with(|| ControlState::of(block));
        st.color = fields.has_key("color").then(|| grab_safely(fields.get_integer("color"), 5));
        return true;
    }
    if !persist::is_auto_control(block) {
        return false;
    }
    let a = column.auto.get_or_insert_with(AutoControl::default);
    for (key, v) in [("levelLower", &mut a.level_lower), ("levelUpper", &mut a.level_upper), ("heatLower", &mut a.heat_lower), ("heatUpper", &mut a.heat_upper)] {
        if fields.has_key(key) {
            *v = fields.get_double(key);
        }
    }
    if fields.has_key("function") {
        a.function = grab_safely(fields.get_integer("function"), 3);
    }
    true
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
    fn only_the_control_rods_take_commands_and_a_save_from_before_reads_the_defaults() {
        let mut blank = RbmkColumn::of("ntm:rbmk_blank");
        assert_eq!((blank.auto.clone(), blank.control.clone()), (None, None));
        assert!(matches!(receive("ntm:rbmk_blank", &mut blank, &Fields::default()), Err(Refusal::Invalid(_))));
        assert!(snapshot("ntm:rbmk_blank", &blank).is_none());
        assert_eq!(RbmkColumn::of("ntm:rbmk_control").auto, None, "a manual rod has no settings");
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

    const MANUAL: &str = "ntm:rbmk_control";

    fn level(v: f64) -> Fields {
        let mut f = Fields::default();
        f.set("level", Value::Double(v));
        f
    }

    fn color(c: i32) -> Fields {
        let mut f = Fields::default();
        f.set("color", Value::Int(c));
        f
    }

    #[test]
    fn a_manual_rod_takes_its_target_and_toggles_its_group() {
        let mut c = RbmkColumn::of(MANUAL);
        c.control.as_mut().unwrap().level = 0.4;
        // `setTarget`: the target as sent (the screen's 1, 0.75 .. 0; anything else stored as is), the start where the rod is.
        assert_eq!(receive(MANUAL, &mut c, &level(0.25)), Ok(true));
        let st = c.control.clone().unwrap();
        assert_eq!((st.target_level, st.starting_level), (0.25, 0.4));
        receive(MANUAL, &mut c, &level(7.5)).unwrap();
        assert_eq!(c.control.as_ref().unwrap().target_level, 7.5);
        // A group: `abs(i) % 5`; the same again takes the rod out of it.
        for (i, expect) in [(2, Some(2)), (2, None), (-3, Some(3)), (9, Some(4)), (4, None), (5, Some(0))] {
            receive(MANUAL, &mut c, &color(i)).unwrap();
            assert_eq!(c.control.as_ref().unwrap().color, expect, "color {i}");
        }
        // Both keys: the level, then the group. `abs(Integer.MIN_VALUE)` throws in the Java: refused whole.
        let before = c.clone();
        let mut both = level(0.5);
        both.set("color", Value::Int(i32::MIN));
        assert!(matches!(receive(MANUAL, &mut c, &both), Err(Refusal::Invalid(_))));
        assert_eq!(c, before);
        // The snapshot: the TE's pack, the group -1 for none.
        let s = snapshot(MANUAL, &c).unwrap();
        let keys: Vec<&str> = s.0.keys().map(String::as_str).collect();
        assert_eq!(keys, ["color", "hasPower", "level", "power", "startingLevel", "targetLevel"]);
        assert_eq!(s.get_integer("color"), 0);
        c.control.as_mut().unwrap().color = None;
        assert_eq!(snapshot(MANUAL, &c).unwrap().get_integer("color"), -1);
    }

    #[test]
    fn settings_copy_and_paste_as_the_java_does() {
        let mut from = RbmkColumn::of(MANUAL);
        receive(MANUAL, &mut from, &color(3)).unwrap();
        let s = settings(MANUAL, &from).unwrap();
        let mut to = RbmkColumn::of("ntm:rbmk_control_reasim");
        assert!(paste_settings("ntm:rbmk_control_reasim", &mut to, &s));
        assert_eq!(to.control.as_ref().unwrap().color, Some(3));
        // A rod in no group copies an empty compound, which takes the other rod out of its group.
        let none = settings(MANUAL, &RbmkColumn::of(MANUAL)).unwrap();
        assert!(none.0.is_empty());
        paste_settings("ntm:rbmk_control_reasim", &mut to, &none);
        assert_eq!(to.control.as_ref().unwrap().color, None);
        // `grabEnumSafely`: abs(i % n).
        paste_settings(MANUAL, &mut to, &color(-7));
        assert_eq!(to.control.as_ref().unwrap().color, Some(2));
        // An automatic rod: each key it has, the curve by `grabEnumSafely` (3 curves).
        let mut a = RbmkColumn::of(AUTO);
        let mut f = Fields::default();
        f.set("heatUpper", Value::Double(900.0)).set("function", Value::Int(-4));
        assert!(paste_settings(AUTO, &mut a, &f));
        let got = a.auto.clone().unwrap();
        assert_eq!((got.heat_upper, got.level_upper, got.function), (900.0, 0.0, 1));
        assert_eq!(auto_of(&settings(AUTO, &a).unwrap()), got);
        assert!(settings("ntm:rbmk_blank", &RbmkColumn::of("ntm:rbmk_blank")).is_none());
    }
}

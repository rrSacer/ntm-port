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

/// Whether a column of `block` has a screen with no machine slot, which still needs a menu (the player's inventory,
/// the screen's session or buttons): the control rods' (`ContainerRBMKControlAuto`, T06/T07), the boiler's
/// (`ContainerRBMKGeneric`, T08) and the console's (`ContainerRBMKGeneric`, T10).
pub fn has_slotless_menu(block: &str) -> bool {
    takes_commands(block) || block == "ntm:rbmk_boiler" || block == CONSOLE
}

/// The console's block (`RBMKConsole`, T10).
pub const CONSOLE: &str = "ntm:rbmk_console";

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

/// What a taken command does to a column, as the queue keeps it until the column's transaction (T06 review P3(a)): the
/// last value of each kind the command sets outright (a manual rod's target, an automatic rod's settings or curve), and
/// a manual rod's group toggles composed into one map of the group before to the group after (index 0 none, 1..=5 the
/// five), which is what any number of `color` presses in a row does. A column's queue holds at most one of each.
///
/// The console (T10) adds two kinds: a boiler's compressor pressed `n` times (`compressor` of the selection; four
/// presses bring the grade round, so `n` is kept in 1..=4), and the console's own screens (each screen's `toggle`s,
/// counted round its six types, and the columns an `id` gave it last).
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    Set(&'static str, Fields),
    Groups([Option<u8>; 6]),
    Compress(u8),
    Screens([u8; 6], [Option<Vec<i32>>; 6]),
}

fn group_index(g: Option<u8>) -> usize {
    g.map_or(0, |g| usize::from(g) + 1)
}

/// The toggles of a command, one `Pending` per kind it sets (a command may set a manual rod's target and group).
pub fn pending_of(block: &str, fields: &Fields) -> Vec<Pending> {
    let mut out = Vec::new();
    if persist::is_manual_control(block) {
        if fields.has_key("level") {
            let mut f = Fields::default();
            f.set("level", Value::Double(fields.get_double("level")));
            out.push(Pending::Set("level", f));
        }
        if fields.has_key("color") {
            let c = (fields.get_integer("color").wrapping_abs() % 5).clamp(0, 4) as u8;
            let toggle = |x: Option<u8>| if x == Some(c) { None } else { Some(c) };
            out.push(Pending::Groups(std::array::from_fn(|i| toggle(if i == 0 { None } else { Some(i as u8 - 1) }))));
        }
    } else if fields.has_key("function") {
        out.push(Pending::Set("function", fields.clone()));
    } else {
        out.push(Pending::Set("settings", fields.clone()));
    }
    out
}

/// `next` into a column's queue: a value of a kind already there replaces it (the Java would set both, the later
/// last); group toggles compose with those waiting.
pub fn coalesce(queue: &mut Vec<Pending>, next: Pending) {
    match next {
        Pending::Set(kind, f) => {
            queue.retain(|p| !matches!(p, Pending::Set(k, _) if *k == kind));
            queue.push(Pending::Set(kind, f));
        }
        Pending::Groups(after) => match queue.iter_mut().find_map(|p| if let Pending::Groups(m) = p { Some(m) } else { None }) {
            Some(before) => *before = std::array::from_fn(|i| after[group_index(before[i])]),
            None => queue.push(Pending::Groups(after)),
        },
        Pending::Compress(n) => match queue.iter_mut().find_map(|p| if let Pending::Compress(m) = p { Some(m) } else { None }) {
            Some(m) => *m = (*m + n - 1) % 4 + 1,
            None => queue.push(Pending::Compress(n)),
        },
        Pending::Screens(toggles, columns) => match queue.iter_mut().find_map(|p| if let Pending::Screens(t, c) = p { Some((t, c)) } else { None }) {
            Some((t, c)) => {
                for i in 0..6 {
                    t[i] = (t[i] + toggles[i]) % crate::rbmk::console::SCREEN_TYPES;
                    if columns[i].is_some() {
                        c[i] = columns[i].clone();
                    }
                }
            }
            None => queue.push(Pending::Screens(toggles, columns)),
        },
    }
}

/// A console's command as its own queue keeps it (`Pending::Screens`), what `console::receive` did to its screens.
pub fn console_pending(before: &crate::rbmk::console::ConsoleState, after: &crate::rbmk::console::ConsoleState) -> Option<Pending> {
    let n = crate::rbmk::console::SCREEN_TYPES;
    let toggles: [u8; 6] = std::array::from_fn(|i| (after.screens[i].kind + n - before.screens[i].kind) % n);
    let columns: [Option<Vec<i32>>; 6] = std::array::from_fn(|i| (after.screens[i].columns != before.screens[i].columns).then(|| after.screens[i].columns.clone()));
    (toggles.iter().any(|t| *t != 0) || columns.iter().any(Option::is_some)).then_some(Pending::Screens(toggles, columns))
}

/// A console's waiting screens applied in its transaction.
pub fn apply_console(console: &mut crate::rbmk::console::ConsoleState, pending: &Pending) {
    if let Pending::Screens(toggles, columns) = pending {
        for i in 0..6 {
            let s = &mut console.screens[i];
            s.kind = (s.kind + toggles[i]) % crate::rbmk::console::SCREEN_TYPES;
            if let Some(c) = &columns[i] {
                s.columns = c.clone();
            }
        }
    }
}

/// A waiting command applied to the column, in its transaction.
pub fn apply(block: &str, column: &mut RbmkColumn, pending: &Pending) {
    match pending {
        Pending::Set(_, f) => {
            let _ = receive(block, column, f);
        }
        Pending::Groups(map) => {
            let st = column.control.get_or_insert_with(|| ControlState::of(block));
            st.color = map[group_index(st.color)];
        }
        Pending::Compress(n) => {
            use crate::server::container_bridge::Slotted;
            for _ in 0..*n {
                column.button(0);
            }
        }
        Pending::Screens(..) => {}
    }
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

    #[test]
    fn a_queue_keeps_one_of_each_kind_and_does_what_the_commands_in_order_do() {
        // Every sequence of up to four commands of a manual rod (levels and groups), from every group: the composed
        // queue leaves the rod as the commands one after the other do, and holds at most one entry of each kind.
        let commands: Vec<Fields> = [level(0.25), level(0.75), color(0), color(1), color(4)].into_iter().collect();
        let mut seqs: Vec<Vec<usize>> = vec![vec![]];
        for _ in 0..4 {
            seqs = seqs.iter().flat_map(|s| (0..commands.len()).map(move |i| [s.clone(), vec![i]].concat())).collect();
            for seq in &seqs {
                for start in [None, Some(0), Some(1), Some(4)] {
                    let mut fresh = RbmkColumn::of(MANUAL);
                    fresh.control.as_mut().unwrap().color = start;
                    let mut one_by_one = fresh.clone();
                    for &i in seq {
                        receive(MANUAL, &mut one_by_one, &commands[i]).unwrap();
                    }
                    let mut queue = Vec::new();
                    for &i in seq {
                        for p in pending_of(MANUAL, &commands[i]) {
                            coalesce(&mut queue, p);
                        }
                    }
                    assert!(queue.len() <= 2, "{seq:?}: {queue:?}");
                    let mut queued = fresh.clone();
                    for p in &queue {
                        apply(MANUAL, &mut queued, p);
                    }
                    assert_eq!(queued, one_by_one, "{seq:?} from {start:?}");
                }
            }
        }
        // An automatic rod: the last settings and the last curve.
        let mut queue = Vec::new();
        let mut a = Fields::default();
        a.set("levelUpper", Value::Double(1.0));
        let mut b = Fields::default();
        b.set("levelUpper", Value::Double(2.0));
        let mut f = Fields::default();
        f.set("function", Value::Int(1));
        for x in [&a, &f, &b, &f] {
            for p in pending_of(AUTO, x) {
                coalesce(&mut queue, p);
            }
        }
        assert_eq!(queue, vec![Pending::Set("settings", b), Pending::Set("function", f)]);
    }
}

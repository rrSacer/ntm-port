//! The RBMK console (`TileEntityRBMKConsole`, T10): the 15×15 columns around its target, what its screen sends it
//! (`receiveControl`) and what that does to the columns. Pure: the server finds the columns and applies the edits.

use crate::rbmk::persist;
use crate::screens::control::{Fields, Refusal};
use serde::{Deserialize, Serialize};

/// The window's side and cells (`columns = new RBMKColumn[15 * 15]`).
pub const SIDE: i32 = 15;
pub const CELLS: i32 = SIDE * SIDE;
/// `screens = new RBMKScreen[6]`.
pub const SCREENS: usize = 6;
/// `ScreenType`: NONE, COL_TEMP, ROD_EXTRACTION, FUEL_DEPLETION, FUEL_POISON, FUEL_TEMP.
pub const SCREEN_TYPES: u8 = 6;

/// One of the six screens: its `ScreenType` ordinal and the columns it reads (indices of the window).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    pub kind: u8,
    pub columns: Vec<i32>,
}

/// What `writeToNBT` keeps: the target (`tX`, `tY`, `tZ`: 0, 0, 0 before any link, as the fields start), the
/// rotation and the six screens.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleState {
    pub target: [i32; 3],
    pub rotation: u8,
    pub screens: [Screen; SCREENS],
}

/// `getXFromIndex` and `getZFromIndex`: the cell of window index `col` relative to the target, turned by the console's
/// `rotation` (0°, 90°, 180°, 270°).
pub fn offset(rotation: u8, col: i32) -> (i32, i32) {
    let (i, j) = (col % SIDE - 7, col / SIDE - 7);
    match rotation {
        1 => (-j, i),
        2 => (-i, -j),
        3 => (j, -i),
        _ => (i, j),
    }
}

/// `TileEntityRBMKConsole.rotate`.
pub fn rotate(state: &mut ConsoleState) {
    state.rotation = (state.rotation + 1) % 4;
}

/// `ColumnType` of a column of `block` (`getConsoleType`), by its ordinal's `offset` order: what the console's map
/// shows and what AZ-5 picks (CONTROL: the manual rods; the automatic ones are CONTROL_AUTO).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    Blank,
    Fuel,
    FuelSim,
    Control,
    ControlAuto,
    Boiler,
    Moderator,
    Absorber,
    Reflector,
    Outgasser,
    Breeder,
    Storage,
    Cooler,
    Heatex,
}

pub fn column_type(block: &str) -> Option<ColumnType> {
    Some(match block {
        "ntm:rbmk_blank" => ColumnType::Blank,
        "ntm:rbmk_rod" | "ntm:rbmk_rod_mod" => ColumnType::Fuel,
        "ntm:rbmk_rod_reasim" | "ntm:rbmk_rod_reasim_mod" => ColumnType::FuelSim,
        b if persist::is_manual_control(b) => ColumnType::Control,
        b if persist::is_auto_control(b) => ColumnType::ControlAuto,
        "ntm:rbmk_boiler" => ColumnType::Boiler,
        "ntm:rbmk_moderator" => ColumnType::Moderator,
        "ntm:rbmk_absorber" => ColumnType::Absorber,
        "ntm:rbmk_reflector" => ColumnType::Reflector,
        "ntm:rbmk_outgasser" => ColumnType::Outgasser,
        "ntm:rbmk_storage" => ColumnType::Storage,
        "ntm:rbmk_cooler" => ColumnType::Cooler,
        "ntm:rbmk_heater" => ColumnType::Heatex,
        _ => return None,
    })
}

/// What the console's last scan saw of a column (`rescan`, every 10 ticks): its type and the values its map and
/// screens read (`heat`; a control rod's `level` and group, `RBMKColor` ordinal).
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnView {
    pub kind: ColumnType,
    pub heat: f64,
    pub level: Option<f64>,
    pub color: Option<u8>,
}

/// The console as the server keeps it: its saved state and, in memory only (the Java's `columns` are refilled by the
/// next `rescan`), its last scan of the 225 cells.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Console {
    #[serde(flatten)]
    pub state: ConsoleState,
    #[serde(skip)]
    pub scan: Vec<Option<ColumnView>>,
}

impl crate::machine::Logic for Console {
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn view(&self) -> crate::view::ViewModel {
        crate::view::ViewModel::new("RBMK console")
    }
}

/// What the console's screen reads (`serialize`'s columns and screens, typed, D6): the target, the rotation, the six
/// screens (`tN` type, `sN` columns) and the last scan, `k` the 225 column types (`ColumnType` ordinal, -1 none) and,
/// for each column seen, `hN` its heat, `lN` a control rod's level and `cN` a manual rod's group (-1 none).
pub fn snapshot(console: &Console) -> Fields {
    use crate::screens::control::Value;
    let mut f = Fields::default();
    let st = &console.state;
    f.set("tX", Value::Int(st.target[0])).set("tY", Value::Int(st.target[1])).set("tZ", Value::Int(st.target[2]));
    f.set("rotation", Value::Byte(st.rotation as i8));
    for (i, s) in st.screens.iter().enumerate() {
        f.set(&format!("t{i}"), Value::Byte(s.kind as i8)).set(&format!("s{i}"), Value::IntArray(s.columns.clone()));
    }
    let kinds = (0..CELLS as usize).map(|i| console.scan.get(i).cloned().flatten().map_or(-1, |c| c.kind as i32)).collect();
    f.set("k", Value::IntArray(kinds));
    for (i, c) in console.scan.iter().enumerate() {
        let Some(c) = c else { continue };
        f.set(&format!("h{i}"), Value::Double(c.heat));
        if let Some(l) = c.level {
            f.set(&format!("l{i}"), Value::Double(l));
        }
        if c.kind == ColumnType::Control {
            f.set(&format!("c{i}"), Value::Int(c.color.map_or(-1, i32::from)));
        }
    }
    f
}

/// What a command does to a column at a cell relative to the target (each only to the column kind the Java casts to;
/// another kind there, or none, is untouched).
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// `rod.startingLevel = rod.level; rod.setTarget(clamp(level, 0, 1))` of a manual rod.
    Target { at: (i32, i32), level: f64 },
    /// `rod.color = grabEnumSafely(RBMKColor, color)` of a manual rod.
    Color { at: (i32, i32), color: u8 },
    /// `cyceCompressor` of a boiler.
    Compress { at: (i32, i32) },
}

/// `EnumUtil.grabEnumSafely`: `abs(i % n)`.
fn grab_safely(i: i32, n: i32) -> u8 {
    (i % n).unsigned_abs() as u8
}

/// A window index the console may act on. The Java takes any int a packet carries (a modified client could reach a
/// column far outside the window, or crash the handler with a screen slot out of 0..5): refused whole here (D2).
fn cell(col: i32) -> Result<i32, Refusal> {
    if (0..CELLS).contains(&col) { Ok(col) } else { Err(Refusal::Invalid(format!("column index {col}"))) }
}

/// `TileEntityRBMKConsole.receiveControl`, in the Java's order: `level` with the `sel_` keys (each a window index),
/// `toggle` (the next screen type of a screen), `id` (a screen's columns, `s0`..`s224`), `assignColor` and
/// `compressor` (each with the window indices in `cols`). The state changes here; the edits to the columns are
/// returned for the server. An index out of the window, a screen out of 0..5 or `cols` that is not an int array (the
/// Java's crash report) refuses the whole command (D2) before anything changes.
pub fn receive(state: &mut ConsoleState, fields: &Fields) -> Result<Vec<Edit>, Refusal> {
    let mut edits = Vec::new();
    let mut next = state.clone();
    if fields.has_key("level") {
        let level = fields.get_double("level").clamp(0.0, 1.0);
        // `MathHelper.clamp_double` lets a NaN through (`v < min ? min : v > max ? max : v`).
        let level = if fields.get_double("level").is_nan() { f64::NAN } else { level };
        for key in fields.0.keys().filter(|k| k.starts_with("sel_")) {
            let col = cell(fields.get_integer(key))?;
            edits.push(Edit::Target { at: offset(state.rotation, col), level });
        }
    }
    if fields.has_key("toggle") {
        let slot = screen(fields.get_byte("toggle"))?;
        next.screens[slot].kind = (next.screens[slot].kind + 1) % SCREEN_TYPES;
    }
    if fields.has_key("id") {
        let slot = screen(fields.get_byte("id"))?;
        next.screens[slot].columns = (0..CELLS).filter(|i| fields.get_boolean(&format!("s{i}"))).collect();
    }
    if fields.has_key("assignColor") {
        let color = grab_safely(i32::from(fields.get_byte("assignColor")), 5);
        let cols = fields.get_int_array("cols").map_err(|m| Refusal::Invalid(format!("getIntArray({})", m.0)))?;
        for c in cols {
            edits.push(Edit::Color { at: offset(state.rotation, cell(c)?), color });
        }
    }
    if fields.has_key("compressor") {
        let cols = fields.get_int_array("cols").map_err(|m| Refusal::Invalid(format!("getIntArray({})", m.0)))?;
        for c in cols {
            edits.push(Edit::Compress { at: offset(state.rotation, cell(c)?) });
        }
    }
    *state = next;
    Ok(edits)
}

fn screen(slot: i8) -> Result<usize, Refusal> {
    usize::try_from(slot).ok().filter(|s| *s < SCREENS).ok_or_else(|| Refusal::Invalid(format!("screens[{slot}]")))
}

/// What `GUIRBMKConsole` sends for AZ-5 (its lid open, 3 s since the last press): `level` 0 with `sel_j` for every
/// column of the window its last scan saw as CONTROL (the manual rods).
pub fn az5(columns: &[Option<ColumnType>]) -> Fields {
    use crate::screens::control::Value;
    let mut f = Fields::default();
    f.set("level", Value::Double(0.0));
    for (j, c) in columns.iter().enumerate() {
        if *c == Some(ColumnType::Control) {
            f.set(&format!("sel_{j}"), Value::Int(j as i32));
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::control::Value;

    /// The window's corners and centre in the four rotations, as `getXFromIndex`/`getZFromIndex` turn them.
    #[test]
    fn indices_turn_with_the_console() {
        assert_eq!(offset(0, 112), (0, 0), "the centre is the target");
        assert_eq!([offset(0, 0), offset(0, 14), offset(0, 210), offset(0, 224)], [(-7, -7), (7, -7), (-7, 7), (7, 7)]);
        assert_eq!(offset(1, 14), (7, 7));
        assert_eq!(offset(2, 14), (-7, 7));
        assert_eq!(offset(3, 14), (-7, -7));
        for r in 0..4 {
            let cells: std::collections::BTreeSet<(i32, i32)> = (0..CELLS).map(|c| offset(r, c)).collect();
            assert_eq!(cells.len(), 225, "rotation {r} covers the window once");
        }
        let mut s = ConsoleState::default();
        for want in [1, 2, 3, 0] {
            rotate(&mut s);
            assert_eq!(s.rotation, want);
        }
    }

    /// AZ-5: every CONTROL column of the scan, at level 0; the automatic rods and the rest are not picked.
    #[test]
    fn az5_picks_the_manual_rods() {
        let mut cols = vec![None; CELLS as usize];
        cols[3] = Some(ColumnType::Control);
        cols[4] = Some(ColumnType::ControlAuto);
        cols[100] = Some(ColumnType::Control);
        cols[101] = Some(ColumnType::Fuel);
        let f = az5(&cols);
        let mut s = ConsoleState::default();
        // In key order (the Java's is a HashSet's): every one gets the same level, so the order changes nothing.
        let mut at: Vec<(i32, i32)> = receive(&mut s, &f).unwrap().into_iter().map(|e| match e {
            Edit::Target { at, level: 0.0 } => at,
            other => panic!("{other:?}"),
        }).collect();
        at.sort();
        let mut want = vec![offset(0, 3), offset(0, 100)];
        want.sort();
        assert_eq!(at, want);
        assert_eq!(column_type("ntm:rbmk_control_reasim"), Some(ColumnType::Control));
        assert_eq!(column_type("ntm:rbmk_control_reasim_auto"), Some(ColumnType::ControlAuto));
    }

    /// The level is clamped (a NaN passes, as `clamp_double` lets it); the screens toggle and take their columns; a
    /// colour and the compressor go to `cols`; bad input refuses the whole command and changes nothing.
    #[test]
    fn commands_do_what_receive_control_does() {
        let mut s = ConsoleState::default();
        let mut f = Fields::default();
        f.set("level", Value::Double(1.7)).set("sel_a", Value::Int(112));
        assert_eq!(receive(&mut s, &f).unwrap(), vec![Edit::Target { at: (0, 0), level: 1.0 }]);
        f.set("level", Value::Double(f64::NAN));
        assert!(matches!(receive(&mut s, &f).unwrap()[0], Edit::Target { level, .. } if level.is_nan()));
        let mut f = Fields::default();
        f.set("toggle", Value::Byte(2));
        receive(&mut s, &f).unwrap();
        receive(&mut s, &f).unwrap();
        assert_eq!(s.screens[2].kind, 2);
        for _ in 0..4 {
            receive(&mut s, &f).unwrap();
        }
        assert_eq!(s.screens[2].kind, 0, "six types, round");
        let mut f = Fields::default();
        f.set("id", Value::Byte(5)).set("s7", Value::Byte(1)).set("s9", Value::Byte(0)).set("s224", Value::Byte(1));
        receive(&mut s, &f).unwrap();
        assert_eq!(s.screens[5].columns, vec![7, 224]);
        let mut f = Fields::default();
        f.set("assignColor", Value::Byte(-7)).set("cols", Value::IntArray(vec![0, 224]));
        assert_eq!(receive(&mut s, &f).unwrap(), vec![Edit::Color { at: (-7, -7), color: 2 }, Edit::Color { at: (7, 7), color: 2 }]);
        let mut f = Fields::default();
        f.set("compressor", Value::Byte(1)).set("cols", Value::IntArray(vec![112]));
        assert_eq!(receive(&mut s, &f).unwrap(), vec![Edit::Compress { at: (0, 0) }]);
        // Refused whole: an index out of the window, a screen out of 0..5, `cols` of another tag.
        let before = s.clone();
        for bad in [
            Fields(std::collections::BTreeMap::from([("level".into(), Value::Double(0.0)), ("sel_x".into(), Value::Int(225))])),
            Fields(std::collections::BTreeMap::from([("toggle".into(), Value::Byte(6))])),
            Fields(std::collections::BTreeMap::from([("id".into(), Value::Byte(-1))])),
            Fields(std::collections::BTreeMap::from([("compressor".into(), Value::Byte(1)), ("cols".into(), Value::Int(3))])),
            Fields(std::collections::BTreeMap::from([("toggle".into(), Value::Byte(0)), ("compressor".into(), Value::Byte(1)), ("cols".into(), Value::IntArray(vec![-1]))])),
        ] {
            assert!(receive(&mut s, &bad).is_err(), "{bad:?}");
            assert_eq!(s, before, "nothing changed: {bad:?}");
        }
    }
}

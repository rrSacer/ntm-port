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

/// What the console's last scan saw of a column (`rescan`, every 10 ticks: its type, `getNBTForConsole` and
/// `heat`; `maxHeat` is 1500 for every column, `moderated` set for a moderated manual rod).
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnView {
    pub kind: ColumnType,
    pub heat: f64,
    pub moderated: bool,
    pub data: ColumnData,
}

/// A column's `getNBTForConsole`, by the Java class that writes it (the others write none).
#[derive(Clone, Debug, PartialEq)]
pub enum ColumnData {
    None,
    /// `TileEntityRBMKControl`: `level` (and `targetLevel`, read by nothing); a manual rod's `color` (-1 none).
    Control { level: f64, color: Option<i16> },
    /// `TileEntityRBMKRod`: the rod's `enrichment`, `xenon`, `c_heat`, `c_coreHeat`, `c_maxHeat`; none without a rod.
    Fuel(Option<[f64; 5]>),
    /// `TileEntityRBMKBoiler`, `TileEntityRBMKHeater`: `water`, `maxWater`, `steam`, `maxSteam`, `type` and the
    /// heater's `hottype` (fluid ids).
    Tanks { water: i32, max_water: i32, steam: i32, max_steam: i32, kind: u16, hot: u16 },
    /// `TileEntityRBMKOutgasser`: `gas`, `maxGas`, `type`, `progress` (read by no screen).
    Gas { gas: i32, max_gas: i32, kind: u16, progress: f64 },
}

/// `RBMKColumn.data.getDouble("level")`: 0 for a column that wrote none.
impl ColumnView {
    pub fn level(&self) -> Option<f64> {
        match self.data {
            ColumnData::Control { level, .. } => Some(level),
            _ => None,
        }
    }
}

/// The `Fluids` id of a tank's `Fluids` name (NONE 0).
fn fluid_id(name: &str) -> u16 {
    crate::fluid::types().iter().find(|t| t.name == name).map_or(0, |t| t.id)
}

/// What `rescan` reads of the column of `block`: none for a block that is no column.
pub fn view_of(block: &str, col: &persist::RbmkColumn) -> Option<ColumnView> {
    let kind = column_type(block)?;
    let manual = persist::is_manual_control(block);
    let data = match kind {
        ColumnType::Control | ColumnType::ControlAuto => {
            let st = col.control.clone().unwrap_or_default();
            ColumnData::Control { level: st.level, color: manual.then(|| st.color.map_or(-1, i16::from)) }
        }
        ColumnType::Fuel | ColumnType::FuelSim => ColumnData::Fuel(col.slots.first().and_then(Option::as_ref).and_then(|s| {
            let rod = persist::rod_of(&s.id)?;
            let mut r = persist::stack_rod(s);
            Some([r.enrichment(rod), r.poison(rod), r.hull_heat(rod), r.core_heat(rod), rod.melting_point])
        })),
        ColumnType::Boiler | ColumnType::Heatex => {
            let Some(p) = &col.piped else { return Some(ColumnView { kind, heat: col.heat, moderated: false, data: ColumnData::None }) };
            let (feed, steam) = (&p.input, &p.output);
            // The boiler's `type` is its steam's (the compressor), the heater's its feed's.
            let (kind_id, hot) = if kind == ColumnType::Boiler { (fluid_id(&steam.fluid), 0) } else { (fluid_id(&feed.fluid), fluid_id(&steam.fluid)) };
            ColumnData::Tanks { water: feed.fill, max_water: feed.max, steam: steam.fill, max_steam: steam.max, kind: kind_id, hot }
        }
        ColumnType::Outgasser => match &col.outgasser {
            Some(o) => ColumnData::Gas { gas: o.gas.fill, max_gas: o.gas.max, kind: fluid_id(&o.gas.fluid), progress: o.progress },
            None => ColumnData::None,
        },
        _ => ColumnData::None,
    };
    // `isModerated`: `TileEntityRBMKControlManual` and `TileEntityRBMKRod` (the ReaSim channels are rods) answer it by
    // their block's `moderated`; the base says no.
    let moderated = (manual || persist::is_channel(block)) && persist::kind_of(block).is_some_and(|(_, m)| m);
    Some(ColumnView { kind, heat: col.heat, moderated, data })
}

/// The scan as one int array (D6: a snapshot has room for few keys, an array for the 225 columns): for each cell a
/// head, -1 for none, else `ColumnType` ordinal | moderated << 8 | data kind << 9, then its data, a double as its
/// two halves (exact).
pub fn pack(scan: &[Option<ColumnView>]) -> Vec<i32> {
    let mut out = Vec::new();
    let double = |out: &mut Vec<i32>, v: f64| {
        let b = v.to_bits();
        out.extend([(b >> 32) as i32, b as i32]);
    };
    for c in scan {
        let Some(c) = c else {
            out.push(-1);
            continue;
        };
        let tag = match c.data {
            ColumnData::None => 0,
            ColumnData::Control { color: None, .. } => 1,
            ColumnData::Control { color: Some(_), .. } => 2,
            ColumnData::Fuel(None) => 3,
            ColumnData::Fuel(Some(_)) => 4,
            ColumnData::Tanks { .. } => 5,
            ColumnData::Gas { .. } => 6,
        };
        out.push(c.kind as i32 | i32::from(c.moderated) << 8 | tag << 9);
        double(&mut out, c.heat);
        match &c.data {
            ColumnData::None | ColumnData::Fuel(None) => {}
            ColumnData::Control { level, color } => {
                double(&mut out, *level);
                if let Some(color) = color {
                    out.push(i32::from(*color));
                }
            }
            ColumnData::Fuel(Some(v)) => v.iter().for_each(|v| double(&mut out, *v)),
            ColumnData::Tanks { water, max_water, steam, max_steam, kind, hot } => {
                out.extend([*water, *max_water, *steam, *max_steam, i32::from(*kind), i32::from(*hot)])
            }
            ColumnData::Gas { gas, max_gas, kind, progress } => {
                out.extend([*gas, *max_gas, i32::from(*kind)]);
                double(&mut out, *progress);
            }
        }
    }
    out
}

/// [`pack`] read back; none for an array it did not write.
pub fn unpack(packed: &[i32]) -> Option<Vec<Option<ColumnView>>> {
    const KINDS: [ColumnType; 14] = [
        ColumnType::Blank,
        ColumnType::Fuel,
        ColumnType::FuelSim,
        ColumnType::Control,
        ColumnType::ControlAuto,
        ColumnType::Boiler,
        ColumnType::Moderator,
        ColumnType::Absorber,
        ColumnType::Reflector,
        ColumnType::Outgasser,
        ColumnType::Breeder,
        ColumnType::Storage,
        ColumnType::Cooler,
        ColumnType::Heatex,
    ];
    struct Reader<'a>(std::slice::Iter<'a, i32>);
    impl Reader<'_> {
        fn int(&mut self) -> Option<i32> {
            self.0.next().copied()
        }
        fn double(&mut self) -> Option<f64> {
            Some(f64::from_bits(u64::from(self.int()? as u32) << 32 | u64::from(self.int()? as u32)))
        }
    }
    let mut r = Reader(packed.iter());
    let mut out = Vec::new();
    while out.len() < CELLS as usize {
        let head = r.int()?;
        if head == -1 {
            out.push(None);
            continue;
        }
        let kind = *KINDS.get((head & 0xFF) as usize)?;
        let heat = r.double()?;
        let data = match head >> 9 {
            0 => ColumnData::None,
            1 => ColumnData::Control { level: r.double()?, color: None },
            2 => ColumnData::Control { level: r.double()?, color: Some(r.int()? as i16) },
            3 => ColumnData::Fuel(None),
            4 => ColumnData::Fuel(Some([r.double()?, r.double()?, r.double()?, r.double()?, r.double()?])),
            5 => ColumnData::Tanks { water: r.int()?, max_water: r.int()?, steam: r.int()?, max_steam: r.int()?, kind: r.int()? as u16, hot: r.int()? as u16 },
            6 => ColumnData::Gas { gas: r.int()?, max_gas: r.int()?, kind: r.int()? as u16, progress: r.double()? },
            _ => return None,
        };
        out.push(Some(ColumnView { kind, heat, moderated: head >> 8 & 1 == 1, data }));
    }
    Some(out)
}

/// The console as the server keeps it: its saved state and, in memory only (the Java's `columns` are refilled by the
/// next `rescan`), its last scan of the 225 cells.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Console {
    #[serde(flatten)]
    pub state: ConsoleState,
    #[serde(skip)]
    pub scan: Vec<Option<ColumnView>>,
    /// `fluxBuffer` (not saved: 60 zeros after a load).
    #[serde(skip)]
    pub flux: Vec<i32>,
}

impl crate::machine::Logic for Console {
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn view(&self) -> crate::view::ViewModel {
        crate::view::ViewModel::new("RBMK console")
    }
}

/// `fluxBuffer`'s length: the flux graph's last 60 scans.
pub const FLUX: usize = 60;

/// `rescan`'s end: the buffer moves one place and takes `(int) flux`, the sum of the fuel channels' `lastFluxQuantity`
/// (Java's cast: a NaN is 0, beyond an int its bound).
pub fn push_flux(buffer: &mut Vec<i32>, flux: f64) {
    buffer.resize(FLUX, 0);
    buffer.remove(0);
    buffer.push(flux as i32);
}

/// What the console's screen reads (`serialize`'s full packet, typed, D6): the target, the rotation, the six screens
/// (`tN` type, `sN` columns), `cols` the last scan ([`pack`]) and `flux` the flux graph.
pub fn snapshot(console: &Console) -> Fields {
    use crate::screens::control::Value;
    let mut f = Fields::default();
    let st = &console.state;
    f.set("tX", Value::Int(st.target[0])).set("tY", Value::Int(st.target[1])).set("tZ", Value::Int(st.target[2]));
    f.set("rotation", Value::Byte(st.rotation as i8));
    for (i, s) in st.screens.iter().enumerate() {
        f.set(&format!("t{i}"), Value::Byte(s.kind as i8)).set(&format!("s{i}"), Value::IntArray(s.columns.clone()));
    }
    let mut scan = console.scan.clone();
    scan.resize(CELLS as usize, None);
    f.set("cols", Value::IntArray(pack(&scan)));
    let mut flux = console.flux.clone();
    flux.resize(FLUX, 0);
    f.set("flux", Value::IntArray(flux));
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

    fn every_kind() -> Vec<Option<ColumnView>> {
        let view = |kind, data| Some(ColumnView { kind, heat: 20.5, moderated: false, data });
        vec![
            None,
            view(ColumnType::Blank, ColumnData::None),
            view(ColumnType::ControlAuto, ColumnData::Control { level: 0.3, color: None }),
            Some(ColumnView { kind: ColumnType::Control, heat: f64::NAN, moderated: true, data: ColumnData::Control { level: -0.0, color: Some(-1) } }),
            view(ColumnType::Control, ColumnData::Control { level: 1.0, color: Some(4) }),
            view(ColumnType::Fuel, ColumnData::Fuel(None)),
            view(ColumnType::FuelSim, ColumnData::Fuel(Some([0.99, 12.5, 2000.25, f64::INFINITY, 3000.0]))),
            view(ColumnType::Boiler, ColumnData::Tanks { water: 1, max_water: 10000, steam: 2, max_steam: 1000000, kind: 7, hot: 0 }),
            view(ColumnType::Heatex, ColumnData::Tanks { water: -3, max_water: 16000, steam: i32::MAX, max_steam: 16000, kind: 600, hot: 601 }),
            view(ColumnType::Outgasser, ColumnData::Gas { gas: 5, max_gas: 64000, kind: 9, progress: 1.5e-300 }),
        ]
    }

    /// Every column the scan may see reads back as it was, doubles by their bits (a NaN heat, a -0 level).
    #[test]
    fn the_scan_packs_exactly() {
        let mut scan = every_kind();
        scan.resize(CELLS as usize, None);
        let back = unpack(&pack(&scan)).expect("read back");
        assert_eq!(back.len(), scan.len());
        for (a, b) in scan.iter().zip(&back) {
            let bits = |v: &Option<ColumnView>| format!("{:?}", v.as_ref().map(|c| (c.kind, c.heat.to_bits(), c.moderated, format!("{:?}", c.data))));
            assert_eq!(bits(a), bits(b));
        }
        assert_eq!(back[3].as_ref().unwrap().level().map(f64::to_bits), Some((-0.0f64).to_bits()));
        assert!(unpack(&pack(&scan)[..10]).is_none(), "cut short");
        assert!(unpack(&[1 | 7 << 9, 0, 0]).is_none(), "a data kind it never writes");
    }

    /// The fullest window there is (225 fuel channels with rods, six screens of every column) still fits a snapshot,
    /// and AZ-5 with 225 manual rods fits a command.
    #[test]
    fn a_full_window_fits_the_messages() {
        use crate::screens::control::{Command, Snapshot};
        let fuel = Some(ColumnView { kind: ColumnType::Fuel, heat: 1.0, moderated: false, data: ColumnData::Fuel(Some([1.0; 5])) });
        let mut console = Console { scan: vec![fuel; CELLS as usize], ..Console::default() };
        for s in console.state.screens.iter_mut() {
            s.columns = (0..CELLS).collect();
        }
        let snap = Snapshot { session: 1, generation: 1, pos: [0; 3], tick: 0, fields: snapshot(&console) };
        assert!(snap.encode().is_ok(), "{:?}", snap.encode().err());
        let az5 = az5(&vec![Some(ColumnType::Control); CELLS as usize]);
        let cmd = Command { session: 1, generation: 1, pos: [0; 3], seq: 1, fields: az5 };
        assert!(cmd.encode().is_ok(), "{:?}", cmd.encode().err());
    }

    /// `fluxBuffer`: 60 zeros at first, then each scan's sum at the end, the oldest out; `(int)` of a NaN is 0.
    #[test]
    fn the_flux_graph_slides() {
        let mut b = Vec::new();
        push_flux(&mut b, 12.9);
        assert_eq!((b.len(), b[59], b[58]), (FLUX, 12, 0));
        push_flux(&mut b, f64::NAN);
        push_flux(&mut b, 1e300);
        assert_eq!(&b[57..], &[12, 0, i32::MAX]);
    }

    /// `isModerated` as the Java answers it: the moderated manual rod and the two moderated fuel channels; every other
    /// column (the automatic rods, the moderator itself) no.
    #[test]
    fn the_moderated_columns_are_the_javas() {
        let moderated: Vec<&str> = persist::COLUMNS.iter().copied().filter(|b| view_of(b, &persist::RbmkColumn::of(b)).unwrap().moderated).collect();
        assert_eq!(moderated, vec!["ntm:rbmk_rod_mod", "ntm:rbmk_rod_reasim_mod", "ntm:rbmk_control_mod"]);
    }
}

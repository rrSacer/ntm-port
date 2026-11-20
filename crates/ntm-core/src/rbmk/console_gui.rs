//! The RBMK console's screen (`GUIRBMKConsole`, T10), its logic: what each cell of the map draws from the console's
//! last scan, the hover texts, and what a click does (the selection, the AZ-5 lid and its 3 s, the sounds, the
//! commands). The glue draws the parts and plays the sounds; nothing here touches the game.
use super::console::{self, ColumnData, ColumnType, ColumnView, CELLS};
use crate::screens::control::{Fields, Value};
use crate::screens::number::{self, Saved};

/// `xSize`, `ySize`.
pub const SIZE: [i32; 2] = [244, 172];
/// The map's top left (`bX`, `bY`) and a cell's side.
const MAP: [i32; 2] = [86, 11];
const CELL: i32 = 10;
/// `GuiTextField(guiLeft + 9, guiTop + 84, 35, 9)`, of three characters.
pub const FIELD: [i32; 4] = [9, 84, 35, 9];
pub const FIELD_MAX: usize = 3;
/// `TileEntityRBMKBase.maxHeat`: the same for every column.
const MAX_HEAT: f64 = 1500.0;

/// A part of the screen's texture: `uv` (x, y, width, height) drawn at `at`, from the screen's top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub uv: [i32; 4],
    pub at: [i32; 2],
}

/// `ColumnType.offset`: where the type's cell is on the texture's row 172.
pub fn type_offset(kind: ColumnType) -> i32 {
    match kind {
        ColumnType::Blank => 0,
        ColumnType::Fuel => 10,
        ColumnType::FuelSim => 90,
        ColumnType::Control => 20,
        ColumnType::ControlAuto => 30,
        ColumnType::Boiler => 40,
        ColumnType::Moderator => 50,
        ColumnType::Absorber => 60,
        ColumnType::Reflector => 70,
        ColumnType::Outgasser => 80,
        ColumnType::Breeder => 100,
        ColumnType::Storage => 110,
        ColumnType::Cooler => 120,
        ColumnType::Heatex => 130,
    }
}

/// `ColumnType.toString()`: the enum's name.
pub fn type_name(kind: ColumnType) -> &'static str {
    match kind {
        ColumnType::Blank => "BLANK",
        ColumnType::Fuel => "FUEL",
        ColumnType::FuelSim => "FUEL_SIM",
        ColumnType::Control => "CONTROL",
        ColumnType::ControlAuto => "CONTROL_AUTO",
        ColumnType::Boiler => "BOILER",
        ColumnType::Moderator => "MODERATOR",
        ColumnType::Absorber => "ABSORBER",
        ColumnType::Reflector => "REFLECTOR",
        ColumnType::Outgasser => "OUTGASSER",
        ColumnType::Breeder => "BREEDER",
        ColumnType::Storage => "STORAGE",
        ColumnType::Cooler => "COOLER",
        ColumnType::Heatex => "HEATEX",
    }
}

/// `(int) Math.ceil(v)`: Java's cast saturates and makes NaN 0.
fn ceil(v: f64) -> i32 {
    v.ceil() as i32
}

/// The id of the `Fluids` named `name`.
fn fluid(name: &str) -> u16 {
    crate::fluid::types().iter().find(|t| t.name == name).map_or(u16::MAX, |t| t.id)
}

/// `Fluids.fromID(id).getLocalizedName()`, as the port names its fluids.
pub fn fluid_name(id: u16) -> String {
    crate::fluid::types().iter().find(|t| t.id == id).map_or_else(|| "NONE".into(), |t| crate::fluid::label(&t.string_id))
}

/// A bar of height `h` whose bottom is a pixel above the cell's: `uv` from (`u`, `191 - h`).
fn bar(out: &mut Vec<Part>, x: i32, y: i32, u: i32, w: i32, h: i32) {
    if h > 0 {
        out.push(Part { uv: [u, 191 - h, w, h], at: [x, y + CELL - h - 1] });
    }
}

/// `drawGuiContainerBackgroundLayer`'s loop for the cell of window index `i`: its type, its heat, what its type
/// shows, and the selection's frame. A height of 0 or less draws nothing.
pub fn cell_parts(i: usize, view: &ColumnView, selected: bool) -> Vec<Part> {
    let (x, y) = (MAP[0] + CELL * (i as i32 % 15), MAP[1] + CELL * (i as i32 / 15));
    let mut out = vec![Part { uv: [type_offset(view.kind), 172, CELL, CELL], at: [x, y] }];
    let h = ceil((view.heat - 20.0) * 10.0 / MAX_HEAT).min(10);
    if h > 0 {
        out.push(Part { uv: [0, 192 - h, 10, h], at: [x, y + CELL - h] });
    }
    match (view.kind, &view.data) {
        (ColumnType::Control | ColumnType::ControlAuto, ColumnData::Control { level, color }) => {
            // CONTROL falls through to CONTROL_AUTO's level.
            if view.kind == ColumnType::Control && color.is_some_and(|c| c > -1) {
                out.push(Part { uv: [i32::from(color.unwrap()) * CELL, 202, 10, 10], at: [x, y] });
            }
            let fr = 8 - ceil(level * 8.0);
            if fr > 0 {
                out.push(Part { uv: [24, 183, 2, fr], at: [x + 4, y + 1] });
            }
        }
        (ColumnType::Fuel | ColumnType::FuelSim, ColumnData::Fuel(Some([enrichment, xenon, hull, _, melting]))) => {
            bar(&mut out, x + 1, y, 11, 2, ceil((hull - 20.0) * 8.0 / melting).min(8));
            bar(&mut out, x + 4, y, 14, 2, ceil(enrichment * 8.0).min(8));
            bar(&mut out, x + 7, y, 17, 2, ceil(xenon * 8.0 / 100.0).min(8));
        }
        (ColumnType::Boiler | ColumnType::Heatex, ColumnData::Tanks { water, max_water, steam, max_steam, kind, .. }) => {
            // `getInteger(..) * 8 / getDouble(..)`: an int product, then a double quotient; no cap.
            let (fw, fs) = (ceil(f64::from(water.wrapping_mul(8)) / f64::from(*max_water)), ceil(f64::from(steam.wrapping_mul(8)) / f64::from(*max_steam)));
            let u = if view.kind == ColumnType::Boiler { 41 } else { 131 };
            bar(&mut out, x + 1, y, u, 3, fw);
            bar(&mut out, x + 6, y, u + 5, 3, fs);
            if view.kind == ColumnType::Boiler {
                for (n, name) in ["STEAM", "HOTSTEAM", "SUPERHOTSTEAM", "ULTRAHOTSTEAM"].iter().enumerate() {
                    if *kind == fluid(name) {
                        out.push(Part { uv: [44, 183 + 2 * n as i32, 2, 2], at: [x + 4, y + 1 + 2 * n as i32] });
                    }
                }
            }
        }
        _ => {}
    }
    if selected {
        out.push(Part { uv: [0, 192, 10, 10], at: [x, y] });
    }
    out
}

/// The parts over the background: the AZ-5 lid while it is shut, each screen's type, then every cell seen.
pub fn parts(gui: &Gui, scan: &[Option<ColumnView>], screens: &[u8; 6]) -> Vec<Part> {
    let mut out = Vec::new();
    if gui.lid {
        out.push(Part { uv: [228, 172, 28, 28], at: [30, 138] });
    }
    for j in 0..3 {
        for k in 0..2 {
            let id = (j * 2 + k) as usize;
            out.push(Part { uv: [i32::from(screens[id]) * 18, 238, 18, 18], at: [6 + 40 * k, 8 + 21 * j] });
        }
    }
    for (i, c) in scan.iter().enumerate().take(CELLS as usize) {
        if let Some(c) = c {
            out.extend(cell_parts(i, c, gui.selection[i]));
        }
    }
    out
}

/// `(int) v / d` written as `Double.toString` writes it.
fn trunc(v: f64, d: f64) -> String {
    super::tooltip::java_double(f64::from(v as i32) / d)
}

/// `RBMKColumn.getFancyStats` after the type's name: the hover text of a cell, in the English of `en_US.lang`.
pub fn stats(view: &ColumnView) -> Vec<String> {
    let mut out = vec![type_name(view.kind).to_string(), format!("§eColumn temperature: {}°C", trunc(view.heat * 10.0, 10.0))];
    match (view.kind, &view.data) {
        (ColumnType::Fuel | ColumnType::FuelSim, ColumnData::Fuel(rod)) => {
            // A channel without a rod wrote no keys: `getDouble` reads 0.
            let [e, x, hull, core, melting] = rod.unwrap_or([0.0; 5]);
            out.push(format!("§aDepletion: {}%", trunc((1.0 - e) * 100000.0, 1000.0)));
            out.push(format!("§5Xenon poison: {}%", trunc(x * 1000.0, 1000.0)));
            out.push(format!("§4Core temperature: {}°C", trunc(core * 10.0, 10.0)));
            out.push(format!("§cSkin temperature: {}°C / {}°C", trunc(hull * 10.0, 10.0), trunc(melting * 10.0, 10.0)));
        }
        (ColumnType::Boiler, ColumnData::Tanks { water, max_water, steam, max_steam, kind, .. }) => {
            out.push(format!("§9Feedwater: {water} / {max_water}"));
            out.push(format!("§fSteam: {steam} / {max_steam}"));
            out.push(format!("§eCompressor: {}", fluid_name(*kind)));
        }
        (ColumnType::Control | ColumnType::ControlAuto, ColumnData::Control { level, color }) => {
            const GROUPS: [&str; 5] = ["§cRed Group", "§eYellow Group", "§aGreen Group", "§1Blue Group", "§5Purple Group"];
            if view.kind == ColumnType::Control
                && let Some(g) = color.and_then(|c| usize::try_from(c).ok()).and_then(|c| GROUPS.get(c))
            {
                out.push(format!("§e{g}"));
            }
            out.push(format!("§e{}%", (level * 100.0) as i32));
        }
        (ColumnType::Heatex, ColumnData::Tanks { water, max_water, steam, max_steam, kind, hot }) => {
            out.push(format!("§9{} {water}/{max_water}mB", fluid_name(*kind)));
            out.push(format!("§c{} {steam}/{max_steam}mB", fluid_name(*hot)));
        }
        _ => {}
    }
    if view.moderated {
        out.push("§eModerated".into());
    }
    out
}

/// `drawCustomInfoStat` and `mouseClicked`'s tests: x from the left edge in, y past the top edge.
fn inside(rect: [i32; 4], x: i32, y: i32) -> bool {
    rect[0] <= x && rect[0] + rect[2] > x && rect[1] < y && rect[1] + rect[3] >= y
}

/// `ScreenType`'s `rbmk.console.<type>` in English.
const SCREEN_TEXTS: [&str; 6] = [
    "Off",
    "Monitor average column temperature",
    "Monitor average control rod extraction",
    "Monitor average fuel depletion",
    "Monitor average xenon poisoning",
    "Monitor average fuel temperature",
];
const GROUP_NAMES: [(&str, &str); 5] = [("§c", "red"), ("§e", "yellow"), ("§a", "green"), ("§9", "blue"), ("§d", "purple")];

/// `drawScreen`'s hover texts at (`x`, `y`) from the screen's top left: a cell seen (index above 0, as the Java
/// tests it), the buttons, the screens.
pub fn tooltip(scan: &[Option<ColumnView>], screens: &[u8; 6], x: i32, y: i32) -> Vec<String> {
    // `guiTop + 11 + 10150 >= mouseY`: the Java's test reaches far below the map; the index keeps it inside.
    if MAP[0] <= x && MAP[0] + 150 > x && MAP[1] < y {
        let index = (x - MAP[0]) / CELL + (y - MAP[1]) / CELL * 15;
        if index > 0
            && index < CELLS
            && let Some(Some(c)) = scan.get(index as usize)
        {
            return stats(c);
        }
    }
    if inside([61, 70, 10, 10], x, y) {
        return vec!["Select all control rods".into()];
    }
    if inside([72, 70, 10, 10], x, y) {
        return vec!["Deselect all".into()];
    }
    for i in 0..3 {
        for j in 0..2 {
            let id = i * 2 + j + 1;
            if inside([6 + 40 * j, 8 + 21 * i, 18, 18], x, y) {
                return vec![format!("§e{}", SCREEN_TEXTS[usize::from(screens[(id - 1) as usize]).min(5)])];
            }
            if inside([24 + 40 * j, 8 + 21 * i, 18, 18], x, y) {
                return vec![format!("Assign selected columns to screen #{id}")];
            }
        }
    }
    for (k, (code, name)) in GROUP_NAMES.iter().enumerate() {
        if inside([6 + 11 * k as i32, 70, 10, 10], x, y) {
            return vec![format!("{code}Left click: Select {name} group"), format!("{code}Right click: Assign {name} group")];
        }
    }
    if inside([70, 82, 12, 12], x, y) {
        return vec!["Cycle steam channel compressor setting".into()];
    }
    Vec::new()
}

/// What the open screen keeps: the selection, the AZ-5 lid and the last AZ-5 (`System.currentTimeMillis`).
#[derive(Clone, Debug, PartialEq)]
pub struct Gui {
    pub selection: Vec<bool>,
    pub lid: bool,
    pub last_press: i64,
}

impl Default for Gui {
    fn default() -> Self {
        Gui { selection: vec![false; CELLS as usize], lid: true, last_press: 0 }
    }
}

/// What a click did: the sounds to play (`gui.button.press` and the mod's, by pitch, at the GUI's volume), the
/// commands to send, and the field's new text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Click {
    pub sounds: Vec<(&'static str, f32)>,
    pub send: Vec<Fields>,
    pub field: Option<String>,
}

const PRESS: &str = "minecraft:ui.button.click";

fn picked(scan: &[Option<ColumnView>], selection: &[bool], kind: ColumnType) -> Vec<i32> {
    (0..CELLS).filter(|&j| scan.get(j as usize).is_some_and(|c| c.as_ref().is_some_and(|c| c.kind == kind)) && selection[j as usize]).collect()
}

fn of_kind(scan: &[Option<ColumnView>], kind: ColumnType) -> impl Iterator<Item = usize> + '_ {
    (0..CELLS as usize).filter(move |&j| scan.get(j).is_some_and(|c| c.as_ref().is_some_and(|c| c.kind == kind)))
}

impl Gui {
    /// `mouseClicked` at (`x`, `y`) from the screen's top left with `button` (0 left, 1 right), `now` in ms, the field
    /// holding `field` (the field's own click is the glue's, before this).
    pub fn click(&mut self, scan: &[Option<ColumnView>], x: i32, y: i32, button: i32, now: i64, field: &str) -> Click {
        let mut out = Click::default();
        let control = |level: f64, sel: &mut dyn Iterator<Item = usize>| {
            let mut f = Fields::default();
            f.set("level", Value::Double(level));
            for j in sel {
                f.set(&format!("sel_{j}"), Value::Int(j as i32));
            }
            f
        };
        // Toggle a column's selection.
        if MAP[0] <= x && MAP[0] + 150 > x && MAP[1] < y && MAP[1] + 150 >= y {
            let index = ((x - MAP[0]) / CELL + (y - MAP[1]) / CELL * 15) as usize;
            if index < CELLS as usize && scan.get(index).is_some_and(Option::is_some) {
                self.selection[index] = !self.selection[index];
                out.sounds.push((PRESS, 0.75 + if self.selection[index] { 0.25 } else { 0.0 }));
                return out;
            }
        }
        if inside([72, 70, 10, 10], x, y) {
            self.selection = vec![false; CELLS as usize];
            out.sounds.push((PRESS, 0.5));
            return out;
        }
        if inside([61, 70, 10, 10], x, y) {
            self.selection = vec![false; CELLS as usize];
            for j in of_kind(scan, ColumnType::Control) {
                self.selection[j] = true;
            }
            out.sounds.push((PRESS, 1.5));
            return out;
        }
        if inside([70, 82, 12, 12], x, y) {
            let mut f = Fields::default();
            f.set("compressor", Value::Byte(1)).set("cols", Value::IntArray(picked(scan, &self.selection, ColumnType::Boiler)));
            out.send.push(f);
            out.sounds.push((PRESS, 1.0));
        }
        for k in 0..5 {
            if !inside([6 + k * 11, 70, 10, 10], x, y) {
                continue;
            }
            if button == 0 {
                self.selection = vec![false; CELLS as usize];
                for j in of_kind(scan, ColumnType::Control) {
                    if matches!(scan[j].as_ref().map(|c| &c.data), Some(ColumnData::Control { color: Some(c), .. }) if i32::from(*c) == k) {
                        self.selection[j] = true;
                    }
                }
            }
            if button == 1 {
                let mut f = Fields::default();
                f.set("assignColor", Value::Byte(k as i8)).set("cols", Value::IntArray(picked(scan, &self.selection, ColumnType::Control)));
                out.send.push(f);
            }
            out.sounds.push((PRESS, 0.8 + k as f32 * 0.1));
            return out;
        }
        if inside([30, 138, 28, 28], x, y) {
            if self.lid {
                self.lid = false;
                out.sounds.push(("ntm:block.rbmk_az5_cover", 0.5));
            } else if self.last_press + 3000 < now {
                out.sounds.push(("ntm:block.shutdown", 1.0));
                self.last_press = now;
                let kinds: Vec<Option<ColumnType>> = scan.iter().map(|c| c.as_ref().map(|c| c.kind)).collect();
                out.send.push(console::az5(&kinds));
                out.sounds.push((PRESS, 1.0));
            }
            return out;
        }
        if inside([48, 82, 12, 12], x, y) {
            // `isNumber` then `(int) clamp(parseDouble(..), 0, 100)`; a text `parseDouble` throws on would crash the
            // 1.7.10 client there: here nothing is sent.
            let Saved::Number(j) = number::save(field, 100.0) else { return out };
            out.field = Some(j.to_string());
            let selection = self.selection.clone();
            out.send.push(control(f64::from(j) * 0.01, &mut (0..CELLS as usize).filter(|&s| selection[s])));
            out.sounds.push((PRESS, 1.0));
        }
        for j in 0..3 {
            for k in 0..2 {
                let id = j * 2 + k;
                if inside([6 + 40 * k, 8 + 21 * j, 18, 18], x, y) {
                    let mut f = Fields::default();
                    f.set("toggle", Value::Byte(id as i8));
                    out.send.push(f);
                    out.sounds.push((PRESS, 0.5));
                    return out;
                }
                if inside([24 + 40 * k, 8 + 21 * j, 18, 18], x, y) {
                    let mut f = Fields::default();
                    f.set("id", Value::Byte(id as i8));
                    for s in (0..CELLS as usize).filter(|&s| self.selection[s]) {
                        f.set(&format!("s{s}"), Value::Byte(1));
                    }
                    out.send.push(f);
                    out.sounds.push((PRESS, 0.75));
                    return out;
                }
            }
        }
        out
    }
}

/// The flux graph: the buffer's highest and lowest, and each segment's ends from the screen's top left
/// (`x = 7 + k·74/60`, `y = 127 - (flux - lowest)·24/max(range, 1)`), green, two pixels wide.
pub fn flux_graph(buffer: &[i32]) -> (i32, i32, Vec<[[f64; 2]; 2]>) {
    let highest = buffer.iter().copied().fold(i32::MIN, i32::max);
    let lowest = buffer.iter().copied().fold(i32::MAX, i32::min);
    // `int range = highest - lowest`: Java's int arithmetic wraps.
    let range = highest.wrapping_sub(lowest).max(1);
    let n = buffer.len() as f64;
    let point = |k: usize| [7.0 + k as f64 * 74.0 / n, 127.0 - f64::from(buffer[k].wrapping_sub(lowest)) * 24.0 / f64::from(range)];
    let lines = (0..buffer.len().saturating_sub(1)).map(|i| [point(i), point(i + 1)]).collect();
    (highest, lowest, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(kind: ColumnType, heat: f64, data: ColumnData) -> ColumnView {
        ColumnView { kind, heat, moderated: false, data }
    }

    fn scan(cells: &[(usize, ColumnView)]) -> Vec<Option<ColumnView>> {
        let mut s = vec![None; CELLS as usize];
        for (i, v) in cells {
            s[*i] = Some(v.clone());
        }
        s
    }

    /// A cell's parts as the Java draws them: the type, the heat from the bottom, a manual rod's group and its level
    /// from the top, a rod's three bars, a boiler's tanks and compressor light, the selection.
    #[test]
    fn a_cell_draws_what_its_column_wrote() {
        let rod = view(ColumnType::Control, 770.0, ColumnData::Control { level: 0.5, color: Some(2) });
        assert_eq!(
            cell_parts(16, &rod, true),
            vec![
                Part { uv: [20, 172, 10, 10], at: [96, 21] },
                // ceil(750 · 10 / 1500) = 5.
                Part { uv: [0, 187, 10, 5], at: [96, 26] },
                Part { uv: [20, 202, 10, 10], at: [96, 21] },
                Part { uv: [24, 183, 2, 4], at: [100, 22] },
                Part { uv: [0, 192, 10, 10], at: [96, 21] },
            ]
        );
        // No group (-1), all the way in (no bar), cold (no heat), an automatic rod never shows a group.
        let none = view(ColumnType::Control, 20.0, ColumnData::Control { level: 1.0, color: Some(-1) });
        assert_eq!(cell_parts(0, &none, false), vec![Part { uv: [20, 172, 10, 10], at: [86, 11] }]);
        let auto = view(ColumnType::ControlAuto, 20.0, ColumnData::Control { level: 0.0, color: Some(3) });
        assert_eq!(cell_parts(0, &auto, false)[1..], [Part { uv: [24, 183, 2, 8], at: [90, 12] }]);
        // A rod: hull (1020 - 20) · 8 / 2000 = 4, enrichment 0.5 → 4, xenon 50 % → 4; capped at 8.
        let fuel = view(ColumnType::Fuel, 20.0, ColumnData::Fuel(Some([0.5, 50.0, 1020.0, 0.0, 2000.0])));
        assert_eq!(
            cell_parts(0, &fuel, false)[1..],
            [Part { uv: [11, 187, 2, 4], at: [87, 16] }, Part { uv: [14, 187, 2, 4], at: [90, 16] }, Part { uv: [17, 187, 2, 4], at: [93, 16] }]
        );
        let hot = view(ColumnType::FuelSim, 5000.0, ColumnData::Fuel(Some([2.0, 900.0, 9e9, 0.0, 10.0])));
        assert_eq!(cell_parts(0, &hot, false)[1..], [
            Part { uv: [0, 182, 10, 10], at: [86, 11] },
            Part { uv: [11, 183, 2, 8], at: [87, 12] },
            Part { uv: [14, 183, 2, 8], at: [90, 12] },
            Part { uv: [17, 183, 2, 8], at: [93, 12] }
        ]);
        assert_eq!(cell_parts(0, &view(ColumnType::Fuel, 20.0, ColumnData::Fuel(None)), false).len(), 1, "no rod: no bars");
        // A boiler: water 5000 / 10000 → 4, steam 1 / 1000000 → 1; dense steam's light is the second.
        let boiler = view(ColumnType::Boiler, 20.0, ColumnData::Tanks { water: 5000, max_water: 10000, steam: 1, max_steam: 1_000_000, kind: fluid("HOTSTEAM"), hot: 0 });
        assert_eq!(
            cell_parts(0, &boiler, false)[1..],
            [Part { uv: [41, 187, 3, 4], at: [87, 16] }, Part { uv: [46, 190, 3, 1], at: [92, 19] }, Part { uv: [44, 185, 2, 2], at: [90, 14] }]
        );
        let heatex = view(ColumnType::Heatex, 20.0, ColumnData::Tanks { water: 16000, max_water: 16000, steam: 0, max_steam: 16000, kind: 0, hot: 0 });
        assert_eq!(cell_parts(0, &heatex, false)[1..], [Part { uv: [131, 183, 3, 8], at: [87, 12] }]);
    }

    /// The lid, then the six screens' types, then the cells seen (the selection only where a column is).
    #[test]
    fn the_screen_draws_its_lid_screens_and_map() {
        let mut gui = Gui::default();
        let s = scan(&[(224, view(ColumnType::Blank, 20.0, ColumnData::None))]);
        let p = parts(&gui, &s, &[0, 1, 2, 3, 4, 5]);
        assert_eq!(p[0], Part { uv: [228, 172, 28, 28], at: [30, 138] });
        assert_eq!(p[1..7].iter().map(|p| (p.uv[0], p.at)).collect::<Vec<_>>(), vec![(0, [6, 8]), (18, [46, 8]), (36, [6, 29]), (54, [46, 29]), (72, [6, 50]), (90, [46, 50])]);
        assert_eq!(p[7], Part { uv: [0, 172, 10, 10], at: [226, 151] });
        gui.lid = false;
        gui.selection[3] = true;
        assert_eq!(parts(&gui, &s, &[0; 6]).len(), 7, "no lid; a selected empty cell draws nothing");
    }

    /// `getFancyStats` and its type name, in the Java's numbers.
    #[test]
    fn a_cells_hover_text_is_its_fancy_stats() {
        let mut fuel = view(ColumnType::Fuel, 123.456, ColumnData::Fuel(Some([0.876543, 1.23456, 500.04, 812.39, 2865.0])));
        assert_eq!(
            stats(&fuel),
            vec!["FUEL", "§eColumn temperature: 123.4°C", "§aDepletion: 12.345%", "§5Xenon poison: 1.234%", "§4Core temperature: 812.3°C", "§cSkin temperature: 500.0°C / 2865.0°C"]
        );
        fuel.data = ColumnData::Fuel(None);
        assert_eq!(stats(&fuel)[2..], ["§aDepletion: 100.0%", "§5Xenon poison: 0.0%", "§4Core temperature: 0.0°C", "§cSkin temperature: 0.0°C / 0.0°C"]);
        let mut rod = view(ColumnType::Control, 20.0, ColumnData::Control { level: 0.999, color: Some(4) });
        rod.moderated = true;
        assert_eq!(stats(&rod)[2..], ["§e§5Purple Group", "§e99%", "§eModerated"]);
        rod.data = ColumnData::Control { level: 0.5, color: Some(-1) };
        assert_eq!(stats(&rod)[2..], ["§e50%", "§eModerated"]);
        let boiler = view(ColumnType::Boiler, 20.0, ColumnData::Tanks { water: 1, max_water: 2, steam: 3, max_steam: 4, kind: fluid("STEAM"), hot: 0 });
        assert_eq!(stats(&boiler)[2..], ["§9Feedwater: 1 / 2", "§fSteam: 3 / 4", &format!("§eCompressor: {}", fluid_name(fluid("STEAM")))]);
        assert_eq!(stats(&view(ColumnType::Absorber, 1e8, ColumnData::None)), ["ABSORBER", "§eColumn temperature: 1.0E8°C"]);
    }

    /// The hover texts of the buttons and screens (1-based in the assign text), and none for the map's first cell (the
    /// Java tests `index > 0`).
    #[test]
    fn the_buttons_say_what_they_do() {
        let s = scan(&[(0, view(ColumnType::Blank, 20.0, ColumnData::None)), (1, view(ColumnType::Blank, 20.0, ColumnData::None))]);
        assert!(tooltip(&s, &[0; 6], 87, 12).is_empty());
        assert_eq!(tooltip(&s, &[0; 6], 97, 12)[0], "BLANK");
        assert_eq!(tooltip(&s, &[0, 0, 0, 2, 0, 0], 47, 30), ["§eMonitor average control rod extraction"]);
        assert_eq!(tooltip(&s, &[0; 6], 64, 51), ["Assign selected columns to screen #6"]);
        assert_eq!(tooltip(&s, &[0; 6], 50, 71), ["§dLeft click: Select purple group", "§dRight click: Assign purple group"]);
        assert_eq!(tooltip(&s, &[0; 6], 61, 80), ["Select all control rods"]);
        assert_eq!(tooltip(&s, &[0; 6], 61, 70), Vec::<String>::new(), "the top edge is out");
        assert_eq!(tooltip(&s, &[0; 6], 81, 94), ["Cycle steam channel compressor setting"]);
    }

    fn rods() -> Vec<Option<ColumnView>> {
        scan(&[
            (3, view(ColumnType::Control, 20.0, ColumnData::Control { level: 1.0, color: Some(1) })),
            (4, view(ColumnType::Control, 20.0, ColumnData::Control { level: 1.0, color: Some(-1) })),
            (5, view(ColumnType::ControlAuto, 20.0, ColumnData::Control { level: 1.0, color: None })),
            (6, view(ColumnType::Boiler, 20.0, ColumnData::Tanks { water: 0, max_water: 1, steam: 0, max_steam: 1, kind: 0, hot: 0 })),
        ])
    }

    /// The selection: a cell with a column toggles, an empty one does not; all the manual rods; a group (left);
    /// clearing.
    #[test]
    fn clicks_pick_columns() {
        let s = rods();
        let mut g = Gui::default();
        let at = |i: i32| (86 + 10 * (i % 15) + 1, 11 + 10 * (i / 15) + 1);
        let (x, y) = at(6);
        assert_eq!(g.click(&s, x, y, 0, 0, "").sounds, [(PRESS, 1.0)]);
        assert!(g.selection[6]);
        assert_eq!(g.click(&s, x, y, 0, 0, "").sounds, [(PRESS, 0.75)]);
        let (x, y) = at(7);
        assert_eq!(g.click(&s, x, y, 0, 0, ""), Click::default(), "no column there");
        g.click(&s, 61, 71, 0, 0, "");
        assert_eq!((0..8).filter(|&i| g.selection[i]).collect::<Vec<_>>(), vec![3, 4], "the manual rods only");
        assert_eq!(g.click(&s, 17, 71, 0, 0, "").sounds, [(PRESS, 0.8 + 0.1)]);
        assert_eq!((0..8).filter(|&i| g.selection[i]).collect::<Vec<_>>(), vec![3], "the yellow group");
        g.click(&s, 72, 71, 0, 0, "");
        assert!(g.selection.iter().all(|s| !s));
    }

    /// The commands: a group to the picked manual rods (right), the compressor to the picked boilers, a level to all
    /// that is picked, a screen's type and its columns.
    #[test]
    fn clicks_send_commands() {
        let s = rods();
        let mut g = Gui::default();
        for i in [3, 5, 6] {
            g.selection[i] = true;
        }
        let c = g.click(&s, 39, 71, 1, 0, "");
        assert_eq!((c.send[0].get_byte("assignColor"), c.send[0].get_int_array("cols").unwrap()), (3, vec![3]));
        let c = g.click(&s, 70, 83, 0, 0, "");
        assert_eq!((c.send[0].get_boolean("compressor"), c.send[0].get_int_array("cols").unwrap()), (true, vec![6]));
        let c = g.click(&s, 48, 83, 0, 0, "150");
        assert_eq!(c.field.as_deref(), Some("100"));
        assert_eq!((c.send[0].get_double("level"), c.send[0].0.keys().filter(|k| k.starts_with("sel_")).count()), (1.0, 3));
        assert_eq!(c.send[0].get_integer("sel_6"), 6);
        assert_eq!(g.click(&s, 48, 83, 0, 0, "abc"), Click::default(), "not a number: nothing");
        assert_eq!(g.click(&s, 48, 83, 0, 0, "1L"), Click::default(), "the Java's crash: nothing");
        let c = g.click(&s, 46, 51, 0, 0, "");
        assert_eq!((c.send[0].get_byte("toggle"), c.sounds.clone()), (5, vec![(PRESS, 0.5)]));
        let c = g.click(&s, 24, 9, 0, 0, "");
        assert_eq!(c.send[0].get_byte("id"), 0);
        assert_eq!(c.send[0].0.keys().filter(|k| k.starts_with('s')).count(), 3);
    }

    /// AZ-5: the first press opens the lid; then a press sends level 0 to every manual rod seen, and the next works only
    /// more than 3 s after it.
    #[test]
    fn az5_has_a_lid_and_three_seconds() {
        let s = rods();
        let mut g = Gui::default();
        let c = g.click(&s, 31, 139, 0, 10_000, "");
        assert_eq!((c.sounds, c.send.len(), g.lid), (vec![("ntm:block.rbmk_az5_cover", 0.5)], 0, false));
        let c = g.click(&s, 31, 139, 0, 10_000, "");
        assert_eq!(c.sounds, [("ntm:block.shutdown", 1.0), (PRESS, 1.0)]);
        assert_eq!(c.send[0], console::az5(&s.iter().map(|c| c.as_ref().map(|c| c.kind)).collect::<Vec<_>>()));
        assert_eq!(c.send[0].0.keys().filter(|k| k.starts_with("sel_")).count(), 2);
        assert_eq!(g.click(&s, 31, 139, 0, 13_000, ""), Click::default(), "3 s are not more than 3 s");
        assert_eq!(g.click(&s, 31, 139, 0, 13_001, "").send.len(), 1);
    }

    /// The graph spans the buffer's range over 24 pixels; a flat one sits on its bottom line.
    #[test]
    fn the_flux_graph_spans_its_range() {
        let mut b = vec![0; 60];
        b[59] = 48;
        let (hi, lo, lines) = flux_graph(&b);
        assert_eq!((hi, lo, lines.len()), (48, 0, 59));
        assert_eq!(lines[0][0], [7.0, 127.0]);
        assert_eq!(lines[58][1], [7.0 + 59.0 * 74.0 / 60.0, 103.0]);
        let (_, _, flat) = flux_graph(&[5; 60]);
        assert!(flat.iter().all(|l| l[0][1] == 127.0 && l[1][1] == 127.0));
    }
}

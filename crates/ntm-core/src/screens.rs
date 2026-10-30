//! The machines' screens (L2): what `horizon_screen/<name>.json` of the
//! content says, read and worked out without the engine, so `cargo test`
//! covers it. The bundle's glue (native/ntm/mixins/ntm/screens.rs) draws it
//! on the native screen hooks. Ported from the fork's `mod_screens.rs`.
//!
//! Coordinates are texture pixels from the screen's top left. The glue draws,
//! in order: the background, the bars, the gauges, the overlays, the texts.
//! The title and the "Inventory" label go with them (the engine leaves them
//! out: `NativeScreen::own_labels`). Values are the menu's data slots, i32.
//!
//! A screen file: `texture`, `size`, `title` (`[x, y]` or `[x, y, "center"]`),
//! `title_color`, `inventory_label`, `bars`, `gauges`, `overlays`, `tooltips`,
//! `texts`, `buttons`, `slots` (see [`ScreenDef`]). A machine file
//! (`horizon_machine/<block>.json`) names its `screen`, the other `screens`
//! its buttons switch to, its `title`, the slots' places (`x`/`y` or a
//! `grid` `[x, y, columns, rows]`) and the player `inventory`'s top left.
pub mod list;

use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Max {
    /// The index of a value holding the maximum.
    Value(usize),
    Fixed(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub value: usize,
    /// The value at which it starts to fill.
    pub from: i32,
    pub max: Max,
    pub rect: [f32; 4],
    pub uv: Option<[f32; 2]>,
    /// The index of a value holding 0xRRGGBB, in place of `uv`.
    pub color: Option<usize>,
    pub dir: Dir,
}

/// HBM's `GUIElements.drawSmoothGauge`: a needle turning round its centre.
#[derive(Clone, Debug, PartialEq)]
pub struct Gauge {
    pub value: usize,
    pub max: Max,
    pub at: [f32; 2],
    /// The needle's length forward, back, and its half width at the back.
    pub tip: f32,
    pub back: f32,
    pub side: f32,
    /// 0xRRGGBB.
    pub color: u32,
}

/// The needle's three corners from its centre at `progress` (0..=1), as
/// drawSmoothGauge turns them (`Vec3.rotateAroundZ`, GUI y down), scaled
/// by `mult` (1.5 for its black outline).
pub fn needle(progress: f32, tip: f32, back: f32, side: f32, mult: f32) -> [[f32; 2]; 3] {
    let a = (-progress.clamp(0.0, 1.0) * 270.0 - 45.0).to_radians();
    let (sin, cos) = a.sin_cos();
    let turn = |[x, y]: [f32; 2]| [(x * cos + y * sin) * mult, (y * cos - x * sin) * mult];
    [turn([0.0, tip]), turn([side, -back]), turn([-side, -back])]
}

#[derive(Clone, Debug, PartialEq)]
pub enum When {
    Slot(usize),
    Value { index: usize, min: i32, max: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    pub when: When,
    pub rect: [f32; 4],
    pub uv: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub rect: [f32; 4],
    pub lines: Vec<String>,
}

/// Where a line of `texts` sits against its `at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// A line written on the screen, with values filled in ([`format`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: [f32; 2],
    pub text: String,
    /// 0xRRGGBB.
    pub color: u32,
    pub align: Align,
    pub shadow: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub id: u32,
    /// The screen it switches to, when it sends nothing.
    pub screen: Option<String>,
    /// The id sent while shift is held, when not `id`.
    pub shift: Option<u32>,
    pub rect: [f32; 4],
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenDef {
    /// The texture's sprite in the GUI atlas: `ntm:gui/processing/gui_assembler`.
    pub sprite: String,
    pub size: [f32; 2],
    /// Where the title goes, and whether it is centred on that x.
    pub title: Option<([f32; 2], bool)>,
    /// The title's colour as 0xRRGGBB, when not vanilla's grey.
    pub title_color: Option<u32>,
    pub inventory_label: Option<[f32; 2]>,
    pub bars: Vec<Bar>,
    pub gauges: Vec<Gauge>,
    pub overlays: Vec<Overlay>,
    pub tips: Vec<Tip>,
    pub texts: Vec<Label>,
    pub buttons: Vec<Button>,
    /// The machine's slots this screen moves (`Some`) or leaves out (`None`).
    pub slots: HashMap<usize, Option<[f32; 2]>>,
}

/// A texture reference (`ntm:textures/gui/x.png`) as the atlas names it
/// (`ntm:gui/x`).
pub fn texture_sprite(reference: &str) -> Option<String> {
    let (ns, path) = reference.split_once(':')?;
    let path = path.strip_prefix("textures/")?;
    Some(format!("{ns}:{}", path.strip_suffix(".png").unwrap_or(path)))
}

fn floats<const N: usize>(v: &Value) -> Option<[f32; N]> {
    let a = v.as_array().filter(|a| a.len() >= N)?;
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = x.as_f64()? as f32;
    }
    Some(out)
}

fn lines(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn rgb(v: &Value) -> Option<u32> {
    u32::from_str_radix(v.as_str()?.strip_prefix('#')?, 16).ok()
}

fn index(v: &Value) -> Option<usize> {
    v.as_u64().map(|i| i as usize)
}

fn max_of(v: &Value) -> Option<Max> {
    match (index(&v["max"]), v["max_value"].as_i64()) {
        (Some(i), _) => Some(Max::Value(i)),
        (None, Some(v)) => Some(Max::Fixed(v as i32)),
        _ => None,
    }
}

/// Reads one screen file.
pub fn parse(doc: &Value) -> Result<ScreenDef, String> {
    let texture = doc["texture"].as_str().ok_or("no texture")?;
    let sprite = texture_sprite(texture).ok_or("texture: not <ns>:textures/<path>")?;
    let size = floats::<2>(&doc["size"]).unwrap_or([176.0, 166.0]);
    let title = match &doc["title"] {
        Value::Null => Some(([8.0, 6.0], false)),
        t => floats::<2>(t).map(|p| (p, t.get(2).and_then(Value::as_str) == Some("center"))),
    };
    let inventory_label = match doc.get("inventory_label") {
        None => Some([8.0, size[1] - 94.0]),
        Some(v) => floats::<2>(v),
    };
    let dir = |v: &Value| match v.as_str() {
        Some("up") => Dir::Up,
        Some("down") => Dir::Down,
        Some("left") => Dir::Left,
        _ => Dir::Right,
    };
    let mut bars = Vec::new();
    for b in doc["bars"].as_array().into_iter().flatten() {
        bars.push(Bar {
            value: index(&b["value"]).ok_or("a bar without value")?,
            from: b["from"].as_i64().unwrap_or(0) as i32,
            max: max_of(b).ok_or("a bar without max or max_value")?,
            rect: floats::<4>(&b["rect"]).ok_or("a bar without rect")?,
            uv: floats::<2>(&b["uv"]),
            color: index(&b["color"]),
            dir: dir(&b["dir"]),
        });
    }
    let gauges = doc["gauges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let f = |k: &str, d: f32| g[k].as_f64().map_or(d, |v| v as f32);
            Some(Gauge {
                value: index(&g["value"])?,
                max: max_of(g)?,
                at: floats::<2>(&g["at"])?,
                tip: f("tip", 5.0),
                back: f("back", 2.0),
                side: f("side", 1.0),
                color: rgb(&g["color"]).unwrap_or(0x7f0000),
            })
        })
        .collect();
    let mut overlays = Vec::new();
    for o in doc["overlays"].as_array().into_iter().flatten() {
        let when = match (index(&o["slot"]), index(&o["value"])) {
            (Some(s), _) => When::Slot(s),
            (None, Some(i)) => When::Value {
                index: i,
                min: o["min"].as_i64().unwrap_or(1) as i32,
                max: o["max"].as_i64().map_or(i32::MAX, |m| m as i32),
            },
            _ => return Err("an overlay without slot or value".into()),
        };
        overlays.push(Overlay {
            when,
            rect: floats::<4>(&o["rect"]).ok_or("an overlay without rect")?,
            uv: floats::<2>(&o["uv"]).ok_or("an overlay without uv")?,
        });
    }
    let tips = doc["tooltips"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| Some(Tip { rect: floats::<4>(&t["rect"])?, lines: lines(&t["lines"]) }))
        .collect();
    let texts = doc["texts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            Some(Label {
                at: floats::<2>(&t["at"])?,
                text: t["text"].as_str()?.to_owned(),
                color: rgb(&t["color"]).unwrap_or(0x404040),
                align: match t["align"].as_str() {
                    Some("center") => Align::Center,
                    Some("right") => Align::Right,
                    _ => Align::Left,
                },
                shadow: t["shadow"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    let buttons = doc["buttons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| {
            let screen = b["screen"].as_str().map(str::to_owned);
            Some(Button {
                id: b["id"].as_u64().or(screen.as_ref().map(|_| 0))? as u32,
                screen,
                shift: b["shift"].as_u64().map(|i| i as u32),
                rect: floats::<4>(&b["rect"])?,
                lines: lines(&b["lines"]),
            })
        })
        .collect();
    let slots = doc["slots"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.parse().ok()?, floats::<2>(v))))
        .collect();
    Ok(ScreenDef {
        sprite,
        size,
        title,
        title_color: rgb(&doc["title_color"]),
        inventory_label,
        bars,
        gauges,
        overlays,
        tips,
        texts,
        buttons,
        slots,
    })
}

/// What a machine file says about its screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The first screen, `ns:name` (the block's id without one).
    pub screen: String,
    /// The others its buttons switch to.
    pub screens: Vec<String>,
    /// A translation key, or None for the block's own name.
    pub title: Option<String>,
    /// Where each slot sits by default.
    pub slots: Vec<[f32; 2]>,
    /// The player inventory's top left (vanilla's is [8, 84]).
    pub inventory: [f32; 2],
}

/// Block `block`'s machine file (`horizon_machine/<block>.json`, as
/// tools/machine-gen embeds it), read.
pub fn layout(block: &str, file: &Value) -> Layout {
    let mut slots = Vec::new();
    for s in file["slots"].as_array().into_iter().flatten() {
        if let Some([x, y, columns, rows]) = floats::<4>(&s["grid"]) {
            for r in 0..rows.max(0.0) as usize {
                for c in 0..columns.max(0.0) as usize {
                    slots.push([x + c as f32 * 18.0, y + r as f32 * 18.0]);
                }
            }
        } else if let Some([x, y]) = s["x"].as_f64().zip(s["y"].as_f64()).map(|(x, y)| [x as f32, y as f32]) {
            slots.push([x, y]);
        } else {
            slots.push([0.0, 0.0]);
        }
    }
    Layout {
        screen: file["screen"].as_str().unwrap_or(block).to_owned(),
        screens: lines(&file["screens"]),
        title: file["title"].as_str().map(str::to_owned),
        slots,
        inventory: floats::<2>(&file["inventory"]).unwrap_or([8.0, 84.0]),
    }
}

/// Block `block`'s layout, from the machine files embedded by
/// tools/machine-gen (none for a block without one).
pub fn layout_of(block: &str) -> Option<Layout> {
    static FILES: std::sync::OnceLock<HashMap<String, Value>> = std::sync::OnceLock::new();
    let files = FILES.get_or_init(|| serde_json::from_str(include_str!("../data/machine_files.json")).unwrap_or_default());
    files.get(block).map(|file| layout(block, file))
}

/// `{0}` is value 0; `{0:si}` writes it with an SI prefix (`12.5k`), as
/// HBM's `BobMathUtil.getShortNumber`.
pub fn format(line: &str, values: &[i32]) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &rest[start + 1..start + end];
        let (index, si) = match inner.split_once(':') {
            Some((i, "si")) => (i, true),
            _ => (inner, false),
        };
        match index.parse::<usize>().ok().and_then(|i| values.get(i)) {
            Some(&v) if si => out.push_str(&short_number(i64::from(v))),
            Some(v) => out.push_str(&v.to_string()),
            None => out.push_str(&rest[start..=start + end]),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

fn short_number(v: i64) -> String {
    let a = v.unsigned_abs() as f64;
    for (limit, suffix) in [(1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "k")] {
        if a >= limit {
            let x = v as f64 / limit;
            return format!("{}{suffix}", (x * 10.0).round() / 10.0);
        }
    }
    v.to_string()
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// The share of a bar `value` fills between `from` and `max`; None for a
/// bar with nothing to show (`max` not above `from`).
pub fn fill(value: i32, from: i32, max: i32) -> Option<f32> {
    let span = i64::from(max) - i64::from(from);
    (span > 0).then(|| ((i64::from(value) - i64::from(from)) as f32 / span as f32).clamp(0.0, 1.0))
}

/// The part of `rect` a bar of `dir` shows at share `f`, as HBM's GUIs cut
/// it: `[dx, dy, w, h]` from the rect's top left. None for an empty part.
pub fn bar_part(rect: [f32; 4], dir: Dir, f: f32) -> Option<[f32; 4]> {
    let [_, _, w, h] = rect;
    let part = match dir {
        Dir::Right => [0.0, 0.0, (w * f).ceil(), h],
        Dir::Left => [w - (w * f).ceil(), 0.0, (w * f).ceil(), h],
        Dir::Up => [0.0, h - (h * f).ceil(), w, (h * f).ceil()],
        Dir::Down => [0.0, 0.0, w, (h * f).ceil()],
    };
    (part[2] > 0.0 && part[3] > 0.0).then_some(part)
}

impl Max {
    pub fn of(self, values: &[i32]) -> i32 {
        match self {
            Max::Value(i) => values.get(i).copied().unwrap_or(0),
            Max::Fixed(v) => v,
        }
    }
}

impl Overlay {
    /// Whether it shows: `filled(slot)` says a slot holds something.
    pub fn on(&self, values: &[i32], filled: impl Fn(usize) -> bool) -> bool {
        match self.when {
            When::Slot(s) => filled(s),
            When::Value { index, min, max } => {
                (min..=max).contains(&values.get(index).copied().unwrap_or(0))
            }
        }
    }
}

impl ScreenDef {
    /// The tooltip under the pointer (screen-relative), filled in.
    pub fn tooltip(&self, x: f32, y: f32, values: &[i32]) -> Option<Vec<String>> {
        let lines = self
            .tips
            .iter()
            .find(|t| inside(t.rect, x, y))
            .map(|t| &t.lines)
            .or_else(|| {
                self.buttons
                    .iter()
                    .find(|b| inside(b.rect, x, y) && !b.lines.is_empty())
                    .map(|b| &b.lines)
            })?;
        Some(lines.iter().map(|l| format(l, values)).collect())
    }

    /// The id the button under `(x, y)` (screen-relative) sends, `shift`
    /// held or not; None for a button that switches screens.
    pub fn button_at(&self, x: f32, y: f32, shift: bool) -> Option<u32> {
        self.buttons
            .iter()
            .find(|b| inside(b.rect, x, y) && b.screen.is_none())
            .map(|b| b.shift.filter(|_| shift).unwrap_or(b.id))
    }

    /// The screen the button under `(x, y)` switches to.
    pub fn switch_at(&self, x: f32, y: f32) -> Option<&str> {
        self.buttons.iter().find(|b| inside(b.rect, x, y)).and_then(|b| b.screen.as_deref())
    }

    /// Where this screen puts machine slot `i`: `default` (the machine
    /// file's place) unless the screen moves it; None when it leaves it out.
    pub fn slot_at(&self, i: usize, default: [f32; 2]) -> Option<[f32; 2]> {
        self.slots.get(&i).copied().unwrap_or(Some(default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_take_values() {
        assert_eq!(format("{0:si} / {1:si} HE", &[12_500, 1_000_000]), "12.5k / 1M HE");
        assert_eq!(format("{2} ticks {x}", &[1, 2, 3]), "3 ticks {x}");
    }

    #[test]
    fn a_needle_turns_from_lower_left_to_lower_right() {
        let at = |p| needle(p, 5.0, 2.0, 1.0, 1.0)[0];
        let close = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4;
        let d = 5.0 * std::f32::consts::FRAC_1_SQRT_2;
        // GUI y points down: 0 at the lower left, half at the top, 1 at the lower right.
        assert!(close(at(0.0), [-d, d]), "{:?}", at(0.0));
        assert!(close(at(0.5), [0.0, -5.0]), "{:?}", at(0.5));
        assert!(close(at(1.0), [d, d]), "{:?}", at(1.0));
        assert!(close(at(2.0), at(1.0)));
    }

    #[test]
    fn a_bar_fills_from_its_start() {
        assert_eq!(fill(50, 0, 100), Some(0.5));
        // The second of four columns: empty below 250, full from 500.
        assert_eq!(fill(100, 250, 500), Some(0.0));
        assert_eq!(fill(375, 250, 500), Some(0.5));
        assert_eq!(fill(900, 250, 500), Some(1.0));
        assert_eq!(fill(5, 0, 0), None);
        assert_eq!(fill(i32::MAX, i32::MIN, i32::MAX), Some(1.0));
    }

    #[test]
    fn bars_cut_the_part_they_show_as_hbm_does() {
        let rect = [152.0, 18.0, 16.0, 61.0];
        // Up: the lower ceil(h*f) rows, so a nearly empty tank still shows a row.
        assert_eq!(bar_part(rect, Dir::Up, 0.5), Some([0.0, 61.0 - 31.0, 16.0, 31.0]));
        assert_eq!(bar_part(rect, Dir::Up, 0.001), Some([0.0, 60.0, 16.0, 1.0]));
        assert_eq!(bar_part(rect, Dir::Up, 0.0), None);
        assert_eq!(bar_part(rect, Dir::Right, 1.0), Some([0.0, 0.0, 16.0, 61.0]));
        assert_eq!(bar_part(rect, Dir::Left, 0.5), Some([8.0, 0.0, 8.0, 61.0]));
        assert_eq!(bar_part(rect, Dir::Down, 0.5), Some([0.0, 0.0, 16.0, 31.0]));
    }

    #[test]
    fn overlays_follow_a_slot_or_a_range_of_values() {
        let by_value = Overlay { when: When::Value { index: 1, min: 1, max: 3 }, rect: [0.0; 4], uv: [0.0; 2] };
        assert!(by_value.on(&[0, 2], |_| false));
        assert!(!by_value.on(&[0, 4], |_| false));
        assert!(!by_value.on(&[0], |_| false));
        let by_slot = Overlay { when: When::Slot(2), rect: [0.0; 4], uv: [0.0; 2] };
        assert!(by_slot.on(&[], |s| s == 2));
        assert!(!by_slot.on(&[], |s| s == 1));
    }

    #[test]
    fn a_screen_reads() {
        let doc: Value = serde_json::from_str(
            r##"{"texture":"ntm:textures/gui/a.png","size":[176,256],"title":[70,6,"center"],
                "bars":[{"value":0,"max":1,"rect":[1,2,3,4],"uv":[176,0],"dir":"up"}],
                "overlays":[{"slot":0,"rect":[1,1,2,2],"uv":[3,3]}],
                "tooltips":[{"rect":[0,0,10,10],"lines":["{0}"]}],
                "texts":[{"at":[73,31],"text":"{0}","color":"#00ff00","shadow":true},
                         {"at":[88,6],"text":"x","align":"center"}],
                "buttons":[{"id":2,"rect":[20,20,5,5]},{"id":3,"shift":4,"rect":[30,30,5,5]},
                           {"screen":"ntm:b","rect":[40,40,5,5]}],
                "slots":{"1":[9,9],"2":null}}"##,
        )
        .unwrap();
        let def = parse(&doc).unwrap();
        assert_eq!(def.sprite, "ntm:gui/a");
        assert_eq!(def.title, Some(([70.0, 6.0], true)));
        assert_eq!(def.title_color, None);
        assert_eq!(def.inventory_label, Some([8.0, 162.0]));
        assert_eq!(def.tooltip(5.0, 5.0, &[42]), Some(vec!["42".into()]));
        assert_eq!(def.button_at(21.0, 21.0, false), Some(2));
        assert_eq!(def.button_at(21.0, 21.0, true), Some(2));
        assert_eq!(def.button_at(31.0, 31.0, false), Some(3));
        assert_eq!(def.button_at(31.0, 31.0, true), Some(4));
        assert_eq!(def.button_at(0.0, 0.0, false), None);
        // A switch sends nothing: it shows the machine's other screen.
        assert_eq!(def.button_at(41.0, 41.0, false), None);
        assert_eq!(def.switch_at(41.0, 41.0), Some("ntm:b"));
        assert_eq!(def.switch_at(21.0, 21.0), None);
        assert_eq!(def.slot_at(0, [1.0, 1.0]), Some([1.0, 1.0]));
        assert_eq!(def.slot_at(1, [1.0, 1.0]), Some([9.0, 9.0]));
        assert_eq!(def.slot_at(2, [1.0, 1.0]), None);
        assert_eq!(
            def.texts,
            vec![
                Label { at: [73.0, 31.0], text: "{0}".into(), color: 0x00ff00, align: Align::Left, shadow: true },
                Label { at: [88.0, 6.0], text: "x".into(), color: 0x404040, align: Align::Center, shadow: false },
            ]
        );
    }

    #[test]
    fn a_machine_file_gives_slots_and_screens() {
        let file: Value = serde_json::from_str(
            r#"{"screen":"ntm:a","screens":["ntm:b"],"title":"container.ntm.a","inventory":[8,174],
                "slots":[{"x":26,"y":53},{"grid":[8,18,2,2]},{"x":9,"y":9,"output":true}]}"#,
        )
        .unwrap();
        let l = layout("ntm:machine_a", &file);
        assert_eq!(l.screen, "ntm:a");
        assert_eq!(l.screens, ["ntm:b"]);
        assert_eq!(l.title.as_deref(), Some("container.ntm.a"));
        assert_eq!(l.inventory, [8.0, 174.0]);
        assert_eq!(l.slots, [[26.0, 53.0], [8.0, 18.0], [26.0, 18.0], [8.0, 36.0], [26.0, 36.0], [9.0, 9.0]]);
        let plain = layout("ntm:machine_p", &serde_json::json!({}));
        assert_eq!((plain.screen.as_str(), plain.inventory, plain.slots.len()), ("ntm:machine_p", [8.0, 84.0], 0));
    }

    #[test]
    fn broken_screens_are_refused_with_a_reason() {
        for (doc, why) in [
            (r#"{}"#, "no texture"),
            (r#"{"texture":"x.png"}"#, "texture"),
            (r#"{"texture":"a:textures/g.png","bars":[{"value":0,"rect":[0,0,1,1]}]}"#, "max"),
            (r#"{"texture":"a:textures/g.png","overlays":[{"rect":[0,0,1,1],"uv":[0,0]}]}"#, "overlay"),
        ] {
            let e = parse(&serde_json::from_str(doc).unwrap()).unwrap_err();
            assert!(e.contains(why), "{doc}: {e}");
        }
    }
}

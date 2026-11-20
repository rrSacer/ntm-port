// The screens that send their block commands (RBMK T06), on the client: the automatic control rod's
// (GUIRBMKControlAuto), drawn over its L2 screen. Its four text fields are 1.7.10 GuiTextFields
// (ntm_core::screens::field), its save rule is the Java's (ntm_core::screens::number), and what it sends goes
// as a command of the session the server gave the screen when it opened (ntm_core::screens::control).
use horizon_client::hud::HudBatch;
use ntm_core::screens::control::{Command, Fields, Grant, Snapshot, Value};
use ntm_core::screens::field::{self, Key, TextField};
use ntm_core::screens::number::{self, Saved};
use std::sync::Mutex;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// `GUIRBMKControlAuto.initGui`: the fields at (30, 27 + 11i), 26 by 6, of 3, 3, 4 and 4 characters, without a
/// background; what they hold and how a save clamps them.
const FIELDS: [[i32; 4]; 4] = [[30, 27, 26, 6], [30, 38, 26, 6], [30, 49, 26, 6], [30, 60, 26, 6]];
const MAX: [usize; 4] = [3, 3, 4, 4];
const CLAMP: [f64; 4] = [100.0, 100.0, 9999.0, 9999.0];
const KEYS: [&str; 4] = ["levelUpper", "levelLower", "heatUpper", "heatLower"];
/// `mouseClicked`'s save rectangle and the three curves' (x, y, width, height), as its tests read them.
const SAVE: [i32; 4] = [28, 70, 30, 10];
const CURVES: [[i32; 4]; 3] = [[61, 48, 22, 10], [61, 59, 22, 10], [61, 70, 22, 10]];

/// The screen open now, as its session started.
struct Open {
    pos: [i32; 3],
    session: u64,
    generation: u64,
    /// The commands it sent (the next one is `seq + 1`).
    seq: u32,
    fields: Vec<TextField>,
    /// The block as the last snapshot showed it.
    shown: Fields,
    since: std::time::Instant,
}

static OPEN: Mutex<Option<Open>> = Mutex::new(None);

pub(super) fn takes_commands(block: &str) -> bool {
    ntm_core::rbmk::screen::takes_commands(block)
}

/// The session of the screen showing now (its extra data), started again when it is another screen's.
fn with_open<R>(game: &crate::game::Game<'_>, f: impl FnOnce(&mut Open) -> R) -> Option<R> {
    let pos = game.native_screen_pos()?;
    let grant = Grant::decode(game.native_screen_extra_data()?).ok()?;
    let mut guard = lock(&OPEN);
    if guard.as_ref().is_none_or(|o| o.session != grant.session || o.pos != pos) {
        let auto = ntm_core::rbmk::screen::auto_of(&grant.snapshot.fields);
        // `initGui`: the maximum first, then the stored values, cast to int.
        let fields = [auto.level_upper, auto.level_lower, auto.heat_upper, auto.heat_lower]
            .iter()
            .zip(MAX)
            .map(|(v, max)| {
                let mut t = TextField::new(max);
                t.background = false;
                t.set_text(&number::java_int(*v).to_string());
                t
            })
            .collect();
        *guard = Some(Open {
            pos,
            session: grant.session,
            generation: grant.snapshot.generation,
            seq: 0,
            fields,
            shown: grant.snapshot.fields,
            since: std::time::Instant::now(),
        });
    }
    guard.as_mut().map(f)
}

/// A snapshot from the server (`native_net_from_server`): the open screen's, kept.
pub(super) fn receive(_game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    if channel != crate::game::ntm_glue::SCREEN_SNAPSHOT {
        return;
    }
    let Ok(s) = Snapshot::decode(data) else { return };
    if let Some(open) = lock(&OPEN).as_mut().filter(|o| o.session == s.session && o.pos == s.pos) {
        open.shown = s.fields;
    }
}

/// The fields over the L2 screen (`GuiTextField.drawTextBox` without a background): the text with its shadow, the
/// selection, and the blinking cursor (`cursorCounter / 6 % 2`: six ticks on, six off).
pub(super) fn draw(game: &crate::game::Game<'_>, block: &str, batch: &mut HudBatch, origin: [f32; 2]) {
    if !takes_commands(block) {
        return;
    }
    with_open(game, |open| {
        let ticks = open.since.elapsed().as_millis() / 50;
        for (f, rect) in open.fields.iter().zip(FIELDS) {
            let [x, y] = [origin[0] + rect[0] as f32, origin[1] + rect[1] as f32];
            let text = f.text();
            let units: Vec<u16> = text.encode_utf16().collect();
            let width = |n: usize| game.native_text_width(&String::from_utf16_lossy(&units[..n.min(units.len())]));
            let (a, b) = (f.cursor().min(f.selection_end()), f.cursor().max(f.selection_end()));
            if f.focused && a != b {
                game.native_fill(batch, [x + width(a), y - 1.0, width(b) - width(a), 10.0], 0x800000FF);
            }
            game.native_text(batch, &text, [x, y], 0xFFFFFF, true);
            if f.focused && ticks / 6 % 2 == 0 {
                let cx = x + width(f.cursor());
                // A bar inside the text or in a full field, an underscore after it.
                if f.cursor() < f.len() || f.len() >= f.max() {
                    game.native_fill(batch, [cx, y - 1.0, 1.0, 10.0], 0xFFD0D0D0);
                } else {
                    game.native_text(batch, "_", [cx, y], 0xFFFFFF, true);
                }
            }
        }
    });
}

fn inside(rect: [i32; 4], x: i32, y: i32) -> bool {
    // `drawCustomInfoStat` and `mouseClicked`'s tests: x from the left edge in, y past the top edge.
    rect[0] <= x && rect[0] + rect[2] > x && rect[1] < y && rect[1] + rect[3] >= y
}

/// `GUIRBMKControlAuto.drawScreen`'s hover texts.
pub(super) fn tooltip(game: &crate::game::Game<'_>, block: &str, at: [f32; 2], lines: &mut Vec<String>) -> bool {
    if !takes_commands(block) {
        return false;
    }
    let (x, y) = (at[0].floor() as i32, at[1].floor() as i32);
    with_open(game, |open| {
        let level = open.shown.get_double("level");
        let function = match open.shown.get_integer("function") {
            1 => "Quadratic",
            2 => "Inverse Quadratic",
            _ => "Linear",
        };
        let texts: [([i32; 4], Vec<String>); 10] = [
            ([124, 29, 16, 56], vec![format!("{}%", number::java_int(level * 100.0))]),
            ([58, 26, 28, 19], vec![format!("Function: {function}")]),
            (CURVES[0], vec!["Select linear interpolation".into()]),
            (CURVES[1], vec!["Select quadratic interpolation".into()]),
            (CURVES[2], vec!["Select inverse quadratic interpolation".into()]),
            ([28, 26, 30, 10], vec!["Level at max heat".into(), "Should be smaller than level at min heat".into()]),
            ([28, 37, 30, 10], vec!["Level at min heat".into(), "Should be larger than level at max heat".into()]),
            ([28, 48, 30, 10], vec!["Max heat".into(), "Must be larger than min heat".into()]),
            ([28, 59, 30, 10], vec!["Min heat".into(), "Must be smaller than max heat".into()]),
            (SAVE, vec!["Save parameters".into()]),
        ];
        // The powered rod's charge (`isPowered`) comes with T07's power.
        if let Some((_, l)) = texts.into_iter().find(|(r, _)| inside(*r, x, y)) {
            lines.extend(l);
        }
    })
    .is_some()
}

/// The clipboard of the game, as the fields read it.
struct GameClipboard<'a>(&'a mut horizon_client::clipboard::SystemClipboard);

impl field::Clipboard for GameClipboard<'_> {
    fn get(&mut self) -> String {
        horizon_client::clipboard::Clipboard::get(self.0)
    }
    fn set(&mut self, text: &str) {
        horizon_client::clipboard::Clipboard::set(self.0, text)
    }
}

/// A command of the open session, to the server.
fn send(game: &mut crate::game::Game<'_>, open: &mut Open, fields: Fields) {
    open.seq += 1;
    let cmd = Command { session: open.session, generation: open.generation, pos: open.pos, seq: open.seq, fields };
    match cmd.encode() {
        Ok(bytes) => {
            if let Err(error) = game.native_net_send_to_server(crate::game::ntm_glue::SCREEN_COMMAND, &bytes) {
                eprintln!("ntm screen command: {error}");
            }
        }
        Err(error) => eprintln!("ntm screen command: {error:?}"),
    }
}

/// `mouseClicked`'s save: each field's text through the save rule, the field shown as saved, then one command with
/// the four values. Where the Java's `parseDouble` throws (text `isNumber` passes, `1L`, `0x1`), the 1.7.10 client
/// dies on the spot: here the save stops there and sends nothing, the fields before it already shown as saved.
fn save(game: &mut crate::game::Game<'_>, open: &mut Open) {
    let mut values = [0.0; 4];
    for k in 0..4 {
        match number::save(&open.fields[k].text(), CLAMP[k]) {
            Saved::Number(j) => {
                open.fields[k].set_text(&j.to_string());
                values[k] = f64::from(j);
            }
            Saved::NotANumber => open.fields[k].set_text("0"),
            Saved::Throws => return,
        }
    }
    let mut f = Fields::default();
    for (key, v) in KEYS.iter().zip(values) {
        f.set(key, Value::Double(v));
    }
    send(game, open, f);
}

/// A press, a key or typed text on the screen: the fields', the save's and the curves'. True when the screen took it.
pub(super) fn event(game: &mut crate::game::Game<'_>, block: &str, event: &crate::game::native_hooks::NativeScreenEvent) -> bool {
    use crate::game::native_hooks::NativeScreenEvent as E;
    if !takes_commands(block) {
        return false;
    }
    let mut taken = false;
    if with_open(game, |_| ()).is_none() {
        return false;
    }
    let mut guard = lock(&OPEN);
    let Some(open) = guard.as_mut() else { return false };
    match event {
        E::Press { at, right } => {
            let (x, y) = (at[0].floor() as i32, at[1].floor() as i32);
            let button = i32::from(*right);
            for (f, rect) in open.fields.iter_mut().zip(FIELDS) {
                f.mouse_clicked(x, y, button, rect, |units, width| fit(game, units, width));
                taken |= x >= rect[0] && x < rect[0] + rect[2] && y >= rect[1] && y < rect[1] + rect[3];
            }
            if inside(SAVE, x, y) {
                game.audio.play_button_click();
                save(game, open);
                taken = true;
            }
            for (k, rect) in CURVES.iter().enumerate() {
                if inside(*rect, x, y) {
                    game.audio.play_button_click();
                    let mut f = Fields::default();
                    f.set("function", Value::Int(k as i32));
                    send(game, open, f);
                    taken = true;
                }
            }
        }
        E::Key { key, text, ctrl, shift, .. } => {
            let code = match key.as_str() {
                "key.keyboard.backspace" => Key::Backspace,
                "key.keyboard.home" => Key::Home,
                "key.keyboard.left" => Key::Left,
                "key.keyboard.right" => Key::Right,
                "key.keyboard.end" => Key::End,
                "key.keyboard.delete" => Key::Delete,
                _ => Key::Other,
            };
            // LWJGL's characters: Ctrl with A, C, V or X gives 1, 3, 22 or 24; a key with no text gives 0.
            let control = match (ctrl, key.as_str()) {
                (true, "key.keyboard.a") => Some(1),
                (true, "key.keyboard.c") => Some(3),
                (true, "key.keyboard.v") => Some(22),
                (true, "key.keyboard.x") => Some(24),
                _ => None,
            };
            let mut units: Vec<u16> = match control {
                Some(c) => vec![c],
                None => text.encode_utf16().collect(),
            };
            if units.is_empty() {
                units.push(if code == Key::Backspace { 8 } else { 0 });
            }
            let mut board = GameClipboard(&mut game.clipboard);
            // `keyTyped`: the first field that takes the key ends it.
            for c in units {
                for f in open.fields.iter_mut() {
                    if f.key_typed(c, code, *ctrl, *shift, &mut board) {
                        taken = true;
                        break;
                    }
                }
            }
        }
        E::Text(text) => {
            let mut board = GameClipboard(&mut game.clipboard);
            for c in text.encode_utf16() {
                let code = if c == 8 { Key::Backspace } else { Key::Other };
                for f in open.fields.iter_mut() {
                    if f.key_typed(c, code, false, false, &mut board) {
                        taken = true;
                        break;
                    }
                }
            }
        }
        E::Wheel { .. } => {}
    }
    taken
}

/// `FontRenderer.trimStringToWidth`: how many of `units` fit in `width` pixels.
fn fit(game: &crate::game::Game<'_>, units: &[u16], width: i32) -> usize {
    (0..=units.len()).take_while(|n| game.native_text_width(&String::from_utf16_lossy(&units[..*n])) <= width as f32).last().unwrap_or(0)
}

// The RBMK console's screen (GUIRBMKConsole, RBMK T10), on the client, drawn over its L2 screen (the texture alone:
// no title, no inventory). What it draws, says and does is ntm_core::rbmk::console_gui (tested there): the map from
// the console's last scan, the six screens, the selection, the AZ-5 lid, the flux graph and the level's field. Its
// commands go as commands of the session the server gave the screen when it opened (ntm_core::screens::control).
use horizon_client::hud::HudBatch;
use horizon_client::sound_engine::Sounds;
use ntm_core::rbmk::console::{self, ColumnView};
use ntm_core::rbmk::console_gui::{self as gui, Gui};
use ntm_core::screens::control::{Grant, Snapshot};
use ntm_core::screens::field::TextField;
use std::sync::Mutex;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The screen open now, as its session started.
struct Open {
    pos: [i32; 3],
    session: u64,
    generation: u64,
    seq: u32,
    /// The console's last scan and flux graph, the screens' types, as the last snapshot showed them.
    scan: Vec<Option<ColumnView>>,
    flux: Vec<i32>,
    screens: [u8; 6],
    gui: Gui,
    field: TextField,
    since: std::time::Instant,
}

impl Open {
    fn show(&mut self, s: &ntm_core::screens::control::Fields) {
        if let Some(scan) = s.get_int_array("cols").ok().and_then(|c| console::unpack(&c)) {
            self.scan = scan;
        }
        if let Ok(flux) = s.get_int_array("flux") {
            self.flux = flux;
        }
        self.screens = std::array::from_fn(|i| s.get_byte(&format!("t{i}")).clamp(0, 5) as u8);
    }
}

static OPEN: Mutex<Option<Open>> = Mutex::new(None);

/// The session of the screen showing now (its extra data), started again when it is another screen's: a new screen
/// (`initGui`) has its lid shut, nothing picked and an empty field.
fn with_open<R>(game: &crate::game::Game<'_>, f: impl FnOnce(&mut Open) -> R) -> Option<R> {
    let pos = game.native_screen_pos()?;
    let grant = Grant::decode(game.native_screen_extra_data()?).ok()?;
    let mut guard = lock(&OPEN);
    if guard.as_ref().is_none_or(|o| o.session != grant.session || o.pos != pos) {
        let mut field = TextField::new(gui::FIELD_MAX);
        field.background = false;
        let mut open = Open {
            pos,
            session: grant.session,
            generation: grant.snapshot.generation,
            seq: 0,
            scan: vec![None; console::CELLS as usize],
            flux: vec![0; console::FLUX],
            screens: [0; 6],
            gui: Gui::default(),
            field,
            since: std::time::Instant::now(),
        };
        open.show(&grant.snapshot.fields);
        *guard = Some(open);
    }
    guard.as_mut().map(f)
}

/// A snapshot from the server: the open console's, kept.
pub(super) fn receive(_game: &mut crate::game::Game<'_>, data: &[u8]) {
    let Ok(s) = Snapshot::decode(data) else { return };
    if let Some(open) = lock(&OPEN).as_mut().filter(|o| o.session == s.session && o.pos == s.pos) {
        open.show(&s.fields);
    }
}

/// `drawGuiContainerBackgroundLayer` over the texture: the lid, the screens, the map; the flux graph (the Java's GL
/// lines, two screen pixels wide: one GUI pixel here) and its highest and lowest at half size; the field, green.
pub(super) fn draw(game: &crate::game::Game<'_>, batch: &mut HudBatch, origin: [f32; 2]) {
    let Some(sprite) = crate::game::ntm_screens::sprite(game, ntm_core::rbmk::screen::CONSOLE) else { return };
    with_open(game, |open| {
        for p in gui::parts(&open.gui, &open.scan, &open.screens) {
            let uv = p.uv.map(|v| v as f32);
            game.native_sprite(batch, &sprite, uv, [origin[0] + p.at[0] as f32, origin[1] + p.at[1] as f32, uv[2], uv[3]]);
        }
        let (highest, lowest, lines) = gui::flux_graph(&open.flux);
        for [a, b] in lines {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt();
            if len == 0.0 {
                continue;
            }
            let (nx, ny) = ((-dy / len * 0.5) as f32, (dx / len * 0.5) as f32);
            let [ax, ay, bx, by] = [a[0], a[1], b[0], b[1]].map(|v| v as f32);
            let [ox, oy] = origin;
            game.native_quad(batch, [[ox + ax + nx, oy + ay + ny], [ox + bx + nx, oy + by + ny], [ox + bx - nx, oy + by - ny], [ox + ax - nx, oy + ay - ny]], 0xFF00FF00);
        }
        // `fontRendererObj.drawString` at half scale: the left two at x 8, the right two ending at x 80; the highest
        // at y 98, the lowest above y 133.
        let small = |batch: &mut HudBatch, text: &str, x: f32, y: f32| {
            let start = batch.vertices.len();
            game.native_text(batch, text, [x, y], 0x00FF00, false);
            batch.scale_since(start, x, y, 0.5, 0.5);
        };
        let half = |text: &str| game.native_text_width(text) * 0.5;
        for (value, y) in [(highest, 98.0), (lowest, 133.0 - 4.5)] {
            let text = value.to_string();
            small(batch, &text, origin[0] + 8.0, origin[1] + y);
            small(batch, &text, origin[0] + 80.0 - half(&text), origin[1] + y);
        }
        crate::game::ntm_control_screens::draw_field(game, batch, origin, &open.field, gui::FIELD, 0x00FF00, open.since);
    });
}

/// `drawScreen`'s hover texts.
pub(super) fn tooltip(game: &crate::game::Game<'_>, at: [f32; 2], lines: &mut Vec<String>) -> bool {
    let (x, y) = (at[0].floor() as i32, at[1].floor() as i32);
    with_open(game, |open| lines.extend(gui::tooltip(&open.scan, &open.screens, x, y))).is_some()
}

/// `mouseClicked` (the field's own click first) and `keyTyped` (the field's).
pub(super) fn event(game: &mut crate::game::Game<'_>, event: &crate::game::native_hooks::NativeScreenEvent) -> bool {
    use crate::game::native_hooks::NativeScreenEvent as E;
    if with_open(game, |_| ()).is_none() {
        return false;
    }
    let mut guard = lock(&OPEN);
    let Some(open) = guard.as_mut() else { return false };
    match event {
        E::Press { at, right } => {
            let (x, y) = (at[0].floor() as i32, at[1].floor() as i32);
            let button = i32::from(*right);
            let rect = gui::FIELD;
            open.field.mouse_clicked(x, y, button, rect, |units, width| crate::game::ntm_control_screens::fit(game, units, width));
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
            let click = open.gui.click(&open.scan, x, y, button, now, &open.field.text());
            if let Some(text) = &click.field {
                open.field.set_text(text);
            }
            for (id, pitch) in &click.sounds {
                let sound = horizon_client::sound_engine::SoundInstance::for_ui(id, *pitch, 0.25, horizon_client::random_source::create());
                game.audio.play_instance(sound, false);
            }
            for fields in click.send.iter().cloned() {
                crate::game::ntm_control_screens::send_command(game, open.session, open.generation, open.pos, &mut open.seq, fields);
            }
            let on_field = x >= rect[0] && x < rect[0] + rect[2] && y >= rect[1] && y < rect[1] + rect[3];
            on_field || !click.sounds.is_empty() || !click.send.is_empty()
        }
        E::Key { .. } | E::Text(_) => crate::game::ntm_control_screens::field_keys(game, std::slice::from_mut(&mut open.field), event),
        E::Wheel { .. } => false,
    }
}

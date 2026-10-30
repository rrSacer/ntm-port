// The machines' screens (L2): `horizon_screen/<name>.json` of the content,
// read through native_asset and drawn on the native screen hooks. What the
// files say and how it is worked out is ntm_core::screens (tested there);
// this is the drawing, the clicks and the tooltips.
use horizon_client::hud::HudBatch;
use ntm_core::screens::{self as model, Layout, ScreenDef};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn layout(block: &str) -> Option<Layout> {
    model::layout_of(block)
}

/// Screen `name` (`ntm:machine_battery`), read once from the mounted
/// resources; None for a file that is missing or broken (said once).
fn screen(game: &crate::game::Game<'_>, name: &str) -> Option<Arc<ScreenDef>> {
    static CACHE: Mutex<Option<HashMap<String, Option<Arc<ScreenDef>>>>> = Mutex::new(None);
    if let Some(found) = lock(&CACHE).get_or_insert_with(Default::default).get(name) {
        return found.clone();
    }
    let (ns, file) = name.split_once(':')?;
    let read = game
        .native_asset(&format!("assets/{ns}/horizon_screen/{file}.json"))
        .ok_or_else(|| "no such screen file".to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
        .and_then(|doc| model::parse(&doc));
    let def = match read {
        Ok(def) => Some(Arc::new(def)),
        Err(e) => {
            eprintln!("ntm screens: {name}: {e}");
            None
        }
    };
    lock(&CACHE).get_or_insert_with(Default::default).insert(name.to_string(), def.clone());
    def
}

/// The screen a window shows now: (block, screen name). A press on a button
/// that switches changes the name; opening a window sets it again.
static OPEN: Mutex<Option<(String, String)>> = Mutex::new(None);

fn current(game: &crate::game::Game<'_>, block: &str) -> Option<Arc<ScreenDef>> {
    let name = match &*lock(&OPEN) {
        Some((b, name)) if b == block => name.clone(),
        _ => layout(block)?.screen,
    };
    screen(game, &name)
}

/// A machine's screen opens: its size, where the slots sit on it, the player inventory.
pub(super) fn open(game: &crate::game::Game<'_>, block: &str, out: &mut Option<crate::game::native_hooks::NativeScreen>) {
    let Some(layout) = layout(block) else { return };
    let Some(def) = screen(game, &layout.screen) else { return };
    *lock(&OPEN) = Some((block.to_string(), layout.screen.clone()));
    *out = Some(crate::game::native_hooks::NativeScreen {
        width: def.size[0],
        height: def.size[1],
        slots: layout.slots.iter().enumerate().map(|(i, at)| def.slot_at(i, *at)).collect(),
        inventory_x: layout.inventory[0],
        inventory_y: layout.inventory[1],
        own_labels: true,
    });
}

fn argb(rgb: u32) -> u32 {
    0xFF00_0000 | (rgb & 0xFF_FFFF)
}

/// The background, then bars, needles, overlays and texts, then the title and
/// the inventory's label.
pub(super) fn draw(game: &crate::game::Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let Some(def) = current(game, block) else { return };
    let [ox, oy] = origin;
    let [w, h] = def.size;
    game.native_sprite(batch, &def.sprite, [0.0, 0.0, w, h], [ox, oy, w, h]);
    for bar in &def.bars {
        let value = data.get(bar.value).copied().unwrap_or(0);
        let Some(f) = model::fill(value, bar.from, bar.max.of(data)) else { continue };
        let Some([cx, cy, cw, ch]) = model::bar_part(bar.rect, bar.dir, f) else { continue };
        let [x, y, ..] = bar.rect;
        if let Some([u, v]) = bar.uv {
            game.native_sprite(batch, &def.sprite, [u + cx, v + cy, cw, ch], [ox + x + cx, oy + y + cy, cw, ch]);
        } else if let Some(index) = bar.color {
            let rgb = data.get(index).copied().unwrap_or(0) as u32;
            game.native_fill(batch, [ox + x + cx, oy + y + cy, cw, ch], argb(rgb));
        }
    }
    for g in &def.gauges {
        let value = data.get(g.value).copied().unwrap_or(0);
        let Some(f) = model::fill(value, 0, g.max.of(data)) else { continue };
        let [cx, cy] = [ox + g.at[0], oy + g.at[1]];
        let rgba = |rgb: u32| {
            let [_, r, g, b] = rgb.to_be_bytes();
            horizon_client::hud::rgba(r, g, b, 255)
        };
        for (mult, color) in [(1.5, rgba(0)), (1.0, rgba(g.color))] {
            let [t, l, r] = model::needle(f, g.tip, g.back, g.side, mult).map(|[x, y]| [cx + x, cy + y]);
            batch.fill_quad(&game.gui.atlas, [t, l, r, r], color);
        }
    }
    let items = game.container.and_then(|key| game.world.block_entities.get(&key)).map(|e| e.slots());
    let filled = |slot: usize| items.is_some_and(|s| s.get(slot).is_some_and(Option::is_some));
    for o in def.overlays.iter().filter(|o| o.on(data, filled)) {
        let [x, y, w, h] = o.rect;
        game.native_sprite(batch, &def.sprite, [o.uv[0], o.uv[1], w, h], [ox + x, oy + y, w, h]);
    }
    for t in &def.texts {
        let line = model::format(&t.text, data);
        let width = game.native_text_width(&line);
        let x = match t.align {
            model::Align::Left => t.at[0],
            model::Align::Center => t.at[0] - width / 2.0,
            model::Align::Right => t.at[0] - width,
        };
        game.native_text(batch, &line, [ox + x, oy + t.at[1]], t.color, t.shadow);
    }
    if let Some(([x, y], centred)) = def.title {
        let title = game.native_screen_title().unwrap_or_else(|| block.to_string());
        let x = if centred { x - game.native_text_width(&title) / 2.0 } else { x };
        game.native_text(batch, &title, [ox + x, oy + y], def.title_color.unwrap_or(0x404040), false);
    }
    if let Some([x, y]) = def.inventory_label {
        let label = game.translate("container.inventory", "Inventory");
        game.native_text(batch, &label, [ox + x, oy + y], 0x404040, false);
    }
}

/// A press on the screen: a button's id (its shift id while shift is held),
/// or a switch to the machine's other screen, which sends nothing.
pub(super) fn click(game: &crate::game::Game<'_>, block: &str, at: [f32; 2], button: &mut Option<i32>) {
    let Some(def) = current(game, block) else { return };
    if let Some(next) = def.switch_at(at[0], at[1]) {
        *lock(&OPEN) = Some((block.to_string(), next.to_string()));
        return;
    }
    let shift = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
        || game.keys.contains(&winit::keyboard::KeyCode::ShiftRight);
    *button = def.button_at(at[0], at[1], shift).and_then(|id| i32::try_from(id).ok());
}

/// The tooltip of a bar, an icon or a button under the pointer.
pub(super) fn tooltip(game: &crate::game::Game<'_>, block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if let Some(found) = current(game, block).and_then(|def| def.tooltip(at[0], at[1], data)) {
        lines.extend(found);
    }
}

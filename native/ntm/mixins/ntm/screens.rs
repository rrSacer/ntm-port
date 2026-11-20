// The machines' screens (L2): `horizon_screen/<name>.json` of the content,
// read through native_asset and drawn on the native screen hooks. What the
// files say and how it is worked out is ntm_core::screens (tested there);
// this is the drawing, the clicks and the tooltips.
use horizon_client::hud::HudBatch;
use ntm_core::native::{self as machines, RecipeRow};
use ntm_core::screens::list::{self, Config, Entry, Outcome, selector};
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

/// The texture of the screen block `block` shows now (`horizon_screen`'s `texture`), for a screen that draws on it.
pub(super) fn sprite(game: &crate::game::Game<'_>, block: &str) -> Option<String> {
    current(game, block).map(|def| def.sprite.clone())
}

/// A machine's screen as the engine lays it out.
fn machine_screen(game: &crate::game::Game<'_>, block: &str, name: &str) -> Option<crate::game::native_hooks::NativeScreen> {
    let layout = layout(block)?;
    let def = screen(game, name)?;
    Some(crate::game::native_hooks::NativeScreen {
        width: def.size[0],
        height: def.size[1],
        slots: layout.slots.iter().enumerate().map(|(i, at)| def.slot_at(i, *at)).collect(),
        inventory_x: layout.inventory[0],
        inventory_y: layout.inventory[1],
        own_labels: true,
        own_background: true,
        hide_inventory: def.hide_inventory,
    })
}

/// A machine's screen opens: its size, where the slots sit on it, the player inventory.
pub(super) fn open(game: &crate::game::Game<'_>, block: &str, out: &mut Option<crate::game::native_hooks::NativeScreen>) {
    if block=="ntm:machine_armor_table"{*out=Some(crate::game::ntm_armor_table::screen());return;}
    *lock(&LIST) = None;
    let Some(layout) = layout(block) else { return };
    let Some(shown) = machine_screen(game, block, &layout.screen) else { return };
    *lock(&OPEN) = Some((block.to_string(), layout.screen.clone()));
    *out = Some(shown);
}

fn argb(rgb: u32) -> u32 {
    0xFF00_0000 | (rgb & 0xFF_FFFF)
}

/// The background, then bars, needles, overlays and texts, then the title and
/// the inventory's label.
pub(super) fn draw(game: &crate::game::Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if block=="ntm:machine_armor_table"{crate::game::ntm_armor_table::draw(game,batch,origin);return;}
    if let Some(open) = lock(&LIST).as_ref().filter(|open| open.block == block) {
        draw_selector(game, open, batch, origin);
        return;
    }
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
    // A screen that sends its block commands draws its text fields over (RBMK T06).
    crate::game::ntm_control_screens::draw(game, block, batch, origin);
}

/// A press on the screen: a button's id (its shift id while shift is held),
/// or a switch to the machine's other screen, which sends nothing.
pub(super) fn click(game: &crate::game::Game<'_>, block: &str, at: [f32; 2], button: &mut Option<i32>) {
    let Some(def) = current(game, block) else { return };
    if let Some(next) = def.switch_at(at[0], at[1]) {
        *lock(&OPEN) = Some((block.to_string(), next.to_string()));
        return;
    }
    let shift = game.keys.raw(&winit::keyboard::KeyCode::ShiftLeft)
        || game.keys.raw(&winit::keyboard::KeyCode::ShiftRight);
    *button = def.button_at(at[0], at[1], shift).and_then(|id| i32::try_from(id).ok());
}

/// The tooltip of a bar, an icon or a button under the pointer.
pub(super) fn tooltip(game: &crate::game::Game<'_>, block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if let Some(open) = lock(&LIST).as_ref().filter(|open| open.block == block) {
        selector_tooltip(game, open, at, lines);
        return;
    }
    if crate::game::ntm_control_screens::tooltip(game, block, at, lines) {
        return;
    }
    if let Some(found) = current(game, block).and_then(|def| def.tooltip(at[0], at[1], data)) {
        lines.extend(found);
    }
}

/// The recipe selector open over a machine's screen (NTM's
/// GUIScreenRecipeSelector): the grid, the search and the choice live here;
/// only the choice, once it changed, leaves as a menu button on closing.
struct Selector {
    block: String,
    state: list::State,
    rows: Vec<RecipeRow>,
    /// The machine's own layout, laid out again when the selector closes.
    restore: crate::game::native_hooks::NativeScreen,
}

static LIST: Mutex<Option<Selector>> = Mutex::new(None);

/// A press, key, text or wheel on a machine's screen (API 31): the selector's
/// when it is open, else the press on the button that opens it (button 0 of
/// a machine with a recipe list). True when this took the event.
pub(super) fn event(game: &mut crate::game::Game<'_>, block: &str, data: &[i32], event: &crate::game::native_hooks::NativeScreenEvent) -> bool {
    use crate::game::native_hooks::NativeScreenEvent as E;
    if crate::game::ntm_control_screens::takes_commands(block) {
        return crate::game::ntm_control_screens::event(game, block, event);
    }
    let mut guard = lock(&LIST);
    let Some(open) = guard.as_mut().filter(|open| open.block == block) else {
        drop(guard);
        let E::Press { at, .. } = event else { return false };
        return switch_screen(game, block, *at) || open_selector(game, block, data, *at);
    };
    let before = (open.state.offset, open.state.selected);
    let outcome = match event {
        E::Press { at, .. } => open.state.press(at[0], at[1]),
        E::Key { key, text, .. } => {
            let inventory = game.settings.keys.get("key.inventory").to_string();
            open.state.key(key, text, &inventory)
        }
        E::Text(text) => open.state.text(text),
        E::Wheel { steps, .. } => open.state.wheel(*steps),
    };
    let moved = before != (open.state.offset, open.state.selected);
    if outcome == Outcome::Close {
        let closed = guard.take();
        drop(guard);
        if let Some(closed) = closed {
            if let Some(button) = closed.state.commit().and_then(|b| i32::try_from(b).ok()) {
                game.native_screen_send_button(button);
            }
            game.native_screen_set_layout(closed.restore);
        }
        game.audio.play_button_click();
    } else if moved {
        game.audio.play_button_click();
    }
    true
}

/// A press at `at` on a button that shows the machine's other screen (the
/// electrolyser's): the screen changes and the slots are laid out again at
/// once, since each screen puts them where its texture has them.
fn switch_screen(game: &mut crate::game::Game<'_>, block: &str, at: [f32; 2]) -> bool {
    let Some(def) = current(game, block) else { return false };
    let Some(next) = def.switch_at(at[0], at[1]).map(str::to_owned) else { return false };
    let Some(shown) = machine_screen(game, block, &next) else { return false };
    *lock(&OPEN) = Some((block.to_string(), next));
    game.native_screen_set_layout(shown);
    game.audio.play_button_click();
    true
}

/// A press at `at` on the button that opens the machine's recipe list.
fn open_selector(game: &mut crate::game::Game<'_>, block: &str, data: &[i32], at: [f32; 2]) -> bool {
    let Some(rows) = machines::recipes(block) else { return false };
    let Some(slot) = machines::recipe_slot(block) else { return false };
    let Some(def) = current(game, block) else { return false };
    let shift = false;
    if def.button_at(at[0], at[1], shift) != Some(0) {
        return false;
    }
    let Some(layout) = layout(block) else { return false };
    let Some(restore) = machine_screen(game, block, &current_name(block, &layout)) else { return false };
    // The data slot holds the recipe + 1; zero is none.
    let selected = data.get(slot).copied().filter(|v| *v > 0).map(|v| (v - 1) as u32);
    let entries = rows
        .iter()
        .map(|r| Entry { index: r.index, label: r.label.clone(), icon: r.icon.clone(), inputs: r.inputs.clone(), outputs: r.outputs.clone() })
        .collect();
    let shown = crate::game::native_hooks::NativeScreen {
        width: selector::SIZE[0],
        height: selector::SIZE[1],
        slots: vec![None; layout.slots.len()],
        inventory_x: 8.0,
        inventory_y: 84.0,
        own_labels: true,
        own_background: true,
        hide_inventory: true,
    };
    if !game.native_screen_set_layout(shown) {
        return false;
    }
    *lock(&LIST) = Some(Selector {
        block: block.to_string(),
        state: list::State::new(Config::selector(), entries, selected),
        rows,
        restore,
    });
    game.audio.play_button_click();
    true
}

/// The name of the screen a machine shows now.
fn current_name(block: &str, layout: &Layout) -> String {
    match &*lock(&OPEN) {
        Some((b, name)) if b == block => name.clone(),
        _ => layout.screen.clone(),
    }
}

/// An item's name for a recipe line.
fn item_name(game: &crate::game::Game<'_>, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    for kind in ["item", "block"] {
        let named = game.translate(&format!("{kind}.{ns}.{path}"), "\u{0}");
        if named != "\u{0}" {
            return named;
        }
    }
    path.replace('_', " ")
}

fn rect_hit(rect: [f32; 4], at: [f32; 2]) -> bool {
    selector::hits(rect, at[0], at[1])
}

fn draw_selector(game: &crate::game::Game<'_>, open: &Selector, batch: &mut HudBatch, origin: [f32; 2]) {
    let [ox, oy] = origin;
    let [w, h] = selector::SIZE;
    let cursor = game.native_cursor();
    let at = [cursor[0] - ox, cursor[1] - oy];
    let state = &open.state;
    game.native_sprite(batch, selector::SPRITE, [0.0, 0.0, w, h], [ox, oy, w, h]);
    if state.search.focused {
        let [x, y, sw, sh] = selector::SEARCH_FOCUS;
        let [u, v] = selector::SEARCH_FOCUS_UV;
        game.native_sprite(batch, selector::SPRITE, [u, v, sw, sh], [ox + x, oy + y, sw, sh]);
    }
    for b in [&selector::UP, &selector::DOWN, &selector::CLOSE, &selector::CLEAR_SEARCH, &selector::FOCUS_HINT] {
        if rect_hit(b.rect, at) {
            let [x, y, bw, bh] = b.rect;
            game.native_sprite(batch, selector::SPRITE, [b.uv[0], b.uv[1], bw, bh], [ox + x, oy + y, bw, bh]);
        }
    }
    let shown: Vec<&Entry> = state.visible().collect();
    for (i, entry) in shown.iter().enumerate() {
        if state.selected == Some(entry.index) {
            let [x, y, cw, ch] = state.config.cell(i);
            let [u, v] = selector::SELECTED_CELL_UV;
            game.native_sprite(batch, selector::SPRITE, [u, v, 18.0, 18.0], [ox + x, oy + y, cw, ch]);
        }
    }
    for (i, entry) in shown.iter().enumerate() {
        let [x, y, ..] = state.config.cell(i);
        game.native_item(batch, &entry.icon, [ox + x + 1.0, oy + y + 1.0], 16.0);
    }
    if let Some(entry) = state.selected.and_then(|s| state.entries.iter().find(|e| e.index == s)) {
        let [x, y] = selector::SELECTED_ICON;
        game.native_item(batch, &entry.icon, [ox + x, oy + y], 16.0);
    }
    let [x, y, field, _] = selector::SEARCH_TEXT;
    let width = |c: char| game.native_text_width(&c.to_string());
    let (first, text) = state.search.visible(field, width);
    game.native_text(batch, &text, [ox + x, oy + y], 0xFFFFFF, false);
    if state.search.focused && (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()) / 500).is_multiple_of(2) {
        let before: String = text.chars().take(state.search.caret().saturating_sub(first)).collect();
        let cx = ox + x + game.native_text_width(&before);
        game.native_fill(batch, [cx, oy + y - 1.0, 1.0, 10.0], 0xFFFF_FFFF);
    }
}

/// A recipe as the selector lists it on hover: its name, then what it
/// takes and gives.
fn recipe_lines(game: &crate::game::Game<'_>, open: &Selector, index: u32, lines: &mut Vec<String>) {
    let Some(row) = open.rows.iter().find(|r| r.index == index) else { return };
    lines.push(format!("\u{a7}e{}", row.label));
    let stacks = |label: &str, items: &[(String, u32)], fluids: &[(String, u32)], lines: &mut Vec<String>| {
        if items.is_empty() && fluids.is_empty() {
            return;
        }
        lines.push(format!("\u{a7}l{label}:"));
        for (id, n) in items {
            lines.push(format!("  {n}x {}", item_name(game, id)));
        }
        for (id, mb) in fluids {
            lines.push(format!("  {mb} mB {}", id.rsplit(':').next().unwrap_or(id).replace('_', " ")));
        }
    };
    stacks("Input", &row.inputs, &row.fluids_in, lines);
    stacks("Output", &row.outputs, &row.fluids_out, lines);
}

fn selector_tooltip(game: &crate::game::Game<'_>, open: &Selector, at: [f32; 2], lines: &mut Vec<String>) {
    let state = &open.state;
    if let Some(entry) = state.entry_at(at[0], at[1]) {
        recipe_lines(game, open, entry.index, lines);
    } else if rect_hit(selector::SELECTED_FRAME, at) {
        if let Some(selected) = state.selected {
            recipe_lines(game, open, selected, lines);
        }
    } else if rect_hit(selector::CLOSE.rect, at) {
        lines.push("\u{a7}eClose".to_string());
    } else if rect_hit(selector::CLEAR_SEARCH.rect, at) {
        lines.push("\u{a7}eClear search".to_string());
    } else if rect_hit(selector::FOCUS_HINT.rect, at) {
        lines.push("\u{a7}oPress ENTER to toggle focus".to_string());
    }
}

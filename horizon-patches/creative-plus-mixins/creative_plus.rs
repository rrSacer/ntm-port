// creative-plus on the creative screen: the item panel of a recipe viewer
// mod (Recipes) right of it, as beside the survival inventory, and the page
// buttons of the content mods' tabs (`horizon_client::creative::mod_tabs`).
use super::{Game, Screen, select_creative_tab};
use horizon_client::{
    creative::{Creative, Tab, mod_tabs},
    hud::HudBatch,
    text::Text,
    ui,
};
use std::{
    ops::ControlFlow,
    sync::atomic::{AtomicBool, Ordering},
};

/// The panel draws the carried stack again over itself: no panel then.
static DRAWING: AtomicBool = AtomicBool::new(false);

fn inside(p: (f32, f32), [x, y, w, h]: [f32; 4]) -> bool {
    p.0 >= x && p.1 >= y && p.0 < x + w && p.1 < y + h
}

/// Before the creative screen's carried stack: the page buttons, then the
/// panel, so the carried stack and the tooltips stay on top.
pub(crate) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    if game.screen != Screen::Creative || DRAWING.swap(true, Ordering::Relaxed) {
        return;
    }
    draw_pages(game, batch);
    game.draw_mod_browser(batch);
    DRAWING.store(false, Ordering::Relaxed);
}

fn draw_pages(game: &Game<'_>, batch: &mut HudBatch) {
    let pages = mod_tabs::pages();
    if pages < 2 {
        return;
    }
    let (ox, oy) = Game::creative_origin(batch);
    let atlas = &game.gui.atlas;
    for (i, [x, y, w, h]) in mod_tabs::buttons().into_iter().enumerate() {
        let (x, y) = (ox + x, oy + y);
        let hover = inside(game.cursor, [x, y, w, h]);
        let label = Text::literal(if i == 0 { "<" } else { ">" });
        ui::button(batch, atlas, x, y, w, h, &label, true, hover);
    }
    let label = format!("{} / {}", mod_tabs::page() + 1, pages);
    let width = HudBatch::text_width(atlas, &label);
    let x = (ox + (Creative::WIDTH - width) / 2.0).floor();
    batch.text(atlas, &label, x, oy - 44.0, u32::MAX, true);
}

/// A press on the creative screen: the panel's first, then the page buttons.
pub(crate) fn click(game: &mut Game<'_>, right: bool) -> ControlFlow<()> {
    if game.mod_browser_press(right) {
        return ControlFlow::Break(());
    }
    if right || mod_tabs::pages() < 2 {
        return ControlFlow::Continue(());
    }
    let Some(size) = game
        .renderer
        .as_ref()
        .map(|r| [r.config.width, r.config.height])
    else {
        return ControlFlow::Continue(());
    };
    let batch = HudBatch::new(size);
    let (ox, oy) = Game::creative_origin(&batch);
    for (i, [x, y, w, h]) in mod_tabs::buttons().into_iter().enumerate() {
        if !inside(game.cursor, [ox + x, oy + y, w, h]) {
            continue;
        }
        let step = if i == 0 { -1 } else { 1 };
        if let Some(tab) = mod_tabs::turn(&game.creative_screen, step) {
            select_creative_tab(&mut game.creative_screen, &mut game.inv_clicks, tab);
            game.inv_drag = None;
            game.last_inv_click = None;
        }
        game.audio
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}

/// A mod tab's title in the player's language.
pub(crate) fn title(game: &Game<'_>, tab: &Tab) -> ControlFlow<String> {
    match mod_tabs::fallback(&tab.id) {
        Some(fallback) => ControlFlow::Break(game.translate(&tab.title, &fallback)),
        None => ControlFlow::Continue(()),
    }
}

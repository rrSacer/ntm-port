// What the player's client shows of the NTM (L4): the Geiger box and the
// radioactive items' tooltip lines. What they say is ntm_core::client_rad
// (tested there); the numbers are the server's, per player.
use horizon_client::{extras::Corner, extras_hud::Corners, hud::HudBatch, items::ItemStack, text::Text};
use ntm_core::client_rad;

/// The Geiger counter: a box in the top left with the rate and the dose, in
/// the colour of the dose, once the server has sent one.
pub(super) fn hud(game: &mut crate::game::Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_corner_shown(Corner::TopLeft) {
        return;
    }
    let dose = game.native_player_value(client_rad::KEY_DOSE);
    let rate = game.native_player_value(client_rad::KEY_RATE);
    let Some((lines, color)) = client_rad::geiger(dose, rate) else { return };
    let width = lines.iter().map(|l| game.native_text_width(l)).fold(0.0, f32::max) + 6.0;
    let height = lines.len() as f32 * 10.0 + 4.0;
    let (x, y) = corners.place(Corner::TopLeft, width, height);
    game.native_fill(batch, [x, y, width, height], 0x7800_0000);
    for (i, line) in lines.iter().enumerate() {
        game.native_text(batch, line, [x + 3.0, y + 2.0 + i as f32 * 10.0], color, true);
    }
}

/// A radioactive item's lines, after the others.
pub(super) fn tooltip(_game: &crate::game::Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    for line in client_rad::tooltip_lines(&stack.item, stack.count) {
        lines.push((Text::legacy(&line), u32::MAX));
    }
}

// The Geiger counter box (L4, client side).
fn ntm_hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, corners: &mut horizon_client::extras_hud::Corners) {
    crate::game::ntm_client::hud(game, batch, corners);
}

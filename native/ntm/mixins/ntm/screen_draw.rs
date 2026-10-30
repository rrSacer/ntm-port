// A machine's screen draws (L2).
fn ntm_screen_draw(game: &crate::game::Game<'_>, block: &str, data: &[i32], batch: &mut horizon_client::hud::HudBatch, origin: [f32; 2]) {
    crate::game::ntm_screens::draw(game, block, data, batch, origin);
}

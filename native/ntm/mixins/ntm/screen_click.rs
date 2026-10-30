// A press on a machine's screen (L2).
fn ntm_screen_click(game: &crate::game::Game<'_>, block: &str, at: [f32; 2], button: &mut Option<i32>) {
    crate::game::ntm_screens::click(game, block, at, button);
}

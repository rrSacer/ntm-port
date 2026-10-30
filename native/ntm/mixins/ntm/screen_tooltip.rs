// A tooltip on a machine's screen (L2).
fn ntm_screen_tooltip(game: &crate::game::Game<'_>, block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    crate::game::ntm_screens::tooltip(game, block, data, at, lines);
}

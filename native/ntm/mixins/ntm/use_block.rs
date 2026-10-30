// A right click on a machine's cell: its lever, when it has one there.
fn ntm_use_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], core: [i32; 3], claimed: &mut bool) {
    if !*claimed && crate::game::ntm_glue::use_block(game, pos, core) {
        *claimed = true;
    }
}

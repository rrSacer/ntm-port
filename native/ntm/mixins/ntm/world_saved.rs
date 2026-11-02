fn ntm_world_saved(game: &mut crate::game::Game<'_>, wait: bool) {
    crate::game::ntm_glue::world_saved(game, wait);
}

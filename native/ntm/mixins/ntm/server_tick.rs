// The end of each running server tick: the NTM server's phases.
fn ntm_server_tick(game: &mut crate::game::Game<'_>, phase: crate::game::native_lifecycle::ServerTickPhase) {
    if let crate::game::native_lifecycle::ServerTickPhase::ServerTickEnd { world_running: true } = phase {
        crate::game::ntm_glue::server_tick(game);
    }
}

// The end of each running server tick: the NTM server's phases.
fn ntm_server_tick(game: &mut crate::game::Game<'_>, phase: crate::game::native_lifecycle::ServerTickPhase) {
    if let crate::game::native_lifecycle::ServerTickPhase::ServerTickEnd { world_running } = phase {
        // The hub is free here: menus requested during a player's turn open now.
        crate::game::ntm_armor_table::drain(game);
        crate::game::ntm_items::drain(game);
        if world_running {
            crate::game::ntm_glue::server_tick(game);
        }
    }
}

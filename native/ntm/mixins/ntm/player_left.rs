// A player left (server): their screen session ends (RBMK T06).
fn ntm_player_left(player: u128) {
    crate::game::ntm_glue::player_logged_out(player);
}

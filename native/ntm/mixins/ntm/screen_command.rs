// A message from a player's client (server): a screen's command (RBMK T06).
fn ntm_screen_command(game: &crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::ntm_glue::net_from_client(game, player, channel, data);
}

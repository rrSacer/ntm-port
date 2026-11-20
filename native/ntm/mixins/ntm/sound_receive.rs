// A message from the server (client): the nuclear sounds' cues, and the open screen's snapshots (RBMK T06).
fn ntm_sound_receive(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::ntm_sounds::receive(game, channel, data);
    crate::game::ntm_control_screens::receive(game, channel, data);
}

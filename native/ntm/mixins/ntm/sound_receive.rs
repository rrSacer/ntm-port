fn ntm_sound_receive(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::ntm_sounds::receive(game, channel, data);
}

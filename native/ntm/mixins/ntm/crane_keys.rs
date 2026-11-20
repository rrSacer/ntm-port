// A key went down or up (client): the RBMK crane's keys (T11, `HbmKeybinds`) go to the server at each change.
fn ntm_crane_keys(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::ntm_glue::crane_key(game, key, pressed);
}

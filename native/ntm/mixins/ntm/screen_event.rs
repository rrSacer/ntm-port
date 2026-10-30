// A press, key, text or wheel on a machine's screen (L2: the recipe list).
fn ntm_screen_event(game: &mut crate::game::Game<'_>, block: &str, data: &[i32], event: &crate::game::native_hooks::NativeScreenEvent, claimed: &mut bool) {
    if crate::game::ntm_screens::event(game, block, data, event) {
        *claimed = true;
    }
}

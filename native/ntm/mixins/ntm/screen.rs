// A machine's screen opens (L2).
fn ntm_screen(game: &crate::game::Game<'_>, block: &str, screen: &mut Option<crate::game::native_hooks::NativeScreen>) {
    crate::game::ntm_screens::open(game, block, screen);
}

fn click(game: &mut Game<'_>, right: bool) -> std::ops::ControlFlow<()> {
    horizon_client::creative::mod_tabs::ensure(&mut game.creative_screen);
    crate::game::creative_plus::click(game, right)
}

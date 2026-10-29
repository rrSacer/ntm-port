fn title(game: &Game<'_>, tab: &horizon_client::creative::Tab) -> std::ops::ControlFlow<String> {
    crate::game::creative_plus::title(game, tab)
}

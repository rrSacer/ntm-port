fn ntm_item_clone(game: &mut crate::game::Game<'_>, player: u128, was_death: bool) {
    crate::game::ntm_items::cloned(game, player, was_death);
}

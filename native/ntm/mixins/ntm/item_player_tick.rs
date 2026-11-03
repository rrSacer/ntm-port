fn ntm_item_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::ntm_items::tick(game);
    crate::game::ntm_armor_table::tick(game);
}

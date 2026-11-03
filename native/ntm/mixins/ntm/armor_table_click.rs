fn ntm_armor_table_click(
    game: &mut crate::game::Game<'_>,
    uuid: u128,
    window: i32,
    slot: i32,
    button: i32,
    mode: i32,
) {
    crate::game::ntm_armor_table::clicked(game, uuid, window, slot, button, mode);
}

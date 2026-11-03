fn ntm_item_start(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    canceled: &mut bool,
) {
    crate::game::ntm_items::start(game, stack, canceled);
}

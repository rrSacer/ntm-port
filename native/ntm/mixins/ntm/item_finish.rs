fn ntm_item_finish(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) {
    crate::game::ntm_items::complete(game, stack);
}

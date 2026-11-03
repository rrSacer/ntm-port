fn ntm_item_equipment(
    game: &crate::game::Game<'_>,
    slot: &str,
    new: Option<&horizon_client::items::ItemStack>,
) {
    crate::game::ntm_items::equipped(game, slot, new);
}

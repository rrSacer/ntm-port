// A radioactive item's tooltip (L4, client side).
fn ntm_tooltip(game: &crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    crate::game::ntm_client::tooltip(game, stack, lines);
}

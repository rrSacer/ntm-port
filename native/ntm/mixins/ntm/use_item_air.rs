// A player's item used in the air: the detonator sets its bomb off.
fn ntm_use_item_air(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    if !*claimed && crate::game::ntm_glue::use_item_air(game, stack) {
        *claimed = true;
    }
}

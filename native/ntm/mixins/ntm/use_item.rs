// A player's item used on a block: the fluid identifier types a duct.
fn ntm_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, on: Option<([i32; 3], [i32; 3])>, claimed: &mut bool) {
    if let Some((pos, _)) = on
        && !*claimed
        && crate::game::ntm_glue::use_item(game, stack, pos)
    {
        *claimed = true;
    }
}

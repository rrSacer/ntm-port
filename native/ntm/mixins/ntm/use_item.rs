// A player's item used on a block: the fluid identifier types a duct.
fn ntm_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    if !*claimed && crate::game::ntm_glue::use_item(game, stack, pos, face) {
        *claimed = true;
    }
}

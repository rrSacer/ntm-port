// A player placed an item as a block (BlockItem.place): D3 reads the sawmill's item.
fn ntm_place_block(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3]) {
    crate::game::ntm_glue::place_block(game, stack, pos);
}

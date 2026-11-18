// The drops a loot table rolled (BlockDropsEvent): D3 gives a sawmill without its blade its variant.
fn ntm_loot_drops(game: &mut crate::game::Game<'_>, context: &horizon_client::mod_api::NativeLootDrops<'_>, drops: &mut [horizon_client::items::ItemStack]) {
    crate::game::ntm_glue::loot_drops(game, context.block, drops);
}

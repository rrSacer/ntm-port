fn ntm_item_animation(
    stack: &horizon_client::items::ItemStack,
    animation: &mut horizon_world::consumable::UseAnimation,
) {
    crate::game::ntm_items::animation(stack, animation);
}

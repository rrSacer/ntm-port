// ItemRBMKRod.showDurabilityBar/getDurabilityForDisplay: the bar under a rod's icon, from its tag.
fn ntm_rbmk_rod_bar(stack: &horizon_client::items::ItemStack, bar: &mut Option<(i32, u32)>) {
    if let Some(rod) = ntm_core::rbmk::persist::rod_of(&stack.item) {
        let mut state = crate::game::ntm_glue::stack(&Some(stack.clone())).map(|s| ntm_core::rbmk::persist::stack_rod(&s)).unwrap_or_default();
        *bar = ntm_core::rbmk::tooltip::durability_bar(rod, &mut state);
    }
}

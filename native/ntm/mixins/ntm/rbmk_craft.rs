// ItemRBMKRod.onCreated: a crafted RBMK rod comes out of the grid with its tag set up (setNBTDefaults).
fn ntm_rbmk_rod_crafted(result: &mut Option<horizon_client::items::ItemStack>) {
    crate::game::ntm_glue::rbmk_crafted(result);
}

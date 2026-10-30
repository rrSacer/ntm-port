// What the clients see of a machine: its data and facing.
fn ntm_block_entity_update_tag(id: &str, nbt: &horizon_world::nbt::Compound, tag: &mut Option<horizon_world::nbt::Compound>) {
    crate::game::ntm_glue::update_tag(id, nbt, tag);
}

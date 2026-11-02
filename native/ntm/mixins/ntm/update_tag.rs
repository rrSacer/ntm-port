// The official update-tag hook supplies the whole mod entity.
fn ntm_block_entity_update_tag(entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<horizon_world::nbt::Compound>) {
    crate::game::ntm_glue::update_tag(&entity.id, &entity.nbt, tag);
}

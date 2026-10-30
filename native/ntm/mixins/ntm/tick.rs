// A machine's server tick.
fn ntm_block_entity_tick(entity: &mut horizon_client::containers::ModBlockEntity) {
    crate::game::ntm_glue::tick(entity);
}

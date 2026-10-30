// A machine block placed: its block entity, with its new state.
fn ntm_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::ntm_glue::entity(block, entity);
}

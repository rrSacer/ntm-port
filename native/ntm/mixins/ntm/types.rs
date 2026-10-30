// The machines' block entity type ids, the same on every peer.
fn ntm_block_entity_types(types: &mut Vec<String>) {
    crate::game::ntm_glue::types(types);
}

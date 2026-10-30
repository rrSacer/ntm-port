// The machines' slots, faces and menu.
fn ntm_block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::ntm_glue::container(block, container);
}

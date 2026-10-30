// The rules of a machine's slots.
fn ntm_slot_filters(block: &str, filters: &mut Vec<horizon_client::containers::ModSlotFilter>) {
    crate::game::ntm_glue::slot_filters(block, filters);
}

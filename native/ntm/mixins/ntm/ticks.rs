// The machines' block entities tick.
fn ntm_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::ntm_glue::ticks(block, ticks);
}

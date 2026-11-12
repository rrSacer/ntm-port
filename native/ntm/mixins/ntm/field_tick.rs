// The field's source blocks: a scheduled tick gives the world's radiation by chunk and schedules the next (slice 4d).
// `handled` stays: the block's own tick (a falling block's fall) runs after, as `super.updateTick`.
fn ntm_field_tick(
    level: &mut crate::game::GameLevel<'_>,
    ticks: &mut horizon_world::level::Ticks,
    dimension: horizon_world::level::Dimension,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    handled: &mut bool,
) {
    let _ = handled;
    crate::game::ntm_glue::field_scheduled_tick(level, ticks, dimension, pos, state);
}

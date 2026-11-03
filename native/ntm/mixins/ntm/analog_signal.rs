// The signal such a comparator reads, from the machine's saved state.
fn ntm_analog_output(
    level: &crate::game::GameLevel<'_>,
    dimension: horizon_world::level::Dimension,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    face: horizon_core::geometry::Direction,
    output: &mut i32,
) {
    let _ = (dimension, face);
    crate::game::ntm_glue::analog_output(level, pos, state, output);
}

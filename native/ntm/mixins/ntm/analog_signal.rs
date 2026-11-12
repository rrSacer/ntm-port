// The signal such a comparator reads, from the machine's saved state (the Geiger counter block's, from the field).
fn ntm_analog_output(
    level: &crate::game::GameLevel<'_>,
    dimension: horizon_world::level::Dimension,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    face: horizon_core::geometry::Direction,
    output: &mut i32,
) {
    let _ = face;
    let dim = crate::game::ntm_glue::level_dim_id(dimension);
    crate::game::ntm_glue::analog_output(level, dim, pos, state, output);
}

// A comparator beside a battery, a fluid tank or a barrel reads its charge or fill
// (Java `hasComparatorInputOverride`).
fn ntm_has_analog_output(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    output: &mut bool,
) {
    crate::game::ntm_glue::has_analog_output(level, state, output);
}

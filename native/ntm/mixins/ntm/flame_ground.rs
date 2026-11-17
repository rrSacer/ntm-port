// The ground of a digamma flame (`DigammaFlame.canPlaceBlockAt`, `onNeighborBlockChange`): its dead bush template asks
// it as it is placed and on every shape update.
fn ntm_flame_ground(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    ground: horizon_world::registry::BlockStateId,
    may: &mut Option<bool>,
) {
    crate::game::ntm_glue::flame_ground(level, state, ground, may);
}

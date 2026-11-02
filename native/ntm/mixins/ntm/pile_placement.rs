// BlockPileDevice.onBlockPlaced/onBlockPlacedBy: IO faces the clicked side.
fn ntm_pile_placement(
    level: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    if answer.is_some() { return; }
    let Some(id) = level.native_state_block(state) else { return; };
    if !id.starts_with("ntm:pile_device__") { return; }
    use horizon_core::geometry::Direction;
    let mut facing = match context.face { Direction::South => "south", Direction::West => "west", Direction::East => "east", _ => "north" };
    if id == "ntm:pile_device__control" && facing == "north" {
        facing = ["north","east","south","west"][((context.yaw*4.0/360.0+0.5).floor() as i32 & 3) as usize];
    }
    *answer = Some(level.native_state_with_property(state,"facing",facing));
}

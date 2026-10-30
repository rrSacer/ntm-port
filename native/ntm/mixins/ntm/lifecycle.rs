// A world opened: the bundle says it is in, and how many machine blocks
// its logic knows.
fn ntm_world_lifecycle(phase: crate::game::native_lifecycle::WorldPhase) {
    if phase == crate::game::native_lifecycle::WorldPhase::WorldOpenReady {
        eprintln!("ntm native: {} machine blocks", ntm_core::registry::machine_ids().count());
    }
}

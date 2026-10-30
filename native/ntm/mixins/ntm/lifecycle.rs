// A world opened or closed: the NTM server starts over with it.
fn ntm_world_lifecycle(phase: crate::game::native_lifecycle::WorldPhase) {
    use crate::game::native_lifecycle::WorldPhase as P;
    match phase {
        P::WorldOpenReady => crate::game::ntm_glue::world(true),
        P::WorldCloseFinal | P::WorldCloseFailed | P::WorldOpenFailed => crate::game::ntm_glue::world(false),
    }
}

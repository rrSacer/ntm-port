// Read-only API 22 diagnostic. It never edits the explosion's block list.
fn ntm_trace_explosion(game:&crate::game::Game<'_>,dimension:&str,center:[f64;3],blocks:&[[i32;3]]){
    crate::game::ntm_glue::trace_explosion(game,dimension,center,blocks);
}

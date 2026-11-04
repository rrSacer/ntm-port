// The entities of a chunk were put away to be saved: one gone from there is not paid.
fn ntm_ledger_entities_unload(dimension: &str, chunk: [i32; 2]) {
    crate::game::ntm_glue::entities_put_away(dimension, chunk);
}

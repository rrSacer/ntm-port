// The saved entities of a chunk joined the world: the debts it carries can be judged.
fn ntm_ledger_entities_load(dimension: &str, chunk: [i32; 2]) {
    crate::game::ntm_glue::entities_loaded(dimension, chunk);
}

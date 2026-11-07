// A chunk is about to leave: what it owes is given to its record first, which goes with it.
fn ntm_ledger_chunk_unload(game: &mut crate::game::Game<'_>, dimension: &str, chunk: [i32; 2]) {
    crate::game::ntm_glue::chunk_unloaded(game, dimension, chunk);
}

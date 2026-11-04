// A chunk loaded: the debts it carries are read.
fn ntm_ledger_chunk_load(game: &mut crate::game::Game<'_>, dimension: &str, chunk: [i32; 2]) {
    crate::game::ntm_glue::chunk_loaded(game, dimension, chunk);
}

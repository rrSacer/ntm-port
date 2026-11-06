// A chunk is about to leave: a due record whose items it may hold goes first; its own debts stay in its record.
fn ntm_ledger_chunk_unload(game: &mut crate::game::Game<'_>, dimension: &str, chunk: [i32; 2]) {
    crate::game::ntm_glue::chunk_unloaded(game, dimension, chunk);
}

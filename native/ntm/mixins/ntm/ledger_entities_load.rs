// The saved entities of a chunk joined the world: the debts it carries can be judged.
fn ntm_ledger_entities_load(game: &mut crate::game::Game<'_>, dimension: &str, chunk: [i32; 2]) {
    crate::game::ntm_glue::entities_loaded(game, dimension, chunk);
}

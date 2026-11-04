// A chunk left: its debts stay in its record.
fn ntm_ledger_chunk_unload(dimension: &str, chunk: [i32; 2]) {
    crate::game::ntm_glue::chunk_unloaded(dimension, chunk);
}

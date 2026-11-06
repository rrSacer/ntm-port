// A pickup that went ahead: the ledger tells it from a fold of a stack it follows.
fn ntm_ledger_item_picked_up(_value: (), item: &str, count: u32, canceled: bool) {
    crate::game::ntm_glue::item_picked_up(item, count, canceled);
}

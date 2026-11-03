// A button on a machine's screen.
fn ntm_menu_button(game: &crate::game::Game<'_>, pos: [i32; 3], id: i32,entity:&horizon_client::containers::ModBlockEntity) {
    crate::game::ntm_glue::menu_button(game, pos, id,entity);
}

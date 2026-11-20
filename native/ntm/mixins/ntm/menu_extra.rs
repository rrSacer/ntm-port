// A block's menu opens (server): a screen that takes commands gets its session (RBMK T06).
fn ntm_menu_extra_data(game: &crate::game::Game<'_>, player: u128, block: &str, pos: [i32; 3], data: &mut Option<Vec<u8>>) {
    crate::game::ntm_glue::menu_extra_data(game, player, block, pos, data);
}

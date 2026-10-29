// Creative::tab_position, with the tabs of other pages out of sight.
fn tab_position(tab: &Tab) -> (f32, f32) {
    if !crate::creative::mod_tabs::on_page(tab) {
        return crate::creative::mod_tabs::HIDDEN;
    }
    let mut x = tab.column as f32 * 27.0;
    if tab.column >= 5 {
        x = Creative::WIDTH - 27.0 * (7 - tab.column) as f32 + 1.0;
    }
    let y = if tab.top { -28.0 } else { Creative::HEIGHT - 4.0 };
    (x, y)
}

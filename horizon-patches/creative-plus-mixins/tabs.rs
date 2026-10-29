// Opening the screen: the content mods' tabs, once.
fn tabs(creative: &mut Creative) {
    crate::creative::mod_tabs::ensure(creative);
}

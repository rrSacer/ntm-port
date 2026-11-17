//! D3 of Onda 2: the sawmill's item without its blade, across placing and breaking it (MachineSawmill.onBlockPlacedBy /
//! getDrops, the Java's damage 1).
use super::*;
use crate::sawmill::Sawmill;

const SAWMILL: &str = "ntm:machine_sawmill[facing=north,honey_level=0]";

fn sawmill(g: &mut Game, p: Pos) -> Sawmill {
    let Data::Sawmill(s) = g.machine(p).data else { panic!("no sawmill at {p:?}") };
    s
}

#[test]
fn a_sawmill_placed_from_its_item_without_blade_has_none_and_drops_that_variant() {
    let mut g = game();
    let p = [0, 70, 0];
    g.server.native_sawmill_placed(OW, p, true);
    g.place(p, SAWMILL);
    // Placed and not taken yet (a break in the same tick, before its event): already without its blade.
    assert!(g.server.native_sawmill_without_blade(OW, p));
    assert_eq!(g.server.native_sawmills_without_blade(), [(OW.to_string(), p)]);
    g.run(2);
    assert!(!sawmill(&mut g, p).has_blade);
    assert!(g.server.native_sawmill_without_blade(OW, p));
    assert_eq!(g.server.native_sawmills_without_blade(), [(OW.to_string(), p)], "the model alone, the word taken");
    // Its record keeps it: the world opened again.
    let mut reopened = Game::new(Rc::new(RefCell::new(g.world.borrow().clone())));
    reopened.run(2);
    assert!(!sawmill(&mut reopened, p).has_blade);
    assert!(reopened.server.native_sawmill_without_blade(OW, p));
}

#[test]
fn a_plain_sawmill_keeps_its_blade_and_drops_its_plain_item() {
    let mut g = game();
    let p = [0, 70, 0];
    g.server.native_sawmill_placed(OW, p, false);
    g.place(p, SAWMILL);
    g.run(2);
    assert!(sawmill(&mut g, p).has_blade);
    assert!(!g.server.native_sawmill_without_blade(OW, p));
    assert!(!g.server.native_sawmill_without_blade(OW, [5, 70, 5]), "no machine there");
}

#[test]
fn the_item_without_blade_belongs_to_its_own_placement_only() {
    let mut g = game();
    let (here, there) = ([0, 70, 0], [8, 70, 0]);
    g.server.native_sawmill_placed(OW, here, true);
    // Another sawmill placed elsewhere keeps its blade.
    g.place(there, SAWMILL);
    g.run(2);
    assert!(sawmill(&mut g, there).has_blade);
    g.place(here, SAWMILL);
    g.run(2);
    assert!(!sawmill(&mut g, here).has_blade);
    // Taken by that placement: broken and placed again from a plain item, it has its blade.
    g.break_block(here);
    g.run(2);
    g.place(here, SAWMILL);
    g.run(2);
    assert!(sawmill(&mut g, here).has_blade, "the earlier placement's word is spent");
    // A plain item placed after a word without blade at the same cell speaks last.
    g.break_block(here);
    g.run(2);
    g.server.native_sawmill_placed(OW, here, true);
    g.server.native_sawmill_placed(OW, here, false);
    g.place(here, SAWMILL);
    g.run(2);
    assert!(sawmill(&mut g, here).has_blade);
}

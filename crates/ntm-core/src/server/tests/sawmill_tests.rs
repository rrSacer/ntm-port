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
    // A word comes just before its own block change (the host gives both together): another sawmill placed elsewhere
    // in the same delivery keeps its blade.
    g.server.native_sawmill_placed(OW, here, true);
    g.place(there, SAWMILL);
    g.place(here, SAWMILL);
    g.run(2);
    assert!(sawmill(&mut g, there).has_blade);
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

/// A host that took the block changes before the word of the item that made one (the review's P1-1): the word still
/// reaches the machine its event created, and is kept.
#[test]
fn a_word_given_after_its_placed_event_still_takes_the_blade() {
    let mut g = game();
    let p = [0, 70, 0];
    g.place(p, SAWMILL);
    g.server.native_record_events(&mut g.sh);
    assert!(sawmill(&mut g, p).has_blade, "its event taken first, with the blade");
    g.server.native_sawmill_placed(OW, p, true);
    assert!(!sawmill(&mut g, p).has_blade);
    assert_eq!(g.server.native_sawmills_without_blade(), [(OW.to_string(), p)], "the model, no word left over");
    g.run(2);
    assert!(!sawmill(&mut g, p).has_blade);
    let mut reopened = Game::new(Rc::new(RefCell::new(g.world.borrow().clone())));
    reopened.run(2);
    assert!(!sawmill(&mut reopened, p).has_blade, "kept in its record");
    // A word after its events of an earlier delivery is for no machine of now: a plain sawmill keeps its blade.
    let q = [8, 70, 0];
    g.place(q, SAWMILL);
    g.run(1);
    g.server.native_sawmill_placed(OW, q, true);
    g.run(2);
    assert!(sawmill(&mut g, q).has_blade);
}

/// One word, one placement: in one delivery, a sawmill placed from its item without blade, broken, and another put at
/// the same cell without a word (a command); the second has its blade.
#[test]
fn a_word_is_taken_by_one_placement_only() {
    let mut g = game();
    let p = [0, 70, 0];
    g.server.native_sawmill_placed(OW, p, true);
    g.place(p, SAWMILL);
    g.place(p, "minecraft:air");
    g.place(p, SAWMILL);
    // The second `Placed` may wait behind the first one's removal: a few ticks.
    g.run(6);
    assert!(sawmill(&mut g, p).has_blade);
    assert!(!g.server.native_sawmill_without_blade(OW, p));
}

/// A word whose placement never makes its `Placed` event (undone before the server took the changes) is spent with
/// the events it came with: a sawmill put there later by other means has its blade.
#[test]
fn a_word_whose_placement_was_undone_is_spent() {
    let mut g = game();
    let p = [0, 70, 0];
    g.server.native_sawmill_placed(OW, p, true);
    g.run(1);
    assert!(!g.server.native_sawmill_without_blade(OW, p));
    assert!(g.server.native_sawmills_without_blade().is_empty());
    g.place(p, SAWMILL);
    g.run(2);
    assert!(sawmill(&mut g, p).has_blade);
}

#[test]
fn a_sawmill_on_a_hot_firebox_saws_a_log_into_planks() {
    let mut g = game();
    let (core, source) = ([0, 71, 0], [0, 70, 0]);
    g.place(source, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.place(core, SAWMILL);
    g.run(2);
    let Data::Sawmill(s) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), core)).unwrap().data else { panic!() };
    s.slots[0] = Some(crate::server::container_bridge::Stack::new("minecraft:spruce_log", 1));
    // 2000 below, less the firebox's own cooling in its step (1998): 199 heat a tick, 19 progress, 32 ticks.
    let hot_tick = |g: &mut Game| {
        let Data::Firebox(f) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), source)).unwrap().data else { panic!() };
        f.heat = 2000;
        g.run(1);
    };
    for _ in 0..31 {
        hot_tick(&mut g);
    }
    assert_eq!(sawmill(&mut g, core).progress, 589);
    hot_tick(&mut g);
    let s = sawmill(&mut g, core);
    assert_eq!(s.slot(0), None);
    assert_eq!(s.slot(1), Some(&crate::server::container_bridge::Stack::new("minecraft:spruce_planks", 6)));
    assert!(s.has_blade);
}

fn hands(g: &mut Game, inventory: Vec<Option<crate::server::container_bridge::Stack>>) {
    let inventory = inventory.into_iter().map(|s| s.map(|stack| HeldStack { encoded: format!("{stack:?}"), stack, plain: true })).collect();
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory, ..Default::default() });
}
fn inventory(g: &Game) -> Vec<Option<crate::server::container_bridge::Stack>> {
    g.world.borrow().hands.as_ref().unwrap().inventory.iter().map(|s| s.as_ref().map(|h| h.stack.clone())).collect()
}
fn stack(id: &str, n: u32) -> Option<crate::server::container_bridge::Stack> {
    Some(crate::server::container_bridge::Stack::new(id, n))
}
fn set_slots(g: &mut Game, p: Pos, slots: Vec<Option<crate::server::container_bridge::Stack>>) {
    let Data::Sawmill(s) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), p)).unwrap().data else { panic!() };
    s.slots = slots;
}

#[test]
fn a_blade_in_hand_goes_back_into_a_sawmill_without_one_in_one_transfer() {
    let mut g = game();
    let p = [0, 70, 0];
    g.server.native_sawmill_placed(OW, p, true);
    g.place(p, SAWMILL);
    g.run(2);
    hands(&mut g, vec![stack(crate::sawmill::BLADE, 2), None]);
    // Refused: nothing moves, the blade stays in hand.
    g.world.borrow_mut().refuse_commits = true;
    assert!(g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    assert!(!sawmill(&mut g, p).has_blade);
    assert_eq!(inventory(&g)[0], stack(crate::sawmill::BLADE, 2));
    assert!(g.world.borrow().without_blade.contains(&(OW.to_string(), p)), "refused: the copy says none still");
    g.world.borrow_mut().refuse_commits = false;
    assert!(g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    assert!(sawmill(&mut g, p).has_blade);
    // The host's copy knows at once (the review's P2-1: a blast before the next tick would else drop the item without
    // its blade, the blade lost).
    assert!(g.world.borrow().without_blade.is_empty());
    assert_eq!(inventory(&g)[0], stack(crate::sawmill::BLADE, 1));
    // With its blade, a blade in hand is no input: nothing applies.
    assert!(!g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    assert_eq!(inventory(&g)[0], stack(crate::sawmill::BLADE, 1));
}

#[test]
fn the_outputs_go_to_the_player_and_what_does_not_fit_is_thrown_once() {
    let mut g = game();
    let p = [0, 70, 0];
    g.place(p, SAWMILL);
    g.run(2);
    set_slots(&mut g, p, vec![None, stack("minecraft:oak_planks", 6), stack(crate::sawmill::SAWDUST, 1)]);
    // 62 planks and a free slot: 2 join them, 4 take the free slot, the sawdust is thrown.
    hands(&mut g, vec![stack("minecraft:oak_planks", 62), None]);
    assert!(g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    assert_eq!(inventory(&g), vec![stack("minecraft:oak_planks", 64), stack("minecraft:oak_planks", 4)]);
    assert!(g.world.borrow().dropped.iter().any(|d| d == &format!("player {} 1", crate::sawmill::SAWDUST)));
    assert_eq!(g.world.borrow().dropped.len(), 1, "only what did not fit, once");
    let s = sawmill(&mut g, p);
    assert!(s.slot(1).is_none() && s.slot(2).is_none());
}

/// `storeItemStack` piles only onto a stack whose whole NBT is the given one's: planks with a name of their own (a
/// component the port's stack does not keep) are another stack (the review's P3).
#[test]
fn the_outputs_never_pile_onto_a_stack_with_other_components() {
    let mut g = game();
    let p = [0, 70, 0];
    g.place(p, SAWMILL);
    g.run(2);
    set_slots(&mut g, p, vec![None, stack("minecraft:oak_planks", 6), None]);
    g.run(1);
    let named = HeldStack { encoded: "{id:\"minecraft:oak_planks\",count:10,components:{\"minecraft:custom_name\":\"Tábuas\"}}".into(), stack: crate::server::container_bridge::Stack::new("minecraft:oak_planks", 10), plain: false };
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory: vec![Some(named.clone()), None], ..Default::default() });
    assert!(g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    let hands = g.world.borrow().hands.clone().unwrap();
    assert_eq!(hands.inventory[0], Some(named), "the named stack untouched");
    assert_eq!(hands.inventory[1].as_ref().map(|h| h.stack.clone()), stack("minecraft:oak_planks", 6));
}

#[test]
fn one_of_the_stack_in_hand_goes_into_an_empty_input_only_when_it_has_an_output() {
    let mut g = game();
    let p = [0, 70, 0];
    g.place(p, SAWMILL);
    g.run(2);
    hands(&mut g, vec![stack("minecraft:stone", 3), None]);
    assert!(!g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"), "stone has no output");
    hands(&mut g, vec![stack("minecraft:oak_log", 3), None]);
    assert!(g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    assert_eq!(sawmill(&mut g, p).slot(0), stack("minecraft:oak_log", 1).as_ref());
    assert_eq!(inventory(&g)[0], stack("minecraft:oak_log", 2));
    assert!(!g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"), "the input is taken");
    assert_eq!(inventory(&g)[0], stack("minecraft:oak_log", 2));
}

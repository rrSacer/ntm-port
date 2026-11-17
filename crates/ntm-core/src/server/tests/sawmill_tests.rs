//! D3 of Onda 2: the sawmill's item without its blade, across placing and breaking it (MachineSawmill.onBlockPlacedBy /
//! getDrops, the Java's damage 1).
use super::*;
use crate::sawmill::Sawmill;
use crate::server::container_bridge::Stack;

const SAWMILL: &str = "ntm:machine_sawmill[facing=north,honey_level=0]";

/// A game whose ticks take their events once, as the game's own (the second review's residual P2: a test of the words
/// must not pass on an extra pass of the events).
fn played() -> Game {
    let mut g = game();
    g.events_once = true;
    g
}

fn sawmill(g: &mut Game, p: Pos) -> Sawmill {
    let Data::Sawmill(s) = g.machine(p).data else { panic!("no sawmill at {p:?}") };
    s
}

#[test]
fn a_sawmill_placed_from_its_item_without_blade_has_none_and_drops_that_variant() {
    let mut g = played();
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
    reopened.events_once = true;
    reopened.run(2);
    assert!(!sawmill(&mut reopened, p).has_blade);
    assert!(reopened.server.native_sawmill_without_blade(OW, p));
}

#[test]
fn a_plain_sawmill_keeps_its_blade_and_drops_its_plain_item() {
    let mut g = played();
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
    let mut g = played();
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

/// The second review's residual P2: a plain sawmill A placed in one tick, broken in the next, and a sawmill without its
/// blade B placed at the same cell before the server took any of it. The word is B's: it never reaches A's old model,
/// and B has no blade (else A's item kept its blade and B had one too).
#[test]
fn an_old_sawmill_never_takes_the_word_of_the_one_placed_after_it() {
    let mut g = played();
    let p = [0, 70, 0];
    g.place(p, SAWMILL);
    g.run(1);
    assert!(sawmill(&mut g, p).has_blade);
    // The next tick's delivery, in the host's order: the break, then B's word before B's own change.
    g.place(p, "minecraft:air");
    g.server.native_sawmill_placed(OW, p, true);
    assert!(sawmill(&mut g, p).has_blade, "A's model is not B's");
    g.place(p, SAWMILL);
    g.run(6);
    assert!(!sawmill(&mut g, p).has_blade, "B has no blade");
    assert_eq!(g.server.native_sawmills_without_blade(), [(OW.to_string(), p)], "B's model, no word left over");
}

/// One word, one placement: in one delivery, a sawmill placed from its item without blade, broken, and another put at
/// the same cell without a word (a command); the second has its blade.
#[test]
fn a_word_is_taken_by_one_placement_only() {
    let mut g = played();
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
    let mut g = played();
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
    let mut g = played();
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
/// The block entity's slots, as a hopper or a player left them: the truth the sawmill reads.
fn set_slots(g: &mut Game, p: Pos, slots: Vec<Option<crate::server::container_bridge::Stack>>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), p)).expect("its block entity").0 = slots;
}
fn engine_slots(g: &Game, p: Pos) -> Vec<Option<crate::server::container_bridge::Stack>> {
    g.world.borrow().engine[&(OW.to_string(), p)].0.clone()
}

#[test]
fn a_blade_in_hand_goes_back_into_a_sawmill_without_one_in_one_transfer() {
    let mut g = played();
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
    let mut g = played();
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
    assert_eq!(engine_slots(&g, p), vec![None; 3], "its block entity too");
}

/// `storeItemStack` piles only onto a stack whose whole NBT is the given one's: planks with a name of their own (a
/// component the port's stack does not keep) are another stack (the review's P3).
#[test]
fn the_outputs_never_pile_onto_a_stack_with_other_components() {
    let mut g = played();
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
    let mut g = played();
    let p = [0, 70, 0];
    g.place(p, SAWMILL);
    g.run(2);
    hands(&mut g, vec![stack("minecraft:stone", 3), None]);
    assert!(!g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"), "stone has no output");
    // A log with data of its own: one of it goes in whole, as the Java's `copy()` (the host copies the held stack).
    let named = Some(crate::server::container_bridge::Stack { custom: Some("{name:1b}".into()), ..Stack::new("minecraft:oak_log", 3) });
    hands(&mut g, vec![named.clone(), None]);
    assert!(g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    let one = named.clone().map(|s| Stack { count: 1, ..s });
    assert_eq!(sawmill(&mut g, p).slot(0), one.as_ref());
    assert_eq!(engine_slots(&g, p), vec![one, None, None]);
    assert_eq!(g.world.borrow().copied, vec![format!("{:?}", named.clone().unwrap())], "one copy of the stack in hand");
    assert_eq!(inventory(&g)[0], named.map(|s| Stack { count: 2, ..s }));
    assert!(!g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"), "the input is taken");
    assert_eq!(inventory(&g)[0].as_ref().map(|s| s.count), Some(2));
}

/// The uses read the block entity's slots, which a hopper may have changed since the machines ran: a log a hopper
/// put in is not doubled by a hand, and outputs a hopper took are not given again.
#[test]
fn a_use_reads_the_block_entity_a_hopper_changed() {
    let mut g = played();
    let p = [0, 70, 0];
    g.place(p, SAWMILL);
    g.run(2);
    set_slots(&mut g, p, vec![stack("minecraft:oak_log", 1), None, None]);
    assert!(sawmill(&mut g, p).slot(0).is_none(), "the model has not seen it yet");
    hands(&mut g, vec![stack("minecraft:oak_log", 3), None]);
    assert!(!g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"), "its input is taken");
    assert_eq!(inventory(&g)[0], stack("minecraft:oak_log", 3));
    // Outputs the model still has, taken by a hopper: nothing to give, the log in hand goes into the free input.
    set_slots(&mut g, p, vec![None, stack("minecraft:oak_planks", 6), None]);
    g.run(1);
    set_slots(&mut g, p, vec![None; 3]);
    assert!(sawmill(&mut g, p).slot(1).is_some());
    assert!(g.server.native_sawmill_use(&mut g.sh, OW, p, p, "p1"));
    assert_eq!(inventory(&g), vec![stack("minecraft:oak_log", 2), None]);
    assert_eq!(engine_slots(&g, p), vec![stack("minecraft:oak_log", 1), None, None]);
    assert!(g.world.borrow().dropped.is_empty());
}

/// The second review's P1 (3c): a sawmill that loses its blade with a log inside, its chunk cut older than the journal
/// that step wrote. The journal's model has no slots (they are the block entity's alone): restored from it, the sawmill
/// takes the block entity's log, never writes empty slots over it.
#[test]
fn a_sawmill_restored_from_the_journal_keeps_the_log_in_its_block_entity() {
    let mut g = played();
    g.world.borrow_mut().explosions = true;
    let (core, source) = ([0, 71, 0], [0, 70, 0]);
    g.place(source, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.place(core, SAWMILL);
    g.run(2);
    set_slots(&mut g, core, vec![stack("minecraft:oak_log", 1), None, None]);
    g.run(1);
    let key = (OW.to_string(), core);
    // The chunk as last saved, before the failure: the model with its blade, the log in the block entity.
    let (old_json, old_slots) = (g.world.borrow().saved[&key].clone(), engine_slots(&g, core));
    assert_eq!(old_slots, vec![stack("minecraft:oak_log", 1), None, None]);
    // 400 heat a tick past 300 overspeed ticks: the blade breaks (with its blast and its item, journaled).
    let Data::Sawmill(s) = &mut g.ready().world.machines.get_mut(&key).unwrap().data else { panic!() };
    s.overspeed = 300;
    let Data::Firebox(f) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), source)).unwrap().data else { panic!() };
    f.heat = 4000;
    g.run(1);
    assert!(!sawmill(&mut g, core).has_blade);
    assert_eq!(engine_slots(&g, core), old_slots, "the failure leaves the slots");
    // The journal (the session's, or a storage an earlier version wrote with it) newer than the chunk's record, still
    // from before the failure: the model comes back from the journal.
    assert!(g.server.ready.world.survivors.contains_key(&key), "the failure is journaled");
    let earlier = g.server.ready.world.kept_as_an_earlier_version(0);
    let world = Rc::new(RefCell::new(g.world.borrow().clone()));
    world.borrow_mut().kv.set(crate::persist::KEY, &earlier).unwrap();
    world.borrow_mut().saved.insert(key.clone(), old_json);
    world.borrow_mut().engine.get_mut(&key).unwrap().0 = old_slots.clone();
    let mut reopened = Game::new(world);
    reopened.events_once = true;
    assert!(reopened.server.ready.restored.contains(&key), "restored from the journal");
    // Its batch (the blast, then the blade) settles first; then the sawmill steps again.
    reopened.run(6);
    assert!(reopened.server.ready.world.machine_effects.is_empty() && reopened.server.ready.bridge.last.contains_key(&key));
    assert!(!sawmill(&mut reopened, core).has_blade, "the journal's model, without its blade");
    assert_eq!(engine_slots(&reopened, core), old_slots, "the log is kept");
    assert_eq!(sawmill(&mut reopened, core).slot(0), stack("minecraft:oak_log", 1).as_ref());
}

/// A hopper's log saws as a hand's: the block entity's slots are the truth, the outputs written back to them.
#[test]
fn a_log_in_the_block_entity_saws_into_its_slots() {
    let mut g = played();
    let (core, source) = ([0, 71, 0], [0, 70, 0]);
    g.place(source, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.place(core, SAWMILL);
    g.run(2);
    assert_eq!(engine_slots(&g, core), vec![None; 3]);
    set_slots(&mut g, core, vec![stack("minecraft:oak_log", 1), None, None]);
    for _ in 0..40 {
        let Data::Firebox(f) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), source)).unwrap().data else { panic!() };
        f.heat = 2000;
        g.run(1);
    }
    let out = engine_slots(&g, core);
    assert_eq!((out[0].clone(), out[1].clone()), (None, stack("minecraft:oak_planks", 6)));
    // The record keeps no slots: they are the block entity's alone.
    let json = g.world.borrow().saved[&(OW.to_string(), core)].clone();
    assert!(!json.contains("slots") && !json.contains("oak"), "{json}");
}

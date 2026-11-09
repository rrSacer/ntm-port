//! T01: an RBMK layer is one coupled group (its columns' block entities and its kept order, random and streams step
//! and commit together), frozen while any part of it is unknown, saved whole; a fuel rod goes in by hand in one
//! transfer with the player.
use super::*;
use crate::rbmk::persist::{Layer, RbmkColumn};
use container_bridge::Stack;

const Y: i32 = 71;

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

/// `n` of `item` in the acting player's hand (hotbar slot 0).
fn hold(g: &mut Game, item: &str, n: u32, creative: bool) {
    let stack = Stack::new(item, n);
    let held = HeldStack { encoded: format!("{stack:?}"), stack };
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory: vec![Some(held), None], creative, ..Default::default() });
}

fn in_hand(g: &Game) -> Option<Stack> {
    g.world.borrow().hands.as_ref().and_then(|h| h.held().map(|s| s.stack.clone()))
}

fn rbmk(g: &mut Game, p: Pos) -> RbmkColumn {
    let Data::Rbmk(c) = g.machine(p).data else { panic!("{p:?} is not an RBMK column") };
    *c
}

fn layer(g: &mut Game) -> Option<Layer> {
    g.ready().world.rbmk.get(&(OW.to_string(), Y)).cloned()
}

fn slot(g: &Game, p: Pos) -> Option<Stack> {
    g.world.borrow().engine[&(OW.to_string(), p)].0[0].clone()
}

/// A small reactor: a ReaSim channel with a Po-210/Be source (it draws on the layer's random every tick it emits),
/// two uranium channels, a moderator and a reflector, all on the layer at Y; each channel fuelled by hand.
fn reactor() -> Game {
    let mut g = game();
    for (p, id) in [([0, Y, 0], "rbmk_rod_reasim"), ([1, Y, 0], "rbmk_rod"), ([2, Y, 0], "rbmk_rod_mod"), ([1, Y, 1], "rbmk_moderator"), ([3, Y, 0], "rbmk_reflector")] {
        g.place(p, &column(id));
    }
    g.run(2);
    for (p, rod) in [([0, Y, 0], "ntm:rbmk_fuel_po210be"), ([1, Y, 0], "ntm:rbmk_fuel_ueu"), ([2, Y, 0], "ntm:rbmk_fuel_meu")] {
        hold(&mut g, rod, 2, false);
        assert!(g.server.native_rbmk_use(&mut g.sh, OW, [p[0], p[1] + 2, p[2]], p, "p1"), "{rod} goes in");
    }
    g
}

/// What the save would hold of the reactor: every column's state and slots, and the layer's state.
fn state(g: &mut Game) -> (Vec<String>, Vec<Option<Stack>>, Option<Layer>) {
    let cols = [[0, Y, 0], [1, Y, 0], [2, Y, 0], [1, Y, 1], [3, Y, 0]];
    let json = cols.iter().map(|p| serde_json::to_string(&rbmk(g, *p)).unwrap()).collect();
    let slots = cols[..3].iter().map(|p| slot(g, *p)).collect();
    (json, slots, layer(g))
}

#[test]
fn a_rod_goes_in_by_hand_with_the_hand_debited_in_the_same_transfer() {
    let mut g = game();
    let at = [0, Y, 0];
    g.place(at, &column("rbmk_rod"));
    g.place([1, Y, 0], &column("rbmk_blank"));
    g.run(2);
    // Not a rod in hand, or not a fuel channel: the click goes on (the menu).
    hold(&mut g, "minecraft:stone", 1, false);
    assert!(!g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    hold(&mut g, "ntm:rbmk_fuel_ueu", 3, false);
    assert!(!g.server.native_rbmk_use(&mut g.sh, OW, [1, Y, 0], [1, Y, 0], "p1"));
    // Refused after its checks: neither side moves.
    g.world.borrow_mut().refuse_commits = true;
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    assert_eq!((slot(&g, at), in_hand(&g).map(|s| s.count)), (None, Some(3)));
    assert_eq!(rbmk(&mut g, at).slots, vec![None], "nor the model");
    g.world.borrow_mut().refuse_commits = false;
    // Accepted: one rod in, one out of the hand, with the click's sound.
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    assert_eq!((slot(&g, at), in_hand(&g).map(|s| s.count)), (Some(Stack::new("ntm:rbmk_fuel_ueu", 1)), Some(2)));
    assert!(g.world.borrow().shown.iter().any(|(_, s)| matches!(s, crate::blast::visuals::Show::Sound { id: "ntm:item.upgradeplug", .. })));
    // A full channel takes nothing: the click goes on (the menu).
    assert!(!g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    assert_eq!(in_hand(&g).map(|s| s.count), Some(2));
    // Creative: the rod is copied, the hand keeps its stack (`!isCreativeMode`).
    let other = [5, Y, 5];
    g.place(other, &column("rbmk_rod_reasim"));
    g.run(1);
    hold(&mut g, "ntm:rbmk_fuel_mox", 1, true);
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, other, other, "p1"));
    assert_eq!((slot(&g, other).map(|s| s.id), in_hand(&g).map(|s| s.count)), (Some("ntm:rbmk_fuel_mox".into()), Some(1)));
    // The last rod of a stack leaves the hand empty.
    let third = [7, Y, 7];
    g.place(third, &column("rbmk_rod_mod"));
    g.run(1);
    hold(&mut g, "ntm:rbmk_fuel_mox", 1, false);
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, third, third, "p1"));
    assert_eq!(in_hand(&g), None);
}

#[test]
fn a_layer_steps_its_columns_and_its_state_together_or_not_at_all() {
    let mut g = reactor();
    g.run(30);
    let (_, slots, l) = state(&mut g);
    let l = l.expect("the layer's state is kept");
    assert_eq!(l.order, vec![[0, 0], [1, 0], [1, 1], [2, 0], [3, 0]], "first registered together: by position");
    assert!(!l.streams.is_empty(), "the source keeps streams queued for the next step");
    assert_ne!(l.seed, Layer::new(OW, Y).seed, "the ReaSim source drew on the layer's random");
    assert!(rbmk(&mut g, [1, Y, 0]).heat > 20.0 && rbmk(&mut g, [1, Y, 0]).has_rod);
    for s in slots {
        let custom = s.unwrap().custom.expect("the rod's tag is in its stack");
        assert!(crate::snbt::fields(&custom).unwrap().contains_key("hull"), "{custom}");
    }
    // A refused commit: nothing of the layer advances, neither the columns nor its random and streams.
    let before = state(&mut g);
    g.world.borrow_mut().refuse_commits = true;
    g.run(3);
    assert_eq!(state(&mut g), before);
    g.world.borrow_mut().refuse_commits = false;
    g.run(1);
    assert_ne!(state(&mut g), before);
}

#[test]
fn a_layer_with_a_column_out_of_reach_waits_whole() {
    let mut g = reactor();
    // A column of the same layer in the next chunk.
    let far = [17, Y, 0];
    g.place(far, &column("rbmk_blank"));
    g.run(5);
    assert!(layer(&mut g).unwrap().order.contains(&[17, 0]));
    g.world.borrow_mut().unloaded.insert((OW.into(), 1, 0));
    let before = state(&mut g);
    g.run(5);
    assert_eq!(state(&mut g), before, "D4: an unknown column is not gone; the layer does not step without it");
    g.world.borrow_mut().unloaded.clear();
    g.run(1);
    assert_ne!(state(&mut g), before);
    // A column that is gone (the world shows something else) leaves the order, its queued streams with it.
    g.break_block(far);
    g.run(2);
    assert!(!layer(&mut g).unwrap().order.contains(&[17, 0]));
}

#[test]
fn a_reopened_layer_waits_for_a_column_not_adopted_yet_and_for_cells_in_reach() {
    let mut g = reactor();
    let far = [17, Y, 0];
    g.place(far, &column("rbmk_blank"));
    g.run(5);
    // The world opened again with the far column's chunk not loaded: its block entity is not adopted yet.
    let mut world = g.world.borrow().clone();
    let json = world.saved.remove(&(OW.to_string(), far)).unwrap();
    world.unloaded.insert((OW.into(), 1, 0));
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    let before = state(&mut reopened);
    reopened.run(4);
    assert_eq!(state(&mut reopened), before, "D4: the layer waits for the column its order names");
    assert!(layer(&mut reopened).unwrap().order.contains(&[17, 0]));
    // Loaded, its block shown, its block entity not ticked yet (adoption comes with its first tick): still waiting.
    reopened.world.borrow_mut().unloaded.clear();
    reopened.run(2);
    assert_eq!(state(&mut reopened), before, "a column the world shows is not gone before it is adopted");
    assert!(reopened.server.native_adopt(OW, far, &json));
    reopened.run(1);
    assert_ne!(state(&mut reopened), before);
    // A cell a stream can reach in a chunk not loaded (x below 0 is the chunk -1): the layer waits too.
    reopened.world.borrow_mut().unloaded.insert((OW.into(), -1, 0));
    let before = state(&mut reopened);
    reopened.run(3);
    assert_eq!(state(&mut reopened), before, "an unknown cell is not air");
}

#[test]
fn a_reopened_world_steps_on_exactly_as_one_never_closed() {
    let mut once = reactor();
    let mut twice = reactor();
    once.run(40);
    twice.run(17);
    // The world opened again from what it holds: block entities, slots and the storage.
    let world = twice.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    reopened.run(23);
    assert_eq!(state(&mut reopened), state(&mut once), "columns, rods, order, random and streams, bit for bit");
}

#[test]
fn the_storage_keeps_the_layer_and_reads_it_back() {
    let mut g = reactor();
    g.run(10);
    let kept = crate::persist::WorldState::load(&mut g.sh);
    assert_eq!(kept.rbmk.get(&(OW.to_string(), Y)), layer(&mut g).as_ref());
    // A layer of another schema is not read as this one.
    let mut l = layer(&mut g).unwrap();
    l.schema += 1;
    g.ready().world.rbmk.insert((OW.to_string(), Y), l);
    g.server.write_world(&mut g.sh).unwrap();
    assert!(crate::persist::WorldState::load(&mut g.sh).rbmk.is_empty());
}

/// A batch of entity effects (debris, T20) as a step leaves it: owned by the reactor's first column.
fn debris_batch(g: &mut Game) -> CommittedEffects {
    let owner = (OW.to_string(), [1, Y, 0]);
    let m = g.machine(owner.1);
    let spawn = |x: f64| EntitySpawn { kind: "minecraft:armor_stand".into(), pos: [x, f64::from(Y) + 4.0, 0.5], nbt: "{Invisible:1b}".into() };
    CommittedEffects {
        owners: vec![owner.clone()],
        origins: vec![CellGuard::machine(&owner, &m)],
        effects: [WorldEffect::Entities { dim: OW.into(), pos: [1, Y + 4, 0], entities: vec![spawn(1.5), spawn(1.25)] }].into(),
        ..Default::default()
    }
}

#[test]
fn entities_of_an_effect_come_out_once_by_its_id_and_wait_on_a_host_without_them() {
    let mut g = reactor();
    g.run(2);
    // A host without entities: the batch waits, its owner frozen, nothing lost.
    g.world.borrow_mut().no_entities = true;
    let batch = debris_batch(&mut g);
    g.ready().world.machine_effects.push_back(batch);
    let held = state(&mut g);
    g.run(3);
    assert_eq!(g.ready().world.machine_effects.len(), 1, "the batch waits");
    assert!(g.world.borrow().entities.is_empty());
    assert_eq!(state(&mut g), held, "its owner's layer does not step past it");
    // Supported: both entities, once, and a receipt kept until a save is confirmed.
    g.world.borrow_mut().no_entities = false;
    g.run(1);
    assert_eq!(g.world.borrow().entities.len(), 2);
    assert!(g.ready().world.machine_effects.is_empty());
    let receipt = g.ready().world.applied_machine_effects.back().cloned().expect("a receipt");
    assert!(matches!(receipt.effects.front(), Some(WorldEffect::Entities { .. })));
    // The same batch again (a replay from a record saved before it was applied): its id is known, nothing more.
    let mut replay = debris_batch(&mut g);
    replay.serial = receipt.serial;
    g.ready().world.machine_effects.push_back(replay);
    g.run(2);
    assert_eq!(g.world.borrow().entities.len(), 2, "a replay adds nothing");
    assert!(g.ready().world.machine_effects.is_empty());
}

#[test]
fn a_storage_that_cannot_be_written_keeps_the_layer_for_the_next_write() {
    let mut g = reactor();
    g.run(5);
    g.world.borrow_mut().kv_fails = true;
    g.run(5);
    let in_memory = layer(&mut g);
    let stored = crate::persist::WorldState::load(&mut g.sh).rbmk.get(&(OW.to_string(), Y)).cloned();
    assert_ne!(stored, in_memory, "the write failed");
    g.world.borrow_mut().kv_fails = false;
    g.run(1);
    assert_eq!(crate::persist::WorldState::load(&mut g.sh).rbmk.get(&(OW.to_string(), Y)).cloned(), layer(&mut g), "written at the next keep");
}

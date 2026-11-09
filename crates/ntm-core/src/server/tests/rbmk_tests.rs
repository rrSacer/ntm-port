//! T01: each RBMK reactor (the component of a layer's columns its streams and heat link) is one coupled group: its
//! columns' block entities and its kept random and streams step and commit together, it waits whole while a column of
//! it is unknown, and it is saved whole; a far column is another reactor; components merge and split
//! deterministically; a fuel rod goes in by hand in one transfer with the player, its whole stack copied.
use super::*;
use crate::rbmk::persist::{CompState, Layer, RbmkColumn};
use container_bridge::Stack;

const Y: i32 = 71;
const A: [i32; 2] = [0, 0];

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

fn layer(g: &mut Game) -> Layer {
    g.ready().world.rbmk.get(&(OW.to_string(), Y)).cloned().unwrap_or_default()
}

fn slot(g: &Game, p: Pos) -> Option<Stack> {
    g.world.borrow().engine[&(OW.to_string(), p)].0[0].clone()
}

fn fuel(g: &mut Game, p: Pos, rod: &str) {
    hold(g, rod, 2, false);
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, [p[0], p[1] + 2, p[2]], p, "p1"), "{rod} goes in");
}

/// A small reactor: a ReaSim channel with a Po-210/Be source (it draws on its random every tick it emits), two
/// uranium channels, a moderator and a reflector; each channel fuelled by hand.
fn reactor() -> Game {
    let mut g = game();
    for (p, id) in [([0, Y, 0], "rbmk_rod_reasim"), ([1, Y, 0], "rbmk_rod"), ([2, Y, 0], "rbmk_rod_mod"), ([1, Y, 1], "rbmk_moderator"), ([3, Y, 0], "rbmk_reflector")] {
        g.place(p, &column(id));
    }
    g.run(2);
    for (p, rod) in [([0, Y, 0], "ntm:rbmk_fuel_po210be"), ([1, Y, 0], "ntm:rbmk_fuel_ueu"), ([2, Y, 0], "ntm:rbmk_fuel_meu")] {
        fuel(&mut g, p, rod);
    }
    g
}

const COLS: [Pos; 5] = [[0, Y, 0], [1, Y, 0], [2, Y, 0], [1, Y, 1], [3, Y, 0]];

/// What the save holds of the reactor A: every column's state and slots, its members and its component's state.
type Saved = (Vec<String>, Vec<Option<Stack>>, Vec<(u64, [i32; 2])>, Option<CompState>);

fn state(g: &mut Game) -> Saved {
    let json = COLS.iter().map(|p| serde_json::to_string(&rbmk(g, *p)).unwrap()).collect();
    let slots = COLS[..3].iter().map(|p| slot(g, *p)).collect();
    let l = layer(g);
    let members = COLS.iter().map(|p| l.members.get(&[p[0], p[2]]).map(|m| (m.seq, m.comp)).unwrap_or((u64::MAX, [0, 0]))).collect();
    (json, slots, members, l.comps.get(&A).cloned())
}

#[test]
fn a_rod_goes_in_by_hand_with_its_whole_stack_and_the_hand_debited_in_the_same_transfer() {
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
    // Accepted: one rod in, a copy of the whole stack held (`copy()`, P1-2), one out of the hand, with the sound.
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, at, at, "p1"));
    assert_eq!((slot(&g, at), in_hand(&g).map(|s| s.count)), (Some(Stack::new("ntm:rbmk_fuel_ueu", 1)), Some(2)));
    assert_eq!(g.world.borrow().copied, vec![format!("{:?}", Stack::new("ntm:rbmk_fuel_ueu", 3))], "the stack as held");
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
fn a_reactor_steps_its_columns_and_its_state_together_or_not_at_all() {
    let mut g = reactor();
    g.run(30);
    let l = layer(&mut g);
    let order: Vec<[i32; 2]> = {
        let mut o: Vec<_> = l.members.iter().map(|(c, m)| (m.seq, *c)).collect();
        o.sort();
        o.into_iter().map(|(_, c)| c).collect()
    };
    assert_eq!(order, vec![[0, 0], [1, 0], [1, 1], [2, 0], [3, 0]], "first registered together: by position");
    assert!(l.members.values().all(|m| m.comp == A), "one reactor, under its first column");
    let comp = &l.comps[&A];
    assert!(!comp.streams.is_empty(), "the source keeps streams queued for the next step");
    assert_ne!(comp.seed, CompState::fresh(OW, Y, A).seed, "the ReaSim source drew on the reactor's random");
    assert!(rbmk(&mut g, [1, Y, 0]).heat > 20.0 && rbmk(&mut g, [1, Y, 0]).has_rod);
    for s in COLS[..3].iter().map(|p| slot(&g, *p)) {
        let custom = s.unwrap().custom.expect("the rod's tag is in its stack");
        assert!(crate::snbt::fields(&custom).unwrap().contains_key("hull"), "{custom}");
    }
    // A refused commit: nothing of the reactor advances, neither the columns nor its random and streams.
    let before = state(&mut g);
    g.world.borrow_mut().refuse_commits = true;
    g.run(3);
    assert_eq!(state(&mut g), before);
    g.world.borrow_mut().refuse_commits = false;
    g.run(1);
    assert_ne!(state(&mut g), before);
}

#[test]
fn a_reactor_with_a_column_of_its_own_out_of_reach_waits_whole() {
    let mut g = reactor();
    // A blank the ReaSim source's streams reach (five cells west), in the chunk -1: part of the reactor.
    let far = [-5, Y, 0];
    g.place(far, &column("rbmk_blank"));
    g.run(5);
    assert_eq!(layer(&mut g).members[&[-5, 0]].comp, A);
    g.world.borrow_mut().unloaded.insert((OW.into(), -1, 0));
    let before = state(&mut g);
    g.run(5);
    assert_eq!(state(&mut g), before, "D4: an unknown column is not gone; the reactor does not step without it");
    g.world.borrow_mut().unloaded.clear();
    g.run(1);
    assert_ne!(state(&mut g), before);
    // A column that is gone (the world shows something else) leaves its reactor, its queued streams with it.
    g.break_block(far);
    g.run(2);
    assert!(!layer(&mut g).members.contains_key(&[-5, 0]));
}

/// P0-1: a column out of every stream's reach is another reactor: it neither holds the reactor (loaded or not) nor
/// makes its step read the cells between them.
#[test]
fn a_column_far_away_is_another_reactor() {
    let mut g = reactor();
    g.run(3);
    let reads = |g: &mut Game| {
        g.sh.reads = 0;
        g.run(1);
        g.sh.reads
    };
    let alone = reads(&mut g);
    g.place([640, Y, 640], &column("rbmk_blank"));
    g.run(3);
    let l = layer(&mut g);
    assert_eq!(l.members[&[640, 640]].comp, [640, 640], "its own reactor");
    assert!(l.comps.contains_key(&A) && l.comps.contains_key(&[640, 640]));
    let both = reads(&mut g);
    assert!(both <= alone + 40, "the step reads the reactor's rays, not the space between: {alone} then {both}");
    // Its chunk not loaded: the reactor steps on.
    g.world.borrow_mut().unloaded.insert((OW.into(), 40, 40));
    let before = state(&mut g);
    g.run(3);
    assert_ne!(state(&mut g), before, "the far column holds nothing of A");
    assert!(layer(&mut g).members.contains_key(&[640, 640]), "and it is still known (D4)");
    // A chunk between them not loaded: nothing either.
    g.world.borrow_mut().unloaded.insert((OW.into(), 18, 18));
    let before = state(&mut g);
    g.run(2);
    assert_ne!(state(&mut g), before);
}

/// Two reactors that a row of blanks joins are one; a blank broken in the row splits them again. Both are the same in
/// every world, and a world reopened between gives the very same numbers.
#[test]
fn reactors_merge_and_split_deterministically() {
    fn play(reopen_at: Option<usize>) -> (Game, Vec<Layer>) {
        let mut g = reactor();
        // B: a second source fifteen cells east; then A grows by a fuel channel registered after B (their registrations
        // interleave: a merge must order the streams by registration, not by component).
        g.place([20, Y, 0], &column("rbmk_rod_reasim"));
        g.run(1);
        fuel(&mut g, [20, Y, 0], "ntm:rbmk_fuel_po210be");
        g.place([0, Y, -1], &column("rbmk_rod"));
        g.run(1);
        fuel(&mut g, [0, Y, -1], "ntm:rbmk_fuel_ueu");
        let mut seen = Vec::new();
        // Each stage's ticks, then (when asked) the world opened again from what it holds after them.
        let mut step = |g: &mut Game, i: usize| {
            g.run(4);
            if reopen_at == Some(i) {
                let world = g.world.borrow().clone();
                *g = Game::new(Rc::new(RefCell::new(world)));
            }
            seen.push(layer(g));
        };
        step(&mut g, 0);
        let b = layer(&mut g).members[&[20, 0]].comp;
        assert_eq!(b, [20, 0], "B is its own reactor");
        // The row of blanks x = 4..=15 links A's reflector (cardinal) to B's source's rays (five cells).
        for x in 4..=15 {
            g.place([x, Y, 0], &column("rbmk_blank"));
        }
        step(&mut g, 1);
        step(&mut g, 2);
        g.break_block([10, Y, 0]);
        step(&mut g, 3);
        step(&mut g, 4);
        (g, seen)
    }
    let (mut g, once) = play(None);
    let merged = &once[1];
    assert!(merged.members.values().all(|m| m.comp == A), "merged under the first registered column");
    assert!(!merged.comps.contains_key(&[20, 0]), "B's entry gave way");
    let order = |l: &Layer| {
        let mut o: Vec<_> = l.members.iter().map(|(c, m)| (m.seq, *c)).collect();
        o.sort();
        o.into_iter().map(|(_, c)| c).collect::<Vec<_>>()
    };
    // The merged streams follow their origins' registration order.
    let merged_order = order(merged);
    let pos: Vec<usize> = merged.comps[&A].streams.iter().map(|s| merged_order.iter().position(|c| *c == s.origin).unwrap()).collect();
    assert!(pos.windows(2).all(|w| w[0] <= w[1]), "{pos:?}");
    assert!(merged.comps[&A].streams.iter().any(|s| s.origin == [20, 0]) && merged.comps[&A].streams.iter().any(|s| s.origin == [0, -1]));
    assert!(merged.members[&[0, -1]].seq > merged.members[&[20, 0]].seq, "interleaved registrations");
    let split = &once[3];
    // The split: the west part keeps the merged random, the east one starts from the seed derived from it; each then
    // draws once a tick for its ReaSim source over the stage's four ticks (`nextInt(4)`).
    let parent = &once[2].comps[&A];
    let drawn = |mut r: crate::rbmk::jrandom::JavaRandom| {
        for _ in 0..4 {
            r.next_int(4);
        }
        r
    };
    assert_eq!(split.comps[&A].rng(), drawn(parent.rng()), "the part with the first column keeps the random");
    assert_eq!(split.comps[&[20, 0]].rng(), drawn(CompState::derived(parent, [20, 0]).rng()), "the other part's seed derives from it");
    let east = split.members[&[20, 0]].comp;
    assert_ne!(east, A, "split again");
    assert_eq!(east, [20, 0], "the east part under its first registered column (B's source, before the row)");
    for (k, c) in &split.comps {
        assert!(c.streams.iter().all(|s| split.members[&s.origin].comp == *k), "each stream with its origin");
    }
    assert_eq!(split.members[&[0, 0]].comp, A, "the part with A's first column keeps A");
    // The same in every world, and through a reopening at each stage.
    for at in 0..5 {
        let (mut again, seen) = play(Some(at));
        assert_eq!(seen[4], once[4], "reopened after stage {at}");
        assert_eq!(state(&mut again), state(&mut g));
    }
}

/// P1-1: a column replaced in place by another column (a command), or broken and put back while its reactor waits,
/// leaves its reactor; the new one is registered once adopted, and the reactor steps on.
#[test]
fn a_column_replaced_in_place_leaves_and_the_reactor_steps_on() {
    let mut g = reactor();
    g.run(5);
    let seq = layer(&mut g).members[&[1, 0]].seq;
    g.place([1, Y, 0], &column("rbmk_blank"));
    g.run(3);
    let l = layer(&mut g);
    assert_eq!(l.members[&[1, 0]].block, "ntm:rbmk_blank");
    assert!(l.members[&[1, 0]].seq > seq, "registered anew");
    assert_eq!(g.machine([1, Y, 0]).block, "ntm:rbmk_blank");
    let before = state(&mut g);
    g.run(2);
    assert_ne!(state(&mut g), before, "the reactor steps on");
    // Broken and put back while the reactor waits (a column of it out of reach), then the reach is back.
    g.place([-5, Y, 0], &column("rbmk_blank"));
    g.run(2);
    g.world.borrow_mut().unloaded.insert((OW.into(), -1, 0));
    g.run(2);
    let rods = |g: &Game| g.world.borrow().dropped.iter().filter(|d| d.contains("rbmk_fuel")).count();
    let dropped = rods(&g);
    g.break_block([2, Y, 0]);
    g.place([2, Y, 0], &column("rbmk_rod"));
    g.run(3);
    g.world.borrow_mut().unloaded.clear();
    g.run(4);
    assert_eq!(layer(&mut g).members[&[2, 0]].block, "ntm:rbmk_rod");
    assert_eq!(g.machine([2, Y, 0]).block, "ntm:rbmk_rod");
    assert!(rods(&g) <= dropped + 1, "the old rod comes out once at most");
    let before = state(&mut g);
    g.run(2);
    assert_ne!(state(&mut g), before, "and the reactor steps on");
}

/// P2-1: a rod's doubles go through the stack without loss, the non-finite ones too, every tick.
#[test]
fn a_rods_non_finite_double_survives_the_ticks() {
    let mut g = reactor();
    g.run(2);
    let key = (OW.to_string(), [1, Y, 0]);
    let custom = "{core:20.0d,hull:20.0d,xenon:0.0d,yield:-Infinityd}".to_string();
    let mut s = Stack::new("ntm:rbmk_fuel_ueu", 1);
    s.custom = Some(custom);
    g.world.borrow_mut().engine.get_mut(&key).unwrap().0[0] = Some(s);
    g.run(3);
    let after = slot(&g, [1, Y, 0]).unwrap().custom.unwrap();
    // The burn turns -∞ into NaN (`yield - influx·…`, xenon with it): the non-finite values are kept, never the old finite
    // ones the stack held.
    let back = crate::rbmk::persist::rod_stack(Some(&after));
    assert!(!back.yield_.unwrap().is_finite() && !back.xenon.unwrap().is_finite(), "{after}");
    assert!(after.contains("yield:NaNd") || after.contains("yield:-Infinityd"), "{after}");
}

/// A batch of entity effects (debris, T20) as a step leaves it: owned by the reactor's first column.
fn debris_batch(g: &mut Game, at: Pos, kind: &str) -> CommittedEffects {
    let owner = (OW.to_string(), [0, Y, 0]);
    let m = g.machine(owner.1);
    let spawn = |x: f64| EntitySpawn { kind: kind.into(), pos: [f64::from(at[0]) + x, f64::from(at[1]), f64::from(at[2]) + 0.5], nbt: "{Invisible:1b}".into() };
    CommittedEffects {
        owners: vec![owner.clone()],
        origins: vec![CellGuard::machine(&owner, &m)],
        effects: [WorldEffect::Entities { dim: OW.into(), pos: at, entities: vec![spawn(0.5), spawn(0.25)] }].into(),
        ..Default::default()
    }
}

/// P2-2: entity effects come out once by the protocol of the item effects (the batch in its home's record, the id kept
/// only while owed), through a reopening from a save before or after they came out; a kind the engine cannot summon,
/// or a host without entities, waits without loss.
#[test]
fn entities_of_an_effect_come_out_once_through_the_records() {
    let mut g = reactor();
    g.run(2);
    // A kind the host cannot summon: refused, the batch waits and its owner's reactor with it.
    let bad = debris_batch(&mut g, [1, Y + 4, 0], "ntm:not_an_entity");
    g.ready().world.machine_effects.push_back(bad);
    let held = state(&mut g);
    g.run(3);
    assert_eq!(g.ready().world.machine_effects.len(), 1, "the batch waits");
    assert!(g.world.borrow().entities.is_empty());
    assert_eq!(state(&mut g), held, "its owner's reactor does not step past it");
    g.ready().world.machine_effects.clear();
    g.run(2);
    // A host without entities: the same.
    g.world.borrow_mut().no_entities = true;
    let batch = debris_batch(&mut g, [1, Y + 4, 0], "minecraft:armor_stand");
    g.ready().world.machine_effects.push_back(batch);
    g.run(2);
    assert!(g.world.borrow().entities.is_empty() && g.ready().world.machine_effects.len() == 1);
    // The save before they come out: the batch is in its home's record, no entity anywhere.
    let mut before = g.world.borrow().clone();
    before.no_entities = false;
    assert_eq!(before.homes.values().map(|(_, b)| b.len()).sum::<usize>(), 1);
    g.world.borrow_mut().no_entities = false;
    g.run(1);
    assert_eq!(g.world.borrow().entities.len(), 2);
    assert!(g.ready().world.machine_effects.is_empty());
    let after = g.world.borrow().clone();
    assert_eq!(after.homes.values().map(|(_, b)| b.len()).sum::<usize>(), 0, "the record no longer owes it");
    // Reopened from the save before: the batch is played again, into a world that has none of them: two, once.
    let mut early = Game::new(Rc::new(RefCell::new(before)));
    early.run(4);
    assert_eq!(early.world.borrow().entities.len(), 2);
    // From the save after: nothing more.
    let mut late = Game::new(Rc::new(RefCell::new(after)));
    late.run(4);
    assert_eq!(late.world.borrow().entities.len(), 2);
    // Owed: the cell's chunk not loaded, they wait in the owner's record under the id, and come out once it loads.
    let batch = debris_batch(&mut late, [40, Y, 40], "minecraft:armor_stand");
    late.ready().world.machine_effects.push_back(batch);
    late.world.borrow_mut().unloaded.insert((OW.into(), 2, 2));
    late.run(2);
    assert_eq!(late.world.borrow().entities.len(), 2);
    let owed: usize = late.world.borrow().ledgers.values().map(|l| l.owed.len()).sum();
    assert_eq!(owed, 2, "owed in the owner's record");
    late.world.borrow_mut().unloaded.clear();
    late.run(2);
    assert_eq!(late.world.borrow().entities.len(), 4);
    assert!(late.world.borrow().ledgers.values().all(|l| l.owed.is_empty()));
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
fn a_reopened_reactor_waits_for_a_column_not_adopted_yet_and_for_cells_in_reach() {
    let mut g = reactor();
    let far = [-5, Y, 0];
    g.place(far, &column("rbmk_blank"));
    g.run(5);
    // The world opened again with the far column's chunk not loaded: its block entity is not adopted yet.
    let mut world = g.world.borrow().clone();
    let json = world.saved.remove(&(OW.to_string(), far)).unwrap();
    world.unloaded.insert((OW.into(), -1, 0));
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    let before = state(&mut reopened);
    reopened.run(4);
    assert_eq!(state(&mut reopened), before, "D4: the reactor waits for the column it knows");
    assert!(layer(&mut reopened).members.contains_key(&[-5, 0]));
    // Loaded, its block shown, its block entity not ticked yet (adoption comes with its first tick): still waiting.
    reopened.world.borrow_mut().unloaded.clear();
    reopened.run(2);
    assert_eq!(state(&mut reopened), before, "a column the world shows is not gone before it is adopted");
    assert!(reopened.server.native_adopt(OW, far, &json));
    reopened.run(1);
    assert_ne!(state(&mut reopened), before);
    // The far column gone, a cell the source's streams can read in a chunk not loaded (x = -5..-1 is the chunk -1, with
    // no column there now): the reactor waits too.
    reopened.break_block(far);
    reopened.run(2);
    assert!(!layer(&mut reopened).members.contains_key(&[-5, 0]));
    reopened.world.borrow_mut().unloaded.insert((OW.into(), -1, 0));
    let before = state(&mut reopened);
    reopened.run(3);
    assert_eq!(state(&mut reopened), before, "an unknown cell is not air");
}

#[test]
fn the_storage_keeps_the_layer_and_reads_it_back() {
    let mut g = reactor();
    g.run(10);
    let kept = crate::persist::WorldState::load(&mut g.sh);
    assert_eq!(kept.rbmk.get(&(OW.to_string(), Y)), Some(&layer(&mut g)));
    // A layer of another schema is not read as this one.
    let mut l = layer(&mut g);
    l.schema += 1;
    g.ready().world.rbmk.insert((OW.to_string(), Y), l);
    g.server.write_world(&mut g.sh).unwrap();
    assert!(crate::persist::WorldState::load(&mut g.sh).rbmk.is_empty());
}

#[test]
fn a_storage_that_cannot_be_written_keeps_the_layer_for_the_next_write() {
    let mut g = reactor();
    g.run(5);
    g.world.borrow_mut().kv_fails = true;
    g.run(5);
    let in_memory = layer(&mut g);
    let stored = crate::persist::WorldState::load(&mut g.sh).rbmk.get(&(OW.to_string(), Y)).cloned();
    assert_ne!(stored.as_ref(), Some(&in_memory), "the write failed");
    g.world.borrow_mut().kv_fails = false;
    g.run(1);
    assert_eq!(crate::persist::WorldState::load(&mut g.sh).rbmk.get(&(OW.to_string(), Y)), Some(&layer(&mut g)), "written at the next keep");
}

/// P2-3 (measured in release: `cargo test --release -p ntm-core --lib keep_world_cost -- --ignored --nocapture`): the
/// keep of a tick whose only change is a stepping 15×15 reactor (225 columns, 900 queued streams), with 0, 5 000 and
/// 50 000 cell generations in the world. The layers have their own key, so the cost is the reactor's, not the world's.
#[test]
#[ignore]
fn keep_world_cost() {
    let mut l = Layer::new();
    let mut c = CompState::fresh(OW, Y, A);
    for x in 0..15 {
        for z in 0..15 {
            l.members.insert([x, z], crate::rbmk::persist::Member { seq: (x * 15 + z) as u64, block: "ntm:rbmk_rod_reasim".into(), comp: A });
        }
    }
    let order: Vec<[i32; 2]> = l.members.keys().copied().collect();
    let queue: Vec<_> = (0..900).map(|i| crate::rbmk::reactor::QueuedStream { origin: i % 225, vx: std::f64::consts::FRAC_1_SQRT_2, vz: -std::f64::consts::FRAC_1_SQRT_2, flux_quantity: 1234.5678 + i as f64, flux_ratio: 0.5 }).collect();
    c.set_queue(&order, &queue);
    l.comps.insert(A, c);
    let mut costs = Vec::new();
    for gens in [0usize, 5_000, 50_000] {
        let mut g = game();
        g.ready().world.rbmk.insert((OW.to_string(), Y), l.clone());
        for i in 0..gens {
            g.ready().world.generations.insert((OW.to_string(), [i as i32, 64, (i / 7) as i32]), 1);
        }
        g.server.keep_world(&mut g.sh);
        let mut total = std::time::Duration::ZERO;
        for i in 0..200u64 {
            // A step changed the reactor's streams, nothing else.
            g.ready().world.rbmk.get_mut(&(OW.to_string(), Y)).unwrap().comps.get_mut(&A).unwrap().streams[0].bits[2] = i;
            let t = std::time::Instant::now();
            g.server.keep_world(&mut g.sh);
            total += t.elapsed();
        }
        let world = g.world.borrow().kv.map.get(crate::persist::KEY).map_or(0, Vec::len);
        let rbmk = g.world.borrow().kv.map.get(crate::persist::RBMK_KEY).map_or(0, Vec::len);
        let per = total.as_secs_f64() * 1000.0 / 200.0;
        println!("generations {gens}: world key {world} B, rbmk key {rbmk} B, keep per tick {per:.4} ms");
        costs.push(per);
    }
    assert!(costs.iter().all(|c| *c < 1.0), "{costs:?}");
    assert!(costs[2] < costs[0] * 1.5 + 0.05, "no growth with the world: {costs:?}");
}

/// The test hooks of the in-game proof (`NTM_TEST_HOOKS=1`): a rod leaves its channel once, as an item entity through
/// the entity receipt of its reactor's step, or to the player's cursor, or as a drop, each in one commit.
#[test]
fn the_test_hooks_move_a_rod_once() {
    let mut g = reactor();
    g.run(2);
    // As an item entity, through the step's entity effect.
    assert!(g.server.native_test_eject(OW, [1, Y, 0], [1.5, f64::from(Y) + 3.0, 4.5]));
    assert!(!g.server.native_test_eject(OW, [1, Y, 1], [0.0; 3]), "a column without a rod has none to put out");
    g.run(2);
    assert_eq!(slot(&g, [1, Y, 0]), None);
    let entities = g.world.borrow().entities.clone();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].1.kind, "minecraft:item");
    assert!(entities[0].1.nbt.contains("ntm:rbmk_fuel_ueu") && entities[0].1.nbt.contains("minecraft:custom_data"), "{}", entities[0].1.nbt);
    g.run(3);
    assert_eq!(g.world.borrow().entities.len(), 1, "once");
    // To the cursor, and as a drop.
    hold(&mut g, "minecraft:stone", 1, false);
    assert!(g.server.native_test_take(&mut g.sh, OW, [2, Y, 0], "p1", true).is_ok());
    assert_eq!(g.world.borrow().hands.as_ref().unwrap().carried.as_ref().map(|s| s.stack.id.clone()), Some("ntm:rbmk_fuel_meu".into()));
    assert_eq!(slot(&g, [2, Y, 0]), None);
    assert_eq!(g.server.native_test_take(&mut g.sh, OW, [2, Y, 0], "p1", false), Err("no rod in the channel"), "an empty channel gives nothing");
    assert!(g.server.native_test_take(&mut g.sh, OW, [0, Y, 0], "p1", false).is_ok());
    assert_eq!(slot(&g, [0, Y, 0]), None);
    assert!(g.world.borrow().dropped.iter().any(|d| d.starts_with("player ntm:rbmk_fuel_po210be 1")));
    // A cursor already full: nothing moves.
    let mut g2 = reactor();
    g2.run(2);
    hold(&mut g2, "minecraft:stone", 1, false);
    g2.world.borrow_mut().hands.as_mut().unwrap().carried = Some(HeldStack { stack: Stack::new("minecraft:dirt", 1), encoded: "dirt".into() });
    assert_eq!(g2.server.native_test_take(&mut g2.sh, OW, [1, Y, 0], "p1", true), Err("the cursor is full"));
    assert!(slot(&g2, [1, Y, 0]).is_some());
}

/// D4 for a column only heat links (no stream reads its chunk): unknown, it holds its component; never taken as gone.
#[test]
fn an_unknown_column_that_only_heat_links_holds_its_component() {
    let mut g = game();
    // Two blanks across the border x = 16, linked by heat only (no fuel channel, so no ray reads the chunk 1).
    g.place([15, Y, 0], &column("rbmk_blank"));
    g.place([16, Y, 0], &column("rbmk_blank"));
    g.run(3);
    assert_eq!(layer(&mut g).members[&[16, 0]].comp, [15, 0]);
    // The world opened again with the chunk 1 not loaded: the blank there is not adopted, its chunk unknown.
    let mut world = g.world.borrow().clone();
    world.saved.remove(&(OW.to_string(), [16, Y, 0]));
    world.unloaded.insert((OW.into(), 1, 0));
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    let Data::Rbmk(c) = &mut reopened.ready().world.machines.get_mut(&(OW.to_string(), [15, Y, 0])).unwrap().data else { panic!() };
    c.heat = 500.0;
    reopened.run(4);
    assert!(layer(&mut reopened).members.contains_key(&[16, 0]), "kept: unknown is not gone");
    assert_eq!(rbmk(&mut reopened, [15, Y, 0]).heat, 500.0, "the component waits for it");
}

/// The cells a step reads for occlusion are every cell a stream can read: with obstacles anywhere around, a reactor
/// that knows only those in `Reactor::ray_cells` steps exactly as one that knows them all, bit for bit.
#[test]
fn the_rays_cells_are_all_the_cells_a_stream_reads() {
    use crate::rbmk::{dials::DialState, jrandom::JavaRandom, persist::rod_of, reactor::{Column, Kind as K, Reactor}, rods::RodStack};
    let mut d = DialState::default();
    d.create(false);
    d.refresh();
    let dials = *d.cached().unwrap();
    let range = dials.flux_range;
    let build = |obstacles: &dyn Fn(i32, i32, i32) -> bool, keep: &dyn Fn(i32, i32) -> bool| {
        let mut r = Reactor::new(dials, JavaRandom::new(7), Y);
        let cols = [((0, 0), K::RodReaSim, "ntm:rbmk_fuel_po210be"), ((3, 1), K::Rod, "ntm:rbmk_fuel_ueu"), ((-2, 4), K::RodReaSim, "ntm:rbmk_fuel_meu"), ((1, -3), K::Rod, "ntm:rbmk_fuel_ueu")];
        for ((x, z), k, rod) in cols {
            let mut c = Column::new(k, x, z);
            let t = rod_of(rod).unwrap();
            c.rod_mut().unwrap().fuel = Some((t, RodStack::fresh(t)));
            r.add(c);
        }
        for x in -12..=12 {
            for z in -12..=12 {
                for h in 0..dials.column_height + 1 {
                    if r.at(x, z).is_none() && obstacles(x, z, h) && keep(x, z) {
                        r.obstacles.insert((x, z, h));
                    }
                }
            }
        }
        r
    };
    let noise = |x: i32, z: i32, h: i32| crate::ledger::fnv(&[&x.to_le_bytes(), &z.to_le_bytes(), &h.to_le_bytes()]).is_multiple_of(3);
    let mut cells = BTreeSet::new();
    for ((x, z), k) in [((0, 0), K::RodReaSim), ((3, 1), K::Rod), ((-2, 4), K::RodReaSim), ((1, -3), K::Rod)] {
        cells.extend(Reactor::ray_cells(k, x, z, range));
    }
    let mut all = build(&noise, &|_, _| true);
    let mut rays = build(&noise, &|x, z| cells.contains(&(x, z)));
    assert!(rays.obstacles.len() < all.obstacles.len(), "the rays leave cells out");
    for _ in 0..60 {
        all.step();
        rays.step();
        assert_eq!(all.queued(), rays.queued());
        assert_eq!(all.rad, rays.rad);
        for i in 0..4 {
            assert_eq!(all.columns[i].heat.to_bits(), rays.columns[i].heat.to_bits());
        }
    }
}

/// The step reads the occlusion of every cell its streams read: a stone between a source and a fuel channel (at a height
/// the column partly covers) changes what the channel receives.
#[test]
fn a_block_in_a_streams_path_occludes_it() {
    let play = |stone: bool| {
        let mut g = game();
        g.place([0, Y, 0], &column("rbmk_rod_reasim"));
        g.place([0, Y, 3], &column("rbmk_rod"));
        g.run(2);
        fuel(&mut g, [0, Y, 0], "ntm:rbmk_fuel_po210be");
        fuel(&mut g, [0, Y, 3], "ntm:rbmk_fuel_ueu");
        if stone {
            g.world.borrow_mut().set(OW, [0, Y + 1, 2], "minecraft:stone");
        }
        g.run(40);
        (rbmk(&mut g, [0, Y, 3]), slot(&g, [0, Y, 3]))
    };
    let (open, occluded) = (play(false), play(true));
    assert!(open.0.has_rod && crate::rbmk::persist::rod_stack(open.1.as_ref().unwrap().custom.as_deref()).yield_ < crate::rbmk::persist::rod_of("ntm:rbmk_fuel_ueu").map(|r| r.yield_), "the channel burned");
    assert_ne!(open.1, occluded.1, "the stone took part of the source's streams: the rod burned less");
}

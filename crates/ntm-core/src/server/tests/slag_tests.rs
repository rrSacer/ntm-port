//! The slag tap (`TileEntityFoundrySlagtap`) and the slag it pours (`BlockDynamicSlag`): a pour is one commit of the
//! source's loss, the slag block and its record, so the metal is never in both and never in neither, a refusal moves
//! nothing, and a broken slag gives back every quantum.
use super::*;
use crate::foundry::flow::Melt;
use crate::server::container_bridge::Slotted;
use crate::foundry::slag::{SLAG_CAP, Slag};

const SRC: Pos = [0, 70, 3];
/// Facing south: it takes metal from the north, where the channel is.
const TAP: Pos = [0, 70, 4];
const GROUND: Pos = [0, 60, 4];
const FIRST: Pos = [0, 61, 4];
const SECOND: Pos = [0, 62, 4];

fn put(g: &mut Game, at: Pos, state: &str) {
    g.place(at, state);
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (Vec::new(), Vec::new()));
    g.run(2);
}

fn edit(g: &mut Game, at: Pos, f: impl FnOnce(&mut Data)) {
    f(&mut g.ready().world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

/// A channel with `q` quanta of `mat` behind a slag tap over a stone floor.
fn pour_setup(mat: &str, q: u32) -> Game {
    let mut g = game();
    g.place(GROUND, "minecraft:stone");
    put(&mut g, SRC, "ntm:foundry_channel[facing=north,honey_level=0]");
    put(&mut g, TAP, "ntm:foundry_slagtap[facing=south,honey_level=0]");
    edit(&mut g, SRC, |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = Melt { mat: Some(mat.into()), amount: q };
    });
    g
}

/// The slag block at `at`, with `q` quanta of `mat`, in the world and in the model.
fn put_slag(g: &mut Game, at: Pos, mat: &str, q: u32) {
    put(g, at, "ntm:slag[layer=8]");
    edit(g, at, |d| {
        let Data::Slag(s) = d else { panic!() };
        *s = Slag::of(mat, q);
    });
}

fn slag_at(g: &mut Game, at: Pos) -> Option<Slag> {
    match g.ready().world.machines.get(&(OW.to_string(), at)).map(|m| m.data.clone()) {
        Some(Data::Slag(s)) => Some(s),
        _ => None,
    }
}

fn channel(g: &mut Game) -> Melt {
    let Data::Channel(c) = g.machine(SRC).data else { panic!() };
    c.melt
}

/// Every quantum the models hold (what a break would give back as scraps).
fn held(g: &mut Game) -> u32 {
    g.ready().world.machines.values().flat_map(|m| m.logic().scraps()).map(|(_, q)| q).sum()
}

/// Every quantum the block entities' records hold, as a world saved now would keep them.
fn recorded(g: &Game) -> u32 {
    g.world.borrow().saved.values().filter_map(|j| serde_json::from_str::<Machine>(j).ok()).flat_map(|m| m.logic().scraps()).map(|(_, q)| q).sum()
}

#[test]
fn a_channel_pours_through_a_slag_tap_into_a_new_slag_block_and_no_tick_has_a_quantum_more_or_less() {
    let mut g = pour_setup("iron", 100);
    g.run(1);
    let mut poured = false;
    for tick in 0..30 {
        g.run(1);
        assert_eq!(held(&mut g), 100, "tick {tick}: the models");
        if channel(&mut g).amount == 0 {
            poured = true;
            // The block, its record and the source's smaller record are there together.
            assert_eq!(g.world.borrow().get(OW, FIRST), "ntm:slag[layer=1]");
            assert_eq!(recorded(&g), 100, "tick {tick}: the records after the pour");
            let saved: Machine = serde_json::from_str(&g.world.borrow().saved[&(OW.to_string(), FIRST)]).unwrap();
            assert_eq!(saved.logic().scraps(), vec![("iron".to_string(), 100)]);
            let source: Machine = serde_json::from_str(&g.world.borrow().saved[&(OW.to_string(), SRC)]).unwrap();
            assert!(source.logic().scraps().is_empty(), "the source's record has let it go");
            break;
        }
        assert_eq!(g.world.borrow().get(OW, FIRST), "minecraft:air", "tick {tick}: nothing poured, no block");
        assert!(recorded(&g) == 0 || recorded(&g) == 100, "tick {tick}: the records");
    }
    assert!(poured, "it poured");
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt, Melt { mat: Some("iron".into()), amount: 100 });
    // Afterwards the same: nothing more happens, nothing is lost.
    g.run(40);
    assert_eq!((held(&mut g), recorded(&g)), (100, 100));
}

#[test]
fn a_refused_commit_pours_nothing_and_the_source_keeps_its_metal() {
    let mut g = pour_setup("iron", 100);
    g.world.borrow_mut().refuse_commits = true;
    g.run(40);
    assert_eq!(channel(&mut g).amount, 100);
    assert_eq!(g.world.borrow().get(OW, FIRST), "minecraft:air");
    assert!(slag_at(&mut g, FIRST).is_none());
    assert!(!g.server.ready.world.machines.contains_key(&(OW.to_string(), FIRST)));
    g.world.borrow_mut().refuse_commits = false;
    g.run(40);
    assert_eq!((channel(&mut g).amount, held(&mut g)), (0, 100));
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 100);
}

#[test]
fn what_the_block_hit_does_not_take_goes_into_a_new_block_above_it() {
    // The same material, nearly full: it takes what it can, the rest goes above (`transfer`, then `above`).
    let mut g = pour_setup("iron", 100);
    put_slag(&mut g, FIRST, "iron", SLAG_CAP - 30);
    g.run(40);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, SLAG_CAP);
    assert_eq!(slag_at(&mut g, SECOND).unwrap().melt, Melt { mat: Some("iron".into()), amount: 70 });
    assert_eq!(g.world.borrow().get(OW, SECOND), "ntm:slag[layer=1]");
    assert_eq!((channel(&mut g).amount, held(&mut g), recorded(&g)), (0, SLAG_CAP + 70, SLAG_CAP + 70));
}

/// What the slag hit cannot take and no cell above it can hold stays in the source: the pour is partial, nothing is lost.
#[test]
fn what_no_block_can_take_stays_in_the_source() {
    let mut g = pour_setup("iron", 144);
    // The slag right under the tap (y 69), nearly full: the cell above it is the tap itself.
    put_slag(&mut g, [0, 69, 4], "iron", SLAG_CAP - 68);
    g.run(40);
    assert_eq!(slag_at(&mut g, [0, 69, 4]).unwrap().melt.amount, SLAG_CAP);
    assert_eq!(channel(&mut g).amount, 76, "the rest stays in the channel");
    assert_eq!((held(&mut g), recorded(&g)), (SLAG_CAP + 76, SLAG_CAP + 76));
}

#[test]
fn another_material_below_takes_nothing_and_the_pour_sits_on_top_of_it() {
    let mut g = pour_setup("iron", 100);
    put_slag(&mut g, FIRST, "copper", 5_000);
    g.run(40);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt, Melt { mat: Some("copper".into()), amount: 5_000 });
    assert_eq!(slag_at(&mut g, SECOND).unwrap().melt, Melt { mat: Some("iron".into()), amount: 100 });
}

#[test]
fn the_slag_tap_pours_only_from_behind_what_its_filter_passes_and_with_something_under_it() {
    // Filtered out: the channel keeps its metal.
    let mut g = pour_setup("iron", 100);
    edit(&mut g, TAP, |d| {
        let Data::Slagtap(o) = d else { panic!() };
        o.filter = Some("steel".into());
    });
    g.run(40);
    assert_eq!((channel(&mut g).amount, held(&mut g)), (100, 100));
    assert!(slag_at(&mut g, FIRST).is_none());
    // Inverted: iron passes.
    edit(&mut g, TAP, |d| {
        let Data::Slagtap(o) = d else { panic!() };
        o.invert = true;
    });
    g.run(40);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 100);
    // A channel at its side is not behind it.
    let mut g = pour_setup("iron", 0);
    let side = [1, 70, 4];
    put(&mut g, side, "ntm:foundry_channel[facing=north,honey_level=0]");
    edit(&mut g, side, |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = Melt { mat: Some("iron".into()), amount: 100 };
    });
    g.run(40);
    assert!(slag_at(&mut g, FIRST).is_none(), "from the side: not taken");
    // The ray reaches 15 blocks down (y 69 to 55): a floor at 54 is out of reach, one at 55 is not.
    let mut g = game();
    g.place([0, 54, 4], "minecraft:stone");
    put(&mut g, SRC, "ntm:foundry_channel[facing=north,honey_level=0]");
    put(&mut g, TAP, "ntm:foundry_slagtap[facing=south,honey_level=0]");
    edit(&mut g, SRC, |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = Melt { mat: Some("iron".into()), amount: 100 };
    });
    g.run(40);
    assert_eq!(channel(&mut g).amount, 100, "nothing within 15 blocks: nothing poured");
    g.place([0, 55, 4], "minecraft:stone");
    g.run(40);
    assert_eq!(slag_at(&mut g, [0, 56, 4]).unwrap().melt.amount, 100);
}

/// The pour stays in the source's chunk: the records of two chunks reach the disk apart. A tap at a chunk border whose
/// slag would be in the next chunk pours nothing, and the channel keeps its metal; one whose pour is all in one chunk
/// pours as anywhere.
#[test]
fn a_pour_that_would_leave_the_sources_chunk_pours_nothing() {
    // The channel at z 15, the tap and the floor at z 16: the slag would be in the next chunk.
    let mut g = game();
    g.place([0, 60, 16], "minecraft:stone");
    put(&mut g, [0, 70, 15], "ntm:foundry_channel[facing=north,honey_level=0]");
    put(&mut g, [0, 70, 16], "ntm:foundry_slagtap[facing=south,honey_level=0]");
    edit(&mut g, [0, 70, 15], |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = Melt { mat: Some("iron".into()), amount: 100 };
    });
    g.run(60);
    let Data::Channel(c) = g.machine([0, 70, 15]).data else { panic!() };
    assert_eq!(c.melt.amount, 100, "the metal stays");
    assert_eq!(g.world.borrow().get(OW, [0, 61, 16]), "minecraft:air");
    assert_eq!((held(&mut g), recorded(&g)), (100, 100), "nothing was lost or made");
    // The same tap with the channel at z 14 and the tap at z 15: the pour is all in chunk 0.
    let mut g = game();
    g.place([0, 60, 15], "minecraft:stone");
    put(&mut g, [0, 70, 14], "ntm:foundry_channel[facing=north,honey_level=0]");
    put(&mut g, [0, 70, 15], "ntm:foundry_slagtap[facing=south,honey_level=0]");
    edit(&mut g, [0, 70, 14], |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = Melt { mat: Some("iron".into()), amount: 100 };
    });
    g.run(60);
    assert_eq!(slag_at(&mut g, [0, 61, 15]).unwrap().melt.amount, 100);
}

/// D4: the slag broken gives every quantum as one exact scrap.
#[test]
fn a_broken_slag_drops_its_exact_scrap() {
    let mut g = pour_setup("iron", 123);
    g.run(40);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 123);
    g.world.borrow_mut().dropped.clear();
    g.world.borrow_mut().set(OW, FIRST, "minecraft:air");
    g.break_block(FIRST);
    g.run(60);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert!(dropped[0].ends_with("ntm:scraps__iron 1 {ntm_amount:123}"), "{dropped:?}");
    assert!(slag_at(&mut g, FIRST).is_none());
}

/// A world reopened with the slag's block entity record: what it held is there, and breaking it gives it back.
#[test]
fn a_slag_reopened_from_its_record_keeps_what_it_holds() {
    let mut g = pour_setup("gold", 1_234);
    g.run(40);
    let world = Rc::new(RefCell::new(g.world.borrow().clone()));
    let mut reopened = Game::new(world);
    reopened.run(8);
    assert_eq!(slag_at(&mut reopened, FIRST).unwrap().melt, Melt { mat: Some("gold".into()), amount: 1_234 });
    assert_eq!(held(&mut reopened), 1_234);
    reopened.world.borrow_mut().dropped.clear();
    reopened.world.borrow_mut().set(OW, FIRST, "minecraft:air");
    reopened.break_block(FIRST);
    reopened.run(60);
    let dropped = reopened.world.borrow().dropped.clone();
    assert!(dropped.iter().any(|d| d.ends_with("ntm:scraps__gold 1 {ntm_amount:1234}")), "{dropped:?}");
}

/// A slag restored from the journal (its record in the storage newer than the chunk's, which has none) keeps all it
/// holds: it has no slots, so nothing of it is the block entity's alone (`slots_recorded`).
#[test]
fn a_slag_restored_from_the_journal_keeps_what_it_holds() {
    let mut g = pour_setup("steel", 777);
    g.run(40);
    let key = (OW.to_string(), FIRST);
    // The chunk's record is an older version of the slag (a smaller pool); the journal has the newer one.
    let mut older = g.server.ready.world.machines[&key].clone();
    let Data::Slag(s) = &mut older.data else { panic!() };
    *s = Slag::of("steel", 100);
    let mut newer = g.server.ready.world.machines[&key].clone();
    newer.rev = older.rev + 1;
    assert!(Slotted::slots_recorded(&Slag::of("steel", 1)));
    let json = serde_json::to_string(&newer).unwrap();
    g.server.ready.world.survivors.insert(key.clone(), Survivor { dim: OW.into(), pos: FIRST, generation: newer.generation, rev: newer.rev, json });
    let earlier = g.server.ready.world.kept_as_an_earlier_version(0);
    let world = Rc::new(RefCell::new(g.world.borrow().clone()));
    world.borrow_mut().kv.set(crate::persist::KEY, &earlier).unwrap();
    world.borrow_mut().saved.insert(key.clone(), serde_json::to_string(&older).unwrap());
    let mut reopened = Game::new(world);
    assert!(reopened.server.ready.restored.contains(&key), "restored from the journal");
    reopened.run(8);
    assert_eq!(slag_at(&mut reopened, FIRST).unwrap().melt, Melt { mat: Some("steel".into()), amount: 777 });
}

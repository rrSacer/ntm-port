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

/// The slag block at `at`, with `q` quanta of `mat`, in the world and in the model, not scheduled to flow.
fn put_slag(g: &mut Game, at: Pos, mat: &str, q: u32) {
    g.place(at, "ntm:slag[layer=8]");
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (Vec::new(), Vec::new()));
    g.run(2);
    edit(g, at, |d| {
        let Data::Slag(s) = d else { panic!() };
        *s = Slag::of(mat, q);
    });
}

/// Stone at the four sides of `at`: nothing of a slag there can spread.
fn walls(g: &mut Game, at: Pos) {
    for (dx, dz) in [(0, -1), (0, 1), (1, 0), (-1, 0)] {
        g.place([at[0] + dx, at[1], at[2] + dz], "minecraft:stone");
    }
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
    for tick in 0..40 {
        g.run(1);
        assert_eq!(channel(&mut g).amount, 100, "tick {tick}: the source keeps its metal");
        assert!(!g.server.ready.world.machines.contains_key(&(OW.to_string(), FIRST)), "tick {tick}: no slag in memory");
        let key = (OW.to_string(), SRC);
        assert_eq!(g.server.ready.native_saved.get(&key), g.world.borrow().saved.get(&key), "tick {tick}: the server remembers what the host holds");
    }
    assert_eq!(channel(&mut g).amount, 100);
    assert_eq!(g.world.borrow().get(OW, FIRST), "minecraft:air");
    assert!(slag_at(&mut g, FIRST).is_none());
    assert!(!g.server.ready.world.machines.contains_key(&(OW.to_string(), FIRST)));
    assert!(g.world.borrow().dropped.is_empty(), "nothing is dropped for a pour that did not happen: {:?}", g.world.borrow().dropped);
    assert_eq!(held(&mut g), 100);
    g.world.borrow_mut().refuse_commits = false;
    g.run(40);
    assert_eq!((channel(&mut g).amount, held(&mut g)), (0, 100));
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 100);
}

/// A refusal that reaches only the pour (the cell above taken by a player in the same tick, a pending change) and not the
/// server's own single-machine commits: the source keeps 100 in memory and in its record, and no slag is born. Were the
/// reduced source kept in memory, the next ordinary commit would publish it alone and the metal would be gone.
#[test]
fn a_pour_refused_alone_leaves_the_source_whole_in_memory_and_in_its_record() {
    let mut g = pour_setup("iron", 100);
    g.world.borrow_mut().refuse_structured = true;
    let key = (OW.to_string(), SRC);
    for tick in 0..60 {
        g.run(1);
        assert_eq!(channel(&mut g).amount, 100, "tick {tick}");
        assert_eq!(recorded(&g), 100, "tick {tick}: the record");
        assert_eq!(g.server.ready.native_saved.get(&key), g.world.borrow().saved.get(&key), "tick {tick}");
        assert!(!g.server.ready.world.machines.contains_key(&(OW.to_string(), FIRST)), "tick {tick}");
    }
    assert!(g.world.borrow().dropped.is_empty());
    g.world.borrow_mut().refuse_structured = false;
    g.run(10);
    assert_eq!((channel(&mut g).amount, held(&mut g), recorded(&g)), (0, 100, 100));
}

/// The ray passes every kind of air: `cave_air` between the tap and the floor is not a hit.
#[test]
fn the_ray_passes_cave_air() {
    let mut g = pour_setup("iron", 100);
    for y in 62..70 {
        g.place([0, y, 4], "minecraft:cave_air");
    }
    g.run(40);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 100, "on the floor, not on the cave air");
}

#[test]
fn what_the_block_hit_does_not_take_goes_into_a_new_block_above_it() {
    // The same material, nearly full: it takes what it can, the rest goes above (`transfer`, then `above`).
    let mut g = pour_setup("iron", 100);
    walls(&mut g, FIRST);
    walls(&mut g, SECOND);
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
    // The slag right under the tap (y 69), nearly full: the cell above it is the tap itself; stone under it and around it.
    g.place([0, 68, 4], "minecraft:stone");
    walls(&mut g, [0, 69, 4]);
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
    // The slag it would fill is in the next chunk too: it takes nothing, the new block above it neither.
    g.place([0, 61, 16], "ntm:slag[layer=8]");
    g.world.borrow_mut().engine.insert((OW.to_string(), [0, 61, 16]), (Vec::new(), Vec::new()));
    g.run(2);
    edit(&mut g, [0, 61, 16], |d| {
        let Data::Slag(s) = d else { panic!() };
        *s = Slag::of("iron", 1_000);
    });
    g.run(60);
    assert_eq!(slag_at(&mut g, [0, 61, 16]).unwrap().melt.amount, 1_000);
    assert_eq!(g.world.borrow().get(OW, [0, 62, 16]), "minecraft:air");
    assert_eq!(held(&mut g), 1_100);
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

/// A tap that does not take the metal (its filter, or the channel at its side) leaves it to the channel's next neighbour,
/// as `canAcceptPartialFlow` answering false does; one that takes it ends the channel's turn. The neighbours are tried in
/// an order that depends on the position, so the rig is put at eight places: at some the tap comes first.
#[test]
fn a_tap_that_refuses_the_metal_leaves_it_to_the_next_neighbour() {
    for dx in 0..8 {
        let at = |p: Pos| [p[0] + dx, p[1], p[2]];
        let (src, tank) = (at(SRC), at([0, 70, 2]));
        let rig = |tap: Pos, filter: Option<&str>| {
            let mut g = game();
            g.place(at(GROUND), "minecraft:stone");
            g.place([tap[0], 60, tap[2]], "minecraft:stone");
            put(&mut g, src, "ntm:foundry_channel[facing=north,honey_level=0]");
            put(&mut g, tap, "ntm:foundry_slagtap[facing=south,honey_level=0]");
            put(&mut g, tank, "ntm:foundry_tank[facing=north,honey_level=0]");
            edit(&mut g, src, |d| {
                let Data::Channel(c) = d else { panic!() };
                c.melt = Melt { mat: Some("iron".into()), amount: 100 };
            });
            edit(&mut g, tap, |d| {
                let Data::Slagtap(o) = d else { panic!() };
                o.filter = filter.map(Into::into);
            });
            // The channel's first turn comes five ticks after it is filled: a tap that took the metal would have ended it, to try
            // again five ticks later.
            g.run(6);
            g
        };
        let amounts = |g: &mut Game| match (g.machine(tank).data, g.machine(src).data) {
            (Data::FoundryTank(t), Data::Channel(c)) => (t.melt.amount, c.melt.amount),
            other => panic!("{other:?}"),
        };
        // Filtered out: the tank takes it all.
        let mut g = rig(at(TAP), Some("steel"));
        assert_eq!(amounts(&mut g), (100, 0), "dx {dx}: filtered out");
        assert!(slag_at(&mut g, at(FIRST)).is_none());
        // The channel at the side of the tap: not behind it, the tank takes it all.
        let side = at([1, 70, 3]);
        let mut g = rig(side, None);
        assert_eq!(amounts(&mut g), (100, 0), "dx {dx}: from the side");
    }
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
    // The general rule (`slots_recorded` false for a record that keeps no slots): the slag has none, its metal is all in its record.
    assert!(!Slotted::slots_recorded(&Slag::of("steel", 1)));
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

fn signal(g: &mut Game, at: Pos, level: u8) {
    g.world.borrow_mut().signal.insert((OW.to_string(), at), level);
}

fn set_invert_redstone(g: &mut Game, at: Pos, on: bool) {
    edit(g, at, |d| match d {
        Data::Slagtap(o) | Data::Outlet(o) => o.invert_redstone = on,
        other => panic!("{other:?}"),
    });
}

/// `isClosed`: a signal closes the tap, `invertRedstone` opens it only with one. Closed, the channel keeps its metal; the
/// reading's absence is closed too, and when it is back the tap opens on the next turn with the metal that stayed.
#[test]
fn redstone_closes_the_slag_tap_and_a_missing_reading_does_not_close_it_for_good() {
    let mut g = pour_setup("iron", 100);
    signal(&mut g, TAP, 15);
    g.run(40);
    assert_eq!((channel(&mut g).amount, held(&mut g)), (100, 100), "powered: closed");
    assert!(slag_at(&mut g, FIRST).is_none());
    signal(&mut g, TAP, 0);
    g.run(40);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 100, "unpowered: open");
    // No reading at all: closed, nothing lost; the reading back, it opens.
    let mut g = pour_setup("iron", 100);
    g.world.borrow_mut().no_redstone = true;
    g.run(40);
    assert_eq!((channel(&mut g).amount, held(&mut g)), (100, 100), "no reading: closed");
    g.world.borrow_mut().no_redstone = false;
    g.run(10);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 100, "the reading is back: open");
    assert_eq!((channel(&mut g).amount, held(&mut g), recorded(&g)), (0, 100, 100));
    // Inverted: closed without a signal, open with one.
    let mut g = pour_setup("iron", 100);
    set_invert_redstone(&mut g, TAP, true);
    g.run(40);
    assert_eq!(channel(&mut g).amount, 100, "inverted, no signal: closed");
    signal(&mut g, TAP, 7);
    g.run(40);
    assert_eq!(slag_at(&mut g, FIRST).unwrap().melt.amount, 100, "inverted, powered: open");
}

/// The same for the outlet, which pours into what is under it (a tank).
#[test]
fn redstone_closes_the_outlet_too() {
    let (tank, outlet) = ([0, 68, 4], TAP);
    for (invert, power, flows) in [(false, 0, true), (false, 15, false), (true, 0, false), (true, 15, true)] {
        let mut g = game();
        put(&mut g, SRC, "ntm:foundry_channel[facing=north,honey_level=0]");
        put(&mut g, outlet, "ntm:foundry_outlet[facing=south,honey_level=0]");
        put(&mut g, tank, "ntm:foundry_tank[facing=north,honey_level=0]");
        set_invert_redstone(&mut g, outlet, invert);
        signal(&mut g, outlet, power);
        edit(&mut g, SRC, |d| {
            let Data::Channel(c) = d else { panic!() };
            c.melt = Melt { mat: Some("iron".into()), amount: 100 };
        });
        g.run(60);
        let Data::FoundryTank(t) = g.machine(tank).data else { panic!() };
        assert_eq!(t.melt.amount, if flows { 100 } else { 0 }, "invert {invert}, signal {power}");
        assert_eq!(held(&mut g), 100);
    }
    // The block entity keeps both inversions in one value, and reads the old one (0 or 1) as before.
    let mut o = crate::foundry::flow::Outlet { invert: true, invert_redstone: true, ..Default::default() };
    let v = Slotted::save_state(&o);
    let mut back = crate::foundry::flow::Outlet::default();
    Slotted::load_state(&mut back, &v);
    assert_eq!(back, o);
    Slotted::load_state(&mut o, &[0, 1]);
    assert!(o.invert && !o.invert_redstone);
}

// ---------------------------------------------------------------------------- the slag's own flow (`updateTick`)

const FLOOR: Pos = [0, 60, 4];

/// A stone floor under `at`'s column at y 60, a 3x3 around the column.
fn floor_under(g: &mut Game, at: Pos) {
    for dx in -2..=2 {
        for dz in -2..=2 {
            g.place([at[0] + dx, 60, at[2] + dz], "minecraft:stone");
        }
    }
}

fn schedule(g: &mut Game, at: Pos) {
    g.ready().slag_active.insert((OW.to_string(), at));
}

fn slag_cells(g: &mut Game) -> Vec<(Pos, u32)> {
    g.ready().world.machines.iter().filter_map(|(k, m)| if let Data::Slag(s) = &m.data { Some((k.1, s.melt.amount)) } else { None }).collect()
}

/// The sum of the models and of the records, after every tick, until `ticks` have passed.
fn conserved_for(g: &mut Game, total: u32, ticks: usize) {
    for tick in 0..ticks {
        g.run(1);
        assert_eq!(held(g), total, "tick {tick}: the models");
        assert_eq!(recorded(g), total, "tick {tick}: the records");
    }
}

/// `updateTick`, down: a slag over air moves whole into the air cell (no drop), one block, and is not scheduled again.
#[test]
fn a_slag_over_air_falls_whole_into_the_cell_below_and_the_one_above_is_gone() {
    let mut g = game();
    floor_under(&mut g, FLOOR);
    put_slag(&mut g, [0, 65, 4], "iron", 1_000);
    schedule(&mut g, [0, 65, 4]);
    conserved_for(&mut g, 1_000, 20);
    assert_eq!(slag_cells(&mut g), vec![([0, 64, 4], 1_000)]);
    assert_eq!(g.world.borrow().get(OW, [0, 65, 4]), "minecraft:air");
    assert!(g.world.borrow().get(OW, [0, 64, 4]).starts_with("ntm:slag"));
    assert!(g.world.borrow().dropped.is_empty(), "a flow is no break: nothing is dropped");
}

/// Down onto the slag of the same material, up to its capacity; the one above keeps the rest and, when emptied, is
/// removed on the next tick (the metal is never in both and never in neither, in the models or the records).
#[test]
fn a_slag_gives_the_slag_below_what_it_can_take_and_an_emptied_one_goes_next_tick() {
    let mut g = game();
    floor_under(&mut g, FLOOR);
    walls(&mut g, [0, 61, 4]);
    walls(&mut g, [0, 62, 4]);
    put_slag(&mut g, [0, 61, 4], "iron", 2_000);
    put_slag(&mut g, [0, 62, 4], "iron", 1_000);
    schedule(&mut g, [0, 62, 4]);
    g.run(1);
    // Step one: the upper is empty and still there, the lower has it all.
    assert_eq!(slag_cells(&mut g), vec![([0, 61, 4], 3_000), ([0, 62, 4], 0)]);
    assert_eq!((held(&mut g), recorded(&g)), (3_000, 3_000));
    g.run(1);
    // Step two: the empty one is gone, with no scrap of 0.
    assert_eq!(slag_cells(&mut g), vec![([0, 61, 4], 3_000)]);
    assert_eq!(g.world.borrow().get(OW, [0, 62, 4]), "minecraft:air");
    assert!(g.world.borrow().dropped.is_empty(), "{:?}", g.world.borrow().dropped);
    // Over a nearly full one: it takes what fits and the upper keeps the rest.
    let mut g = game();
    floor_under(&mut g, FLOOR);
    walls(&mut g, [0, 61, 4]);
    walls(&mut g, [0, 62, 4]);
    put_slag(&mut g, [0, 61, 4], "iron", SLAG_CAP - 100);
    put_slag(&mut g, [0, 62, 4], "iron", 1_000);
    schedule(&mut g, [0, 62, 4]);
    conserved_for(&mut g, SLAG_CAP + 900, 6);
    assert_eq!(slag_cells(&mut g), vec![([0, 61, 4], SLAG_CAP), ([0, 62, 4], 900)]);
    // Another material below takes nothing.
    let mut g = game();
    floor_under(&mut g, FLOOR);
    walls(&mut g, [0, 61, 4]);
    walls(&mut g, [0, 62, 4]);
    put_slag(&mut g, [0, 61, 4], "copper", 1_000);
    put_slag(&mut g, [0, 62, 4], "iron", 1_000);
    schedule(&mut g, [0, 62, 4]);
    conserved_for(&mut g, 2_000, 6);
    assert_eq!(slag_cells(&mut g), vec![([0, 61, 4], 1_000), ([0, 62, 4], 1_000)]);
}

/// Sideways: from a fifth of the capacity (2073), `toSpread = max(amount / (count * 2), 1)` to each free side, north, south,
/// east, west; the sum is exact at every tick. A slag below the threshold stays. (At x 5: x -1 is another chunk.)
#[test]
fn a_slag_of_a_fifth_of_the_capacity_spreads_to_its_free_sides_and_the_sum_is_exact() {
    const P: Pos = [5, 61, 4];
    let mut g = game();
    floor_under(&mut g, [5, 60, 4]);
    put_slag(&mut g, P, "iron", 10_000);
    schedule(&mut g, P);
    conserved_for(&mut g, 10_000, 1);
    let cells = slag_cells(&mut g);
    // 10000 / (4 * 2) = 1250 to each of four sides; 5000 stays.
    assert_eq!(cells.len(), 5, "{cells:?}");
    assert_eq!(cells.iter().find(|c| c.0 == P).unwrap().1, 5_000);
    assert!(cells.iter().filter(|c| c.0 != P).all(|c| c.1 == 1_250), "{cells:?}");
    // The new ones are scheduled: 1250 is under the threshold, they stay. Everything holds on for ever.
    conserved_for(&mut g, 10_000, 30);
    assert_eq!(slag_cells(&mut g).len(), 5);
    // Two free sides only (walls on two): 4000 / (2 * 2) = 1000 each.
    let mut g = game();
    floor_under(&mut g, [5, 60, 4]);
    g.place([6, 61, 4], "minecraft:stone");
    g.place([4, 61, 4], "minecraft:stone");
    put_slag(&mut g, P, "iron", 4_000);
    schedule(&mut g, P);
    conserved_for(&mut g, 4_000, 5);
    let mut cells = slag_cells(&mut g);
    cells.sort();
    assert_eq!(cells, vec![([5, 61, 3], 1_000), (P, 2_000), ([5, 61, 5], 1_000)]);
    // The threshold: 2072 stays, 2073 spreads (2073 / 8 = 259 to each side).
    for (amount, parts) in [(2_072, 1), (2_073, 5)] {
        let mut g = game();
        floor_under(&mut g, [5, 60, 4]);
        put_slag(&mut g, P, "iron", amount);
        schedule(&mut g, P);
        conserved_for(&mut g, amount, 3);
        assert_eq!(slag_cells(&mut g).len(), parts, "{amount}");
    }
    // A full block spreads and what it spreads is scheduled in turn: the sum holds on.
    let mut g = game();
    floor_under(&mut g, [5, 60, 4]);
    put_slag(&mut g, P, "iron", SLAG_CAP);
    schedule(&mut g, P);
    conserved_for(&mut g, SLAG_CAP, 40);
}

/// Nothing crosses the border of the chunk: the east side of x 15 is chunk 1.
#[test]
fn a_slag_at_the_edge_of_a_chunk_does_not_spread_into_the_next() {
    let mut g = game();
    for dx in -2..=2 {
        for dz in -2..=2 {
            g.place([15 + dx, 60, 4 + dz], "minecraft:stone");
        }
    }
    put_slag(&mut g, [15, 61, 4], "iron", 6_000);
    schedule(&mut g, [15, 61, 4]);
    conserved_for(&mut g, 6_000, 10);
    let cells = slag_cells(&mut g);
    // Three sides: 6000 / (3 * 2) = 1000 each; nothing at x 16.
    assert!(cells.iter().all(|c| c.0[0] <= 15), "{cells:?}");
    assert_eq!(cells.iter().find(|c| c.0 == [15, 61, 4]).unwrap().1, 3_000);
    assert_eq!(g.world.borrow().get(OW, [16, 61, 4]), "minecraft:air");
}

/// A refused step changes nothing, in memory, in the records or in the world, whichever kind of commit it is.
#[test]
fn a_refused_flow_step_changes_nothing() {
    for case in 0..3 {
        let mut g = game();
        floor_under(&mut g, FLOOR);
        let (at, amount) = match case {
            0 => ([0, 65, 4], 1_000),
            1 => ([0, 61, 4], 10_000),
            _ => ([0, 61, 4], 1_000),
        };
        if case == 2 {
            walls(&mut g, at);
            put_slag(&mut g, [0, 62, 4], "iron", 500);
            walls(&mut g, [0, 62, 4]);
        }
        put_slag(&mut g, at, "iron", amount);
        let before = slag_cells(&mut g);
        let total = held(&mut g);
        g.world.borrow_mut().refuse_structured = true;
        schedule(&mut g, if case == 2 { [0, 62, 4] } else { at });
        for tick in 0..5 {
            g.run(1);
            assert_eq!(slag_cells(&mut g), before, "case {case} tick {tick}");
            assert_eq!((held(&mut g), recorded(&g)), (total, total));
        }
        assert!(g.world.borrow().dropped.is_empty());
    }
}

/// Breaking an empty slag (the step-one leftover) drops nothing, not a scrap of 0.
#[test]
fn breaking_an_empty_slag_drops_no_scrap() {
    let mut g = game();
    floor_under(&mut g, FLOOR);
    put_slag(&mut g, FIRST, "iron", 0);
    g.world.borrow_mut().dropped.clear();
    g.world.borrow_mut().set(OW, FIRST, "minecraft:air");
    g.break_block(FIRST);
    g.run(40);
    assert!(g.world.borrow().dropped.is_empty(), "{:?}", g.world.borrow().dropped);
}

/// A world reopened between the two steps (the chunk with the upper slag empty and the lower full), from the records and
/// from the journal: the metal is the lower's and the empty one goes after the reopening.
#[test]
fn a_world_reopened_between_the_two_steps_has_the_metal_once_and_the_empty_slag_goes() {
    let mut g = game();
    floor_under(&mut g, FLOOR);
    walls(&mut g, [0, 61, 4]);
    walls(&mut g, [0, 62, 4]);
    put_slag(&mut g, [0, 61, 4], "iron", 2_000);
    put_slag(&mut g, [0, 62, 4], "iron", 1_000);
    schedule(&mut g, [0, 62, 4]);
    g.run(1);
    assert_eq!(slag_cells(&mut g), vec![([0, 61, 4], 3_000), ([0, 62, 4], 0)]);
    // From the records.
    let world = Rc::new(RefCell::new(g.world.borrow().clone()));
    let mut reopened = Game::new(world);
    reopened.run(2);
    assert_eq!(slag_cells(&mut reopened), vec![([0, 61, 4], 3_000)], "the empty one is gone");
    assert_eq!(held(&mut reopened), 3_000);
    assert!(reopened.world.borrow().dropped.is_empty());
    // From the journal: the chunk's records are those from before the step (2000 and 1000), the journal's those after.
    let keys = [(OW.to_string(), [0, 61, 4]), (OW.to_string(), [0, 62, 4])];
    for (k, old) in keys.iter().zip([2_000, 1_000]) {
        let mut older = g.server.ready.world.machines[k].clone();
        let newer = older.clone();
        let Data::Slag(s) = &mut older.data else { panic!() };
        *s = Slag::of("iron", old);
        let mut newer = newer;
        newer.rev = older.rev + 1;
        let json = serde_json::to_string(&newer).unwrap();
        g.server.ready.world.survivors.insert(k.clone(), Survivor { dim: OW.into(), pos: k.1, generation: newer.generation, rev: newer.rev, json });
        g.world.borrow_mut().saved.insert(k.clone(), serde_json::to_string(&older).unwrap());
    }
    let earlier = g.server.ready.world.kept_as_an_earlier_version(0);
    let world = Rc::new(RefCell::new(g.world.borrow().clone()));
    world.borrow_mut().kv.set(crate::persist::KEY, &earlier).unwrap();
    let mut reopened = Game::new(world);
    assert!(keys.iter().all(|k| reopened.server.ready.restored.contains(k)), "restored from the journal");
    reopened.run(2);
    assert_eq!(slag_cells(&mut reopened), vec![([0, 61, 4], 3_000)]);
    assert_eq!(held(&mut reopened), 3_000);
}

/// A tank's pour onto the floor makes a slag of 2592, above a fifth of the capacity: the pour and the spreading after it keep
/// the sum exact at every tick, in the models and in the records.
#[test]
fn a_pour_that_makes_a_heavy_slag_goes_on_spreading_with_the_sum_exact() {
    let mut g = game();
    g.place(GROUND, "minecraft:stone");
    g.place([0, 60, 5], "minecraft:stone");
    for (dx, dz) in [(1, 0), (-1, 0), (0, -1)] {
        g.place([dx, 61, 4 + dz], "minecraft:stone");
    }
    put(&mut g, [0, 70, 3], "ntm:foundry_tank[facing=north,honey_level=0]");
    put(&mut g, TAP, "ntm:foundry_slagtap[facing=south,honey_level=0]");
    edit(&mut g, [0, 70, 3], |d| {
        let Data::FoundryTank(t) = d else { panic!() };
        t.melt = Melt { mat: Some("iron".into()), amount: 2_592 };
    });
    for tick in 0..60 {
        g.run(1);
        assert_eq!(held(&mut g), 2_592, "tick {tick}: the models");
        assert_eq!(recorded(&g), 2_592, "tick {tick}: the records");
    }
    // 2592 / (1 free side * 2) = 1296 spread to the south, 1296 stay.
    let mut cells = slag_cells(&mut g);
    cells.sort();
    assert_eq!(cells, vec![([0, 61, 4], 1_296), ([0, 61, 5], 1_296)]);
}

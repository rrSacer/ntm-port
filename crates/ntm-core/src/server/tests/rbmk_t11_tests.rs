//! T11: the RBMK storage's queue (`TileEntityRBMKStorage`) and the crane (`TileEntityCraneConsole`): what it loads and
//! unloads, and each transfer as one publication of both blocks.
use super::*;
use container_bridge::Stack;

const Y: i32 = 71;
const STORAGE: Pos = [0, Y, 0];

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn slots(g: &mut Game, p: Pos) -> Vec<Option<String>> {
    g.world.borrow().engine.get(&(OW.to_string(), p)).map(|e| e.0.iter().map(|s| s.as_ref().map(|s| s.id.clone())).collect()).unwrap_or_default()
}

/// Every 10 world ticks (in the column's own commit) one pass of the queue: each rod one place towards slot 0.
#[test]
fn the_storage_moves_its_queue_every_ten_ticks() {
    let mut g = game();
    g.place(STORAGE, &column("rbmk_storage"));
    g.run(2);
    let mut s: Vec<Option<Stack>> = vec![None; 12];
    s[2] = Some(Stack::new("ntm:rbmk_fuel_ueu", 1));
    s[11] = Some(Stack::new("ntm:rbmk_fuel_po210be", 1));
    g.world.borrow_mut().engine.insert((OW.to_string(), STORAGE), (s, Vec::new()));
    // Step by step: the queue changes by exactly one pass, once every 10 ticks (the phase is the port's clock's, as the
    // console's scan: `world.ticks`, not `getTotalWorldTime`).
    let mut passes = Vec::new();
    for _ in 0..25 {
        let (time, before) = (g.ready().world.ticks, slots(&mut g, STORAGE));
        g.run(1);
        let after = slots(&mut g, STORAGE);
        let mut expected = before.clone();
        crate::rbmk::crane::storage_pass(&mut expected);
        if after != before {
            assert_eq!(after, expected, "one pass at {time}");
            passes.push(time);
        }
    }
    assert!(passes.len() >= 2 && passes.windows(2).all(|w| w[1] - w[0] == 10), "{passes:?}");
    let end = slots(&mut g, STORAGE);
    assert_eq!((end[0].as_deref(), end[9].as_deref()), (Some("ntm:rbmk_fuel_ueu"), Some("ntm:rbmk_fuel_po210be")), "{end:?}");
}

const CRANE: Pos = [0, Y, -6];
const CHANNEL: Pos = [0, Y, 0];
const STORE: Pos = [2, Y, 0];
const BLANK: Pos = [0, Y, 2];

fn crane_of(g: &mut Game) -> crate::rbmk::crane::Crane {
    let Data::RbmkCrane(c) = g.ready().world.machines[&(OW.to_string(), CRANE)].data.clone() else { panic!("no crane") };
    *c
}

/// A hot rod (its hull at 1200 °C: the hand could not take it), its tag kept whole.
fn hot_rod() -> Stack {
    let mut s = Stack::new("ntm:rbmk_fuel_ueu", 1);
    s.custom = Some("{hull:1200.0d,core:1300.0d,yield:500000.0d,xenon:2.5d,display:{Name:'\"Lot 7\"'}}".into());
    s.tagged = true;
    s
}

/// A crane console facing north, linked to the channel (a hot rod in it); a storage two cells east, a blank two
/// south; each column with its dummies up to the top the crane looks at; player p1 at the console.
fn reactor(g: &mut Game) {
    g.place(CRANE, "ntm:rbmk_crane_console[facing=north,honey_level=0]");
    for (p, id) in [(CHANNEL, "rbmk_rod"), (STORE, "rbmk_storage"), (BLANK, "rbmk_blank")] {
        g.place(p, &column(id));
    }
    g.run(2);
    g.world.borrow_mut().engine.insert((OW.to_string(), CHANNEL), (vec![Some(hot_rod())], Vec::new()));
    assert!(g.server.native_rbmk_crane_link(&mut g.sh, OW, CRANE, CHANNEL));
    g.run(1);
    let top = crane_of(g).center[1] - 1;
    for p in [CHANNEL, STORE, BLANK] {
        for y in Y + 1..=top {
            g.place([p[0], y, p[2]], crate::rbmk::persist::DUMMY);
        }
    }
    g.server.set_players(vec![crate::server::PlayerInfo { id: "p1".into(), name: "p1".into(), dim: OW.into(), pos: [0.5, f64::from(Y), -7.0], creative: false }]);
}

/// The lot-7 rod, whatever its heat now (the channel rewrites its tag as it cools): its id and its name kept.
fn lot7(s: &Option<Stack>) -> bool {
    s.as_ref().is_some_and(|s| s.id == "ntm:rbmk_fuel_ueu" && s.count == 1 && s.custom.as_deref().is_some_and(|c| c.contains("Lot 7") && c.contains("xenon:2.5d")))
}

fn keys(g: &mut Game, k: crate::rbmk::crane::Keys) {
    g.server.native_crane_keys("p1", k.bits());
}

/// Down to the bottom and up again (the load key let go once the hook goes).
fn load(g: &mut Game) {
    use crate::rbmk::crane::Keys;
    keys(g, Keys { load: true, ..Keys::default() });
    g.run(1);
    keys(g, Keys::default());
    for _ in 0..80 {
        g.run(1);
        if !crane_of(g).loading() && !crane_of(g).goes_down {
            break;
        }
    }
}

fn held(g: &mut Game) -> Option<Stack> {
    g.world.borrow().engine.get(&(OW.to_string(), CRANE)).and_then(|e| e.0.first().cloned().flatten())
}

fn slot(g: &mut Game, p: Pos, i: usize) -> Option<Stack> {
    g.world.borrow().engine.get(&(OW.to_string(), p)).and_then(|e| e.0.get(i).cloned().flatten())
}

/// The tool's link: the centre over the channel's top, the spans as far as the room goes (16 in the open); the crane
/// moves only with the player at the console and keys held.
#[test]
fn the_crane_links_and_moves_with_the_operator() {
    use crate::rbmk::crane::Keys;
    let mut g = game();
    reactor(&mut g);
    let c = crane_of(&mut g);
    assert!(c.set_up && c.center[0] == 0 && c.center[2] == 0 && c.center[1] > Y && c.span_f == 16 && c.height == 7, "{c:?}");
    keys(&mut g, Keys { left: true, ..Keys::default() });
    g.run(10);
    assert!((crane_of(&mut g).pos_left - 0.5).abs() < 1e-9, "0.05 a tick, from the first: {}", crane_of(&mut g).pos_left);
    // Nobody at the console: it stands still, keys or not.
    g.server.set_players(vec![crate::server::PlayerInfo { id: "p1".into(), name: "p1".into(), dim: OW.into(), pos: [0.5, f64::from(Y), 5.0], creative: false }]);
    let at = crane_of(&mut g).pos_left;
    g.run(10);
    assert_eq!(crane_of(&mut g).pos_left, at);
    // The screwdriver turns the bridge's look.
    assert!(g.server.native_rbmk_crane_rotate(OW, CRANE));
    g.run(1);
    assert_eq!(crane_of(&mut g).rotation_offset, 90);
}

/// The hot rod from the channel to the storage and back: each transfer at the bottom, both blocks at once, the rod
/// whole (its tag, its name); never in two places, never in none.
#[test]
fn the_crane_carries_a_hot_rod_whole() {
    use crate::rbmk::crane::Keys;
    let mut g = game();
    reactor(&mut g);
    load(&mut g);
    assert!(lot7(&held(&mut g)) && slot(&mut g, CHANNEL, 0).is_none(), "out of the channel, hot (no heat limit): {:?}", held(&mut g));
    // Two cells east: `left` (the console faces north).
    keys(&mut g, Keys { left: true, ..Keys::default() });
    g.run(40);
    keys(&mut g, Keys::default());
    load(&mut g);
    let stored = (0..12).find_map(|i| slot(&mut g, STORE, i));
    assert!(held(&mut g).is_none() && lot7(&stored), "into the storage's slot 11, and down its queue: {stored:?}");
    // Back: the storage gives from slot 0 once the queue brought it there.
    g.run(130);
    assert!(lot7(&slot(&mut g, STORE, 0)));
    load(&mut g);
    assert!(lot7(&held(&mut g)) && (0..12).all(|i| slot(&mut g, STORE, i).is_none()));
}

/// A column the crane cannot use (a blank), an empty channel with nothing held, a full one with a rod held: nothing
/// moves, the hook comes back up as it went.
#[test]
fn the_crane_leaves_what_it_cannot_use() {
    use crate::rbmk::crane::Keys;
    let mut g = game();
    reactor(&mut g);
    // Two cells south: forwards.
    keys(&mut g, Keys { up: true, ..Keys::default() });
    g.run(40);
    keys(&mut g, Keys::default());
    assert!(!crane_of(&mut g).valid_target, "a blank is not loadable");
    load(&mut g);
    assert!(held(&mut g).is_none() && lot7(&slot(&mut g, CHANNEL, 0)));
    // Back over the channel, the rod taken, then down again onto the empty channel with it: it goes in.
    keys(&mut g, Keys { down: true, ..Keys::default() });
    g.run(40);
    keys(&mut g, Keys::default());
    assert!(crane_of(&mut g).valid_target);
    load(&mut g);
    assert!(held(&mut g).is_some());
    g.world.borrow_mut().engine.insert((OW.to_string(), CHANNEL), (vec![Some(Stack::new("ntm:rbmk_fuel_po210be", 1))], Vec::new()));
    load(&mut g);
    assert!(lot7(&held(&mut g)) && slot(&mut g, CHANNEL, 0).map(|s| s.id).as_deref() == Some("ntm:rbmk_fuel_po210be"), "a full channel takes nothing");
}

/// A world that refuses the transfer (a cut, a block changed under it): nothing moves on either side; the next descent
/// tries again. And a reopen in the middle of a descent: the hook is up again, the rod where it was.
#[test]
fn a_refused_transfer_moves_nothing() {
    let mut g = game();
    reactor(&mut g);
    g.world.borrow_mut().refuse_commits = true;
    load(&mut g);
    assert!(held(&mut g).is_none() && lot7(&slot(&mut g, CHANNEL, 0)), "refused: the rod still in the channel");
    g.world.borrow_mut().refuse_commits = false;
    load(&mut g);
    assert!(lot7(&held(&mut g)) && slot(&mut g, CHANNEL, 0).is_none());
    // Halfway down with the rod, the crane as a save reads it back: hook up, not going down, the rod held.
    keys(&mut g, crate::rbmk::crane::Keys { load: true, ..Default::default() });
    g.run(5);
    let json = serde_json::to_string(&crane_of(&mut g)).unwrap();
    let back: crate::rbmk::crane::Crane = serde_json::from_str(&json).unwrap();
    assert!(back.progress == 1.0 && !back.goes_down && lot7(&back.held().cloned()), "{back:?}");
}

/// The console broken with the rod held: the rod drops whole (the Java's console loses it; T11 decision 2).
#[test]
fn a_broken_crane_console_drops_what_it_holds() {
    let mut g = game();
    reactor(&mut g);
    load(&mut g);
    assert!(held(&mut g).is_some());
    g.break_block(CRANE);
    g.run(2);
    let dropped = g.world.borrow().dropped.clone();
    assert!(dropped.iter().any(|d| d.contains("ntm:rbmk_fuel_ueu 1")), "{dropped:?}");
}

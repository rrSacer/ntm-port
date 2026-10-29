//! A loaded PC: the client mod ticks slower or faster than the server, the
//! server's `SetTime` sets the client's game time back or ahead, the client
//! stalls for seconds, the server mod restarts. None of it is another world:
//! the session, the screen, the samples and the items go on.

use super::*;
use crate::radiation;

/// `ticks` server ticks; before server tick `i`, `clients(i)` client ticks
/// (0: the client is stalled, 2: it runs ahead). The client's level runs its
/// own game time, and every 20 server ticks takes the server's
/// (`ClientboundSetTimePacket`), back or ahead.
fn drive(g: &mut Game, ticks: usize, mut clients: impl FnMut(usize) -> usize) {
    let mut server_time = g.game_time;
    for i in 0..ticks {
        server_time += 1;
        for _ in 0..clients(i) {
            g.game_time += 1;
            g.client_hook();
        }
        if i % 20 == 19 {
            g.game_time = server_time;
        }
        g.server_step();
    }
}

fn dose(g: &Game) -> u64 {
    match &g.server.session {
        Session::Ready(r) => r.world.radiation.get("p1").map_or(0, |d| d.millirad),
        _ => panic!("world not identified"),
    }
}

fn late_samples(g: &Game) -> usize {
    g.server
        .probes()
        .iter()
        .filter(|p| p.late && matches!(p.purpose, Purpose::Radiation { .. }))
        .count()
}

/// A server slower than the client: the client runs ahead and each
/// `SetTime` puts its game time back. Before, every such step back was
/// "another world": a new session, the screen closed, the world identified
/// again.
#[test]
fn the_servers_time_corrections_never_start_a_session() {
    let mut g = game();
    let at = g.burner();
    let (sid, wid) = (g.client.session(), g.server.wid().map(str::to_string));
    drive(&mut g, 400, |i| 1 + usize::from(i % 2 == 0));
    assert_eq!(g.client.session(), sid, "same session");
    assert_eq!(g.server.wid().map(str::to_string), wid);
    assert!(g.ch.screen.is_some(), "the screen stayed open");
    // And the machine works under it, exactly.
    g.hold("minecraft:coal", 3);
    for _ in 0..3 {
        let held = g.held_now();
        g.client.select(&mut g.ch, actions::BURNER_FUEL, held);
        drive(&mut g, 10, |i| 1 + usize::from(i % 2 == 0));
    }
    drive(&mut g, 200, |i| 1 + usize::from(i % 2 == 0));
    assert_eq!(g.held(), None, "3 coal taken");
    assert_eq!(g.fuel_taken(at), 3, "and 3 credited");
    assert_eq!(g.client.session(), sid);
}

/// The client stalls 15 s (a frozen frame) while a give is in flight: its
/// game time jumps 300 ahead. Same session, the screen stays, the items come
/// once.
#[test]
fn a_client_stalled_for_seconds_keeps_its_session_and_gives_once() {
    let mut g = game();
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    let sid = g.client.session();
    g.client.select(&mut g.ch, actions::FURNACE_TAKE, None);
    drive(&mut g, 300, |_| 0);
    drive(&mut g, 400, |_| 1);
    assert_eq!(g.client.session(), sid, "same session");
    assert!(g.ch.screen.is_some(), "the screen stayed open");
    assert_eq!(g.inv("minecraft:iron_ingot"), 5, "given once");
    assert!(g.server.txs().is_empty());
    assert!(no_orphan_tokens(&g));
}

/// A client that stalls 1.5 s and then catches up (3 ticks a server tick,
/// as the game does after a long frame), with the corrections back and
/// ahead: no session change, no sample timed out, and the dose keeps its
/// rate (one sample stands for the ticks since the last, not for its
/// period: its round trip is longer on such a client).
#[test]
fn radiation_keeps_its_rate_on_a_stalling_client() {
    let mut g = game();
    g.world.borrow_mut().inventory.insert("ntm:ingot_uranium".into(), 2);
    g.run(250);
    let sid = g.client.session();
    let start = dose(&g);
    let secs = 30;
    drive(&mut g, secs * 20, |i| if i % 45 < 30 { 0 } else { 3 });
    let per_s = (dose(&g) - start) as f64 / secs as f64;
    assert!((630.0..=770.0).contains(&per_s), "{per_s} mRAD/s for 700");
    assert_eq!(late_samples(&g), 0, "no sample timed out");
    assert_eq!(g.client.session(), sid);
}

/// The client stalls 7.5 s with a sample in flight: the sample isn't timed
/// out (its time counts from the client's ack), and the dose of the stall
/// is counted by the sample after it.
#[test]
fn a_stall_never_times_out_a_sample() {
    let mut g = game();
    g.world.borrow_mut().inventory.insert("ntm:ingot_uranium".into(), 2);
    g.run(250);
    while !g
        .server
        .probes()
        .iter()
        .any(|p| !p.late && matches!(p.purpose, Purpose::Radiation { .. }))
    {
        g.step();
    }
    let start = dose(&g);
    drive(&mut g, 150, |_| 0);
    assert_eq!(late_samples(&g), 0, "waiting for the client, not late");
    drive(&mut g, 250, |_| 1);
    assert_eq!(late_samples(&g), 0);
    let per_s = (dose(&g) - start) as f64 / 20.0;
    assert!((630.0..=770.0).contains(&per_s), "{per_s} mRAD/s for 700");
}

/// The client stalls 4.5 s right after its hello (the column's lines are
/// out): the world-id probe waits for it instead of failing (twice, and
/// the player is told to turn cheats on).
#[test]
fn a_stall_while_identifying_is_no_failure() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w, kv, 1);
    // The hello goes; then only the server ticks until the column's lines
    // are out, unread.
    g.client_step();
    g.server.tick(&mut g.sh, g.tick, g.players.clone());
    while !g
        .server
        .probes()
        .iter()
        .any(|p| p.purpose == Purpose::WorldId && p.sent())
    {
        g.server_step();
    }
    let sid = g.client.session();
    drive(&mut g, 90, |_| 0);
    drive(&mut g, 20, |_| 1);
    assert_eq!(g.client.session(), sid);
    assert!(g.server.wid().is_some(), "identified");
    assert_eq!(g.server.id_attempts, 0, "no failed attempt");
    assert!(g.server.probes().iter().all(|p| !p.late), "none timed out");
    assert!(g.sh.told.is_empty(), "{:?}", g.sh.told);
}

/// The server mod restarts in the same world (a reload, a stop): the
/// client starts a session for it, with a hello saying where its player
/// is, and drops what was the old one's (its lines still queued, the
/// screen). Before, it went on in the old session.
#[test]
fn a_restarted_server_mod_gets_a_new_session() {
    let mut g = game();
    g.place([3, 70, 0], "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(3);
    let wid = g.server.wid().unwrap().to_string();
    let sid = g.client.session();
    g.server = Server::new(&mut g.sh, g.tick);
    g.run(30);
    assert_eq!(g.server.wid(), Some(wid.as_str()), "the same world, identified again");
    assert_eq!(g.ready().world.machines.len(), 1);
    assert_ne!(g.client.session(), sid, "a new session for the new server");
}

/// Another world opened at once (no frames without a world in between, and
/// a game time close to the last one): the new server mod is the sign, and
/// the screen of the old world's machine closes.
#[test]
fn another_world_is_told_by_its_new_server() {
    let mut g = game();
    let bat = [3, 70, 0];
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(3);
    g.open(bat);
    assert!(g.ch.screen.is_some());
    let wid1 = g.server.wid().unwrap().to_string();
    let w2 = Rc::new(RefCell::new(World::new()));
    g.world = w2.clone();
    g.sh.world = w2.clone();
    g.ch.world = w2;
    g.server = Server::new(&mut g.sh, 0);
    g.run(30);
    let wid2 = g.server.wid().expect("world 2 identified").to_string();
    assert_ne!(wid1, wid2);
    assert!(g.ready().world.machines.is_empty());
    assert!(g.ch.screen.is_none(), "world 1's screen closed");
    assert_eq!(g.client.wid.as_deref(), Some(wid2.as_str()));
}

/// Probes a previous run left (a run with many sessions, then a stop)
/// keep only their cells: their lines are never sent again, and storing
/// them each tick costs what their cells weigh.
#[test]
fn inherited_probes_are_stored_without_their_lines() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut probes = Vec::new();
    for i in 0..20u32 {
        let x = i as i32 % 8;
        let col = worldid::column(OW, x, i as i32 / 8);
        let cells = col
            .iter()
            .map(|p| Cell {
                pos: *p,
                restore: Restore::State("minecraft:deepslate".into()),
                seen: None,
            })
            .collect();
        let mut p = Probe::new(i + 1, None, OW, cells, Purpose::WorldId);
        p.lines = vec!["x".repeat(170); 81];
        p.sent_to = Some(7);
        probes.push(p);
    }
    probes::Store::default().store(&mut *kv.borrow_mut(), &probes).unwrap();
    let mut g = Game::new(w, kv.clone(), 3);
    g.run(30);
    assert!(g.server.wid().is_some());
    let stored: usize = probes::KEYS
        .iter()
        .filter_map(|k| kv.borrow_mut().get(k).map(|b| b.len()))
        .max()
        .unwrap();
    assert!(stored < 40_000, "{stored} bytes");
    assert!(g.server.probes().iter().all(|p| p.lines.is_empty()));
}

#[test]
fn exposure_counts_the_ticks_since_the_last_sample() {
    use radiation::{Mode, exposure_ticks};
    assert_eq!(exposure_ticks(Mode::Tags, None, 500), radiation::SAMPLE_TICKS);
    assert_eq!(exposure_ticks(Mode::Tags, Some(470), 500), 30);
    assert_eq!(exposure_ticks(Mode::Tags, Some(0), 5000), radiation::MAX_SAMPLE_GAP);
    assert_eq!(exposure_ticks(Mode::Ids, None, 500), radiation::ids_period());
    assert!(exposure_ticks(Mode::Ids, Some(0), 5000) >= 2 * radiation::ids_period());
}

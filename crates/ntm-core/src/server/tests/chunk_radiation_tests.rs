//! Onda 1, 4a: the world's radiation by chunk in the server. The map is the Java's (`ChunkRadiationHandlerSimple`):
//! loads put the saved value in, unloads leave the chunk in the map and save its value, every 20 server ticks from the
//! server's start the map spreads; the saves are the NTM storage's own key.
use super::*;
use crate::chunk_radiation::Saved;

const C: [i32; 2] = [0, 0];

fn fresh() -> Game {
    Game::new(Rc::new(RefCell::new(World::new())))
}

fn set(g: &mut Game, chunk: [i32; 2], amount: &str) -> Result<String, String> {
    let at = [chunk[0] as f64 * 16.0 + 8.5, 70.0, chunk[1] as f64 * 16.0 + 8.5];
    g.server.native_radiation_command(OW, at, &format!("ntmrad set {amount}")).unwrap()
}

fn raw(g: &Game, chunk: [i32; 2]) -> Option<f32> {
    g.server.native_chunk_radiation_raw(OW, chunk)
}

fn saved(g: &Game) -> Saved {
    g.world.borrow().kv.map.get(crate::persist::CHUNK_RADIATION_KEY).map(|b| serde_json::from_slice(b).unwrap()).unwrap_or_default()
}

#[test]
fn the_map_spreads_every_20_server_ticks_counted_from_the_servers_start() {
    let mut g = fresh();
    g.server.native_chunk_radiation_loaded(OW, C);
    assert_eq!(set(&mut g, C, "100"), Ok("Radiation set.".into()));
    g.run(19);
    assert_eq!(raw(&g, C), Some(100.0), "eggTimer reaches 20 on the 20th tick");
    g.run(1);
    assert_eq!(raw(&g, C), Some(100.0f32 * 0.6 * 0.99 - 0.05));
    assert_eq!(raw(&g, [1, 0]), Some(100.0f32 * 0.075), "a chunk never loaded gets its share");
    g.run(19);
    let once = raw(&g, C);
    g.run(1);
    assert_ne!(raw(&g, C), once, "and again 20 ticks later");
}

#[test]
fn set_only_writes_a_loaded_chunk_clamped_and_the_reading_clamps() {
    let mut g = fresh();
    assert_eq!(set(&mut g, [3, 3], "50"), Ok("Radiation set.".into()));
    assert_eq!(raw(&g, [3, 3]), None, "world.blockExists: not loaded, nothing");
    g.server.native_chunk_radiation_loaded(OW, C);
    assert!(set(&mut g, C, "100001").unwrap_err().contains("too big"));
    assert!(set(&mut g, C, "-1").unwrap_err().contains("too small"));
    assert_eq!(set(&mut g, C, "x"), Err("'x' is not a valid number".into()));
    assert_eq!(g.server.native_radiation_command(OW, [0.0; 3], "ntmrad"), Some(Err("/ntmrad <set/clear>".into())));
    assert_eq!(g.server.native_radiation_command(OW, [0.0; 3], "ntmbomba x"), None);
    // The sender's block coordinates (getPlayerCoordinates: floor): just west and north of 0, 0 is chunk -1, -1.
    g.server.native_chunk_radiation_loaded(OW, [-1, -1]);
    g.server.native_radiation_command(OW, [-0.5, 70.0, -0.5], "ntmrad set 7").unwrap().unwrap();
    assert_eq!((raw(&g, [-1, -1]), raw(&g, C)), (Some(7.0), Some(0.0)));
    set(&mut g, C, "100000").unwrap();
    g.server.native_chunk_field_for_test(OW).put(C, 250_000.0);
    assert_eq!(g.server.native_chunk_radiation(OW, C), 100_000.0, "getRadiation clamps; the map does not");
    assert_eq!(g.server.native_radiation_command(OW, [0.0; 3], "ntmrad clear"), Some(Ok("Cleared radiation data!".into())));
    assert_eq!(raw(&g, C), None);
}

#[test]
fn an_unloaded_chunk_stays_in_the_map_its_save_keeps_the_value_it_left_with_and_a_load_puts_that_back() {
    let mut g = fresh();
    g.server.native_chunk_radiation_loaded(OW, C);
    set(&mut g, C, "1000").unwrap();
    g.server.native_chunk_radiation_unloaded(OW, C);
    g.run(1);
    assert_eq!(saved(&g).get(OW, C), 1000.0, "receiveChunkSave as it unloads, written within the tick");
    g.run(40);
    let evolved = raw(&g, C).unwrap();
    assert!(evolved != 1000.0 && evolved > 0.0, "receiveChunkUnload removes nothing: it spreads and decays ({evolved})");
    assert_eq!(saved(&g).get(OW, C), 1000.0, "an unloaded chunk's save is not written again");
    g.server.native_chunk_radiation_loaded(OW, C);
    assert_eq!(raw(&g, C), Some(1000.0), "receiveChunkLoad puts the saved value over the map's");
    // An unload of a chunk the server does not hold as loaded saves nothing, though the spread put it in the map.
    assert!(raw(&g, [1, 0]).is_some_and(|r| r > 0.0));
    g.server.native_chunk_radiation_unloaded(OW, [1, 0]);
    g.run(1);
    assert_eq!(saved(&g).0.get(OW).map(|m| m.len()), Some(1));
}

#[test]
fn a_save_writes_every_loaded_chunk_and_a_reopened_server_reads_them_back() {
    let world = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(world.clone());
    for c in [C, [1, 0], [0, -1]] {
        g.server.native_chunk_radiation_loaded(OW, c);
    }
    set(&mut g, C, "5000").unwrap();
    g.run(20);
    let before: Vec<f32> = [C, [1, 0], [0, -1], [1, 1]].iter().map(|c| raw(&g, *c).unwrap()).collect();
    g.server.native_world_saving(&mut g.sh);
    g.server.native_world_saved(&mut g.sh, true);
    let s = saved(&g);
    assert_eq!([s.get(OW, C), s.get(OW, [1, 0]), s.get(OW, [0, -1])], [before[0], before[1], before[2]]);
    assert_eq!(s.get(OW, [1, 1]), 0.0, "a chunk never loaded has no save");
    drop(g);
    let mut g = Game::new(world);
    assert_eq!(raw(&g, C), None, "nothing until its chunk loads");
    g.server.native_chunk_radiation_loaded(OW, C);
    g.server.native_chunk_radiation_loaded(OW, [1, 1]);
    assert_eq!((raw(&g, C), raw(&g, [1, 1])), (Some(before[0]), Some(0.0)));
}

/// A chunk saved at 0 leaves the storage (`getFloat` of a missing key is 0), and the key is written alone: the world's
/// key does not move for it.
#[test]
fn the_saved_field_is_its_own_key_and_a_chunk_at_zero_leaves_it() {
    let mut g = fresh();
    g.server.native_chunk_radiation_loaded(OW, C);
    set(&mut g, C, "10").unwrap();
    g.run(30);
    let world_before = g.world.borrow().kv.map.get(crate::persist::KEY).cloned();
    let written = g.world.borrow().kv.written;
    g.server.native_chunk_radiation_unloaded(OW, C);
    g.run(1);
    let own = g.world.borrow().kv.map.get(crate::persist::CHUNK_RADIATION_KEY).map_or(0, Vec::len) as u64;
    assert!(saved(&g).get(OW, C) > 0.0);
    let w = g.world.borrow();
    assert_eq!(w.kv.map.get(crate::persist::KEY).cloned(), world_before);
    assert_eq!(w.kv.written - written, own, "the tick wrote the field's key alone");
    drop(w);
    g.server.native_chunk_radiation_loaded(OW, C);
    set(&mut g, C, "0").unwrap();
    g.server.native_chunk_radiation_unloaded(OW, C);
    g.run(1);
    assert_eq!(saved(&g), Saved::default());
}

/// Coordinator's condition 1 of 4a: the field's cost, in release (`cargo test --release -p ntm-core --lib
/// chunk_radiation_cost -- --ignored --nocapture`). A world of 15 × 15 loaded chunks, all at 100 000 (the spread's
/// widest map), with 50 000 block generations in the world's state, run for 50 000 updates (a million ticks): the tick
/// that updates and the save (each loaded chunk into its save, the key written), over the run, a source holding the
/// loaded chunks at 100 000.
#[test]
#[ignore]
fn chunk_radiation_cost() {
    use std::time::Instant;
    let mut g = fresh();
    for i in 0..50_000 {
        g.ready().world.generations.insert((OW.to_string(), [i % 1000, 64, i / 1000]), 1);
    }
    for x in -7..=7 {
        for z in -7..=7 {
            g.server.native_chunk_radiation_loaded(OW, [x, z]);
            g.server.set_chunk_radiation(OW, [x, z], 100_000.0);
        }
    }
    let report = |label: &str, ticks: &mut Vec<f64>, saves: &mut Vec<f64>, entries: usize| {
        ticks.sort_by(f64::total_cmp);
        saves.sort_by(f64::total_cmp);
        let p = |v: &Vec<f64>, q: f64| v[((v.len() - 1) as f64 * q) as usize];
        println!(
            "{label}: map {entries} chunks; update tick median {:.3} ms p99 {:.3} max {:.3}; save median {:.3} ms max {:.3}",
            p(ticks, 0.5), p(ticks, 0.99), p(ticks, 1.0), p(saves, 0.5), p(saves, 1.0)
        );
        ticks.clear();
        saves.clear();
    };
    let (mut ticks, mut saves) = (Vec::new(), Vec::new());
    for update in 1..=50_000u32 {
        // A source holding every loaded chunk at 100 000 (not timed): the map at its widest, as it stays.
        for x in -7..=7 {
            for z in -7..=7 {
                g.server.set_chunk_radiation(OW, [x, z], 100_000.0);
            }
        }
        // 19 ticks without the update (the timer alone), then the one with it.
        for _ in 0..19 {
            g.server.chunk_radiation_tick();
        }
        let t = Instant::now();
        g.server.chunk_radiation_tick();
        ticks.push(t.elapsed().as_secs_f64() * 1e3);
        // An autosave every 900 ticks (45 updates): the loaded chunks into their saves, the key written.
        if update % 45 == 0 {
            let t = Instant::now();
            g.server.save_loaded_chunk_radiation();
            g.server.write_chunk_radiation(&mut g.sh).unwrap();
            saves.push(t.elapsed().as_secs_f64() * 1e3);
        }
        if [100, 1_000, 10_000, 50_000].contains(&update) {
            let entries = g.server.ready.chunk_field[OW].len();
            report(&format!("updates ..{update}"), &mut ticks, &mut saves, entries);
        }
    }
    let bytes = g.world.borrow().kv.map.get(crate::persist::CHUNK_RADIATION_KEY).map_or(0, Vec::len);
    println!("saved key {bytes} B");
}

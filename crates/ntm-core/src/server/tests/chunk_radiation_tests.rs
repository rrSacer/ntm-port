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

fn survivor(dim: &str) -> PlayerInfo {
    PlayerInfo { dim: dim.into(), creative: false, ..player() }
}

fn dose(g: &Game) -> u64 {
    g.server.player_dose("p1").millirad
}

/// One tick that is not an update's (the field is read before the tick's update, so a tick off the 20 reads what was
/// set): what the tick added to the dose.
fn tick_dose(g: &mut Game) -> u64 {
    let before = dose(g);
    g.run(1);
    dose(g) - before
}

/// 4b: `handleRadiationFX` gives each player `contaminate(CREATIVE, rad / 20F)` every tick from the field where they
/// stand: rad / 20 RAD a tick, as milliRAD; nothing in the first 200 ticks, nothing to a creative player.
#[test]
fn the_field_doses_a_player_rad_over_20_a_tick_after_the_grace() {
    let mut g = fresh();
    g.server.set_players(vec![survivor(OW)]);
    g.server.native_chunk_radiation_loaded(OW, C);
    g.run(1);
    set(&mut g, C, "100").unwrap();
    assert_eq!(tick_dose(&mut g), 0, "the first 200 ticks: ContaminationUtil:287");
    g.run(205);
    set(&mut g, C, "100").unwrap();
    while g.server.field.timer >= 18 {
        g.run(1);
        set(&mut g, C, "100").unwrap();
    }
    assert_eq!(tick_dose(&mut g), 5_000, "100 RAD/s: 5 RAD a tick");
    // calculateRadiationMod: a full grey hazmat, 10^-2 as a float. 100 RAD/s * 0.01: 50 milliRAD a tick.
    set(&mut g, C, "100").unwrap();
    g.world.borrow_mut().radiation_factor = f64::from(10f32.powf(-2.0));
    assert_eq!(tick_dose(&mut g), 50);
    g.world.borrow_mut().radiation_factor = 1.0;
    set(&mut g, C, "100000").unwrap();
    let room = crate::radiation::MAX_DOSE - dose(&g);
    assert_eq!(tick_dose(&mut g), room, "5000 RAD a tick, up to the 2500 RAD cap (incrementRadiation)");
    g.server.set_players(vec![PlayerInfo { creative: true, ..survivor(OW) }]);
    g.ready().world.radiation.clear();
    set(&mut g, C, "100").unwrap();
    assert_eq!(tick_dose(&mut g), 0, "creative: ContaminationUtil:284");
}

/// A weak field doses over time: the fraction of a milliRAD each tick leaves is carried, as the Java's float sum.
#[test]
fn a_weak_field_doses_through_the_carried_fraction() {
    let mut g = fresh();
    g.server.set_players(vec![survivor(OW)]);
    g.server.native_chunk_radiation_loaded(OW, C);
    g.run(220);
    let mut total = 0;
    for _ in 0..40 {
        // 0.01 RAD/s: 0.5 milliRAD a tick, held against the update.
        set(&mut g, C, "0.01").unwrap();
        total += tick_dose(&mut g);
    }
    assert!((19..=20).contains(&total), "{total} milliRAD over 40 ticks at 0.5 a tick");
    // radEnv carries its fractions too: 0.5 milliRAD a tick is 10 milliRAD a second in the Geiger's buffer.
    while !g.server.tick.is_multiple_of(20) {
        set(&mut g, C, "0.01").unwrap();
        g.run(1);
    }
    for _ in 0..20 {
        set(&mut g, C, "0.01").unwrap();
        g.run(1);
    }
    assert!((9..=10).contains(&g.server.rad_buf("p1")), "{}", g.server.rad_buf("p1"));
}

/// The player's block is floor(pos): just west and north of 0, 0 is chunk -1, -1.
#[test]
fn the_player_reads_the_chunk_of_their_floored_block() {
    let mut g = fresh();
    g.server.set_players(vec![PlayerInfo { pos: [-0.5, 70.0, -0.5], ..survivor(OW) }]);
    g.server.native_chunk_radiation_loaded(OW, [-1, -1]);
    g.server.native_chunk_radiation_loaded(OW, C);
    g.run(220);
    g.server.native_radiation_command(OW, [-0.5, 70.0, -0.5], "ntmrad clear").unwrap().unwrap();
    while g.server.field.timer >= 18 {
        g.run(1);
    }
    g.server.native_radiation_command(OW, [-0.5, 70.0, -0.5], "ntmrad set 100").unwrap().unwrap();
    assert_eq!(raw(&g, [-1, -1]), Some(100.0));
    assert_eq!(tick_dose(&mut g), 5_000);
}

/// In the Nether the field is at least hellRad, 0.1 RAD/s: 5 milliRAD a tick with no field at all; the overworld
/// with no field doses nothing.
#[test]
fn the_nether_is_at_least_hell_rad() {
    let mut g = fresh();
    g.server.set_players(vec![survivor("minecraft:the_nether")]);
    g.run(220);
    let gained: u64 = (0..20).map(|_| tick_dose(&mut g)).sum();
    // (float) 0.1D / 20F, times 1000, a tick: 5 milliRAD, give or take the float's last digit carried.
    assert!((99..=100).contains(&gained), "{gained}");
    g.server.set_players(vec![survivor(OW)]);
    let gained: u64 = (0..20).map(|_| tick_dose(&mut g)).sum();
    assert_eq!(gained, 0);
}

/// `radEnv` takes the amount before the gates: a creative player's Geiger hears the field.
#[test]
fn the_geiger_buffer_hears_the_field_through_the_gates() {
    let mut g = fresh();
    g.server.native_chunk_radiation_loaded(OW, C);
    g.run(1);
    for _ in 0..60 {
        set(&mut g, C, "40").unwrap();
        g.run(1);
    }
    // 40 RAD/s: 2 RAD a tick, 20 ticks a second.
    assert_eq!(g.server.rad_buf("p1"), 40_000);
    assert_eq!(dose(&g), 0, "creative, and in the grace");
}

/// The 16 × 16 columns of chunk 0, 0 with `top` at y 64 (and `above` at 65 when given).
fn lay(g: &mut Game, top: &str, above: Option<&str>) {
    let mut w = g.world.borrow_mut();
    for x in 0..16 {
        for z in 0..16 {
            w.blocks.insert((OW.to_string(), [x, 64, z]), top.into());
            if let Some(a) = above {
                w.blocks.insert((OW.to_string(), [x, 65, z]), a.into());
            }
        }
    }
}

fn count(g: &Game, y: i32, block: &str) -> usize {
    let w = g.world.borrow();
    (0..16).flat_map(|x| (0..16).map(move |z| [x, y, z])).filter(|p| w.get(OW, *p).starts_with(block)).count()
}

/// 4c: `handleWorldDestruction`, every tick: a loaded chunk at 10 RAD/s or more has its surface turned, grass to dead
/// earth and tall grass to air (five draws of the one entry, ten passes each, a column in three at its height or the
/// block below).
#[test]
fn a_radiated_chunk_turns_its_grass_and_tall_grass() {
    let mut g = fresh();
    lay(&mut g, "minecraft:grass_block[snowy=false]", Some("minecraft:short_grass"));
    g.server.native_chunk_radiation_loaded(OW, C);
    g.server.native_chunk_field_for_test(OW).put(C, 50.0);
    g.run(1);
    let (earth, air) = (count(&g, 64, "ntm:waste_earth"), count(&g, 65, "minecraft:air"));
    // The map holds the one chunk: its five draws are all of it, fifty passes, and nearly every column turns.
    assert!(earth >= 250 && air >= 250, "{earth} dead earth, {air} tall grass gone of 256");
    assert_eq!(count(&g, 64, "minecraft:grass_block"), 256 - earth);
}

#[test]
fn below_10_rad_or_unloaded_nothing_turns() {
    let mut g = fresh();
    lay(&mut g, "minecraft:grass_block[snowy=false]", None);
    g.server.native_chunk_radiation_loaded(OW, C);
    g.server.native_chunk_field_for_test(OW).put(C, 9.99);
    g.run(5);
    assert_eq!(count(&g, 64, "minecraft:grass_block"), 256, "the threshold: getValue() < 10");
    g.server.native_chunk_radiation_unloaded(OW, C);
    g.server.native_chunk_field_for_test(OW).put(C, 5000.0);
    g.run(5);
    assert_eq!(count(&g, 64, "minecraft:grass_block"), 256, "chunkExists: not loaded");
}

/// Leaves (Material.leaves) become dead leaves six times in seven, air the seventh; dead leaves are left.
#[test]
fn leaves_turn_to_dead_leaves_or_air() {
    let mut g = fresh();
    lay(&mut g, "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]", None);
    g.server.native_chunk_radiation_loaded(OW, C);
    g.server.native_chunk_field_for_test(OW).put(C, 50.0);
    g.run(3);
    let (dead, air) = (count(&g, 64, "ntm:waste_leaves"), count(&g, 64, "minecraft:air"));
    assert!(dead + air > 200 && dead > 3 * air && air > 0, "{dead} dead leaves, {air} air");
    lay(&mut g, "ntm:waste_leaves[persistent=true]", None);
    g.run(3);
    assert_eq!(count(&g, 64, "ntm:waste_leaves"), 256);
}

/// The fog (`updateSystem:104-117`): a share written over 100 RAD/s, in 1 of 20, puts a fog in the chunk that gave
/// it, its 25 quads as `ntm:rad_fog_<n>` around a column of that chunk, `getHeightValue` up to 4 above.
#[test]
fn the_fog_rises_from_the_chunk_that_spreads() {
    let mut g = fresh();
    lay(&mut g, "minecraft:stone", None);
    g.server.native_chunk_radiation_loaded(OW, C);
    let mut fogs = Vec::new();
    for _ in 0..12 {
        g.world.borrow_mut().shown.clear();
        g.server.native_chunk_field_for_test(OW).clear();
        g.server.native_chunk_field_for_test(OW).put(C, 5000.0);
        g.run(20);
        let shown = g.world.borrow().shown.clone();
        let quads: Vec<_> = shown
            .iter()
            .filter_map(|(_, s)| match s {
                crate::blast::visuals::Show::Particles { id, at, .. } if id.starts_with("ntm:rad_fog_") => Some((*id, *at)),
                _ => None,
            })
            .collect();
        assert_eq!(quads.len() % 25, 0);
        fogs.extend(quads.chunks(25).map(|q| q.to_vec()));
    }
    assert!(!fogs.is_empty(), "12 updates of 9 shares over 100, 1 in 20 each: some fog");
    let layout = crate::chunk_radiation::fog_quads();
    for fog in &fogs {
        // The first quad's place less its offset is the fog's column: in chunk 0, 0, from 65 (the stone's height) up.
        let origin = [fog[0].1[0] - layout[0].0[0], fog[0].1[1] - layout[0].0[1], fog[0].1[2] - layout[0].0[2]];
        assert!((0.0..16.0).contains(&origin[0]) && (0.0..16.0).contains(&origin[2]), "{origin:?}");
        assert!((65.0..70.0).contains(&origin[1]), "{origin:?}");
        for (n, (id, at)) in fog.iter().enumerate() {
            assert_eq!(*id, super::chunk_radiation_server::FOG_PARTICLES[n]);
            for k in 0..3 {
                assert!((at[k] - origin[k] - layout[n].0[k]).abs() < 1e-9);
            }
        }
    }
}

/// `rad > fogRad`: at 150 RAD/s the largest share (its own, 150 × 0.6 × 0.99 − 0.05 ≈ 89) is below 100, no fog.
#[test]
fn no_fog_from_shares_at_or_below_100() {
    let mut g = fresh();
    lay(&mut g, "minecraft:stone", None);
    g.server.native_chunk_radiation_loaded(OW, C);
    for _ in 0..12 {
        g.server.native_chunk_field_for_test(OW).clear();
        g.server.native_chunk_field_for_test(OW).put(C, 150.0);
        g.run(20);
    }
    let fog = g.world.borrow().shown.iter().filter(|(_, s)| matches!(s, crate::blast::visuals::Show::Particles { id, .. } if id.starts_with("ntm:rad_fog_"))).count();
    assert_eq!(fog, 0);
}

/// A chunk not loaded gives no fog, though its share is over 100 and its columns have a height.
#[test]
fn no_fog_from_a_chunk_not_loaded() {
    let mut g = fresh();
    lay(&mut g, "minecraft:stone", None);
    for _ in 0..12 {
        g.server.native_chunk_field_for_test(OW).clear();
        g.server.native_chunk_field_for_test(OW).put(C, 5000.0);
        g.run(20);
    }
    let fog = g.world.borrow().shown.iter().filter(|(_, s)| matches!(s, crate::blast::visuals::Show::Particles { id, .. } if id.starts_with("ntm:rad_fog_"))).count();
    assert_eq!(fog, 0);
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
            g.server.chunk_radiation_tick(&mut g.sh);
        }
        let t = Instant::now();
        g.server.chunk_radiation_tick(&mut g.sh);
        ticks.push(t.elapsed().as_secs_f64() * 1e3);
        // An autosave every 900 ticks (45 updates): the loaded chunks into their saves, the key written.
        if update % 45 == 0 {
            let t = Instant::now();
            g.server.save_loaded_chunk_radiation();
            g.server.write_chunk_radiation(&mut g.sh).unwrap();
            saves.push(t.elapsed().as_secs_f64() * 1e3);
        }
        if [100, 1_000, 10_000, 50_000].contains(&update) {
            let entries = g.server.field.maps[OW].len();
            report(&format!("updates ..{update}"), &mut ticks, &mut saves, entries);
        }
    }
    let bytes = g.world.borrow().kv.map.get(crate::persist::CHUNK_RADIATION_KEY).map_or(0, Vec::len);
    println!("saved key {bytes} B");
}

/// Slice 4c's cost, in release (`cargo test --release -p ntm-core --lib chunk_radiation_effects_cost -- --ignored
/// --nocapture`): 4a's world (15 × 15 loaded chunks held at 100 000, every one over the threshold) under a grass
/// surface, every tick timed: handleWorldDestruction each tick (five draws, ten passes of 256 heights and some 85
/// reads each), the fog with the update. The heights are the test host's (a lookup down a hash map), not the
/// engine's heightmap.
#[test]
#[ignore]
fn chunk_radiation_effects_cost() {
    use std::time::Instant;
    let mut g = fresh();
    {
        let mut w = g.world.borrow_mut();
        for x in -112..128 {
            for z in -112..128 {
                w.blocks.insert((OW.to_string(), [x, 64, z]), "minecraft:grass_block[snowy=false]".into());
            }
        }
    }
    for x in -7..=7 {
        for z in -7..=7 {
            g.server.native_chunk_radiation_loaded(OW, [x, z]);
        }
    }
    let p = |v: &mut Vec<f64>, q: f64| {
        v.sort_by(f64::total_cmp);
        v[((v.len() - 1) as f64 * q) as usize]
    };
    let (mut update, mut plain) = (Vec::new(), Vec::new());
    let mut fogs = 0;
    for n in 1..=2_000u32 {
        for x in -7..=7 {
            for z in -7..=7 {
                g.server.set_chunk_radiation(OW, [x, z], 100_000.0);
            }
        }
        for t in 0..20 {
            let shown = g.world.borrow().shown.len();
            let start = Instant::now();
            g.server.chunk_radiation_tick(&mut g.sh);
            let ms = start.elapsed().as_secs_f64() * 1e3;
            // The test host's writes land after the tick, as the game's do not: applied here, out of the timing.
            g.world.borrow_mut().apply_writes();
            if t == 19 { update.push(ms) } else { plain.push(ms) }
            fogs += (g.world.borrow().shown.len() - shown) / 25;
        }
        if [100, 2_000].contains(&n) {
            println!(
                "updates ..{n}: tick without update median {:.3} ms p99 {:.3} max {:.3}; update tick median {:.3} ms p99 {:.3} max {:.3}; fogs {fogs}",
                p(&mut plain, 0.5), p(&mut plain, 0.99), p(&mut plain, 1.0), p(&mut update, 0.5), p(&mut update, 0.99), p(&mut update, 1.0)
            );
            update.clear();
            plain.clear();
            fogs = 0;
        }
    }
    let earth = g.world.borrow().blocks.values().filter(|b| b.starts_with("ntm:waste_earth")).count();
    println!("dead earth {earth} of {}", 240 * 240);
}

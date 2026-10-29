use super::*;
use crate::radiation;

fn dose(g: &Game) -> u64 {
    match &g.server.session {
        Session::Ready(r) => r.world.radiation.get("p1").map_or(0, |d| d.millirad),
        _ => panic!("world not identified"),
    }
}

#[test]
fn inventory_radiation_counts_stacks_stops_and_persists() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world
        .borrow_mut()
        .inventory
        .insert("ntm:ingot_uranium".into(), 2);
    world
        .borrow_mut()
        .inventory
        .insert("ntm:powder_thorium".into(), 3);
    world.borrow_mut().held = Some(("ntm:nugget_uranium".into(), 7));
    let mut g = Game::new(world.clone(), kv.clone(), 71);
    g.run(220); // Initial 200-tick immunity, then a completed sample.
    let before = dose(&g);
    g.run(400);
    // A sample stands for the ticks since the last: the dose keeps the
    // rate (within a sample's cycle at the ends of the 20 s).
    let per_s = (dose(&g) - before) as f64 / 20.0;
    assert!((1845.0 * 0.94..=1845.0 * 1.06).contains(&per_s), "{per_s} mRAD/s for 1845");
    let wid = g.server.wid().unwrap().to_string();
    let (_, automatic) = persist::load(&mut g.sh, &wid).unwrap();
    let saved_dose = automatic.radiation["p1"].millirad;
    assert!(saved_dose <= dose(&g));
    assert!(
        dose(&g) - saved_dose <= 5 * 1845,
        "at most five seconds await persistence"
    );
    let reading = g.client.radiation.as_ref().unwrap();
    assert_eq!(reading.rate_millirad_s, 1845);
    assert_eq!(
        world.borrow().held.as_ref().unwrap().1,
        7,
        "count never consumes"
    );
    assert_eq!(world.borrow().inventory["ntm:ingot_uranium"], 2);

    world.borrow_mut().inventory.clear();
    world.borrow_mut().held = None;
    g.run(40); // Let any sample already in flight finish.
    let retained = dose(&g);
    g.run(60);
    assert_eq!(dose(&g), retained);
    assert_eq!(g.client.radiation.as_ref().unwrap().rate_millirad_s, 0);
    let wid = g.server.wid().unwrap().to_string();
    let (_, saved) = persist::load(&mut g.sh, &wid).unwrap();
    assert_eq!(saved.radiation["p1"].millirad, retained);

    let mut reloaded = Game::new(world, kv, 72);
    reloaded.run(40);
    assert_eq!(reloaded.server.wid(), Some(wid.as_str()));
    assert_eq!(dose(&reloaded), retained);
}

#[test]
fn incomplete_inventory_probe_never_adds_stale_exposure() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world
        .borrow_mut()
        .inventory
        .insert("ntm:ingot_uranium".into(), 64);
    let mut g = Game::new(world.clone(), kv, 73);
    g.run(250);
    assert!(dose(&g) > 0);
    world.borrow_mut().cheats = false;
    g.run(100);
    let after_timeout = dose(&g);
    g.run(100);
    assert_eq!(dose(&g), after_timeout);
    assert!(
        g.client
            .radiation
            .as_ref()
            .unwrap()
            .status
            .contains("confirmação")
    );
}

#[test]
fn radiation_probe_recovers_after_crash_without_stale_dose() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world
        .borrow_mut()
        .inventory
        .insert("ntm:ingot_uranium".into(), 2);
    let mut g = Game::new(world.clone(), kv.clone(), 74);
    g.run(250);
    let probe = (0..40)
        .find_map(|_| {
            g.step();
            g.server
                .probes
                .iter()
                .find(|p| {
                    matches!(p.purpose, Purpose::Radiation { .. })
                        && p.cells.iter().any(|c| c.seen.is_some())
                        && p.cells.iter().any(|c| c.seen.is_none())
                })
                .cloned()
        })
        .expect("crash while a radiation column is partially filled");
    let wid = g.server.wid().unwrap().to_string();
    let (_, durable) = persist::load(&mut g.sh, &wid).unwrap();
    let before = durable.radiation["p1"].millirad;
    assert!(
        !crate::probes::Store::load(&mut g.sh).0.is_empty(),
        "probe was persisted before its commands"
    );

    let mut reloaded = Game::new(world.clone(), kv, 75);
    reloaded.run(15);
    assert!(reloaded.server.wid().is_some());
    reloaded.server.radiation.next_sample = u64::MAX;
    reloaded.run(100);
    assert_eq!(
        dose(&reloaded),
        before,
        "recovered old samples never irradiate again"
    );
    for cell in probe.cells {
        if let Restore::State(original) = cell.restore {
            assert_eq!(world.borrow().get(&probe.dim, cell.pos), original);
        }
    }
}

#[test]
fn sickness_threshold_persists_before_periodic_dose_save() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world
        .borrow_mut()
        .inventory
        .insert("ntm:ingot_uranium".into(), 1);
    let mut g = Game::new(world, kv, 76);
    g.run(250);
    if let Session::Ready(r) = &mut g.server.session {
        r.world.radiation.get_mut("p1").unwrap().millirad = 199_900;
    }
    g.server.radiation.next_dose_save = u64::MAX;
    g.server.radiation.dose_dirty = false;
    g.run(20);
    assert!(dose(&g) >= 200_000);
    let wid = g.server.wid().unwrap().to_string();
    let (_, durable) = persist::load(&mut g.sh, &wid).unwrap();
    assert_eq!(durable.radiation["p1"].millirad, dose(&g));
}

#[test]
fn ordinary_dose_updates_do_not_request_a_save_each_second() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world
        .borrow_mut()
        .inventory
        .insert("ntm:ingot_uranium".into(), 1);
    let mut g = Game::new(world, kv, 77);
    g.run(230);
    // Isolate radiation's requested writes from the independent energy autosave.
    if let Session::Ready(r) = &mut g.server.session {
        r.next_save = u64::MAX;
    }
    let wid = g.server.wid().unwrap().to_string();
    let generation = persist::load(&mut g.sh, &wid).unwrap().0;
    let before = dose(&g);
    g.run(60);
    assert!(dose(&g) > before);
    assert_eq!(persist::load(&mut g.sh, &wid).unwrap().0, generation);
    g.run(60);
    assert_eq!(persist::load(&mut g.sh, &wid).unwrap().0, generation + 1);
}

/// A cell of the sample's column changes (a block put there) between the
/// server choosing it and the lines running: the block stays.
#[test]
fn a_replaced_sample_cell_is_never_overwritten() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world.borrow_mut().inventory.insert("ntm:ingot_uranium".into(), 1);
    let mut g = Game::new(world.clone(), kv, 76);
    g.run(15);
    world.borrow_mut().paused = true;
    let mut cell = None;
    for _ in 0..400 {
        g.step();
        if let Some(p) = g
            .server
            .probes()
            .iter()
            .find(|p| matches!(p.purpose, Purpose::Radiation { .. }))
        {
            cell = Some((p.dim.clone(), p.cells[0].pos));
            break;
        }
    }
    let (dim, pos) = cell.expect("a sample was planned");
    world.borrow_mut().set(&dim, pos, "minecraft:diamond_block");
    world.borrow_mut().paused = false;
    g.run(200);
    assert_eq!(world.borrow().get(&dim, pos), "minecraft:diamond_block");
}

// ------------------------------------------------- the full catalog

/// Runs one sample's lines in a world whose inventory is `items` (and
/// whose loaded tags are `tags`) and reads the rate the column spells.
fn sample_in(items: &[(&str, u32)], tags: Option<BTreeMap<String, BTreeSet<String>>>) -> Option<u32> {
    match sample_as(radiation::Mode::Tags, items, tags, &[]) {
        radiation::Sample::Rate(r) => Some(r),
        _ => None,
    }
}

/// One sample of either kind, in a world with these tags and without
/// the `unknown` item ids.
fn sample_as(
    mode: radiation::Mode,
    items: &[(&str, u32)],
    tags: Option<BTreeMap<String, BTreeSet<String>>>,
    unknown: &[&str],
) -> radiation::Sample {
    thread_local! {
        static BASE: RefCell<World> = RefCell::new(World::new());
    }
    BASE.with_borrow_mut(|w| {
        w.tags = tags.unwrap_or_else(content_tags);
        w.unknown_items = unknown.iter().map(|s| s.to_string()).collect();
        w.inventory.clear();
        w.held = None;
        w.scores.clear();
        w.nbt.clear();
        for (id, n) in items {
            *w.inventory.entry(id.to_string()).or_default() += n;
        }
        let cells: [(Pos, String); radiation::CELLS] =
            std::array::from_fn(|i| ([5, -64 + i as i32, 5], w.get(OW, [5, -64 + i as i32, 5])));
        let lines = match mode {
            radiation::Mode::Tags => radiation::sample_lines(9, OW, &cells),
            radiation::Mode::Ids => radiation::ids_sample_lines(9, OW, &cells),
        };
        for line in lines {
            assert!(line.len() <= token::MAX_LINE, "{line}");
            if w.parses(&line) {
                w.exec(OW, &line);
            }
        }
        let levels: Vec<Option<u8>> =
            cells.iter().map(|(p, _)| token::token_level(&w.get(OW, *p))).collect();
        for (p, orig) in &cells {
            w.set(OW, *p, orig);
        }
        radiation::read(9, &levels)
    })
}

fn expected(items: &[(&str, u32)]) -> Option<u32> {
    Some(radiation::rate_of(items.iter().copied()).min(u64::from(radiation::MAX_RATE)) as u32)
}

/// Every rate of the catalog, each through its own tags: the count is
/// exact, item by item, at several stack sizes.
#[test]
fn every_radioactive_item_counts_exactly() {
    let hazards = radiation::hazards();
    assert!(hazards.len() > 200);
    for (i, h) in hazards.iter().enumerate() {
        for n in [1, 1 + (i as u32 * 7) % 64] {
            let items = [(h.id.as_str(), n)];
            assert_eq!(sample_in(&items, None), expected(&items), "{} x{n}", h.id);
        }
    }
}

/// One item of every rate bucket together, and mixed inventories: the
/// sum of all of them, exactly, until the wire's ceiling.
#[test]
fn every_bucket_together_sums_exactly() {
    let mut by_rate: BTreeMap<u32, &str> = BTreeMap::new();
    for h in radiation::hazards() {
        by_rate.entry(h.radiation_millirad_s).or_insert(&h.id);
    }
    assert!(by_rate.len() > 90, "the catalog's distinct rates");
    // The lower rates fit under the ceiling: an exact sum of many buckets.
    let low: Vec<(&str, u32)> = by_rate
        .iter()
        .filter(|(r, _)| **r < 100_000)
        .map(|(_, id)| (*id, 3))
        .collect();
    let exact = expected(&low).unwrap();
    assert!(exact > 1_000_000 && exact < radiation::MAX_RATE);
    assert_eq!(sample_in(&low, None), Some(exact));
    // With vanilla items and a stack of junk around them nothing changes.
    let mut mixed = low.clone();
    mixed.extend([("minecraft:stone", 64), ("minecraft:iron_ingot", 12)]);
    assert_eq!(sample_in(&mixed, None), Some(exact));
    // All of them: well past the ceiling, read as it.
    let all: Vec<(&str, u32)> = by_rate.values().map(|id| (*id, 1)).collect();
    assert_eq!(sample_in(&all, None), Some(radiation::MAX_RATE));
}

/// The ceiling, from both sides, and huge inventories: the game's scores
/// are 32-bit, and the partial sum never wraps to a small number.
#[test]
fn the_ceiling_is_exact_and_nothing_wraps() {
    let one = radiation::hazards()
        .iter()
        .find(|h| h.radiation_millirad_s == 1)
        .expect("a 1 mRAD/s item");
    let max = radiation::MAX_RATE;
    assert_eq!(sample_in(&[(&one.id, max - 1)], None), Some(max - 1));
    assert_eq!(sample_in(&[(&one.id, max)], None), Some(max));
    assert_eq!(sample_in(&[(&one.id, max + 1)], None), Some(max));
    let hottest = radiation::hazards()
        .iter()
        .max_by_key(|h| h.radiation_millirad_s)
        .unwrap();
    // Without the clamps along the sum, 8 of them would wrap negative.
    for n in (1..=300).chain([41 * 64, 1 << 20]) {
        assert_eq!(sample_in(&[(&hottest.id, n)], None), Some(max), "{} x{n}", hottest.id);
    }
    // Every item of the catalog, a full stack each.
    let all: Vec<(&str, u32)> = radiation::hazards().iter().map(|h| (h.id.as_str(), 64)).collect();
    assert_eq!(sample_in(&all, None), Some(max));
}

/// A content mod without one of the tags (an older `ntm.hmod`): the line
/// naming it doesn't parse, and the sample is never confirmed, rather than
/// read short. With none of them, the same.
#[test]
fn a_missing_tag_is_never_read_as_a_lower_rate() {
    let uranium = [("ntm:ingot_uranium", 2)];
    assert_eq!(sample_in(&uranium, None), Some(700));
    for &k in radiation::bits() {
        let mut tags = content_tags();
        tags.remove(&radiation::bit_tag(k));
        assert_eq!(sample_in(&uranium, Some(tags)), None, "without bit_{k}");
    }
    assert_eq!(sample_in(&uranium, Some(BTreeMap::new())), None);
    // And it says so, apart from a sample that never ran.
    assert_eq!(
        sample_as(radiation::Mode::Tags, &uranium, Some(BTreeMap::new()), &[]),
        radiation::Sample::TagsUnknown
    );
}

/// The cadence in the running server: about one confirmed sample a second,
/// never more than 16 radiation lines in a tick, and within the 64 the
/// game runs a tick even with a machine's transaction in the same ticks.
#[test]
fn samples_keep_their_cadence_beside_machines() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world.borrow_mut().inventory.insert("ntm:ingot_uranium".into(), 2);
    let mut g = Game::new(world.clone(), kv, 78);
    g.run(250);
    let start = dose(&g);
    g.run(400);
    let per_s = (dose(&g) - start) as f64 / 20.0;
    assert!((660.0..=740.0).contains(&per_s), "{per_s} mRAD/s for 700");
    // A burner fed while samples run: the coal is taken exactly, and the
    // doses keep coming.
    let at = g.burner();
    g.hold("minecraft:coal", 3);
    let (before, since) = (dose(&g), g.tick);
    g.open(at);
    for _ in 0..3 {
        g.click(actions::BURNER_FUEL);
        g.run(5);
    }
    g.run(200);
    assert_eq!(g.held(), None, "3 coal taken");
    assert_eq!(g.fuel_taken(at), 3, "and 3 credited");
    let per_s = (dose(&g) - before) as f64 * 20.0 / (g.tick - since) as f64;
    assert!((630.0..=770.0).contains(&per_s), "samples went on beside the machine: {per_s}");
    let mut machine_hooks = 0;
    for hook in &world.borrow().hooks {
        let rad = hook.iter().filter(|l| l.contains(radiation::OBJECTIVE)).count();
        assert!(rad <= radiation::COMMANDS_PER_TICK);
        // A transaction's lines and the sample's that reach the game with
        // them fit one tick of the consumer: the machine's never wait.
        if hook.iter().any(|l| l.contains("ntm_tx")) {
            machine_hooks += 1;
            assert!(hook.len() <= 64, "{} lines", hook.len());
        }
    }
    assert!(machine_hooks >= 3);
}

// --------------------------------------------- the fallback by item ids

/// By ids, in a game that knows none of the mod's tags: the same exact
/// counts, item by item, mixed, and at the ceiling.
#[test]
fn id_samples_count_exactly_without_tags() {
    use radiation::{Mode::Ids, Sample::Rate};
    let none = || Some(BTreeMap::new());
    for (i, h) in radiation::hazards().iter().enumerate() {
        let n = 1 + (i as u32 * 5) % 64;
        let items = [(h.id.as_str(), n)];
        assert_eq!(sample_as(Ids, &items, none(), &[]), Rate(expected(&items).unwrap()), "{} x{n}", h.id);
    }
    let mut by_rate: BTreeMap<u32, &str> = BTreeMap::new();
    for h in radiation::hazards() {
        by_rate.entry(h.radiation_millirad_s).or_insert(&h.id);
    }
    let mut low: Vec<(&str, u32)> =
        by_rate.iter().filter(|(r, _)| **r < 100_000).map(|(_, id)| (*id, 3)).collect();
    low.push(("minecraft:stone", 64));
    assert_eq!(sample_as(Ids, &low, none(), &[]), Rate(expected(&low).unwrap()));
    let max = radiation::MAX_RATE;
    let one = radiation::hazards().iter().find(|h| h.radiation_millirad_s == 1).unwrap();
    for n in [max - 1, max, max + 1] {
        assert_eq!(sample_as(Ids, &[(&one.id, n)], none(), &[]), Rate(n.min(max)));
    }
    let hottest = radiation::hazards().iter().max_by_key(|h| h.radiation_millirad_s).unwrap();
    for n in (1..=40).chain([1 << 20]) {
        assert_eq!(sample_as(Ids, &[(&hottest.id, n)], none(), &[]), Rate(max), "x{n}");
    }
    let all: Vec<(&str, u32)> = radiation::hazards().iter().map(|h| (h.id.as_str(), 64)).collect();
    assert_eq!(sample_as(Ids, &all, none(), &[]), Rate(max));
    // Two items each saturating its weighted count: their sum must not
    // wrap the 32-bit total.
    let mut hot: Vec<&radiation::Hazard> = radiation::hazards().iter().collect();
    hot.sort_by_key(|h| std::cmp::Reverse(h.radiation_millirad_s));
    for a in &hot[..6] {
        for b in &hot[..6] {
            if a.id != b.id {
                let items = [(a.id.as_str(), 64), (b.id.as_str(), 64)];
                assert_eq!(sample_as(Ids, &items, none(), &[]), Rate(max), "{} + {}", a.id, b.id);
            }
        }
    }
}

/// An id the game doesn't have (an older content mod) counts 0, which no
/// player can carry anyway, and never repeats the item before it.
#[test]
fn an_unknown_id_never_reads_another_items_count() {
    use radiation::{Mode::Ids, Sample::Rate};
    let h = radiation::hazards();
    let (a, b) = (&h[0], &h[1]);
    let carried = [(a.id.as_str(), 10)];
    assert_eq!(
        sample_as(Ids, &carried, Some(BTreeMap::new()), &[b.id.as_str()]),
        Rate(10 * a.radiation_millirad_s)
    );
}

fn mode(g: &Game) -> radiation::Mode {
    g.server.radiation.mode
}

/// The game doesn't know the mod's tags: after a few unconfirmed tag
/// samples, samples go by ids, the HUD says so, and the dose keeps its
/// pace (one id sample stands for its whole period).
#[test]
fn unknown_tags_switch_to_id_samples_and_keep_the_dose() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world.borrow_mut().tags.clear();
    world.borrow_mut().inventory.insert("ntm:ingot_uranium".into(), 2);
    let mut g = Game::new(world.clone(), kv.clone(), 81);
    g.run(250);
    assert_eq!(mode(&g), radiation::Mode::Ids, "switched after the tag misses");
    let per_sample = 700 * radiation::ids_period() / 20;
    assert_eq!(dose(&g) % per_sample, 0, "dose only from confirmed id samples, none from the misses");
    let reading = g.client.radiation.clone().unwrap();
    g.run(radiation::ids_period() as usize * 2);
    let start = dose(&g);
    let secs = 30;
    g.run(secs * 20);
    let per_s = (dose(&g) - start) as f64 / secs as f64;
    assert!((600.0..=800.0).contains(&per_s), "{per_s} mRAD/s for 700");
    let r = g.client.radiation.as_ref().unwrap();
    assert_eq!(r.rate_millirad_s, 700);
    assert!(r.status.contains("ids"), "{:?} (before: {reading:?})", r.status);
    // Every id sample's lines stay within 16 a tick.
    for hook in &world.borrow().hooks {
        let rad = hook.iter().filter(|l| l.contains(radiation::OBJECTIVE)).count();
        assert!(rad <= radiation::COMMANDS_PER_TICK * 2);
    }

    // The game learns the tags (a patched Horizon): the next retry goes
    // back to tag samples.
    world.borrow_mut().tags = content_tags();
    g.server.radiation.retry_tags_at = 0;
    g.run(radiation::ids_period() as usize + 120);
    assert_eq!(mode(&g), radiation::Mode::Tags);
    let start = dose(&g);
    g.run(400);
    let per_s = (dose(&g) - start) as f64 / 20.0;
    assert!((660.0..=740.0).contains(&per_s), "tag samples again: {per_s}");

    // Entering the world again always tries tags first.
    world.borrow_mut().tags.clear();
    let mut again = Game::new(world, kv, 82);
    again.run(15);
    assert_eq!(mode(&again), radiation::Mode::Tags);
}

/// Samples that never ran (no cheats) are not unknown tags: no switch.
#[test]
fn unconfirmed_samples_without_cheats_never_switch_modes() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let world = Rc::new(RefCell::new(World::new()));
    world.borrow_mut().inventory.insert("ntm:ingot_uranium".into(), 2);
    let mut g = Game::new(world.clone(), kv, 83);
    g.run(250);
    world.borrow_mut().cheats = false;
    g.run(400);
    assert_eq!(mode(&g), radiation::Mode::Tags);
    assert_eq!(g.server.radiation.tag_misses, 0);
}

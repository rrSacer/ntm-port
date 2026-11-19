//! T04: the eighteen columns are nodes of the streams and neighbours in the heat; occlusion is Java's `isOpaqueCube`;
//! the dials are a cache per dimension over the save's rules (3A); the steps' `SatelliteRayScan` events are kept by
//! dimension with their lifetime (4A); a NaN increment and a broken layer in the save cannot break the world (D2).
use super::*;
use crate::rbmk::persist::{self, RbmkColumn};
use container_bridge::Stack;

const Y: i32 = 71;
const NETHER: &str = "minecraft:the_nether";

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn rbmk_in(g: &mut Game, dim: &str, p: Pos) -> RbmkColumn {
    let Data::Rbmk(c) = g.ready().world.machines[&(dim.to_string(), p)].data.clone() else {
        panic!("{p:?} is not an RBMK column")
    };
    *c
}

fn rbmk(g: &mut Game, p: Pos) -> RbmkColumn {
    rbmk_in(g, OW, p)
}

fn place_in(g: &mut Game, dim: &str, p: Pos, state: &str) {
    let old = g.world.borrow().get(dim, p);
    g.world.borrow_mut().set(dim, p, state);
    g.server.native_block_changed(dim, p, &old, state);
}

fn fuel_in(g: &mut Game, dim: &str, p: Pos, rod: &str) {
    let stack = Stack::new(rod, 2);
    let held = HeldStack {
        encoded: format!("{stack:?}"),
        stack,
    };
    g.world.borrow_mut().hands = Some(PlayerSlots {
        hand: Some(PlayerSlot::Inventory(0)),
        inventory: vec![Some(held), None],
        creative: false,
        ..Default::default()
    });
    assert!(
        g.server
            .native_rbmk_use(&mut g.sh, dim, [p[0], p[1] + 2, p[2]], p, "p1"),
        "{rod} goes in"
    );
}

fn command(g: &mut Game, dim: &str, line: &str) -> Result<String, String> {
    let who = PlayerInfo {
        dim: dim.into(),
        ..player()
    };
    g.server
        .native_rbmk_command(&mut g.sh, &who, 2, line)
        .expect("an ntmrbmk command")
}

/// A Po-210/Be source channel (cardinal streams) at (0, 0) and a uranium channel three cells south, with `between` at
/// (0, 2) on the layer and `above` at (0, Y + 1, 2): the first tick the uranium channel takes flux, what it took.
fn first_flux(between: Option<&str>, above: &[(i32, &str)]) -> f64 {
    let mut g = game();
    g.place([0, Y, 0], &column("rbmk_rod"));
    g.place([0, Y, 3], &column("rbmk_rod"));
    if let Some(b) = between {
        g.place([0, Y, 2], &column(b));
    }
    g.run(2);
    fuel_in(&mut g, OW, [0, Y, 0], "ntm:rbmk_fuel_po210be");
    fuel_in(&mut g, OW, [0, Y, 3], "ntm:rbmk_fuel_ueu");
    for (dy, b) in above {
        g.world.borrow_mut().set(OW, [0, Y + dy, 2], b);
    }
    for _ in 0..12 {
        g.run(1);
        let q = rbmk(&mut g, [0, Y, 3]).last_flux_quantity;
        if q != 0.0 {
            return q;
        }
    }
    0.0
}

/// `getHits` counts the opaque cubes over a cell without a column of the layer: stone does, glass does not, and a
/// column's dummy is its column's block (a fuel channel's is not opaque, `RBMKRod:33`; a blank's is, `RBMKBase:58`);
/// the RBMK console and the panels are not (`BlockDummyable:358`, `RBMKMiniPanelBase:42`).
#[test]
fn occlusion_is_javas_opaque_cube() {
    let open = first_flux(None, &[]);
    assert!(open > 0.0, "the channel takes the source's stream");
    let bits = |q: f64| q.to_bits();
    assert_eq!(
        bits(first_flux(None, &[(1, "minecraft:glass")])),
        bits(open),
        "glass is no opaque cube"
    );
    assert_eq!(
        bits(first_flux(None, &[(1, "ntm:rbmk_console[facing=north]")])),
        bits(open),
        "nor is the console"
    );
    assert_eq!(
        bits(first_flux(None, &[(1, "ntm:rbmk_gauge[facing=north]")])),
        bits(open),
        "nor a panel"
    );
    // A column standing below the layer, its dummies through it: as its own block.
    assert_eq!(
        bits(first_flux(
            None,
            &[
                (-1, &column("rbmk_rod")),
                (0, persist::DUMMY),
                (1, persist::DUMMY)
            ]
        )),
        bits(open),
        "a fuel channel's dummies"
    );
    let stone = first_flux(None, &[(1, "minecraft:stone")]);
    let blank = first_flux(
        None,
        &[
            (-1, &column("rbmk_blank")),
            (0, persist::DUMMY),
            (1, persist::DUMMY),
        ],
    );
    // 4 cells high: one opaque cube takes a quarter, two take half.
    assert_eq!(bits(stone), bits(open * 0.75), "one opaque cube of four");
    assert_eq!(bits(blank), bits(open * 0.5), "a blank's two dummies");
}

/// Every column is a node of the streams (`RBMKNeutronHandler.getNodes`): a storage column (`OTHER`) lets a stream by
/// as air does, a control rod at level 0 (as it is placed; T07 moves it) stops it.
#[test]
fn every_column_is_a_node_of_the_streams() {
    let open = first_flux(None, &[]);
    assert_eq!(
        first_flux(Some("rbmk_storage"), &[]).to_bits(),
        open.to_bits(),
        "OTHER passes"
    );
    assert_eq!(
        first_flux(Some("rbmk_boiler"), &[]).to_bits(),
        open.to_bits(),
        "a boiler too"
    );
    assert_eq!(
        first_flux(Some("rbmk_control"), &[]),
        0.0,
        "a control rod at level 0 stops it"
    );
    assert_eq!(
        first_flux(Some("rbmk_control_auto"), &[]),
        0.0,
        "an automatic one too"
    );
}

/// Every column is a neighbour in the heat (`moveHeat`): a hot blank warms the boiler, cooler, storage and control rod
/// beside it, which the step before T04 left out (they were inert).
#[test]
fn every_column_shares_its_heat() {
    let mut g = game();
    g.place([0, Y, 0], &column("rbmk_blank"));
    let around = [
        ([1, Y, 0], "rbmk_boiler"),
        ([-1, Y, 0], "rbmk_cooler"),
        ([0, Y, 1], "rbmk_storage"),
        ([0, Y, -1], "rbmk_control_reasim"),
    ];
    for (p, id) in around {
        g.place(p, &column(id));
    }
    g.run(2);
    let Data::Rbmk(c) = &mut g
        .ready()
        .world
        .machines
        .get_mut(&(OW.to_string(), [0, Y, 0]))
        .unwrap()
        .data
    else {
        panic!()
    };
    c.heat = 900.0;
    g.run(1);
    for (p, id) in around {
        assert!(rbmk(&mut g, p).heat > 20.0, "{id} took heat");
    }
    assert!(rbmk(&mut g, [0, Y, 0]).heat < 900.0);
}

#[test]
fn the_first_dial_read_is_javas() {
    use super::rbmk_columns::first_dial;
    assert_eq!(
        first_dial("ntm:rbmk_rod_reasim_mod", true),
        "KEY_DISABLE_XENON"
    );
    assert_eq!(first_dial("ntm:rbmk_rod", false), "KEY_REASIM_BOILERS");
    for b in [
        "ntm:rbmk_boiler",
        "ntm:rbmk_heater",
        "ntm:rbmk_cooler",
        "ntm:rbmk_outgasser",
        "ntm:rbmk_control_reasim",
        "ntm:rbmk_control_reasim_auto",
    ] {
        assert_eq!(first_dial(b, false), "KEY_COLUMN_HEIGHT", "{b}");
    }
    for b in [
        "ntm:rbmk_blank",
        "ntm:rbmk_storage",
        "ntm:rbmk_control",
        "ntm:rbmk_control_auto",
        "ntm:rbmk_absorber",
    ] {
        assert_eq!(first_dial(b, false), "KEY_REASIM_BOILERS", "{b}");
    }
}

/// 3A in the server: `/ntmrbmk dial` (Java's `/gamerule` + `refresh(sender's world)`) refreshes the dimension of who
/// typed it; a column placed in a dimension without a cache reads the height first (the rule back to its default); after
/// a restart only the overworld has a cache, and a dimension's first step reads as its first column does.
#[test]
fn the_dials_are_a_cache_per_dimension() {
    let mut g = game();
    assert!(command(&mut g, OW, "ntmrbmk dial dialFluxRange 9").is_ok());
    assert!(command(&mut g, NETHER, "ntmrbmk dial dialDisableXenon true").is_ok());
    let (ow, ne) = (
        *g.ready().rbmk_dials.cached(OW).unwrap(),
        *g.ready().rbmk_dials.cached(NETHER).unwrap(),
    );
    assert_eq!(
        (ow.flux_range, ow.xenon_disabled),
        (9, false),
        "the overworld keeps its cache"
    );
    assert_eq!(
        (ne.flux_range, ne.xenon_disabled),
        (9, true),
        "the Nether's from the rules"
    );
    // A column placed in the End: its first read is the height, written back to 4.
    assert!(command(&mut g, OW, "ntmrbmk dial dialColumnHeight 6 confirmar").is_ok());
    let end = "minecraft:the_end";
    place_in(&mut g, end, [0, Y, 0], &column("rbmk_blank"));
    assert_eq!(
        g.ready().world.rbmk_rules.string("dialColumnHeight"),
        "4",
        "written back to its default"
    );
    assert_eq!(g.ready().rbmk_dials.cached(end).unwrap().column_height, 3);
    assert_eq!(
        g.ready().rbmk_dials.cached(OW).unwrap().column_height,
        5,
        "the overworld's cache keeps 6 until refreshed"
    );

    // A fuelled channel and a blank in the Nether; the rules then changed; the world reopened.
    let mut g = game();
    place_in(&mut g, NETHER, [0, Y, 0], &column("rbmk_rod"));
    place_in(&mut g, NETHER, [5, Y, 40], &column("rbmk_blank"));
    g.run(2);
    fuel_in(&mut g, NETHER, [0, Y, 0], "ntm:rbmk_fuel_ueu");
    g.run(2);
    assert!(command(&mut g, OW, "ntmrbmk dial dialDisableXenon true").is_ok());
    assert!(command(&mut g, OW, "ntmrbmk dial dialReasimBoilers true").is_ok());
    g.run(1);
    let world = g.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    assert!(
        reopened.ready().rbmk_dials.cached(NETHER).is_none(),
        "only the overworld's cache at the start"
    );
    reopened.run(1);
    // Two reactors in the Nether: the fuelled channel registered first steps first and reads `getXenon` first.
    assert_eq!(
        reopened.ready().world.rbmk_rules.string("dialDisableXenon"),
        "false",
        "the first read's rule back to its default"
    );
    assert_eq!(
        reopened
            .ready()
            .world
            .rbmk_rules
            .string("dialReasimBoilers"),
        "true",
        "the others as they were"
    );
    let ne = *reopened.ready().rbmk_dials.cached(NETHER).unwrap();
    assert!(!ne.xenon_disabled && ne.reasim_boilers);
}

/// 4A: a fuel channel with flux reports `INFO_NUCLEAR` for 300 ticks every 200 (`TileEntityRBMKRod:124`), kept by
/// dimension in the world's state with the step; a second report of the same place replaces the first where it stands;
/// `updateSystem` takes it away once the time is past it, on the ticks 10 past a multiple of 20; it is saved and read
/// back.
#[test]
fn satellite_events_are_kept_by_dimension_with_their_lifetime() {
    let mut g = game();
    g.place([0, Y, 0], &column("rbmk_rod"));
    g.place([1, Y, 0], &column("rbmk_rod"));
    g.run(2);
    fuel_in(&mut g, OW, [0, Y, 0], "ntm:rbmk_fuel_po210be");
    fuel_in(&mut g, OW, [1, Y, 0], "ntm:rbmk_fuel_po210be");
    let rays = |g: &mut Game| {
        g.ready()
            .world
            .rbmk_rays
            .get(OW)
            .cloned()
            .unwrap_or_default()
    };
    while rays(&mut g).len() < 2 {
        assert!(g.ready().world.ticks < 450, "two events by now");
        g.run(1);
    }
    let first = rays(&mut g);
    let at = first[0].expires_on - 300;
    assert_eq!(at % 200, 0, "reported on a tick multiple of 200");
    assert_eq!(
        first
            .iter()
            .map(|e| (e.pos, e.info.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ([0, Y, 0], persist::INFO_NUCLEAR),
            ([1, Y, 0], persist::INFO_NUCLEAR)
        ]
    );
    // Saved and read back.
    g.run(1);
    let world = g.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    assert_eq!(rays(&mut reopened), first);
    // The next report, 200 ticks on, replaces each in place.
    while rays(&mut g)[0].expires_on == first[0].expires_on {
        g.run(1);
    }
    let next = rays(&mut g);
    assert_eq!(next.len(), 2);
    assert_eq!(next[0].expires_on, first[0].expires_on + 200);
    // Expiry: an event of the past goes on the next tick 10 past a multiple of 20, not before.
    let now = g.ready().world.ticks as i64;
    let mut list = g.ready().world.rbmk_rays.get(OW).cloned().unwrap();
    list.push(persist::RayEvent {
        pos: [9, 9, 9],
        info: "RADIO_WAVES".into(),
        expires_on: now - 1,
    });
    g.ready().world.rbmk_rays.insert(OW.into(), list);
    let mut gone_at = None;
    for _ in 0..25 {
        let t = g.ready().world.ticks;
        g.run(1);
        if gone_at.is_none() && !rays(&mut g).iter().any(|e| e.pos == [9, 9, 9]) {
            gone_at = Some(t);
        }
    }
    assert_eq!(gone_at.map(|t| t % 20), Some(10));
}

/// D2: a rod whose burn gives NaN makes NaN increments of radiation, which Java would store and spread; here they add
/// nothing, and the field's storage reads back.
#[test]
fn a_nan_increment_never_reaches_the_field() {
    let mut g = game();
    g.place([0, Y, 0], &column("rbmk_rod"));
    g.run(2);
    fuel_in(&mut g, OW, [0, Y, 0], "ntm:rbmk_fuel_po210be");
    g.server.native_chunk_radiation_loaded(OW, [0, 0]);
    let key = (OW.to_string(), [0, Y, 0]);
    let mut s = Stack::new("ntm:rbmk_fuel_po210be", 1);
    s.custom = Some("{core:NaNd,hull:NaNd,xenon:0.0d,yield:NaNd}".into());
    g.world.borrow_mut().engine.get_mut(&key).unwrap().0[0] = Some(s);
    g.run(3);
    let tag = g.world.borrow().engine[&key].0[0]
        .clone()
        .unwrap()
        .custom
        .unwrap();
    assert!(tag.contains("yield:NaNd"), "the rod burns NaN: {tag}");
    for _ in 0..40 {
        g.run(1);
        let v = g.server.native_chunk_radiation_raw(OW, [0, 0]);
        assert!(v.is_none_or(|v| !v.is_nan()), "{v:?}");
    }
}

/// D2: a layer in the save that points at a component it does not hold, and streams of a column it does not know, are
/// read without failing: the reactor steps on.
#[test]
fn a_broken_layer_in_the_save_does_not_stop_the_reactor() {
    let mut g = game();
    g.place([0, Y, 0], &column("rbmk_rod"));
    g.place([1, Y, 0], &column("rbmk_blank"));
    g.run(3);
    // The layer as a save from elsewhere could hold it, written straight into the storage.
    let mut l = g.ready().world.rbmk[&(OW.to_string(), Y)].clone();
    for m in l.members.values_mut() {
        m.comp = [77, 77];
    }
    let mut c = persist::CompState::fresh(OW, Y, [0, 0]);
    c.streams = vec![
        persist::KeptStream {
            origin: [50, 50],
            bits: [f64::NAN.to_bits(); 4],
        },
        persist::KeptStream {
            origin: [0, 0],
            bits: [f64::NAN.to_bits(); 4],
        },
    ];
    l.comps.insert([0, 0], c);
    let bytes = serde_json::to_vec(&vec![((OW.to_string(), Y), l)]).unwrap();
    g.world
        .borrow_mut()
        .kv
        .set(crate::persist::RBMK_KEY, &bytes)
        .unwrap();
    let world = g.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    let before = rbmk(&mut reopened, [1, Y, 0]).heat;
    let Data::Rbmk(c) = &mut reopened
        .ready()
        .world
        .machines
        .get_mut(&(OW.to_string(), [0, Y, 0]))
        .unwrap()
        .data
    else {
        panic!()
    };
    c.heat = 700.0;
    reopened.run(3);
    assert_ne!(rbmk(&mut reopened, [1, Y, 0]).heat, before, "it steps");
}

/// A small reactor: a Po-210/Be source, a uranium channel and a moderator between them.
fn small_reactor() -> Game {
    let mut g = game();
    for (p, id) in [
        ([0, Y, 0], "rbmk_rod"),
        ([1, Y, 0], "rbmk_moderator"),
        ([2, Y, 0], "rbmk_rod"),
    ] {
        g.place(p, &column(id));
    }
    g.run(2);
    fuel_in(&mut g, OW, [0, Y, 0], "ntm:rbmk_fuel_po210be");
    fuel_in(&mut g, OW, [2, Y, 0], "ntm:rbmk_fuel_ueu");
    g
}

/// What a reopening must keep step for step: the columns' states and the layer (random and streams).
fn stepped(g: &mut Game) -> (Vec<String>, Option<crate::rbmk::persist::Layer>) {
    let cols = [[0, Y, 0], [1, Y, 0], [2, Y, 0]]
        .iter()
        .map(|p| serde_json::to_string(&rbmk(g, *p)).unwrap())
        .collect();
    (
        cols,
        g.ready().world.rbmk.get(&(OW.to_string(), Y)).cloned(),
    )
}

/// The engine ticks a chunk's block entities from a tick after it loads, and a machine used to be adopted at its block
/// entity's first tick only: the first tick after a world opened ran the clock with the reactor waiting (D4), a step
/// short at every opening (measured in game, T04). Adopted as its chunk loads (`native_chunk_machines_loaded`, the
/// glue's chunk load), the reopened world steps on exactly as one never closed, the clock with it.
#[test]
fn machines_adopted_as_their_chunk_loads_lose_no_step_at_an_opening() {
    let mut once = small_reactor();
    let mut twice = small_reactor();
    once.run(40);
    twice.run(17);
    let world = twice.world.borrow().clone();
    // As the glue: each chunk's machines when the chunk loads, before the first tick.
    let (mut opened, saved) = Game::unadopted(Rc::new(RefCell::new(world.clone())));
    let mut by_dim: BTreeMap<String, Vec<(Pos, String)>> = BTreeMap::new();
    for ((dim, p), json) in saved.clone() {
        by_dim.entry(dim).or_default().push((p, json));
    }
    for (dim, machines) in &by_dim {
        assert_eq!(
            opened.server.native_chunk_machines_loaded(dim, machines),
            machines.len()
        );
        assert_eq!(
            opened.server.native_chunk_machines_loaded(dim, machines),
            0,
            "a machine known is left"
        );
    }
    let start = opened.ready().world.ticks;
    opened.run(23);
    assert_eq!(opened.ready().world.ticks - start, 23);
    assert_eq!(
        stepped(&mut opened),
        stepped(&mut once),
        "23 ticks, 23 steps: columns, random and streams"
    );
    // As before: adopted at the block entities' first tick, a tick after the opening: one step short of the clock.
    let (mut late, saved) = Game::unadopted(Rc::new(RefCell::new(world)));
    late.run(1);
    for ((dim, p), json) in saved {
        assert!(late.server.native_adopt(&dim, p, &json));
    }
    late.run(22);
    assert_ne!(
        stepped(&mut late),
        stepped(&mut once),
        "23 ticks, a step short"
    );
}

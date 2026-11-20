//! T08: the boiler, heater and cooler in the step as their TEs: the tanks their `writeToNBT` saves, what the Java keeps
//! in memory only (the boiler's `ventDelay`, the cooler's `timer` and 5×5 cache), the heater's fluid identifier, and
//! the cooler's reach over columns it does not touch.
use super::*;
use crate::rbmk::persist::{KeptTank, PipedState, RbmkColumn};
use crate::server::container_bridge::Stack;

const Y: i32 = 71;

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn col(g: &mut Game, p: Pos) -> RbmkColumn {
    let Data::Rbmk(c) = g.ready().world.machines[&(OW.to_string(), p)].data.clone() else { panic!("not a column") };
    *c
}

fn with(g: &mut Game, p: Pos, f: impl FnOnce(&mut RbmkColumn)) {
    let Data::Rbmk(c) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), p)).unwrap().data else { panic!("not a column") };
    f(c);
}

fn piped(g: &mut Game, p: Pos) -> PipedState {
    col(g, p).piped.expect("a piped column")
}

fn tank(fluid: &str, fill: i32, max: i32) -> KeptTank {
    KeptTank { fluid: fluid.into(), fill, max }
}

fn reopen(g: &Game) -> Game {
    let world = g.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    reopened.run(1);
    reopened
}

/// A full steam tank vents once, then waits its `ventDelay` (20 + rand(10)) a tick at a time: the delay lives from
/// tick to tick, as the TE's field. The tanks are saved; the delay is not (a reopened boiler starts at 0).
#[test]
fn a_boiler_boils_keeps_its_tanks_and_waits_its_vent_delay_in_memory() {
    let mut g = game();
    let at = [0, Y, 0];
    g.place(at, &column("rbmk_boiler"));
    g.run(2);
    assert_eq!(piped(&mut g, at), PipedState::of("ntm:rbmk_boiler").unwrap(), "a fresh boiler: dry, water and steam");
    let feed = |g: &mut Game| {
        with(g, at, |c| {
            c.heat = 1000.0;
            let st = c.piped.as_mut().unwrap();
            (st.input, st.output.fill) = (tank("WATER", 10_000, 10_000), 999_990);
        })
    };
    feed(&mut g);
    g.world.borrow_mut().shown.clear();
    g.run(1);
    let st = piped(&mut g, at);
    assert_eq!(st.output, tank("STEAM", 1_000_000, 1_000_000), "the steam fills the tank up");
    let vents = |g: &Game| {
        g.world.borrow().shown.iter().filter_map(|(_, s)| match s {
            crate::blast::visuals::Show::Sound { id: "ntm:block.steamengineoperate", at, volume, pitch, .. } => Some((*at, *volume, *pitch)),
            _ => None,
        }).collect::<Vec<_>>()
    };
    let v = vents(&g);
    assert_eq!(v.len(), 1, "one vent: {v:?}");
    assert_eq!((v[0].0, v[0].1), ([0.0, f64::from(Y + 3), 0.0], 2.0), "at the core's corner, the column's height up");
    assert!((1.0..1.25).contains(&v[0].2), "{v:?}");
    assert!(st.consumption > 0 && st.produced > 0, "{st:?}");
    let first = st.vent_delay;
    assert!((20..30).contains(&first), "a vent sets the delay: {first}");
    for k in 1..=5 {
        feed(&mut g);
        g.run(1);
        assert_eq!(piped(&mut g, at).vent_delay, first - k, "the delay counts down from tick to tick");
    }
    assert_eq!(vents(&g).len(), 1, "no vent while the delay runs");
    // The save keeps the tanks and not the delay.
    let mut back = reopen(&g);
    let st = piped(&mut back, at);
    assert_eq!(st.input.fluid, "WATER");
    assert_eq!(st.output.fluid, "STEAM");
    let saved = serde_json::to_value(col(&mut g, at)).unwrap();
    assert!(saved["piped"].get("vent_delay").is_none() && saved["piped"]["output"]["fill"] == 1_000_000, "{saved}");
}

/// The cooler cools every column of its 5×5 at its own Y, also one it does not touch (a reactor of its own without
/// the cooler): they step together. A column put in a cell after the cache was made is not cooled until the next
/// refresh (60 ticks after), as the cache holds what was there.
#[test]
fn a_cooler_reaches_its_5x5_and_cools_only_what_its_cache_holds() {
    let mut g = game();
    let (cooler, far, late) = ([0, Y, 0], [2, Y, 2], [-2, Y, 1]);
    g.place(cooler, &column("rbmk_cooler"));
    g.place(far, &column("rbmk_blank"));
    g.run(2);
    let charge = |g: &mut Game, heat: &[Pos]| {
        with(g, cooler, |c| {
            let st = c.piped.as_mut().unwrap();
            (st.input.fill, st.output.fill) = (1_000, 0);
        });
        for p in heat {
            with(g, *p, |c| c.heat = 900.0);
        }
    };
    charge(&mut g, &[far]);
    g.run(1);
    let after = col(&mut g, far).heat;
    assert!(after < 900.0 - 199.0, "the far blank is cooled by 200: {after}");
    assert_eq!(piped(&mut g, cooler).input.fill, 950, "50 mB of cold per cooling");
    // A column put down now is not in the cache.
    g.place(late, &column("rbmk_blank"));
    g.run(1);
    let mut cooled_at = None;
    for t in 0..70 {
        charge(&mut g, &[far, late]);
        g.run(1);
        let h = col(&mut g, late).heat;
        if h < 900.0 - 199.0 {
            cooled_at = Some(t);
            break;
        }
        assert!(col(&mut g, far).heat < 900.0 - 199.0, "the cached blank is cooled every tick");
    }
    let t = cooled_at.expect("the refresh takes the new column in");
    assert!(t > 50, "not before the cache is refreshed: {t}");
    // The cached column replaced by another in its place: the cache holds the old one (a dead TE in Java), so the new
    // one is not cooled either, until the next refresh.
    g.break_block(far);
    g.place(far, &column("rbmk_blank"));
    g.run(2);
    for _ in 0..10 {
        charge(&mut g, &[far, late]);
        g.run(1);
        assert!(col(&mut g, far).heat > 900.0 - 199.0, "the replaced column is not the cached one");
        assert!(col(&mut g, late).heat < 900.0 - 199.0, "the cached one still is cooled");
    }
}

/// The heater's slot types its feed with a fluid identifier while it is cool or dry (`feed.setType(0, slots)`), and a
/// new type empties the tank; the tanks are saved.
#[test]
fn a_heaters_identifier_types_its_feed() {
    let mut g = game();
    let at = [0, Y, 0];
    g.place(at, &column("rbmk_heater"));
    g.run(2);
    with(&mut g, at, |c| c.piped.as_mut().unwrap().input = tank("COOLANT", 500, 16_000));
    let mut id = Stack::new(native_server::IDENTIFIER, 1);
    id.custom = Some("{ntm_fluid:\"oil\"}".into());
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[0] = Some(id);
    g.run(2);
    let st = piped(&mut g, at);
    assert_eq!((st.input.clone(), st.output.fluid.as_str()), (tank("OIL", 0, 16_000), "HOTOIL"), "{st:?}");
    let mut back = reopen(&g);
    assert_eq!(piped(&mut back, at).input, tank("OIL", 0, 16_000));
}

/// A boiler from a save before T08 (no tanks kept) reads as a fresh one.
#[test]
fn a_column_saved_before_t08_reads_as_a_fresh_te() {
    let mut g = game();
    let at = [0, Y, 0];
    g.place(at, &column("rbmk_boiler"));
    g.run(2);
    let mut v = serde_json::to_value(col(&mut g, at)).unwrap();
    v.as_object_mut().unwrap().remove("piped");
    let old: RbmkColumn = serde_json::from_value(v).unwrap();
    assert!(old.piped.is_none());
    with(&mut g, at, |c| *c = old);
    g.run(1);
    assert_eq!(piped(&mut g, at).input, tank("WATER", 0, 10_000));
}

fn barrel(g: &mut Game, at: Pos) -> crate::fluid::Tank {
    match g.machine(at).data {
        Data::FluidStore(s) => s.tank,
        other => panic!("{other:?}"),
    }
}

fn set_barrel(g: &mut Game, at: Pos, f: impl FnOnce(&mut crate::fluidbox::FluidStore)) {
    if let Some(Data::FluidStore(s)) = g.ready().world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
        f(s);
    }
}

fn duct(g: &mut Game, at: Pos, fluid: &str) {
    g.place(at, "ntm:fluid_duct_neo");
    g.run(1);
    g.type_duct(at, fluid);
}

/// The boiler's ports without a loader (`getPortsNoLoader`): water in by a duct under its core, steam out by one over
/// its top (`y + getColumnHeight`, the dial's 4 tall); a duct at its side reaches nothing.
#[test]
fn a_boiler_takes_water_below_and_gives_steam_above() {
    let mut g = game();
    let at = [0, Y, 0];
    g.place(at, &column("rbmk_boiler"));
    let (water, steam, side) = ([1, Y - 1, 0], [1, Y + 4, 0], [2, Y, 0]);
    for b in [water, steam, side] {
        g.place(b, "ntm:barrel_steel[facing=north,honey_level=0]");
    }
    g.run(2);
    duct(&mut g, [0, Y - 1, 0], "water");
    duct(&mut g, [0, Y + 4, 0], "steam");
    duct(&mut g, [1, Y, 0], "water");
    set_barrel(&mut g, water, |s| {
        s.tank.fill("water", 6_000);
        s.mode = crate::fluidbox::Mode::Output;
    });
    set_barrel(&mut g, side, |s| {
        s.tank.fill("water", 6_000);
        s.mode = crate::fluidbox::Mode::Output;
    });
    g.run(12);
    assert_eq!(piped(&mut g, at).input.fill, 6_000, "the water below came in");
    assert_eq!(barrel(&mut g, side).mb, 6_000, "the side is no port");
    // Hot: it boils, and the steam leaves by the top.
    for _ in 0..3 {
        with(&mut g, at, |c| c.heat = 400.0);
        g.run(1);
    }
    g.run(1);
    let (st, out) = (piped(&mut g, at), barrel(&mut g, steam));
    assert!(out.mb > 0 && out.fluid == "steam", "{out:?}");
    assert_eq!((6_000 - st.input.fill) * 100, out.mb as i32 + st.output.fill, "100 mB of steam a mB of water, all of it kept or sent: {st:?} {out:?}");
}

/// The column's ports are one passthrough group: water put in at the bottom reaches a barrel at the top, sharing with
/// the boiler (as `PortDef.combine` makes one node of all its positions).
#[test]
fn a_columns_ports_pass_fluid_through() {
    let mut g = game();
    let at = [0, Y, 0];
    g.place(at, &column("rbmk_boiler"));
    let (src, top) = ([1, Y - 1, 0], [1, Y + 4, 0]);
    g.place(src, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.place(top, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    duct(&mut g, [0, Y - 1, 0], "water");
    duct(&mut g, [0, Y + 4, 0], "water");
    set_barrel(&mut g, src, |s| {
        s.tank.fill("water", 12_000);
        s.mode = crate::fluidbox::Mode::Output;
    });
    g.run(12);
    let (boiler, up) = (piped(&mut g, at).input.fill, barrel(&mut g, top).mb);
    assert!(boiler > 0 && up > 0, "the barrel over the top gets water through the column, with the boiler: {boiler} {up}");
    assert_eq!(barrel(&mut g, src).mb + boiler as u32 + up, 12_000);
}

/// A loader one below (`getPortsCloseLoader`): the port is the loader's cell, open all round, the core's bottom no
/// more; two below (`getPortsClassicLoader`): the core's bottom and the loader's cell.
#[test]
fn a_loader_moves_the_bottom_port() {
    for (loader_at, side_duct) in [([0, Y - 1, 0], [1, Y - 1, 0]), ([0, Y - 2, 0], [1, Y - 2, 0])] {
        let mut g = game();
        let at = [0, Y, 0];
        g.place(at, &column("rbmk_boiler"));
        g.place(loader_at, "ntm:rbmk_loader[facing=north,honey_level=0]");
        let src = [side_duct[0] + 1, side_duct[1], side_duct[2]];
        // A duct at the core's side: never a port.
        let beside = [-2, Y, 0];
        g.place(src, "ntm:barrel_steel[facing=north,honey_level=0]");
        g.place(beside, "ntm:barrel_steel[facing=north,honey_level=0]");
        g.run(2);
        duct(&mut g, side_duct, "water");
        duct(&mut g, [-1, Y, 0], "water");
        for b in [src, beside] {
            set_barrel(&mut g, b, |s| {
                s.tank.fill("water", 3_000);
                s.mode = crate::fluidbox::Mode::Output;
            });
        }
        g.run(12);
        assert_eq!(piped(&mut g, at).input.fill, 3_000, "by the loader at {loader_at:?}");
        assert_eq!(barrel(&mut g, beside).mb, 3_000, "the core's side is closed with the loader at {loader_at:?}");
        // The loader taken away: once the networks are checked again (every 10 ticks), the side duct reaches nothing.
        g.break_block(loader_at);
        g.run(12);
        set_barrel(&mut g, src, |s| {
            s.tank.fill("water", 3_000);
        });
        g.run(12);
        assert_eq!((piped(&mut g, at).input.fill, barrel(&mut g, src).mb), (3_000, 3_000), "no loader at {loader_at:?}");
    }
}

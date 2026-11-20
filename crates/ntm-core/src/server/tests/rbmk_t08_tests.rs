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

fn tank(fluid: &str, fill: i32) -> KeptTank {
    KeptTank { fluid: fluid.into(), fill }
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
            (st.input, st.output.fill) = (tank("WATER", 10_000), 999_990);
        })
    };
    feed(&mut g);
    g.run(1);
    let st = piped(&mut g, at);
    assert_eq!(st.output, tank("STEAM", 1_000_000), "the steam fills the tank up");
    assert!(st.consumption > 0 && st.produced > 0, "{st:?}");
    let first = st.vent_delay;
    assert!((20..30).contains(&first), "a vent sets the delay: {first}");
    for k in 1..=5 {
        feed(&mut g);
        g.run(1);
        assert_eq!(piped(&mut g, at).vent_delay, first - k, "the delay counts down from tick to tick");
    }
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
    with(&mut g, at, |c| c.piped.as_mut().unwrap().input = tank("COOLANT", 500));
    let mut id = Stack::new(native_server::IDENTIFIER, 1);
    id.custom = Some("{ntm_fluid:\"oil\"}".into());
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[0] = Some(id);
    g.run(2);
    let st = piped(&mut g, at);
    assert_eq!((st.input.clone(), st.output.fluid.as_str()), (tank("OIL", 0), "HOTOIL"), "{st:?}");
    let mut back = reopen(&g);
    assert_eq!(piped(&mut back, at).input, tank("OIL", 0));
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
    assert_eq!(piped(&mut g, at).input, tank("WATER", 0));
}

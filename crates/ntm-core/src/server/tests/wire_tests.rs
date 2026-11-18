//! Integration tests for the wire endpoints: `TileEntityPylonBase.java` and its subclasses, `ItemWiring.java`.
use super::*;
use crate::machine::BatteryMode;
use crate::wire::WireUse;

fn power(g: &mut Game, at: Pos) -> u64 {
    let Data::Battery(b) = g.machine(at).data else { panic!() };
    b.power
}

fn wire(g: &mut Game, at: Pos) -> crate::wire::Wire {
    let Data::Wire(w) = g.machine(at).data else { panic!("no wire at {at:?}") };
    w
}

fn battery(g: &mut Game, at: Pos, mode: BatteryMode, charge: u64) {
    let r = g.ready();
    let Data::Battery(b) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else { panic!() };
    b.mode = mode;
    b.power = charge;
}

/// The two clicks of the cable drum, `start` being what the item remembers after the first.
fn link(g: &mut Game, a: Pos, b: Pos) -> (WireUse, WireUse) {
    let first = g.server.native_wire_use(&mut g.sh, OW, a, None, "p");
    let start = if let WireUse::Started(at) = first { Some(at) } else { None };
    let second = g.server.native_wire_use(&mut g.sh, OW, b, start, "p");
    (first, second)
}

/// source battery - pylon 1 ... pylon 2 - sink battery: the wire carries the energy across.
fn two_pylons(g: &mut Game, block: &str, p1: Pos, p2: Pos) -> (Pos, Pos) {
    let (src, sink) = ([p1[0] - 1, p1[1], p1[2]], [p2[0] + 1, p2[1], p2[2]]);
    g.place(p1, &format!("{block}[facing=north,honey_level=0]"));
    g.place(p2, &format!("{block}[facing=north,honey_level=0]"));
    g.place(src, "ntm:machine_battery[facing=north]");
    g.place(sink, "ntm:machine_battery[facing=north]");
    g.run(2);
    battery(g, src, BatteryMode::Output, 1_000_000);
    battery(g, sink, BatteryMode::Input, 0);
    (src, sink)
}

#[test]
fn a_wire_between_two_pylons_carries_the_power_and_only_after_the_second_click() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    let (src, sink) = two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    g.run(20);
    assert_eq!(power(&mut g, sink), 0, "no wire, no power");

    let (first, second) = link(&mut g, p1, p2);
    assert_eq!(first, WireUse::Started(p1));
    assert_eq!(second, WireUse::Finished("Wire end"));
    assert_eq!(g.sh.told, vec!["Wire start".to_string(), "Wire end".to_string()]);
    assert_eq!((wire(&mut g, p1).connected, wire(&mut g, p2).connected), (vec![p2], vec![p1]));

    g.run(30);
    let gained = power(&mut g, sink);
    assert!(gained > 0, "the wire carries it: {gained}");
    assert_eq!(1_000_000 - power(&mut g, src), gained, "and loses none");
}

#[test]
fn a_wire_that_cannot_be_made_says_why_and_leaves_nothing() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    let (_src, sink) = two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    let far = [60, 70, 0];
    g.place(far, "ntm:red_pylon[facing=north,honey_level=0]");
    g.place([20, 70, 10], "ntm:red_pylon_medium_wood[facing=north,honey_level=0]");
    g.run(2);
    let cases = [
        (p1, p1, "Wire error - Cannot connect to the same pylon"),
        (p1, far, "Wire error - Pylon is too far away"),
        (p1, [20, 70, 10], "Wire error - Pylons are not the same type"),
    ];
    for (a, b, text) in cases {
        let (first, second) = link(&mut g, a, b);
        assert_eq!(first, WireUse::Started(a));
        assert_eq!(second, WireUse::Finished(text), "{a:?} -> {b:?}");
        assert_eq!(g.sh.told.last().map(String::as_str), Some(text));
    }
    assert!(wire(&mut g, p1).connected.is_empty() && wire(&mut g, far).connected.is_empty());
    g.run(30);
    assert_eq!(power(&mut g, sink), 0);
    // The second click on something that is no endpoint any more is "Wire error".
    let gone = g.server.native_wire_use(&mut g.sh, OW, p2, Some([99, 70, 99]), "p");
    assert_eq!(gone, WireUse::Finished("Wire error"));
    // A block that is no endpoint does not take the item.
    assert_eq!(g.server.native_wire_use(&mut g.sh, OW, sink, None, "p"), WireUse::NotTaken);
}

#[test]
fn the_ranges_are_the_shorter_of_the_two_and_the_connector_has_the_least() {
    let mut g = game();
    // pylon 25, connector 10: two connectors 11 apart do not link, 9 apart do.
    g.place([0, 70, 0], "ntm:red_connector[facing=north,honey_level=0]");
    g.place([9, 70, 0], "ntm:red_connector[facing=north,honey_level=0]");
    g.place([11, 70, 5], "ntm:red_connector[facing=north,honey_level=0]");
    g.run(2);
    assert_eq!(link(&mut g, [0, 70, 0], [9, 70, 0]).1, WireUse::Finished("Wire end"));
    assert_eq!(link(&mut g, [0, 70, 0], [11, 70, 5]).1, WireUse::Finished("Wire error - Pylon is too far away"));
    // Two super connectors reach 100.
    g.place([0, 72, 0], "ntm:red_connector_super[facing=north,honey_level=0]");
    g.place([90, 72, 0], "ntm:red_connector_super[facing=north,honey_level=0]");
    g.run(2);
    assert_eq!(link(&mut g, [0, 72, 0], [90, 72, 0]).1, WireUse::Finished("Wire end"));
    // A repeat is kept, as `connected.add` does.
    assert_eq!(link(&mut g, [0, 72, 0], [90, 72, 0]).1, WireUse::Finished("Wire end"));
    assert_eq!(wire(&mut g, [0, 72, 0]).connected, vec![[90, 72, 0], [90, 72, 0]]);
}

#[test]
fn breaking_an_endpoint_takes_its_wires_off_the_other_ends() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    let (_src, sink) = two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    link(&mut g, p1, p2);
    g.run(30);
    let gained = power(&mut g, sink);
    assert!(gained > 0);
    g.break_block(p1);
    g.run(4);
    assert!(wire(&mut g, p2).connected.is_empty(), "disconnectAll {:?}", wire(&mut g, p2));
    g.place(p1, "ntm:red_pylon[facing=north,honey_level=0]");
    g.run(30);
    assert_eq!(power(&mut g, sink), gained, "a new pylon in the old place is not wired");
}

/// `UniNodespace.checkConnection`: the node of every endpoint has its own point among its connections, so a wire listed
/// by ONE end already joins the two (`TileEntityPylonBase.createNode`). Both ends list each other in play; the
/// topology does not need it.
#[test]
fn a_wire_one_end_lists_joins_the_two_as_the_java_does() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    let (_src, _sink) = two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    let nets = |g: &mut Game| crate::server::build_topology(&g.server.ready.world)[OW].networks.clone();
    assert!(nets(&mut g).iter().all(|n| !(n.contains(&p1) && n.contains(&p2))), "no wire, no net");
    {
        let r = g.ready();
        let Data::Wire(w) = &mut r.world.machines.get_mut(&(OW.to_string(), p1)).unwrap().data else { panic!() };
        w.add_connection(p2);
    }
    let joined = nets(&mut g);
    assert!(joined.iter().any(|n| n.contains(&p1) && n.contains(&p2)), "{joined:?}");
    // The same from the other end, whichever of the two the flood fill reaches first.
    for (list, forget) in [(p2, p1), (p1, p2)] {
        let r = g.ready();
        let Data::Wire(w) = &mut r.world.machines.get_mut(&(OW.to_string(), forget)).unwrap().data else { panic!() };
        w.forget(list);
        let Data::Wire(w) = &mut r.world.machines.get_mut(&(OW.to_string(), list)).unwrap().data else { panic!() };
        w.add_connection(forget);
        let joined = nets(&mut g);
        assert!(joined.iter().any(|n| n.contains(&p1) && n.contains(&p2)), "{list:?} lists {forget:?}: {joined:?}");
    }
}

/// The old endpoint's `disconnectAll` took its wires off the others as it went; a new one in its place lists nobody,
/// so the wires the others kept to that place are not the new one's.
#[test]
fn an_endpoint_replaced_in_place_does_not_inherit_the_old_wires() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    let (_src, sink) = two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    link(&mut g, p1, p2);
    g.run(30);
    let gained = power(&mut g, sink);
    assert!(gained > 0);
    // The block replaced by another pylon in the same step: it is a wire machine all the time.
    g.place(p1, "ntm:red_pylon_steel[facing=north,honey_level=0]");
    g.run(6);
    assert!(wire(&mut g, p2).connected.is_empty(), "{:?}", wire(&mut g, p2));
    let after = power(&mut g, sink);
    g.run(30);
    assert_eq!(power(&mut g, sink), after, "and nothing goes through any more");
}

#[test]
fn a_connector_takes_power_only_from_the_block_behind_it() {
    // facing north: the wall is to the south, so the back is +Z. Two connectors 9 apart (range 10).
    for (battery_at, powered) in [([5, 70, 1], true), ([5, 70, -1], false), ([4, 70, 0], false), ([6, 70, 0], false), ([5, 71, 0], false)] {
        let mut g = game();
        g.place([5, 70, 0], "ntm:red_connector[facing=north,honey_level=0]");
        g.place([14, 70, 0], "ntm:red_connector[facing=north,honey_level=0]");
        g.place(battery_at, "ntm:machine_battery[facing=north]");
        g.place([14, 70, 1], "ntm:machine_battery[facing=north]");
        g.run(2);
        battery(&mut g, battery_at, BatteryMode::Output, 1_000_000);
        battery(&mut g, [14, 70, 1], BatteryMode::Input, 0);
        assert_eq!(link(&mut g, [5, 70, 0], [14, 70, 0]).1, WireUse::Finished("Wire end"));
        g.run(30);
        assert_eq!(power(&mut g, [14, 70, 1]) > 0, powered, "battery at {battery_at:?}");
    }
}

#[test]
fn the_faces_of_each_endpoint_are_the_ones_create_node_lists() {
    use crate::energy_network::WireKind;
    let f = |k, facing| crate::wire::faces(k, Some(facing));
    assert_eq!(f(WireKind::Connector, "north"), vec![[0, 0, 1]]);
    assert_eq!(f(WireKind::ConnectorSuper, "east"), vec![[-1, 0, 0]]);
    assert_eq!(f(WireKind::Pylon, "north").len(), 4);
    assert!(f(WireKind::Pylon, "north").iter().all(|d| d[1] == 0));
    assert!(f(WireKind::MediumWood, "north").is_empty());
    assert_eq!(f(WireKind::MediumSteelTransformer, "north"), vec![[0, 0, 1]]);
    assert_eq!(f(WireKind::MediumWoodTransformer, "west"), vec![[1, 0, 0]]);
    assert!(f(WireKind::Large, "north").is_empty());
}

#[test]
fn all_nine_upgrade_from_inert_and_the_state_survives_a_save() {
    for k in crate::energy_network::WireKind::ALL {
        let m = Machine {
            generation: 0,
            rev: 0,
            audio: Vec::new(),
            block: k.id().into(),
            facing: Some("north".into()),
            data: Data::Inert { name: k.id().into() },
        };
        assert_eq!(m.upgraded().kind(), Kind::Wire, "{}", k.id());
    }
    let w = crate::wire::Wire { connected: vec![[1, 2, 3], [4, 5, 6]], color: 0x123456, ..Default::default() };
    let back: crate::wire::Wire = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    assert_eq!(back, w);
    assert_eq!(serde_json::from_str::<crate::wire::Wire>("{}").unwrap(), crate::wire::Wire::default());
}

/// A wire whose other end is not a machine yet (its chunk's machines are not adopted) but whose block is
/// still an endpoint keeps the link: only an end that is gone from a loaded cell comes off.
#[test]
fn a_link_to_a_block_that_is_still_an_endpoint_is_kept() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    link(&mut g, p1, p2);
    g.ready().world.machines.remove(&(OW.to_string(), p2));
    g.run(45);
    assert_eq!(wire(&mut g, p1).connected, vec![p2]);
}

/// A wire in a chunk that is not loaded is left alone, whatever the other end looks like.
#[test]
fn an_unloaded_wire_keeps_its_links() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    link(&mut g, p1, p2);
    g.world.borrow_mut().unloaded.insert((OW.to_string(), p1[0] >> 4, p1[2] >> 4));
    g.ready().world.machines.remove(&(OW.to_string(), p2));
    g.place(p2, "minecraft:air");
    g.run(45);
    assert_eq!(wire(&mut g, p1).connected, vec![p2]);
}

/// A save cut can leave one end with the wire and the other without it (the two block entities are written apart):
/// the Java joins them (`checkConnection`), and the tick's clean-up of replaced endpoints must not undo that.
#[test]
fn a_wire_only_one_end_has_goes_on_conducting_through_the_ticks() {
    let mut g = game();
    let (p1, p2) = ([10, 70, 0], [30, 70, 0]);
    let (src, sink) = two_pylons(&mut g, "ntm:red_pylon", p1, p2);
    link(&mut g, p1, p2);
    g.run(10);
    // The far end loses the wire (as a cut leaves it), the generation the near one wired is still the one there.
    {
        let r = g.ready();
        let Data::Wire(w) = &mut r.world.machines.get_mut(&(OW.to_string(), p2)).unwrap().data else { panic!() };
        w.forget(p1);
        r.topo_dirty = true;
    }
    g.run(2);
    assert_eq!((wire(&mut g, p1).connected, wire(&mut g, p2).connected), (vec![p2], Vec::<Pos>::new()), "kept");
    let before = power(&mut g, sink);
    g.run(30);
    let gained = power(&mut g, sink) - before;
    assert!(gained > 0, "and it conducts: {gained}");
    assert_eq!(before + gained, power(&mut g, sink));
    let _ = src;
    // The same with a wire whose generation is not known (a save from before the field).
    {
        let r = g.ready();
        let Data::Wire(w) = &mut r.world.machines.get_mut(&(OW.to_string(), p1)).unwrap().data else { panic!() };
        w.generations.clear();
    }
    g.run(30);
    assert_eq!(wire(&mut g, p1).connected, vec![p2], "still there");
}

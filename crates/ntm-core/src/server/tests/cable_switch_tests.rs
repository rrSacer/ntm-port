//! Integration tests for `ntm:cable_switch` and `ntm:cable_detector`
//! (`CableSwitch.java`, `CableDetector.java`, `TileEntityCableSwitch.java`).
use super::*;
use crate::machine::BatteryMode;

fn power_of(g: &mut Game, at: Pos) -> u64 {
    match g.machine(at).data {
        Data::Deuterium(d) => d.power,
        Data::Battery(b) => b.power,
        _ => panic!(),
    }
}

fn state(g: &mut Game, at: Pos) -> String {
    g.world.borrow().get(OW, at)
}

fn set_signal(g: &mut Game, at: Pos, s: u8) {
    g.world.borrow_mut().signal.insert((OW.to_string(), at), s);
}

/// battery [0] - cables [1, 2] - switch [3] - cables [4, 5] - extractor [6], along +X at y 70.
fn line(g: &mut Game, middle: &str) {
    let bat = [0, 70, 0];
    g.place(bat, "ntm:machine_battery[facing=north]");
    for x in [1, 2, 4, 5] {
        g.place([x, 70, 0], "ntm:red_cable");
    }
    g.place([3, 70, 0], &format!("{middle}[facing=north,honey_level=0]"));
    g.place([6, 70, 0], "ntm:machine_deuterium_extractor[facing=north,honey_level=0]");
    g.run(2);
    let r = g.ready();
    let Data::Battery(b) = &mut r.world.machines.get_mut(&(OW.to_string(), bat)).unwrap().data else { panic!() };
    b.mode = BatteryMode::Output;
    b.power = 1_000_000;
}

#[test]
fn a_power_switch_conducts_only_while_it_is_on() {
    let mut g = game();
    line(&mut g, "ntm:cable_switch");
    let sw = [3, 70, 0];
    let ex = [6, 70, 0];
    g.run(30);
    assert_eq!(power_of(&mut g, ex), 0, "off: nothing crosses the switch");
    assert!(state(&mut g, sw).contains("honey_level=0"));

    assert!(g.server.native_switch_use(OW, sw), "a click flips it");
    g.run(30);
    let on = power_of(&mut g, ex);
    assert!(on > 0, "on: the extractor charges ({on})");
    assert!(state(&mut g, sw).contains("honey_level=5"), "{}", state(&mut g, sw));
    assert!(matches!(g.machine(sw).data, Data::CableSwitch(ref s) if s.on && s.meta() == 1));

    assert!(g.server.native_switch_use(OW, sw), "a second click turns it off");
    g.run(4);
    let after = power_of(&mut g, ex);
    g.run(30);
    assert_eq!(power_of(&mut g, ex), after, "off again: the net is split and nothing more arrives");
    assert!(state(&mut g, sw).contains("honey_level=0"));
}

#[test]
fn the_redstone_switch_follows_the_signal_when_it_changes_and_takes_no_click() {
    let mut g = game();
    line(&mut g, "ntm:cable_detector");
    let det = [3, 70, 0];
    let ex = [6, 70, 0];
    g.run(4);
    assert!(!g.server.native_switch_use(OW, det), "the redstone switch has no click");
    g.run(20);
    assert_eq!(power_of(&mut g, ex), 0);

    set_signal(&mut g, det, 15);
    g.run(30);
    assert!(power_of(&mut g, ex) > 0, "powered: it conducts");
    assert!(state(&mut g, det).contains("honey_level=5"));

    set_signal(&mut g, det, 0);
    g.run(4);
    let off = power_of(&mut g, ex);
    g.run(30);
    assert_eq!(power_of(&mut g, ex), off, "unpowered: it is split again");
    assert!(state(&mut g, det).contains("honey_level=0"));
}

/// Put beside a powered block, the detector stays off until the signal changes
/// (`onNeighborBlockChange` is not called by the placement itself).
#[test]
fn a_detector_put_beside_a_powered_block_waits_for_a_change() {
    let mut g = game();
    let det = [3, 70, 0];
    set_signal(&mut g, det, 15);
    g.place(det, "ntm:cable_detector[facing=north,honey_level=0]");
    g.run(10);
    assert!(matches!(g.machine(det).data, Data::CableSwitch(ref s) if !s.on));
    set_signal(&mut g, det, 0);
    g.run(3);
    set_signal(&mut g, det, 15);
    g.run(3);
    assert!(matches!(g.machine(det).data, Data::CableSwitch(ref s) if s.on));
}

#[test]
fn both_upgrade_from_inert_and_a_broken_switch_leaves_no_machine() {
    for block in [crate::cable_switch::SWITCH, crate::cable_switch::DETECTOR] {
        let m = Machine {
            generation: 0,
            rev: 0,
            audio: Vec::new(),
            block: block.into(),
            facing: Some("north".into()),
            data: Data::Inert { name: block.into() },
        };
        let up = m.upgraded();
        assert_eq!(up.kind(), Kind::CableSwitch, "{block}");
        let Data::CableSwitch(s) = &up.data else { panic!() };
        assert_eq!((s.on, s.detector), (false, block == crate::cable_switch::DETECTOR));
        let spec = crate::registry::spec(block).unwrap();
        assert!(crate::registry::ports(spec, Some("north")).is_empty(), "no port: it is a cable, not a machine");
    }
    let mut g = game();
    line(&mut g, "ntm:cable_switch");
    assert!(g.server.native_switch_use(OW, [3, 70, 0]));
    g.run(10);
    g.break_block([3, 70, 0]);
    g.run(10);
    let ex = power_of(&mut g, [6, 70, 0]);
    g.run(20);
    assert_eq!(power_of(&mut g, [6, 70, 0]), ex, "no switch, no conduction");
    assert!(!g.server.native_has_machine(OW, [3, 70, 0]));
}

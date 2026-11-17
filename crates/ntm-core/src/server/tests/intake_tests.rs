//! Integration tests for `ntm:machine_intake` (`MachineIntake.java`,
//! `TileEntityMachineIntake.java`).
use super::*;
use crate::machine::BatteryMode;

fn intake(g: &mut Game, at: Pos) -> crate::intake::Intake {
    let Data::Intake(i) = g.machine(at).data else { panic!() };
    i
}

#[test]
fn intake_feeds_air_through_horizontal_proxy_ports_only() {
    let mut g = game();
    let core = [0, 70, 0];
    // facing=north (fwd=+Z, right=-X): proxies are [1,70,0], [0,70,1], [1,70,1].
    g.place(core, "ntm:machine_intake[facing=north,honey_level=0]");
    g.place([-1, 70, 0], "ntm:machine_battery[facing=north]");
    // Duct on proxy [1, 70, 1] via +X side ([2, 70, 1] -> [3, 70, 1] barrel).
    g.place([2, 70, 1], "ntm:fluid_duct_neo");
    g.place([3, 70, 1], "ntm:barrel_steel[facing=north]");
    // Vertical cable above proxy [1, 70, 0] fed sideways by a battery, vertical duct above core [0, 71, 0],
    // AND vertical duct above proxy [0, 70, 1] -> [0, 72, 1]: none of these must connect!
    let top_bat = [2, 71, 0];
    g.place([1, 71, 0], "ntm:red_cable");
    g.place(top_bat, "ntm:machine_battery[facing=north]");
    let top_barrel = [0, 72, 0];
    g.place([0, 71, 0], "ntm:fluid_duct_neo");
    g.place(top_barrel, "ntm:barrel_steel[facing=north]");
    let proxy_top_barrel = [0, 72, 1];
    g.place([0, 71, 1], "ntm:fluid_duct_neo");
    g.place(proxy_top_barrel, "ntm:barrel_steel[facing=north]");
    g.run(2);

    g.type_duct([2, 70, 1], "air");
    g.type_duct([0, 71, 0], "air");
    g.type_duct([0, 71, 1], "air");
    {
        let r = g.ready();
        let Data::Battery(b) = &mut r.world.machines.get_mut(&(OW.into(), [-1, 70, 0])).unwrap().data else { panic!() };
        b.mode = BatteryMode::Output;
        b.power = 10_000;
        let Data::Battery(tb) = &mut r.world.machines.get_mut(&(OW.into(), top_bat)).unwrap().data else { panic!() };
        tb.mode = BatteryMode::Output;
        tb.power = 10_000;
        let Data::FluidStore(f) = &mut r.world.machines.get_mut(&(OW.into(), [3, 70, 1])).unwrap().data else { panic!() };
        f.tank.set_type("air");
        let Data::FluidStore(tf) = &mut r.world.machines.get_mut(&(OW.into(), top_barrel)).unwrap().data else { panic!() };
        tf.tank.set_type("air");
        let Data::FluidStore(pf) = &mut r.world.machines.get_mut(&(OW.into(), proxy_top_barrel)).unwrap().data else { panic!() };
        pf.tank.set_type("air");
    }

    // Fluid networks rebuild every 10 ticks.
    g.run(15);

    let i = intake(&mut g, core);
    assert!(i.on, "intake powered via horizontal port");
    assert_eq!(Server::machine_screen(&mut g.machine(core)), vec![1]);
    assert_eq!(g.machine(core).audio[0].event, "ntm:block.motor");
    assert_eq!(g.machine(core).audio[0].volume, 0.25);

    let Data::FluidStore(barrel) = g.machine([3, 70, 1]).data else { panic!() };
    assert!(barrel.tank.mb > 0, "horizontal air duct fills barrel: {}", barrel.tank.mb);

    let Data::FluidStore(top_b) = g.machine(top_barrel).data else { panic!() };
    assert_eq!(top_b.tank.mb, 0, "vertical duct above core must never connect");

    let Data::FluidStore(proxy_top_b) = g.machine(proxy_top_barrel).data else { panic!() };
    assert_eq!(proxy_top_b.tank.mb, 0, "vertical duct above proxy must never connect");

    let Data::Battery(tb) = g.machine(top_bat).data else { panic!() };
    assert_eq!(tb.power, 10_000, "vertical cable above proxy must never drain battery");

    let spec = crate::registry::spec(crate::intake::BLOCK).unwrap();
    let fp = crate::registry::fluid_ports(spec, Some("north"));
    assert_eq!(fp, vec![vec![[-1, 0, 0], [0, 0, -1]]]);
}

#[test]
fn intake_does_not_conduct_energy_between_opposite_sides() {
    let mut g = game();
    let core = [0, 70, 0];
    g.place(core, "ntm:machine_intake[facing=north,honey_level=0]");
    g.place([-1, 70, 0], "ntm:machine_battery[facing=north]");
    g.place([2, 70, 0], "ntm:machine_battery[facing=north]");
    g.run(2);
    {
        let r = g.ready();
        let Data::Battery(b1) = &mut r.world.machines.get_mut(&(OW.into(), [-1, 70, 0])).unwrap().data else { panic!() };
        b1.mode = BatteryMode::Output;
        b1.power = 10_000;
        let Data::Battery(b2) = &mut r.world.machines.get_mut(&(OW.into(), [2, 70, 0])).unwrap().data else { panic!() };
        b2.mode = BatteryMode::Input;
        b2.power = 0;
    }
    g.run(5);
    let Data::Battery(b2) = g.machine([2, 70, 0]).data else { panic!() };
    assert_eq!(b2.power, 0, "energy must not bridge across intake ports");
}

#[test]
fn inert_intake_upgrades_and_drops_nothing_when_broken() {
    let m = Machine {
        generation: 0,
        rev: 0,
        audio: Vec::new(),
        block: "ntm:machine_intake".into(),
        facing: Some("north".into()),
        data: Data::Inert { name: "ntm:machine_intake".into() },
    };
    assert_eq!(m.upgraded().kind(), Kind::Intake);

    let mut g = game();
    let core = [0, 70, 0];
    g.place(core, "ntm:machine_intake[facing=north,honey_level=0]");
    g.run(2);
    g.break_block(core);
    g.run(22);
    assert!(g.world.borrow().dropped.is_empty());
}

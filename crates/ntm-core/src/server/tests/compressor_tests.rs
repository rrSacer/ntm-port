//! Integration tests for `ntm:machine_compressor` and
//! `ntm:machine_compressor_compact` (`TileEntityMachineCompressorBase.java`).
use super::*;
use crate::compressor::{self, Compressor};
use crate::fluidbox::Mode;
use crate::machine::BatteryMode;

fn comp(g: &mut Game, at: Pos) -> Compressor {
    let Data::Compressor(c) = g.machine(at).data else { panic!() };
    *c
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

/// A press of the screen's button `id`, as the engine reports it.
fn press(g: &mut Game, at: Pos, id: u32) {
    g.world.borrow_mut().presses.push((OW.to_string(), at, id));
    g.run(1);
}

/// Two batteries (1 000 000 HE each, output mode) on a cable: a battery gives `max / 600` =
/// 1 666 HE a tick and a counting compressor takes 2 500, so one is not enough.
fn batteries_on(g: &mut Game, cable: Pos, bats: [Pos; 2]) {
    g.place(cable, "ntm:red_cable");
    for bat in bats {
        g.place(bat, "ntm:machine_battery[facing=north]");
    }
    g.run(2);
    for bat in bats {
        edit(g, bat, |d| {
            let Data::Battery(x) = d else { panic!() };
            x.mode = BatteryMode::Output;
            x.power = 1_000_000;
        });
    }
}

fn barrel(g: &mut Game, at: Pos) -> crate::fluid::Tank {
    let Data::FluidStore(f) = g.machine(at).data else { panic!() };
    f.tank
}

fn total_air(g: &mut Game, parts: &[Pos], barrels: &[Pos]) -> u32 {
    parts.iter().map(|&p| {
        let c = comp(g, p);
        c.input.mb + c.output.mb
    }).sum::<u32>()
        + barrels.iter().map(|&b| barrel(g, b).mb).sum::<u32>()
}

/// A (tall, north) at the origin: its ports are `[0,0,1]` (+Z), `[1,0,0]`
/// (+X) and `[-1,0,0]` (-X). A barrel of air feeds `[0,0,1]` through a duct;
/// the +X port runs a duct to B's `[-1,0,0]` port (B at `[8,0,0]`, also north);
/// a pressure-0 barrel sits on that duct.
#[test]
fn a_compressor_raises_air_to_one_pu_and_only_a_tank_at_one_pu_takes_it() {
    let mut g = game();
    let a = [0, 70, 0];
    let b = [8, 70, 0];
    g.place(a, "ntm:machine_compressor[facing=north,honey_level=0]");
    g.place(b, "ntm:machine_compressor[facing=north,honey_level=0]");
    batteries_on(&mut g, [-2, 70, 0], [[-3, 70, 0], [-2, 70, 1]]);
    let s = [0, 70, 3];
    g.place([0, 70, 2], "ntm:fluid_duct_neo");
    g.place(s, "ntm:barrel_steel[facing=north,honey_level=0]");
    for x in 2..=6 {
        g.place([x, 70, 0], "ntm:fluid_duct_neo");
    }
    let n = [4, 71, 0];
    g.place(n, "ntm:barrel_steel[facing=north,honey_level=0]");
    // B's other port (+X): a cable and two batteries.
    let bats = [[11, 70, 0], [10, 70, 1]];
    batteries_on(&mut g, [10, 70, 0], bats);
    g.run(2);
    g.type_duct([0, 70, 2], "air");
    for x in 2..=6 {
        g.type_duct([x, 70, 0], "air");
    }
    edit(&mut g, s, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("air", 16_000);
        f.mode = Mode::Output;
    });
    edit(&mut g, n, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.set_type("air");
        f.mode = Mode::Input;
    });
    edit(&mut g, a, |d| {
        let Data::Compressor(c) = d else { panic!() };
        c.input.set_type("air");
    });
    edit(&mut g, b, |d| {
        let Data::Compressor(c) = d else { panic!() };
        c.input.set_type("air");
        c.set_compression(1);
    });

    g.run(350);

    let (ca, cb) = (comp(&mut g, a), comp(&mut g, b));
    assert_eq!((ca.output.fluid.as_str(), ca.output.pressure), ("air", 1), "A raises 0 -> 1 PU");
    assert_eq!((cb.input.pressure, cb.output.pressure), (1, 2), "B takes 1 PU and gives 2 PU");
    assert_eq!(total_air(&mut g, &[a, b], &[s, n]), 16_000, "no air lost or made");
    assert_eq!(barrel(&mut g, n).mb, 0, "a 0 PU barrel takes none of the 1 PU air");
    assert!(cb.output.mb >= 1_000 && cb.output.mb % 1_000 == 0, "B made {} mB", cb.output.mb);
    assert!(ca.output.mb + cb.input.mb + cb.output.mb >= 3_000, "A made at least 3 operations");
    let left: u64 = bats.iter().map(|&p| { let Data::Battery(bt) = g.machine(p).data else { panic!() }; bt.power }).sum();
    assert!(left < 2_000_000, "B was fed through its +X port: {left} HE left");
}

/// The same A with its output on a pressure-0 duct and nothing at 1 PU: the
/// air stays in the output tank (the NTM's mismatch: `tank.getPressure() == pressure`).
#[test]
fn the_output_stays_when_no_tank_takes_its_pressure() {
    let mut g = game();
    let a = [0, 70, 0];
    g.place(a, "ntm:machine_compressor[facing=north,honey_level=0]");
    batteries_on(&mut g, [-2, 70, 0], [[-3, 70, 0], [-2, 70, 1]]);
    let s = [0, 70, 3];
    g.place([0, 70, 2], "ntm:fluid_duct_neo");
    g.place(s, "ntm:barrel_steel[facing=north,honey_level=0]");
    for x in 2..=3 {
        g.place([x, 70, 0], "ntm:fluid_duct_neo");
    }
    let n = [4, 70, 0];
    g.place(n, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    g.type_duct([0, 70, 2], "air");
    g.type_duct([2, 70, 0], "air");
    g.type_duct([3, 70, 0], "air");
    edit(&mut g, s, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("air", 16_000);
        f.mode = Mode::Output;
    });
    edit(&mut g, n, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.set_type("air");
    });
    edit(&mut g, a, |d| {
        let Data::Compressor(c) = d else { panic!() };
        c.input.set_type("air");
    });
    g.run(260);
    let ca = comp(&mut g, a);
    assert_eq!(ca.output.mb, 2_000);
    assert_eq!(ca.output.pressure, 1);
    assert_eq!(barrel(&mut g, n).mb, 0);
    assert_eq!(ca.input.mb + ca.output.mb + barrel(&mut g, s).mb, 16_000);
}

#[test]
fn the_compact_one_has_six_ports_on_its_second_layer() {
    let mut g = game();
    let c = [0, 70, 0];
    g.place(c, "ntm:machine_compressor_compact[facing=north,honey_level=0]");
    // Power through the other end port, (-3,1,0) local: cell [-3,71,0], a cable and a battery out of it.
    batteries_on(&mut g, [-4, 71, 0], [[-5, 71, 0], [-4, 71, 1]]);
    // facing=north: fwd=+Z, right=-X. The (-3,1,0) local port is +X side: cell [3,71,0].
    let spec = crate::registry::spec(compressor::COMPACT).unwrap();
    let ports = crate::registry::ports(spec, Some("north"));
    assert_eq!(ports.len(), 6);
    assert_eq!(ports[0], vec![[1, 0, 0]]);
    assert_eq!(ports[1], vec![[-1, 0, 0]]);
    assert_eq!(ports[2], vec![[0, 0, -1]]);
    assert_eq!(crate::registry::fluid_ports(spec, Some("north")), vec![Vec::<Pos>::new()]);
    let cells = super::container_bridge::energy_port_cells(compressor::COMPACT, c, Some("north"));
    assert_eq!(cells[0], [3, 71, 0]);
    assert_eq!(cells.len(), 6);

    // Air through the +X port at [3,71,0] (face +X): a duct at [4,71,0] and a barrel on it.
    g.place([4, 71, 0], "ntm:fluid_duct_neo");
    let s = [5, 71, 0];
    g.place(s, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    g.type_duct([4, 71, 0], "air");
    edit(&mut g, s, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("air", 16_000);
        f.mode = Mode::Output;
    });
    edit(&mut g, c, |d| {
        let Data::Compressor(x) = d else { panic!() };
        x.input.set_type("air");
    });
    g.run(120);
    let x = comp(&mut g, c);
    assert_eq!(x.shape, compressor::Shape::Compact);
    assert_eq!(x.input.mb + x.output.mb + barrel(&mut g, s).mb, 16_000);
    assert!(x.input.mb > 0 && x.output.mb == 1_000, "{x:?}");
}

#[test]
fn the_compressors_upgrade_from_inert_and_a_horizontal_cable_on_the_core_row_is_not_a_port() {
    for block in [compressor::BLOCK, compressor::COMPACT] {
        let m = Machine {
            generation: 0,
            rev: 0,
            audio: Vec::new(),
            block: block.into(),
            facing: Some("north".into()),
            data: Data::Inert { name: block.into() },
        };
        let up = m.upgraded();
        assert_eq!(up.kind(), Kind::Compressor, "{block}");
        let Data::Compressor(c) = &up.data else { panic!() };
        assert_eq!(c.shape, compressor::Shape::of(block).unwrap());
    }
    // A cable above the tall one's +X port does not feed it (the port faces +X only).
    let mut g = game();
    let a = [0, 70, 0];
    g.place(a, "ntm:machine_compressor[facing=north,honey_level=0]");
    g.place([1, 71, 0], "ntm:red_cable");
    let bat = [1, 72, 0];
    g.place(bat, "ntm:machine_battery[facing=north]");
    g.run(2);
    edit(&mut g, bat, |d| {
        let Data::Battery(x) = d else { panic!() };
        x.mode = BatteryMode::Output;
        x.power = 100_000;
    });
    g.run(20);
    assert_eq!(comp(&mut g, a).power, 0);
}

#[test]
fn broken_it_drops_nothing_by_itself_and_the_screen_buttons_set_the_compression() {
    let mut g = game();
    let a = [0, 70, 0];
    g.place(a, "ntm:machine_compressor[facing=north,honey_level=0]");
    g.run(2);
    {
        let m = g.machine(a);
        let Data::Compressor(mut c) = m.data else { panic!() };
        c.input.set_type("petroleum");
        c.input.mb = 100;
        edit(&mut g, a, |d| *d = Data::Compressor(c));
    }
    press(&mut g, a, 1);
    let c = comp(&mut g, a);
    assert_eq!((c.input.pressure, c.input.mb), (1, 0));
    assert_eq!((c.output.fluid.as_str(), c.output.pressure), ("lpg", 0));
    press(&mut g, a, 7);
    assert_eq!(comp(&mut g, a).input.pressure, 1);
    g.break_block(a);
    g.run(2);
    assert!(g.world.borrow().dropped.is_empty());
}

/// The other direction of the mismatch: a plain 0 PU barrel of air does not fill a compressor whose
/// input is at 1 PU (`getDemand` and `transferFluid` compare `tank.getPressure()`).
#[test]
fn a_zero_pressure_barrel_does_not_fill_a_pressurised_input() {
    let mut g = game();
    let a = [0, 70, 0];
    g.place(a, "ntm:machine_compressor[facing=north,honey_level=0]");
    let s = [0, 70, 3];
    g.place([0, 70, 2], "ntm:fluid_duct_neo");
    g.place(s, "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    g.type_duct([0, 70, 2], "air");
    edit(&mut g, s, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("air", 16_000);
        f.mode = Mode::Output;
    });
    edit(&mut g, a, |d| {
        let Data::Compressor(c) = d else { panic!() };
        c.input.set_type("air");
    });
    g.run(60);
    assert_eq!(comp(&mut g, a).input.mb, 16_000, "at 0 PU the barrel fills the input");
    // Now the same machine at 1 PU (the button): emptied, and the barrel's 0 PU air stays out.
    press(&mut g, a, 1);
    edit(&mut g, s, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.mb = 16_000;
    });
    g.run(60);
    let c = comp(&mut g, a);
    assert_eq!((c.input.pressure, c.input.mb), (1, 0), "a 1 PU input takes nothing from a 0 PU barrel");
    assert_eq!(barrel(&mut g, s).mb, 16_000);
}

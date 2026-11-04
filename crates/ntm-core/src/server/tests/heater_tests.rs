//! Energy batch 1's heaters in the fake world (docs/maquinas.md, "Energia,
//! lote 1"): each under a boiler, until it gives steam.
use super::*;
use crate::fluid::FluidNode;
use crate::heater::Heater;
use crate::server::container_bridge::Stack;

fn place(g: &mut Game, at: Pos, block: &str) {
    g.place(at, &format!("{block}[facing=north,honey_level=0]"));
    g.run(2);
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

fn heater<F: FnOnce(&mut Heater)>(g: &mut Game, at: Pos, f: F) {
    edit(g, at, |d| {
        let Data::Heater(h) = d else { panic!("{d:?}") };
        f(h);
    });
}

/// A boiler with water at `at`, run until it has steam (or `ticks`):
/// the ticks it took.
fn boil(g: &mut Game, at: Pos, ticks: u32) -> (u32, crate::boiler::Boiler) {
    place(g, at, "ntm:machine_boiler");
    edit(g, at, |d| {
        let Data::Boiler(b) = d else { panic!() };
        b.fluid_give("water", 16_000);
    });
    let mut t = 0;
    while t < ticks {
        g.run(10);
        t += 10;
        let Data::Boiler(b) = g.machine(at).data else { panic!() };
        if b.output.mb >= 10_000 {
            return (t, b);
        }
    }
    let Data::Boiler(b) = g.machine(at).data else { panic!() };
    (t, b)
}

/// The oil burner is two high: the boiler's core is two above its own and
/// takes the heat through its top centre. Off, nothing; on at 10 mB/t of
/// heating oil, steam.
#[test]
fn an_oil_burner_two_under_a_boiler_boils_its_water() {
    let mut g = game();
    let burner = [0, 70, 0];
    place(&mut g, burner, "ntm:heater_oilburner");
    heater(&mut g, burner, |h| {
        let Heater::Oilburner(b) = h else { panic!() };
        b.fluid_give("heatingoil", 16_000);
    });
    let (_, b) = boil(&mut g, [0, 72, 0], 100);
    assert_eq!(b.output.mb, 0, "desligado não aquece");
    heater(&mut g, burner, |h| {
        let Heater::Oilburner(b) = h else { panic!() };
        b.button(0);
        b.button(100 + 32);
    });
    let (t, b) = boil(&mut g, [0, 72, 0], 600);
    assert!(b.output.mb >= 10_000 && b.output.fluid == "steam", "{t} ticks: {} mB", b.output.mb);
    let Data::Heater(h) = g.machine(burner).data else { panic!() };
    let Heater::Oilburner(o) = *h else { panic!() };
    let burned = 16_000 - o.tank.mb;
    assert!(burned >= 100 && burned % 10 == 0, "10 mB/t: queimou {burned} mB");
    assert!(g.world.borrow().get(OW, burner).contains("honey_level=") && !g.world.borrow().get(OW, burner).contains("honey_level=0"));
}

/// The electric heater takes its power at its plug, two cells north of its
/// core (facing north, the placer south): a battery there runs it at
/// setting 10, 5 023 HE into 1 000 TU a tick.
#[test]
fn an_electric_heater_powered_at_its_plug_boils_a_boilers_water() {
    let mut g = game();
    let core = [10, 70, 0];
    let bat = [10, 70, -3];
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    place(&mut g, core, "ntm:heater_electric");
    edit(&mut g, bat, |d| {
        let Data::Battery(b) = d else { panic!() };
        b.power = b.max;
        b.mode = crate::machine::BatteryMode::Output;
    });
    heater(&mut g, core, |h| {
        let Heater::Electric(e) = h else { panic!() };
        for _ in 0..10 {
            e.toggle();
        }
    });
    let (t, b) = boil(&mut g, [10, 71, 0], 600);
    assert!(b.output.mb >= 10_000, "{t} ticks: {} mB", b.output.mb);
    let Data::Battery(bat) = g.machine(bat).data else { panic!() };
    let paid = bat.max - bat.power;
    assert!(paid >= 5_023 * 10, "a bateria pagou {paid} HE pelo plugue");
}

/// The heat exchanger cools hot coolant into coolant, 300 TU a mB, and the
/// boiler on it boils with that heat.
#[test]
fn a_heat_exchanger_cools_coolant_into_a_boilers_heat() {
    let mut g = game();
    let at = [20, 70, 0];
    place(&mut g, at, "ntm:heater_heatex");
    heater(&mut g, at, |h| {
        let Heater::Heatex(x) = h else { panic!() };
        x.fluid_give("coolant_hot", 2_000);
    });
    let (t, b) = boil(&mut g, [20, 71, 0], 600);
    assert!(b.output.mb >= 10_000, "{t} ticks: {} mB", b.output.mb);
    let Data::Heater(h) = g.machine(at).data else { panic!() };
    let Heater::Heatex(x) = *h else { panic!() };
    assert_eq!((x.input.mb, x.output.fluid.as_str(), x.output.mb), (0, "coolant", 2_000));
}

/// The heating oven on a firebox: it burns its own coal and pulls the
/// firebox's heat at half, and a boiler on it boils.
#[test]
fn a_heating_oven_on_a_firebox_heats_a_boiler() {
    let mut g = game();
    let (fire, oven) = ([30, 70, 0], [30, 71, 0]);
    place(&mut g, fire, "ntm:heater_firebox");
    place(&mut g, oven, "ntm:heater_oven");
    for at in [fire, oven] {
        edit(&mut g, at, |d| {
            let Data::Firebox(f) = d else { panic!() };
            f.slots[0] = Some(Stack::new("minecraft:coal", 64));
        });
    }
    g.run(50);
    let Data::Firebox(o) = g.machine(oven).data else { panic!() };
    let Data::Firebox(f) = g.machine(fire).data else { panic!() };
    assert!(o.oven && o.burn_heat == 1_000, "carvão do forno: 1 000 TU/t");
    assert!(f.heat < 1_000, "o forno puxa tudo o que a firebox tem: {}", f.heat);
    assert!(o.heat > 40 * 1_000, "o forno soma metade do que puxa: {}", o.heat);
    let (t, b) = boil(&mut g, [30, 72, 0], 600);
    assert!(b.output.mb >= 10_000, "{t} ticks: {} mB", b.output.mb);
}

/// Saved inert, they load as what they are now (the oven a firebox).
#[test]
fn inert_heaters_load_as_their_kinds() {
    for (block, kind) in [
        ("ntm:heater_oven", Kind::Firebox),
        ("ntm:heater_oilburner", Kind::Heater),
        ("ntm:heater_electric", Kind::Heater),
        ("ntm:heater_heatex", Kind::Heater),
    ] {
        let m = Machine { generation: 0, rev: 0, block: block.into(), facing: Some("north".into()), data: Data::Inert { name: block.into() } };
        let m = m.upgraded();
        assert_eq!(m.kind(), kind, "{block}");
        if let Data::Firebox(f) = &m.data {
            assert!(f.oven);
        }
    }
}

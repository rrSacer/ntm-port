//! Integration tests for `ntm:machine_deuterium_extractor`
//! (`MachineDeuteriumExtractor.java`, `TileEntityDeuteriumExtractor.java`).
use super::*;
use crate::fluidbox::Mode;
use crate::machine::BatteryMode;

fn deuterium(g: &mut Game, at: Pos) -> crate::deuterium::DeuteriumExtractor {
    let Data::Deuterium(d) = &g.machine(at).data else { panic!() };
    d.clone()
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

#[test]
fn deuterium_extractor_converts_water_into_heavywater_and_accepts_all_round_ports() {
    let mut g = game();
    let core = [0, 70, 0];
    g.place(core, "ntm:machine_deuterium_extractor[facing=north,honey_level=0]");

    // Water barrel & duct on -Z face; heavywater barrel & duct on +Y face; battery & cable on -X face.
    let water_barrel = [0, 70, -2];
    g.place([0, 70, -1], "ntm:fluid_duct_neo");
    g.place(water_barrel, "ntm:barrel_steel[facing=north,honey_level=0]");

    let heavy_barrel = [0, 72, 0];
    g.place([0, 71, 0], "ntm:fluid_duct_neo");
    g.place(heavy_barrel, "ntm:barrel_steel[facing=north,honey_level=0]");

    let bat = [-2, 70, 0];
    g.place([-1, 70, 0], "ntm:red_cable");
    g.place(bat, "ntm:machine_battery[facing=north]");

    g.run(2);
    g.type_duct([0, 70, -1], "water");
    g.type_duct([0, 71, 0], "heavywater");
    edit(&mut g, water_barrel, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("water", 1_000);
        f.mode = Mode::Output;
    });
    edit(&mut g, heavy_barrel, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.set_type("heavywater");
        f.mode = Mode::Input;
    });
    edit(&mut g, bat, |d| {
        let Data::Battery(b) = d else { panic!() };
        b.mode = BatteryMode::Output;
        b.power = 20_000;
    });

    g.run(30);

    let ex = deuterium(&mut g, core);
    let Data::FluidStore(wb) = g.machine(water_barrel).data else { panic!() };
    let Data::FluidStore(hb) = g.machine(heavy_barrel).data else { panic!() };
    let Data::Battery(b) = g.machine(bat).data else { panic!() };
    // 1000 mB water -> 20 mB heavywater (10 ticks of 100 mB -> 2 mB at 500 HE/t = 5000 HE spent).
    assert_eq!(wb.tank.mb, 0);
    assert_eq!(ex.input.mb, 0);
    assert_eq!(hb.tank.mb + ex.output.mb, 20, "exact 50:1 water -> heavywater conservation");
    assert!(hb.tank.mb > 0, "top around-port exports heavywater: {}", hb.tank.mb);
    assert_eq!(b.power + ex.power, 15_000, "exact 5000 HE consumed over 10 active ticks");
}

#[test]
fn deuterium_extractor_upgrades_from_inert_and_drops_nothing_when_broken() {
    let m = Machine {
        generation: 0,
        rev: 0,
        audio: Vec::new(),
        block: crate::deuterium::BLOCK.into(),
        facing: Some("north".into()),
        data: Data::Inert { name: crate::deuterium::BLOCK.into() },
    };
    assert_eq!(m.upgraded().kind(), Kind::Deuterium);

    let spec = crate::registry::spec(crate::deuterium::BLOCK).unwrap();
    assert_eq!(crate::registry::ports(spec, Some("north")), vec![crate::energy::DIRS.to_vec()]);
    assert_eq!(crate::registry::fluid_ports(spec, Some("north")), vec![crate::energy::DIRS.to_vec()]);

    let mut g = game();
    let core = [0, 70, 0];
    g.place(core, "ntm:machine_deuterium_extractor[facing=north,honey_level=0]");
    g.run(2);
    g.break_block(core);
    g.run(2);
    assert!(g.world.borrow().dropped.is_empty());
}

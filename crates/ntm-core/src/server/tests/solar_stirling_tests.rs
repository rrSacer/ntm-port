use super::*;
use crate::solar_stirling::{Shape, SolarStirling};

fn stirling(g: &mut Game, at: Pos) -> crate::solar_stirling::Stirling {
    let Data::SolarStirling(s) = g.machine(at).data else {
        panic!()
    };
    let SolarStirling::Stirling(s) = *s else {
        panic!()
    };
    s
}
#[test]
fn a_stirling_takes_heat_below_and_offers_the_previous_buffer_through_its_side() {
    for (block, shape) in [
        ("machine_stirling", Shape::Iron),
        ("machine_stirling_steel", Shape::Steel),
        ("machine_stirling_creative", Shape::Creative),
    ] {
        let mut g = game();
        let core = [0, 71, 0];
        let source = [0, 70, 0];
        let battery = [3, 71, 0];
        g.place(core, &format!("ntm:{block}[facing=north,honey_level=0]"));
        g.place(source, "ntm:heater_firebox[facing=north,honey_level=0]");
        g.place([2, 71, 0], "ntm:red_cable");
        g.place(battery, "ntm:machine_battery[facing=north]");
        g.run(2);
        let Data::Firebox(f) = &mut g
            .ready()
            .world
            .machines
            .get_mut(&(OW.into(), source))
            .unwrap()
            .data
        else {
            panic!()
        };
        f.heat = 2000;
        g.run(1);
        assert_eq!(
            stirling(&mut g, core).power,
            if shape == Shape::Creative { 200 } else { 100 }
        );
        let power = |g: &mut Game| {
            let Data::Battery(b) = g.machine(battery).data else {
                panic!()
            };
            b.power
        };
        assert_eq!(
            power(&mut g),
            0,
            "first buffer isn't offered in the generation tick"
        );
        g.run(1);
        assert_eq!(
            power(&mut g),
            if shape == Shape::Creative { 200 } else { 100 }
        );
        let saved = g.machine(core);
        let again = Machine {
            data: Data::Inert {
                name: saved.block.clone(),
            },
            ..saved.clone()
        }
        .upgraded();
        assert_eq!(again.kind(), Kind::SolarStirling);
        // Saved machines retain cog state, output and overspeed across sessions.
        let mut loaded = Game::new(g.world.clone());
        loaded.run(2);
        assert_eq!(stirling(&mut loaded, core).shape, shape);
    }
}
#[test]
fn mirrored_heat_waits_for_the_boilers_tick_and_stops_without_sun() {
    let mut g = game();
    let boiler = [0, 80, 0];
    g.place(
        boiler,
        "ntm:machine_solar_boiler[facing=north,honey_level=0]",
    );
    for x in 1..=13 {
        g.place([x, 70, 0], "ntm:solar_mirror[facing=north,honey_level=0]");
    }
    g.run(1);
    let Data::SolarStirling(s) = &mut g
        .ready()
        .world
        .machines
        .get_mut(&(OW.into(), boiler))
        .unwrap()
        .data
    else {
        panic!()
    };
    let SolarStirling::Boiler(b) = &mut **s else {
        panic!()
    };
    b.water.fill("water", 100);
    for x in 1..=13 {
        // A stored link isn't range-checked by tick. This also tests a
        // deliberately non-tool link at the far end.
        let Data::SolarStirling(s) = &mut g
            .ready()
            .world
            .machines
            .get_mut(&(OW.into(), [x, 70, 0]))
            .unwrap()
            .data
        else {
            panic!()
        };
        let SolarStirling::Mirror(m) = &mut **s else {
            panic!()
        };
        m.target = [0, 81, 0];
        g.world
            .borrow_mut()
            .sun
            .insert((OW.into(), [x, 70, 0]), (15, 0, true));
    }
    g.run(1);
    let state = |g: &mut Game| {
        let Data::SolarStirling(s) = g.machine(boiler).data else {
            panic!()
        };
        let SolarStirling::Boiler(b) = *s else {
            panic!()
        };
        b
    };
    assert_eq!((state(&mut g).heat, state(&mut g).steam.mb), (52, 0));
    g.run(1);
    assert_eq!((state(&mut g).heat, state(&mut g).steam.mb), (52, 100));
    g.world.borrow_mut().sun.clear();
    g.run(2);
    assert_eq!((state(&mut g).heat, state(&mut g).steam.mb), (0, 200));
}
#[test]
fn repair_accepts_only_the_gears_material_and_not_on_a_good_engine() {
    let mut g = game();
    let core = [0, 70, 0];
    g.place(core, "ntm:machine_stirling[facing=north,honey_level=0]");
    g.run(1);
    assert!(
        !g.server
            .native_stirling_repair(OW, core, "ntm:gear_large__iron")
    );
    let Data::SolarStirling(s) = &mut g
        .ready()
        .world
        .machines
        .get_mut(&(OW.into(), core))
        .unwrap()
        .data
    else {
        panic!()
    };
    let SolarStirling::Stirling(s) = &mut **s else {
        panic!()
    };
    s.has_cog = false;
    assert!(
        !g.server
            .native_stirling_repair(OW, core, "ntm:gear_large__steel")
    );
    assert!(
        g.server
            .native_stirling_repair(OW, core, "ntm:gear_large__iron")
    );
    assert!(stirling(&mut g, core).has_cog);
}

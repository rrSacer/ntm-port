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
            if shape == Shape::Creative { 199 } else { 99 }
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
            if shape == Shape::Creative { 199 } else { 99 }
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
            .native_stirling_repair(&mut g.sh,OW, core, "ntm:gear_large__iron")
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
            .native_stirling_repair(&mut g.sh,OW, core, "ntm:gear_large__steel")
    );
    assert!(
        g.server
            .native_stirling_repair(&mut g.sh,OW, core, "ntm:gear_large__iron")
    );
    assert!(stirling(&mut g, core).has_cog);
}

#[test]
fn stirling_ports_only_connect_outwards_and_find_the_cable_after_reopening() {
    let mut g = game();
    let core = [0, 71, 0];
    g.place(core, "ntm:machine_stirling[facing=north,honey_level=0]");
    g.place([1, 70, 0], "ntm:red_cable"); // below an arm: not an outward connection
    let bad = [2, 70, 0];
    g.place(bad, "ntm:machine_battery[facing=north]");
    g.run(1);
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
    s.power = 100;
    g.run(1);
    let Data::Battery(b) = g.machine(bad).data else {
        panic!()
    };
    assert_eq!(b.power, 0);
    g.place([2, 71, 0], "ntm:red_cable");
    let good = [3, 71, 0];
    g.place(good, "ntm:machine_battery[facing=north]");
    g.run(1);
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
    s.power = 100;
    // Save the buffer without ticking it away; cables aren't persisted.
    let json = serde_json::to_string(&g.machine(core)).unwrap();
    g.world.borrow_mut().saved.insert((OW.into(), core), json);
    let mut loaded = Game::new(g.world.clone());
    loaded.run(1);
    let Data::Battery(b) = loaded.machine(good).data else {
        panic!()
    };
    assert_eq!(b.power, 100);
}
#[test]
fn reopening_a_hot_engine_does_not_reset_its_saved_overspeed() {
    let mut g = game();
    let core = [0, 71, 0];
    let source = [0, 70, 0];
    g.place(core, "ntm:machine_stirling[facing=north,honey_level=0]");
    g.place(source, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.run(1);
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
    s.overspeed = 300;
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
    f.heat = 4000;
    for p in [core, source] {
        let json = serde_json::to_string(&g.machine(p)).unwrap();
        let data=Server::machine_screen(&mut g.machine(p));
        g.world.borrow_mut().engine.get_mut(&(OW.into(),p)).unwrap().1=data;
        g.world.borrow_mut().saved.insert((OW.into(), p), json);
    }
    let mut loaded = Game::new(g.world.clone());
    loaded.run(1);
    assert!(
        !stirling(&mut loaded, core).has_cog,
        "301st hot tick still loses the gear on reload"
    );
}

#[test]
fn solar_boiler_has_only_the_bottom_and_top_fluid_ports() {
    use super::super::fluid_server::fluid_networks_at;
    let core = [0, 70, 0];
    let machines = BTreeMap::from([
        (core, vec![vec![[0, -1, 0]]]),
        ([0, 68, 0], vec![crate::energy::DIRS.to_vec()]),
        ([0, 74, 0], vec![crate::energy::DIRS.to_vec()]),
    ]);
    let proxies = BTreeMap::from([(core, vec![[0, 72, 0]])]);
    let faces = BTreeMap::from([
        (core, vec![vec![[0, -1, 0]]]),
        ([0, 72, 0], vec![vec![[0, 1, 0]]]),
    ]);
    assert_eq!(
        fluid_networks_at(
            &BTreeSet::from([[0, 69, 0], [0, 73, 0]]),
            &machines,
            &proxies,
            &faces
        )
        .len(),
        2
    );
    // Side access to the middle and to the top is not a real port.
    assert!(
        fluid_networks_at(
            &BTreeSet::from([[0, 71, 0], [1, 71, 0], [1, 72, 0], [1, 73, 0], [1, 74, 0]]),
            &machines,
            &proxies,
            &faces
        )
        .is_empty()
    );
}

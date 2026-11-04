//! Energy batch 3 in the fake world (docs/content-energy-combustion.md): a
//! diesel generator's power through a cable into a furnace, the redstone the
//! diesel and the turbofan read, and where the multiblocks' ports are.
use super::*;
use crate::combustion::Combustion;

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r
        .world
        .machines
        .get_mut(&(OW.to_string(), at))
        .unwrap()
        .data);
}

fn diesel(g: &mut Game, at: Pos) -> crate::combustion::Diesel {
    let Data::Combustion(c) = g.machine(at).data else {
        panic!()
    };
    let Combustion::Diesel(d) = *c else { panic!() };
    d
}

/// Diesel generator → cable → electric furnace: 1 027 HE a tick from 1 mB,
/// the furnace melts its ore on it alone.
#[test]
fn a_diesel_generator_runs_a_furnace_through_a_cable() {
    let mut g = game();
    let (gen_at, furnace) = ([0, 70, 0], [2, 70, 0]);
    g.place(gen_at, "ntm:machine_diesel[facing=north,honey_level=0]");
    g.place([1, 70, 0], "ntm:red_cable");
    g.place(
        furnace,
        "ntm:machine_electric_furnace_off[facing=north,honey_level=0]",
    );
    g.run(2);
    edit(&mut g, gen_at, |d| {
        let Data::Combustion(c) = d else { panic!() };
        let Combustion::Diesel(m) = &mut **c else {
            panic!()
        };
        m.on = true;
        m.tank.fill("diesel", 500);
    });
    edit(&mut g, furnace, |d| {
        let Data::ElectricFurnace(f) = d else {
            panic!()
        };
        f.input = Some(("minecraft:iron_ore".into(), 8));
    });
    g.run(300);
    assert!(
        g.output(furnace).is_some_and(|o| o.1 >= 2),
        "o forno rodou com o diesel: {:?}",
        g.output(furnace)
    );
    let d = diesel(&mut g, gen_at);
    assert!(
        d.tank.mb < 500 && d.was_on,
        "o gerador queimou: {} mB",
        d.tank.mb
    );
}

/// A signal on the diesel's block stops it; the machine starts off.
#[test]
fn the_diesel_stops_on_redstone_and_starts_off() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_diesel[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, at, |d| {
        let Data::Combustion(c) = d else { panic!() };
        let Combustion::Diesel(m) = &mut **c else {
            panic!()
        };
        m.tank.fill("diesel", 100);
        assert!(!m.on);
        m.on = true;
    });
    g.world.borrow_mut().signal.insert((OW.to_string(), at), 15);
    g.run(5);
    assert_eq!(
        (diesel(&mut g, at).tank.mb, diesel(&mut g, at).power),
        (100, 0)
    );
    g.world.borrow_mut().signal.clear();
    g.run(5);
    assert!(diesel(&mut g, at).tank.mb < 100);
}

/// Any of the turbofan's four connectors with a signal stops it.
#[test]
fn the_turbofan_reads_redstone_at_its_connectors() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_turbofan[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, at, |d| {
        let Data::Combustion(c) = d else { panic!() };
        let Combustion::Turbofan(m) = &mut **c else {
            panic!()
        };
        m.tank.fill("kerosene", 1_000);
    });
    g.run(3);
    let fuel = |g: &mut Game| {
        let Data::Combustion(c) = g.machine(at).data else {
            panic!()
        };
        let Combustion::Turbofan(m) = *c else {
            panic!()
        };
        (m.tank.mb, m.power)
    };
    let (mb, power) = fuel(&mut g);
    assert!(mb < 1_000 && power > 0, "queimou sem sinal");
    let f = crate::registry::facing_dir("north").unwrap();
    let connector = crate::server::container_bridge::world_cell(at, f, [1, 0, 1]);
    g.world
        .borrow_mut()
        .signal
        .insert((OW.to_string(), connector), 7);
    g.run(1);
    let held = fuel(&mut g);
    g.run(5);
    assert_eq!(fuel(&mut g), held, "um conector com sinal para a geração");
}

/// The ports: the engine's four cells, the turbofan's four, the gas
/// turbine's power at the end of its row a layer up, its fluids at both ends.
#[test]
fn the_combustion_multiblocks_have_their_ports() {
    use crate::server::container_bridge::{fluid_proxy_cells, proxy_cells};
    let core = [0, 70, 0];
    for (block, n) in [
        ("ntm:machine_combustion_engine", 4),
        ("ntm:machine_turbofan", 4),
        ("ntm:machine_turbinegas", 5),
    ] {
        let power = proxy_cells(block, core, Some("north"));
        assert_eq!(power.len(), n, "{block}");
        assert!(power.iter().all(|c| c != &core), "{block}");
    }
    assert_eq!(
        fluid_proxy_cells("ntm:machine_combustion_engine", core, Some("north")).len(),
        4
    );
    assert_eq!(
        fluid_proxy_cells("ntm:machine_turbofan", core, Some("north")).len(),
        4
    );
    let fluids = fluid_proxy_cells("ntm:machine_turbinegas", core, Some("north"));
    let power = proxy_cells("ntm:machine_turbinegas", core, Some("north"));
    assert_eq!(
        power.last().map(|c| [c[0].abs(), c[1], c[2]]),
        Some([4, 71, 0]),
        "o conector de energia, quatro de distância, uma camada acima"
    );
    assert_eq!(
        fluids.last().map(|c| [c[0].abs(), c[1], c[2]]),
        Some([5, 71, 0])
    );
    assert_ne!(
        power.last().map(|c| c[0].signum()),
        fluids.last().map(|c| c[0].signum()),
        "em pontas opostas"
    );
}

/// Saved inert, they load as what they are now.
#[test]
fn inert_combustion_machines_load_as_their_kinds() {
    for block in [
        "ntm:machine_diesel",
        "ntm:machine_combustion_engine",
        "ntm:machine_turbofan",
        "ntm:machine_turbinegas",
    ] {
        let m = Machine { generation: 0, rev: 0,
            block: block.into(),
            facing: Some("north".into()),
            data: Data::Inert { name: block.into() },
        };
        assert_eq!(m.upgraded().kind(), Kind::Combustion, "{block}");
    }
}

//! Energy batch 2 in the fake world (docs/maquinas.md, "Energia, lote 2"):
//! the first real power in survival, a boiler's steam through an engine
//! into a furnace, and the condenser giving the water back.
use super::*;
use crate::fluid::FluidNode;
use crate::server::container_bridge::Stack;
use crate::steam::Steam;

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

fn duct(g: &mut Game, at: Pos, fluid: &str) {
    g.place(at, "ntm:fluid_duct_neo");
    g.run(2);
    g.type_duct(at, fluid);
}

fn steam(g: &mut Game, at: Pos) -> Steam {
    let Data::Steam(s) = g.machine(at).data else { panic!() };
    *s
}

/// Firebox → boiler → steam engine → furnace; the engine's spent steam to
/// a condenser, its water back to the boiler by duct. The water goes
/// round: 1 mB of it is 100 of steam, then 1 of spent steam, then 1 again.
#[test]
fn a_boiler_runs_a_furnace_through_a_steam_engine_and_gets_its_water_back() {
    let mut g = game();
    let (fire, boiler, engine, condenser, furnace) = ([0, 70, 0], [0, 71, 0], [1, 71, 0], [2, 71, 0], [1, 71, 1]);
    g.place(fire, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.place(boiler, "ntm:machine_boiler[facing=north,honey_level=0]");
    g.place(engine, "ntm:machine_steam_engine[facing=north,honey_level=0]");
    g.place(condenser, "ntm:machine_condenser[facing=north,honey_level=0]");
    g.place(furnace, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]");
    g.run(2);
    for x in 0..=2 {
        duct(&mut g, [x, 71, -1], "water");
    }
    edit(&mut g, fire, |d| {
        let Data::Firebox(f) = d else { panic!() };
        f.slots[0] = Some(Stack::new("minecraft:coal", 64));
    });
    edit(&mut g, boiler, |d| {
        let Data::Boiler(b) = d else { panic!() };
        b.fluid_give("water", 1_000);
    });
    edit(&mut g, furnace, |d| {
        let Data::ElectricFurnace(f) = d else { panic!() };
        f.input = Some(("minecraft:iron_ore".into(), 8));
    });
    let mut condensed = 0;
    let mut he = 0;
    for _ in 0..150 {
        g.run(10);
        if let Steam::Condenser(c) = steam(&mut g, condenser) {
            condensed += u32::from(c.on);
        }
        if let Steam::Engine(e) = steam(&mut g, engine) {
            he = he.max(e.buffer);
        }
    }
    assert!(he > 0, "o motor gerou");
    assert!(condensed > 0, "o condensador trabalhou");
    assert!(g.output(furnace).is_some_and(|o| o.1 >= 2), "o forno rodou com o vapor: {:?}", g.output(furnace));
    // The water is conserved round the loop (a few mB in flight).
    let Data::Boiler(b) = g.machine(boiler).data else { panic!() };
    let Steam::Engine(e) = steam(&mut g, engine) else { panic!() };
    let Steam::Condenser(c) = steam(&mut g, condenser) else { panic!() };
    let water = b.input.mb + b.output.mb / 100 + e.input.mb / 100 + e.output.mb + c.input.mb + c.output.mb;
    assert!((990..=1_000).contains(&water), "a água volta: {water} (caldeira {} + {} vapor)", b.input.mb, b.output.mb);
    assert!(b.input.mb < 1_000, "a caldeira gastou água");
}

/// The turbines' power ports, at the end of their chains: the industrial
/// turbine's at its back a layer up, the Leviathan's ten back.
#[test]
fn steam_machines_give_power_at_their_ports() {
    let cells = crate::server::container_bridge::proxy_cells("ntm:machine_industrial_turbine", [0, 70, 0], Some("north"));
    assert_eq!(cells.last(), Some(&[0, 71, 3]), "porta de energia atrás (longe de quem pôs), uma camada acima");
    let cells = crate::server::container_bridge::proxy_cells("ntm:machine_chungus", [0, 70, 0], Some("east"));
    assert_eq!(cells.last(), Some(&[-10, 70, 0]));
}

/// Saved inert, they load as what they are now.
#[test]
fn inert_steam_machines_load_as_their_kinds() {
    for block in ["ntm:machine_steam_engine", "ntm:machine_industrial_turbine", "ntm:machine_chungus", "ntm:machine_condenser", "ntm:machine_condenser_powered", "ntm:machine_tower_small"] {
        let m = Machine { generation: 0, rev: 0, audio: Vec::new(), block: block.into(), facing: Some("north".into()), data: Data::Inert { name: block.into() } };
        assert_eq!(m.upgraded().kind(), Kind::Steam, "{block}");
    }
}

#[test]
fn tower_small_condenses_via_aux_tower_ports_and_excludes_non_ports_and_power() {
    use crate::fluidbox::Mode;
    use crate::machine::BatteryMode;

    let mut g = game();
    let core = [0, 70, 0];
    g.place(core, "ntm:machine_tower_small[facing=north,honey_level=0]");

    // Port 1 (-Z: [0, 70, -2] facing -Z): spentsteam barrel -> duct -> tower.
    let src_barrel = [0, 70, -4];
    duct(&mut g, [0, 70, -3], "spentsteam");
    g.place(src_barrel, "ntm:barrel_steel[facing=north,honey_level=0]");

    // Ports 2, 3, 4 (+X, +Z, -X): tower -> duct -> water barrel (each capped with 1000 mB free so all three must fill).
    let dst_barrel_e = [4, 70, 0];
    duct(&mut g, [3, 70, 0], "water");
    g.place(dst_barrel_e, "ntm:barrel_steel[facing=north,honey_level=0]");

    let dst_barrel_s = [0, 70, 4];
    duct(&mut g, [0, 70, 3], "water");
    g.place(dst_barrel_s, "ntm:barrel_steel[facing=north,honey_level=0]");

    let dst_barrel_w = [-4, 70, 0];
    duct(&mut g, [-3, 70, 0], "water");
    g.place(dst_barrel_w, "ntm:barrel_steel[facing=north,honey_level=0]");

    // Non-port proxy [1, 70, -2] -> duct at [1, 70, -3] -> barrel [1, 70, -4]: must stay 0.
    let bad_side_barrel = [1, 70, -4];
    duct(&mut g, [1, 70, -3], "water");
    g.place(bad_side_barrel, "ntm:barrel_steel[facing=north,honey_level=0]");

    // Below -Z port ([0, 69, -2]): must NOT connect vertically.
    let bad_below_barrel = [0, 68, -2];
    duct(&mut g, [0, 69, -2], "water");
    g.place(bad_below_barrel, "ntm:barrel_steel[facing=north,honey_level=0]");

    // Energy cable touching core/proxy must NOT drain battery.
    let bat = [-4, 70, 1];
    g.place([-3, 70, 1], "ntm:red_cable");
    g.place([-2, 70, 1], "ntm:red_cable");
    g.place(bat, "ntm:machine_battery[facing=north]");

    g.run(2);
    edit(&mut g, src_barrel, |d| {
        let Data::FluidStore(f) = d else { panic!() };
        f.tank.fill("spentsteam", 4_000);
        f.mode = Mode::Output;
    });
    for b in [dst_barrel_e, dst_barrel_s, dst_barrel_w] {
        edit(&mut g, b, |d| {
            let Data::FluidStore(f) = d else { panic!() };
            f.tank.fill("water", 15_000);
            f.mode = Mode::Input;
        });
    }
    for b in [bad_side_barrel, bad_below_barrel] {
        edit(&mut g, b, |d| {
            let Data::FluidStore(f) = d else { panic!() };
            f.tank.set_type("water");
            f.mode = Mode::Input;
        });
    }
    edit(&mut g, bat, |d| {
        let Data::Battery(b) = d else { panic!() };
        b.mode = BatteryMode::Output;
        b.power = 10_000;
    });

    g.run(30);

    let Data::FluidStore(src) = g.machine(src_barrel).data else { panic!() };
    let Data::FluidStore(be) = g.machine(dst_barrel_e).data else { panic!() };
    let Data::FluidStore(bs) = g.machine(dst_barrel_s).data else { panic!() };
    let Data::FluidStore(bw) = g.machine(dst_barrel_w).data else { panic!() };
    let Steam::Condenser(c) = steam(&mut g, core) else { panic!() };
    assert_eq!(c.input.cap, 1_000);
    assert_eq!(c.output.cap, 1_000);
    assert_eq!(src.tank.mb, 0);
    assert_eq!(be.tank.mb, 16_000, "+X outward port filled");
    assert_eq!(bs.tank.mb, 16_000, "+Z outward port filled");
    assert_eq!(bw.tank.mb, 16_000, "-X outward port filled");
    assert_eq!(c.input.mb, 0);
    assert_eq!(c.output.mb, 1_000, "exact spentsteam -> water conservation (3*1000 + 1000 = 4000)");

    let Data::FluidStore(bad_side) = g.machine(bad_side_barrel).data else { panic!() };
    let Data::FluidStore(bad_below) = g.machine(bad_below_barrel).data else { panic!() };
    assert_eq!(bad_side.tank.mb, 0, "non-port proxy must not connect");
    assert_eq!(bad_below.tank.mb, 0, "vertical face under port must not connect");

    let Data::Battery(b) = g.machine(bat).data else { panic!() };
    assert_eq!(b.power, 10_000, "cooling tower must not draw power");

    let spec = crate::registry::spec(crate::steam::TOWER_SMALL).unwrap();
    assert_eq!(crate::registry::fluid_ports(spec, Some("north")), vec![Vec::<Pos>::new()]);
    assert!(crate::registry::ports(spec, Some("north")).is_empty());

    g.break_block(core);
    g.run(2);
    assert!(g.world.borrow().dropped.is_empty());
}

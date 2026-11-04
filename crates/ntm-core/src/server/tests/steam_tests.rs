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
    for block in ["ntm:machine_steam_engine", "ntm:machine_industrial_turbine", "ntm:machine_chungus", "ntm:machine_condenser", "ntm:machine_condenser_powered"] {
        let m = Machine { generation: 0, rev: 0, audio: Vec::new(), block: block.into(), facing: Some("north".into()), data: Data::Inert { name: block.into() } };
        assert_eq!(m.upgraded().kind(), Kind::Steam, "{block}");
    }
}

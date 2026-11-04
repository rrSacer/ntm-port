//! The oil chain's batch C in the fake world (docs/maquinas.md): boilers
//! and the coker on a firebox, the processors fed through the network.
use super::*;
use crate::fluid::FluidNode;
use crate::server::container_bridge::Stack;

fn place(g: &mut Game, at: Pos, block: &str) {
    g.place(at, &format!("{block}[facing=north,honey_level=0]"));
    g.run(2);
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

fn fire(g: &mut Game, at: Pos) {
    place(g, at, "ntm:heater_firebox");
    edit(g, at, |d| {
        let Data::Firebox(f) = d else { panic!() };
        f.slots[0] = Some(Stack::new("minecraft:coal", 64));
    });
}

const FIRE: Pos = [6, 70, 0];
const BOILER: Pos = [6, 71, 0];

/// A firebox under a boiler: its heat comes up a tenth at a time and boils
/// water into steam, 100 mB a millibucket for 200 TU.
#[test]
fn a_boiler_on_a_firebox_boils_water() {
    let mut g = game();
    fire(&mut g, FIRE);
    place(&mut g, BOILER, "ntm:machine_boiler");
    edit(&mut g, BOILER, |d| {
        let Data::Boiler(b) = d else { panic!() };
        b.fluid_give("water", 1_000);
    });
    g.run(200);
    let Data::Boiler(b) = g.machine(BOILER).data else { panic!() };
    let Data::Firebox(f) = g.machine(FIRE).data else { panic!() };
    assert_eq!(b.output.fluid, "steam");
    assert!(b.output.mb > 0 && b.output.mb == (1_000 - b.input.mb) * 100, "{} {}", b.input.mb, b.output.mb);
    assert!(f.heat < crate::foundry::firebox::MAX_HEAT, "o firebox cede calor");
}

const FIRE2: Pos = [12, 70, 0];
const COKER: Pos = [12, 71, 0];

/// A coker on a firebox: heavy oil into petroleum coke and coker oil.
#[test]
fn a_coker_on_a_firebox_cokes_heavy_oil() {
    let mut g = game();
    fire(&mut g, FIRE2);
    place(&mut g, COKER, "ntm:machine_coker");
    edit(&mut g, COKER, |d| {
        let Data::Coker(c) = d else { panic!() };
        c.fluid_give("heavyoil", 16_000);
    });
    let mut t = 0;
    while t < 3_000 {
        g.run(10);
        t += 10;
        let Data::Coker(c) = g.machine(COKER).data else { panic!() };
        if c.slots[crate::coker::slot::OUTPUT].is_some() {
            break;
        }
    }
    let Data::Coker(c) = g.machine(COKER).data else { panic!() };
    assert_eq!(c.slots[crate::coker::slot::OUTPUT], Some(Stack::new("ntm:coke__petroleum", 1)), "{t} ticks, calor {}", c.heat);
    assert_eq!(c.output.fluid, "oil_coker");
}

/// Saved inert, they load as what they are now.
#[test]
fn inert_batch_c_machines_load_as_their_kinds() {
    for (block, kind) in [
        ("ntm:machine_boiler", Kind::Boiler),
        ("ntm:machine_industrial_boiler", Kind::Boiler),
        ("ntm:machine_coker", Kind::Coker),
        ("ntm:machine_catalytic_cracker", Kind::Processor),
        ("ntm:machine_catalytic_reformer", Kind::Processor),
        ("ntm:machine_hydrotreater", Kind::Processor),
        ("ntm:machine_vacuum_distill", Kind::Processor),
    ] {
        let m = Machine { generation: 0, audio: Vec::new(), block: block.into(), facing: Some("north".into()), data: Data::Inert { name: block.into() } };
        assert_eq!(m.upgraded().kind(), kind, "{block}");
    }
}

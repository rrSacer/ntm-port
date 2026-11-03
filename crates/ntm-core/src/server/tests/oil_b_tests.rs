//! The oil chain's batch B in the fake world (docs/maquinas.md): the
//! refinery, the fractioning tower (stacked) and the mixer.
use super::*;
use crate::fluid::FluidNode;
use crate::server::container_bridge::Stack;

fn machine(g: &mut Game, at: Pos, block: &str, slots: usize) {
    g.place(at, &format!("{block}[facing=north,honey_level=0]"));
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; slots], Vec::new()));
    g.run(2);
}

fn put(g: &mut Game, at: Pos, i: usize, s: Stack) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = Some(s);
}

fn slot(g: &Game, at: Pos, i: usize) -> Option<Stack> {
    g.world.borrow().engine[&(OW.to_string(), at)].0[i].clone()
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

const REF: Pos = [6, 70, 0];

/// 2 000 mB of hot oil: its four fractions and 2 sulfur in slot 11.
#[test]
fn the_refinery_splits_hot_oil_into_four_fractions_and_sulfur() {
    let mut g = game();
    machine(&mut g, REF, "ntm:machine_refinery", 13);
    put(&mut g, REF, 0, Stack::new(crate::machine::CREATIVE_BATTERY, 1));
    edit(&mut g, REF, |d| {
        let Data::Refinery(r) = d else { panic!() };
        r.fluid_give("hotoil", 2_000);
    });
    g.run(30);
    let Data::Refinery(r) = g.machine(REF).data else { panic!() };
    let got: Vec<_> = r.outputs.iter().map(|t| (t.fluid.clone(), t.mb)).collect();
    let want: Vec<(String, u32)> = [("heavyoil", 1_000), ("naphtha", 500), ("lightoil", 300), ("petroleum", 200)].iter().map(|(f, n)| (f.to_string(), *n)).collect();
    assert_eq!(got, want);
    assert_eq!(slot(&g, REF, 11), Some(Stack::new("ntm:sulfur", 2)));
}

const LOW: Pos = [12, 70, 0];
const HIGH: Pos = [12, 73, 0];

/// Two towers stacked: the heavy oil goes up to the top one too, and what
/// it makes comes down to the bottom one, the stack's outlet.
#[test]
fn stacked_fraction_towers_share_the_work() {
    let mut g = game();
    machine(&mut g, LOW, "ntm:machine_fraction_tower", 0);
    machine(&mut g, HIGH, "ntm:machine_fraction_tower", 0);
    edit(&mut g, LOW, |d| {
        let Data::FractionTower(t) = d else { panic!() };
        t.fluid_give("heavyoil", 4_000);
    });
    g.run(60);
    let (Data::FractionTower(low), Data::FractionTower(high)) = (g.machine(LOW).data, g.machine(HIGH).data) else { panic!() };
    assert_eq!(high.tanks[0].fluid, "heavyoil", "o óleo subiu");
    assert!(low.tanks[1].mb > 0 && high.tanks[1].mb == 0, "o betume desceu: {:?} {:?}", low.tanks, high.tanks);
    // All of it accounted for: 100 mB a fractioning, 30 + 70 out.
    let oil = low.tanks[0].mb + high.tanks[0].mb;
    let out = low.tanks[1].mb + low.tanks[2].mb + high.tanks[1].mb + high.tanks[2].mb;
    assert_eq!(oil + out, 4_000);
}

const MIX: Pos = [18, 70, 0];

/// Coolant, picked from the list: a niter and 1 800 mB of water into
/// 2 000 mB of coolant in 50 ticks at 50 HE.
#[test]
fn the_mixer_makes_coolant_from_the_recipe_picked() {
    let mut g = game();
    machine(&mut g, MIX, "ntm:machine_mixer", 5);
    put(&mut g, MIX, 0, Stack::new(crate::machine::CREATIVE_BATTERY, 1));
    edit(&mut g, MIX, |d| {
        let Data::Crafter(c) = d else { panic!() };
        let r = c.station.recipes().iter().find(|r| r.name == "mixer.0.0").unwrap();
        c.set_recipe(r);
        assert_eq!((c.tin[0].fluid.as_str(), c.tout[0].fluid.as_str(), c.tout[0].cap), ("water", "coolant", 24_000));
        c.tin[0].mb = 1_800;
    });
    put(&mut g, MIX, 1, Stack::new("ntm:niter", 1));
    g.run(60);
    let Data::Crafter(c) = g.machine(MIX).data else { panic!() };
    assert_eq!((c.tout[0].mb, c.tin[0].mb), (2_000, 0));
    assert_eq!(slot(&g, MIX, 1), None);
}

/// Saved inert, they load as what they are now.
#[test]
fn inert_batch_b_machines_load_as_their_kinds() {
    for (block, kind) in [("ntm:machine_refinery", Kind::Refinery), ("ntm:machine_fraction_tower", Kind::FractionTower), ("ntm:machine_mixer", Kind::Crafter)] {
        let m = Machine { generation: 0, block: block.into(), facing: Some("north".into()), data: Data::Inert { name: block.into() } };
        assert_eq!(m.upgraded().kind(), kind, "{block}");
    }
}

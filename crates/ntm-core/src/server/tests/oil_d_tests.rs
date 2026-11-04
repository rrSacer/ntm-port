//! The oil chain's batch D in the fake world (docs/maquinas.md): the drills
//! put their pipe down and suck the deposits.
use super::*;
use crate::fluid::FluidNode;
use crate::oil_drill::{self, Drill};
use crate::server::container_bridge::Stack;

fn drill(g: &mut Game, at: Pos, block: &str) {
    g.place(at, &format!("{block}[facing=north,honey_level=0]"));
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; oil_drill::slot::COUNT], Vec::new()));
    g.run(2);
    let mut w = g.world.borrow_mut();
    let s = &mut w.engine.get_mut(&(OW.to_string(), at)).unwrap().0;
    s[0] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
    // Overdrive 3: a working every few ticks.
    s[5] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
}

fn state(g: &mut Game, at: Pos) -> Drill {
    match g.machine(at).data {
        Data::Drill(d) => *d,
        other => panic!("{other:?}"),
    }
}

fn block(g: &Game, p: Pos) -> String {
    g.world.borrow().get(OW, p)
}

/// The pumpjack's pipe goes down through the air to the deposit and sucks
/// it: 750 mB of oil and some gas each time.
#[test]
fn a_pumpjack_pipes_down_to_a_deposit_and_pumps_it() {
    let mut g = game();
    let at = [4, 70, 4];
    g.world.borrow_mut().set(OW, [4, 60, 4], oil_drill::OIL);
    drill(&mut g, at, "ntm:machine_pumpjack");
    g.run(200);
    for y in 61..70 {
        assert_eq!(block(&g, [4, y, 4]), oil_drill::PIPE, "y {y}");
    }
    let d = state(&mut g, at);
    assert!(d.tanks[0].mb >= 750 && d.tanks[0].mb.is_multiple_of(750), "{:?}", d.tanks);
    assert!(d.tanks[1].mb > 0);
    assert!(d.fluid_offer("oil") > 0);
}

/// An emptied deposit under the pipe leads to the oil beside it.
#[test]
fn an_emptied_deposit_leads_to_more_oil() {
    let mut g = game();
    let at = [10, 70, 4];
    {
        let mut w = g.world.borrow_mut();
        w.set(OW, [10, 69, 4], oil_drill::EMPTY);
        w.set(OW, [10, 68, 4], oil_drill::EMPTY);
        w.set(OW, [11, 68, 4], oil_drill::OIL);
    }
    drill(&mut g, at, "ntm:machine_well");
    g.run(60);
    assert!(state(&mut g, at).tanks[0].mb >= 500);
    assert_eq!(block(&g, [10, 69, 4]), oil_drill::EMPTY, "não perfura o depósito vazio que leva ao petróleo");
}

/// The fracker reaches the bedrock oil (the 26.3 floor) and spends 10 mB of
/// fracking solution a suck; without it, it waits.
#[test]
fn the_fracker_takes_bedrock_oil_with_its_solution() {
    let mut g = game();
    let at = [16, -50, 4];
    g.world.borrow_mut().set(OW, [16, -63, 4], oil_drill::BEDROCK_OIL);
    drill(&mut g, at, "ntm:machine_fracking_tower");
    g.run(200);
    let d = state(&mut g, at);
    assert_eq!((d.tanks[0].mb, d.indicator), (0, oil_drill::indicator::NO_SOLUTION), "sem solução");
    assert_eq!(block(&g, [16, -62, 4]), oil_drill::PIPE);
    let r = g.ready();
    if let Some(Data::Drill(d)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
        d.fluid_give("fracksol", 1_000);
    }
    g.run(60);
    let d = state(&mut g, at);
    assert!(d.tanks[0].mb >= 100 && d.tanks[0].mb.is_multiple_of(100), "{:?}", d.tanks);
    assert_eq!(1_000 - d.tanks[2].mb, d.tanks[0].mb / 10, "10 mB por 100 de petróleo");
    assert_eq!(block(&g, [16, -63, 4]), oil_drill::BEDROCK_OIL, "não se esgota");
}

/// Saved inert, they load as drills.
#[test]
fn inert_batch_d_drills_load_as_drills() {
    for block in ["ntm:machine_well", "ntm:machine_pumpjack", "ntm:machine_fracking_tower"] {
        let m = Machine { generation: 0, rev: 0, block: block.into(), facing: Some("north".into()), data: Data::Inert { name: block.into() } };
        assert_eq!(m.upgraded().kind(), Kind::Drill, "{block}");
    }
}

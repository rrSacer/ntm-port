//! T09: the outgasser in the step as its TE (`TileEntityRBMKOutgasser`): its two slots in the recipes' terms, its
//! progress and gas kept as `writeToNBT` keeps them, the gas out by the column's ports, and what automation may put in.
use super::*;
use crate::rbmk::persist::{OutgasserState, RbmkColumn};
use container_bridge::Stack;

const Y: i32 = 71;
const ROD: Pos = [0, Y, 0];
const AT: Pos = [1, Y, 0];

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn col(g: &mut Game, p: Pos) -> RbmkColumn {
    let Data::Rbmk(c) = g.ready().world.machines[&(OW.to_string(), p)].data.clone() else { panic!("not a column") };
    *c
}

fn state(g: &mut Game) -> OutgasserState {
    col(g, AT).outgasser.expect("an outgasser")
}

fn slots(g: &Game) -> Vec<Option<(String, u32)>> {
    g.world.borrow().engine[&(OW.to_string(), AT)].0.iter().map(|s| s.as_ref().map(|s| (s.id.clone(), s.count))).collect()
}

fn put(g: &mut Game, slot: usize, id: &str, count: u32) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[slot] = Some(Stack::new(id, count));
}

/// A Po-210/Be rod (its own neutron source) in a channel beside the outgasser, by hand.
fn reactor(g: &mut Game) {
    g.place(ROD, &column("rbmk_rod"));
    g.place(AT, &column("rbmk_outgasser"));
    g.run(2);
    let stack = Stack::new("ntm:rbmk_fuel_po210be", 1);
    g.world.borrow_mut().hands = Some(PlayerSlots {
        hand: Some(PlayerSlot::Inventory(0)),
        inventory: vec![Some(HeldStack { encoded: format!("{stack:?}"), stack }), None],
        creative: false,
        ..Default::default()
    });
    assert!(g.server.native_rbmk_use(&mut g.sh, OW, [ROD[0], ROD[1] + 2, ROD[2]], ROD, "p1"), "the rod goes in");
}

fn near_done(g: &mut Game) {
    let Data::Rbmk(c) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), AT)).unwrap().data else { panic!() };
    c.outgasser.as_mut().unwrap().progress = 9_999.5;
}

/// Coal tar irradiated past 10 000: one tar used, 100 mB of coal oil in the tank, the progress back to 0; the save keeps
/// the gas and the progress, and the slots are the engine's.
#[test]
fn coal_tar_irradiated_makes_coal_oil_and_the_save_keeps_it() {
    let mut g = game();
    reactor(&mut g);
    assert_eq!(state(&mut g), OutgasserState::of("ntm:rbmk_outgasser").unwrap(), "a fresh outgasser");
    put(&mut g, 0, "ntm:oil_tar__coal", 3);
    g.run(1);
    near_done(&mut g);
    let mut done = false;
    for _ in 0..200 {
        g.run(1);
        if state(&mut g).gas.fill > 0 {
            done = true;
            break;
        }
    }
    assert!(done, "it processed: {:?}", state(&mut g));
    let st = state(&mut g);
    assert_eq!((st.gas.fluid.as_str(), st.gas.fill), ("COALOIL", 100));
    assert!(st.progress < 9_999.5, "the progress starts again: {}", st.progress);
    assert_eq!(slots(&g), vec![Some(("ntm:oil_tar__coal".into(), 2)), None]);
    let world = g.world.borrow().clone();
    let mut back = Game::new(Rc::new(RefCell::new(world)));
    back.run(1);
    let kept = col(&mut back, AT).outgasser.unwrap();
    assert_eq!((kept.gas.fluid.as_str(), kept.gas.fill), ("COALOIL", 100));
}

/// A product goes to the output slot as the port's item: gold makes Au-198, the slot holding the port's ingot.
#[test]
fn the_product_lands_as_the_ports_item() {
    let mut g = game();
    reactor(&mut g);
    put(&mut g, 0, "minecraft:gold_ingot", 2);
    g.run(1);
    near_done(&mut g);
    for _ in 0..200 {
        g.run(1);
        if slots(&g)[1].is_some() {
            break;
        }
    }
    assert_eq!(slots(&g), vec![Some(("minecraft:gold_ingot".into(), 1)), Some(("ntm:ingot_au198".into(), 1))]);
}

/// A mushroom's product (the glowing mushroom) is not in the port: the mushroom has no recipe here, its progress stays
/// at 0 (`canProcess` false resets it every tick) and automation does not put one in.
#[test]
fn a_mushroom_waits_for_the_glowing_mushroom() {
    use crate::native::{slot_allows, SlotAction::*, SlotOrigin::*};
    let mut g = game();
    reactor(&mut g);
    put(&mut g, 0, "minecraft:brown_mushroom", 2);
    g.run(1);
    near_done(&mut g);
    g.run(40);
    assert_eq!((state(&mut g).progress, slots(&g)[0].clone()), (0.0, Some(("minecraft:brown_mushroom".into(), 2))));
    let none = vec![None, None];
    let tar = Stack::new("ntm:oil_tar__coal", 1);
    assert!(slot_allows("ntm:rbmk_outgasser", &none, 0, &tar, Insert, Automation), "a recipe's input by a funnel");
    assert!(!slot_allows("ntm:rbmk_outgasser", &none, 1, &tar, Insert, Automation), "never into the output");
    assert!(!slot_allows("ntm:rbmk_outgasser", &none, 0, &Stack::new("minecraft:brown_mushroom", 1), Insert, Automation));
    assert!(!slot_allows("ntm:rbmk_outgasser", &none, 0, &Stack::new("minecraft:stone", 1), Insert, Automation));
}

/// The gas leaves by the column's ports: a duct under the core takes the coal oil to a barrel.
#[test]
fn the_gas_leaves_by_the_ports() {
    let mut g = game();
    reactor(&mut g);
    g.place([1, Y - 1, 0], "ntm:fluid_duct_neo");
    g.place([2, Y - 1, 0], "ntm:barrel_steel[facing=north,honey_level=0]");
    g.run(2);
    g.type_duct([1, Y - 1, 0], "coaloil");
    let Data::Rbmk(c) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), AT)).unwrap().data else { panic!() };
    c.outgasser.as_mut().unwrap().gas = crate::rbmk::persist::KeptTank { fluid: "COALOIL".into(), fill: 300, max: 64_000 };
    g.run(12);
    let Data::FluidStore(b) = g.machine([2, Y - 1, 0]).data else { panic!() };
    assert_eq!((b.tank.fluid.as_str(), b.tank.mb, state(&mut g).gas.fill), ("coaloil", 300, 0));
}

/// The screen: the progress arrow as `GUIRBMKOutgasser` cuts it (13 pixels at 10 000) and the gas (fill, max).
#[test]
fn the_screen_reads_the_arrow_and_the_gas() {
    let mut g = game();
    g.place(AT, &column("rbmk_outgasser"));
    g.run(2);
    let Data::Rbmk(c) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), AT)).unwrap().data else { panic!() };
    let o = c.outgasser.as_mut().unwrap();
    (o.progress, o.gas.fill) = (7_692.3, 1_500);
    use container_bridge::Slotted;
    let v = c.screen_data();
    assert_eq!(&v[1..5], &[9, 13, 1_500, 64_000], "7692.3 * 13 / 10000 = 9.99: 9 pixels");
}

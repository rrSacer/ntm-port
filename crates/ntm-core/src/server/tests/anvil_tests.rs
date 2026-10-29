//! The anvils in the fake world (docs/maquinas.md, "Bigorna"): the items
//! are the engine's slots and the screen's buttons press through the
//! bridge, in the server's tick. Against the NTM's tables
//! (`data/machines/anvil.json`).
use super::*;
use crate::anvil::{FORGE_OUT, GRID, LEFT, OUT, RIGHT, SLOTS, Tables, buttons};
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [4, 70, 0];

fn anvil(g: &mut Game, block: &str) {
    g.place(AT, &format!("{block}[facing=north,honey_level=0]"));
    g.world.borrow_mut().engine.insert((OW.to_string(), AT), (vec![None; SLOTS], Vec::new()));
    g.run(2);
}

fn put(g: &mut Game, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[i] = s;
}

fn slots(g: &Game) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), AT)].0.clone()
}

fn press(g: &mut Game, b: u32) {
    g.world.borrow_mut().presses.push((OW.to_string(), AT, b));
    g.run(1);
}

fn count(s: &[Option<Stack>], id: &str) -> u32 {
    s.iter().flatten().filter(|x| x.id == id).map(|x| x.count).sum()
}

/// An iron anvil and 10 steel ingots make a steel anvil, on the forge
/// button, in one tick: the inputs go and the result comes together.
#[test]
fn forging_an_anvil_upgrade_through_the_engine_slots() {
    let mut g = game();
    anvil(&mut g, "ntm:anvil_iron");
    put(&mut g, LEFT, Some(Stack::new("ntm:anvil_iron", 1)));
    put(&mut g, RIGHT, Some(Stack::new("ntm:ingot_steel", 12)));
    g.run(2);
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), AT)].1.get(4), Some(&1), "a tela sabe que casa");
    press(&mut g, buttons::FORGE);
    let s = slots(&g);
    assert_eq!((s[LEFT].clone(), s[RIGHT].clone(), s[FORGE_OUT].clone()), (None, Some(Stack::new("ntm:ingot_steel", 2)), Some(Stack::new("ntm:anvil_steel", 1))));
    // Again: no anvil on the left, nothing.
    press(&mut g, buttons::FORGE);
    assert_eq!(count(&slots(&g), "ntm:ingot_steel"), 2);
}

/// Steel plates on a tier 3 anvil from the grid, once and as many as fit;
/// the recipe picked survives a restart (saved with the block entity).
#[test]
fn construction_from_the_grid_and_the_recipe_kept_over_a_restart() {
    let t = Tables::ntm();
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    anvil(&mut g, "ntm:anvil_desh");
    let plate = t
        .listed(3)
        .into_iter()
        .find(|&i| {
            let c = &t.construction[i];
            c.inputs.len() == 1 && c.inputs[0].ids.iter().any(|x| x == "ntm:ingot_steel") && c.outputs.first().is_some_and(|o| o.0 == "ntm:plate_steel")
        })
        .expect("a chapa de aço, nível 3");
    {
        let r = g.ready();
        let Some(Data::Anvil(a)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
        a.recipe = Some(plate);
    }
    put(&mut g, GRID.start, Some(Stack::new("ntm:ingot_steel", 40)));
    put(&mut g, GRID.start + 5, Some(Stack::new("ntm:ingot_steel", 40)));
    g.run(2);
    press(&mut g, buttons::CRAFT);
    assert_eq!(count(&slots(&g)[OUT], "ntm:plate_steel"), 1);
    press(&mut g, buttons::CRAFT_MAX);
    let s = slots(&g);
    assert_eq!(count(&s[OUT], "ntm:plate_steel"), 65, "um pacote de 64 a mais");
    assert_eq!(count(&s[GRID], "ntm:ingot_steel"), 15);
    // The world saved (its slots and values) and the mod's save lost: the
    // recipe comes back from the block entity.
    g.run(5);
    drop(g);
    *kv.borrow_mut() = MemKv::default();
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(10);
    g.place(AT, "ntm:anvil_desh[facing=north,honey_level=0]");
    g.run(3);
    match g.machine(AT).data {
        Data::Anvil(a) => assert_eq!(a.recipe, Some(plate)),
        other => panic!("{other:?}"),
    }
}

/// A tier 1 anvil doesn't make a tier 3 recipe, even pressed.
#[test]
fn an_iron_anvil_refuses_higher_tiers() {
    let t = Tables::ntm();
    let mut g = game();
    anvil(&mut g, "ntm:anvil_iron");
    let Some(high) = (0..t.construction.len()).find(|&i| !t.construction[i].tier_valid(1)) else { return };
    {
        let r = g.ready();
        let Some(Data::Anvil(a)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
        a.recipe = Some(high);
    }
    for (k, inp) in t.construction[high].inputs.iter().enumerate().take(12) {
        put(&mut g, GRID.start + k, Some(Stack::new(&inp.ids[0], inp.count)));
    }
    let before = slots(&g);
    g.run(2);
    press(&mut g, buttons::CRAFT);
    assert_eq!(slots(&g), before);
}

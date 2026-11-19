//! The foundry, stage 1, in the fake world (docs/maquinas.md, "Fundição"):
//! a firebox heats the crucible above it, items melt, an alloy is picked on
//! the old screen, and the waste stream pours into what is under it. Items
//! are set on the machines directly (their slots are the engine's).
use super::*;
use crate::foundry::{crucible::Crucible, firebox::Firebox};
use crate::server::container_bridge::Stack;
use crate::blast::visuals::Show;

const FIRE: Pos = [0, 70, 0];
const CORE: Pos = [0, 71, 0];
/// Facing north: the waste pours 2 blocks south of the core.
const UNDER_WASTE: Pos = [0, 70, 2];

fn crucible(g: &mut Game, at: Pos) -> Crucible {
    match g.machine(at).data {
        Data::Crucible(c) => c,
        other => panic!("{other:?}"),
    }
}

fn firebox(g: &mut Game) -> Firebox {
    match g.machine(FIRE).data {
        Data::Firebox(f) => f,
        other => panic!("{other:?}"),
    }
}

fn edit<F: FnOnce(&mut Data)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    f(&mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data);
}

fn foundry(g: &mut Game) {
    g.place(FIRE, "ntm:heater_firebox[facing=north,honey_level=0]");
    g.place(CORE, "ntm:machine_crucible[facing=north,honey_level=0]");
    g.run(2);
    edit(g, FIRE, |d| {
        let Data::Firebox(f) = d else { panic!() };
        f.slots[0] = Some(Stack::new("minecraft:coal", 16));
    });
}

#[test]
fn a_firebox_heats_the_crucible_and_iron_melts_and_pours() {
    let mut g = game();
    foundry(&mut g);
    // A second crucible under the waste stream catches what pours.
    g.place(UNDER_WASTE, "ntm:machine_crucible[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.slots[1] = Some(Stack::new("minecraft:iron_ingot", 4));
    });
    // Coal in the firebox (200 TU a tick) warms it up; the crucible takes a
    // quarter of the difference a tick, so both climb at ~100 a tick: the
    // 50 000 to melt come after ~500 ticks.
    g.run(650);
    let (c, f) = (crucible(&mut g, CORE), firebox(&mut g));
    assert!(f.on, "a fornalha não acendeu");
    assert!(c.heat >= 50_000, "o cadinho não esquentou: {} (fornalha {})", c.heat, f.heat);
    // Melted, poured out the waste side into the crucible below.
    let mut melted = 0;
    for _ in 0..600 {
        g.run(1);
        let below = crucible(&mut g, UNDER_WASTE);
        melted = below.waste_stack.iter().find(|m| m.0 == "iron").map_or(0, |m| m.1);
        if melted == 4 * 72 {
            break;
        }
    }
    assert_eq!(melted, 4 * 72, "o ferro não chegou ao cadinho de baixo");
    assert!(crucible(&mut g, CORE).waste_stack.is_empty());
    assert_eq!(crucible(&mut g, CORE).slots[1], None);
}

#[test]
fn an_alloy_picked_is_made() {
    let mut g = game();
    foundry(&mut g);
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.recipe = Some("crucible.steel".into());
    });
    // Iron, carbon and flux in the recipe stack become steel; nothing to
    // pour into, so it stays.
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.recipe_stack = vec![("iron".into(), 16), ("carbon".into(), 24), ("flux".into(), 8)];
    });
    g.run(25);
    assert_eq!(crucible(&mut g, CORE).recipe_stack, vec![("steel".to_string(), 16)]);
}

#[test]
fn breaking_a_crucible_drops_its_items_and_scraps() {
    let mut g = game();
    foundry(&mut g);
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.slots[2] = Some(Stack::new("minecraft:iron_ingot", 1));
        c.waste_stack = vec![("iron".into(), 150), ("copper".into(), 30)];
    });
    g.world.borrow_mut().set(OW, CORE, "minecraft:air");
    g.break_block(CORE);
    g.run(60);
    let dropped = g.world.borrow().dropped.clone();
    // The ingot, and one scrap per stack with all of its quanta (D4: the 30 of copper under an ingot too).
    assert_eq!(dropped.len(), 3, "{dropped:?}");
    assert!(dropped.iter().any(|d| d.ends_with("ntm:scraps__iron 1 {ntm_amount:150}")), "{dropped:?}");
    assert!(dropped.iter().any(|d| d.ends_with("ntm:scraps__copper 1 {ntm_amount:30}")), "{dropped:?}");
}

// ------------------------------------------------------------- stage 2

fn melt_at(g: &mut Game, at: Pos) -> (Option<String>, u32) {
    match g.machine(at).data {
        Data::Channel(c) => (c.melt.mat, c.melt.amount),
        Data::FoundryTank(c) => (c.melt.mat, c.melt.amount),
        other => panic!("{other:?}"),
    }
}

/// The waste stream falls on a channel; two channels lead to an outlet
/// (facing south, fed from behind) over two tanks: the upper one fills the
/// lower first. Nothing is lost on the way.
#[test]
fn metal_runs_through_channels_and_an_outlet_into_tanks() {
    let mut g = game();
    g.place(CORE, "ntm:machine_crucible[facing=north,honey_level=0]");
    let (ch1, ch2, outlet, tank1, tank2) = ([0, 70, 2], [0, 70, 3], [0, 70, 4], [0, 68, 4], [0, 67, 4]);
    g.place(ch1, "ntm:foundry_channel[facing=north,honey_level=0]");
    g.place(ch2, "ntm:foundry_channel[facing=north,honey_level=0]");
    g.place(outlet, "ntm:foundry_outlet[facing=south,honey_level=0]");
    g.place(tank1, "ntm:foundry_tank[facing=north,honey_level=0]");
    g.place(tank2, "ntm:foundry_tank[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.waste_stack = vec![("iron".into(), 300)];
    });
    g.run(400);
    let total = |g: &mut Game| {
        let mut n = crucible(g, CORE).waste_stack.iter().map(|m| m.1).sum::<u32>();
        for p in [ch1, ch2, tank1, tank2] {
            n += melt_at(g, p).1;
        }
        n
    };
    assert_eq!(total(&mut g), 300, "metal perdido ou criado no caminho");
    let (low, high) = (melt_at(&mut g, tank2), melt_at(&mut g, tank1));
    assert_eq!(low.0.as_deref(), Some("iron"));
    assert!(low.1 + high.1 >= 280, "nos tanques: {} + {}", low.1, high.1);
    assert!(low.1 >= high.1, "o de baixo enche primeiro: {} e {}", low.1, high.1);
}

/// A pour onto a channel whose network already holds another material is
/// refused; an outlet's filter stops what it doesn't let through.
#[test]
fn channels_keep_one_material_and_outlets_filter() {
    let mut g = game();
    g.place(CORE, "ntm:machine_crucible[facing=north,honey_level=0]");
    let (ch1, ch2) = ([0, 70, 2], [1, 70, 2]);
    g.place(ch1, "ntm:foundry_channel[facing=north,honey_level=0]");
    g.place(ch2, "ntm:foundry_channel[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, ch2, |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = crate::foundry::flow::Melt { mat: Some("copper".into()), amount: 10 };
        c.next = 200; // holds still
    });
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.waste_stack = vec![("iron".into(), 100)];
    });
    g.run(20);
    assert_eq!(crucible(&mut g, CORE).waste_stack, vec![("iron".to_string(), 100)], "derramou ferro numa rede de cobre");
    assert_eq!(melt_at(&mut g, ch1).1, 0);
    // The copper gone (a channel keeps its material at 0 quanta), the
    // network takes iron.
    edit(&mut g, ch2, |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt.amount = 0;
    });
    g.run(20);
    assert!(melt_at(&mut g, ch1).1 + melt_at(&mut g, ch2).1 > 0, "uma rede vazia, que já levou cobre, recusou ferro");
    // The outlet: only copper passes.
    let mut o = crate::foundry::flow::Outlet { filter: Some("copper".into()), ..Default::default() };
    assert!(o.passes("copper") && !o.passes("iron"));
    o.invert = true;
    assert!(!o.passes("copper") && o.passes("iron"));
}

// ------------------------------------------------------------- stage 3

/// The alloy's stream (facing side) falls into an ingot mold: full at 72
/// quanta, it cools for 200 ticks into a steel ingot, and takes no more
/// metal until the ingot is out.
#[test]
fn steel_pours_into_an_ingot_mold_and_casts() {
    let mut g = game();
    g.place(CORE, "ntm:machine_crucible[facing=north,honey_level=0]");
    let mold_at = [0, 70, -2];
    g.place(mold_at, "ntm:foundry_mold[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, mold_at, |d| {
        let Data::Mold(c) = d else { panic!() };
        c.slots[0] = Some(Stack::new("ntm:mold__ingot", 1));
    });
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.recipe = Some("crucible.steel".into());
        c.recipe_stack = vec![("steel".into(), 100)];
    });
    g.run(10);
    let Data::Mold(m) = g.machine(mold_at).data else { panic!() };
    assert_eq!((m.melt.mat.as_deref(), m.melt.amount), (Some("steel"), 72), "o molde enche com 72");
    g.run(210);
    let Data::Mold(m) = g.machine(mold_at).data else { panic!() };
    assert_eq!(m.slots[1], Some(Stack::new("ntm:ingot_steel", 1)));
    g.run(20);
    assert_eq!(crucible(&mut g, CORE).recipe_stack, vec![("steel".to_string(), 28)], "com o lingote no molde, nada mais entra");
}

/// The in-game line (`tools/test-game/foundry.py canais`): a crucible
/// facing east pours its waste 2 blocks west onto a channel; channel,
/// tank, an outlet facing west, and a mold one block under the outlet.
/// Every block is new, with the engine's zeros as its saved values (a
/// block entity the mod never wrote): material 0 is none, not stone.
#[test]
fn a_line_of_channel_tank_and_outlet_fills_a_mold() {
    let mut g = game();
    let core = [0, 71, 0];
    let (ch1, ch2, tank, outlet, mold) = ([-2, 70, 0], [-3, 70, 0], [-4, 70, 0], [-5, 70, 0], [-5, 69, 0]);
    // (position, block, engine slots, data length: `horizon_machine` `data`)
    let line: [(Pos, &str, usize, usize); 6] = [
        (core, "ntm:machine_crucible[facing=east,honey_level=0]", 10, 128),
        (ch1, "ntm:foundry_channel[facing=south,honey_level=0]", 0, 5),
        (ch2, "ntm:foundry_channel[facing=south,honey_level=0]", 0, 5),
        (tank, "ntm:foundry_tank[facing=south,honey_level=0]", 0, 5),
        (outlet, "ntm:foundry_outlet[facing=west,honey_level=0]", 0, 4),
        (mold, "ntm:foundry_mold[facing=west,honey_level=0]", 2, 8),
    ];
    for (p, state, slots, data) in line {
        g.place(p, state);
        g.world.borrow_mut().engine.insert((OW.to_string(), p), (vec![None; slots], vec![0; data]));
    }
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), mold)).unwrap().0[0] = Some(Stack::new("ntm:mold__ingot", 1));
    g.run(2);
    edit(&mut g, core, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.waste_stack = vec![("iron".into(), 144)];
    });
    g.run(600);
    let cast = match g.machine(mold).data {
        Data::Mold(c) => (g.world.borrow().engine[&(OW.to_string(), mold)].0[1].clone(), c.melt.amount),
        other => panic!("{other:?}"),
    };
    assert_eq!(cast.0, Some(Stack::new("minecraft:iron_ingot", 1)), "o molde sob a saída: {cast:?}; tanque {:?}, canais {:?} {:?}", melt_at(&mut g, tank), melt_at(&mut g, ch1), melt_at(&mut g, ch2));
}

#[test]
fn a_pour_splashes_where_it_lands() {
    let mut g = game();
    foundry(&mut g);
    // The same line as above: the waste stream lands in the crucible below.
    g.place(UNDER_WASTE, "ntm:machine_crucible[facing=north,honey_level=0]");
    g.run(2);
    edit(&mut g, CORE, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.slots[1] = Some(Stack::new("minecraft:iron_ingot", 4));
    });
    g.run(650);
    let splashes = |g: &Game| -> Vec<(String, [f64; 3])> {
        g.world
            .borrow()
            .shown
            .iter()
            .filter_map(|(dim, s)| match s {
                Show::Particles { id: "minecraft:falling_lava", at, .. } => Some((dim.clone(), *at)),
                _ => None,
            })
            .collect()
    };
    for _ in 0..600 {
        g.run(1);
        if crucible(&mut g, UNDER_WASTE).waste_stack.iter().any(|m| m.0 == "iron") {
            break;
        }
    }
    let s = splashes(&g);
    assert!(!s.is_empty(), "o metal caiu e não respingou");
    // Splashes on the crucible below, a block above its top: where the stream lands.
    assert!(s.iter().all(|(dim, at)| dim == OW && *at == [0.5, 71.0, 2.5]), "{s:?}");
}

//! The PUREX in the fake world (docs/maquinas.md, "PUREX"): a crafter
//! station on the NTM's table (`data/machines/purex.json`), its items put
//! in by the engine's slots (`Layout::Crafter`, inputs 4..7, outputs
//! 7..13), its tanks typed by the recipe.
use super::*;
use crate::crafter::{Crafter, Station};
use crate::server::container_bridge::{Slots, Stack};

const AT: Pos = [4, 70, 0];

fn purex(g: &mut Game) {
    g.place(AT, "ntm:machine_purex[facing=north,honey_level=0]");
    g.world.borrow_mut().engine.insert((OW.to_string(), AT), (vec![None; 13], Vec::new()));
    g.run(2);
}

fn edit(g: &mut Game, f: &dyn Fn(&mut Crafter)) {
    let r = g.ready();
    let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) else { panic!() };
    f(c);
}

fn crafter(g: &mut Game) -> Crafter {
    match g.machine(AT).data {
        Data::Crafter(c) => c,
        other => panic!("{other:?}"),
    }
}

fn slots(g: &Game) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), AT)].0.clone()
}

fn put(g: &mut Game, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[i] = s;
}

fn count(s: &[Option<Stack>], id: &str) -> u32 {
    s.iter().flatten().filter(|x| x.id == id).map(|x| x.count).sum()
}

/// Spent MEU fuel with kerosene and nitric acid: 3 U-238, 4 plutonium and
/// 2 technetium nuggets a rod, 2 500 HE a tick for 100 ticks. Then the
/// first input of another recipe of the group (MEN fuel) put in: it
/// switches and goes on.
#[test]
fn the_purex_reprocesses_pwr_fuel_and_switches_in_its_group() {
    let mut g = game();
    purex(&mut g);
    assert_eq!(crafter(&mut g).station, Station::Purex);
    let pick = |g: &mut Game, name: &str| {
        let r = Station::Purex.recipes().iter().find(|r| r.name == name).unwrap();
        edit(g, &|c| c.set_recipe(r));
    };
    pick(&mut g, "purex.pwrmeu");
    let fill = |g: &mut Game| {
        edit(g, &|c| {
            c.tin[0].fill("kerosene", 24_000);
            c.tin[1].fill("nitric_acid", 24_000);
            c.power = c.max_power();
        })
    };
    fill(&mut g);
    put(&mut g, 4, Some(Stack::new("ntm:pwr_fuel_depleted__meu", 2)));
    for _ in 0..210 {
        edit(&mut g, &|c| c.power = c.max_power());
        g.run(1);
    }
    let s = slots(&g);
    assert_eq!((count(&s, "ntm:nugget_u238"), count(&s, "ntm:nugget_plutonium"), count(&s, "ntm:nugget_technetium")), (6, 8, 4), "{s:?}");
    assert_eq!(count(&s, "ntm:pwr_fuel_depleted__meu"), 0);
    let c = crafter(&mut g);
    assert_eq!((c.tin[0].mb, c.tin[1].mb), (23_000, 23_500));
    // MEN fuel in: the machine switches to its recipe.
    put(&mut g, 5, Some(Stack::new("ntm:pwr_fuel_depleted__men", 1)));
    for _ in 0..110 {
        edit(&mut g, &|c| c.power = c.max_power());
        g.run(1);
    }
    assert_eq!(crafter(&mut g).recipe.as_deref(), Some("purex.pwrmen"));
    let s = slots(&g);
    assert_eq!(count(&s, "ntm:nugget_pu239"), 4, "{s:?}");
}

/// Depleted thorium salt back to thorium salt, with the U-233 (half the
/// time) and the waste (a quarter) by chance.
#[test]
fn thorium_salt_comes_back_with_chance_outputs() {
    let mut g = game();
    purex(&mut g);
    let r = Station::Purex.recipes().iter().find(|r| r.name == "purex.thoriumsalt").unwrap();
    edit(&mut g, &|c| {
        c.set_recipe(r);
        c.tin[0].cap = 24_000;
    });
    put(&mut g, 4, Some(Stack::new("ntm:nugget_th232", 64)));
    for k in 0..32 {
        edit(&mut g, &|c| {
            c.tin[0].fill("thorium_salt_depleted", 16_000);
        });
        for _ in 0..21 {
            edit(&mut g, &|c| c.power = c.max_power());
            g.run(1);
        }
        let c = crafter(&mut g);
        assert_eq!((c.tin[0].mb, c.tout[0].fluid.as_str(), c.tout[0].mb), (0, "thorium_salt", 16_000), "vez {k}");
        edit(&mut g, &|c| c.tout[0].mb = 0);
    }
    let s = slots(&g);
    assert_eq!(count(&s, "ntm:nugget_th232"), 0, "32 vezes 2: {s:?}");
    let u233 = count(&s, "ntm:nugget_u233");
    assert!((8..=24).contains(&u233), "metade de 32, mais ou menos: {u233}");
    assert!(count(&s, "ntm:nuclear_waste_tiny") <= 16);
}

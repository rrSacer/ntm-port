//! The bedrock chain, stage 1, in the fake world (docs/maquinas.md,
//! "Bedrock"): the excavator digs its shaft down to a deposit and draws
//! raw ore from it; the ore slopper turns it into BASE ores by the
//! densities the raw ore carries.
use super::*;
use crate::bedrock::{self, slopper};
use crate::excavator::{MAX_POWER, slot};
use crate::server::container_bridge::{Slots, Stack};

/// The fake world's overworld: bedrock at −64, deepslate −63..−51.
const CORE: Pos = [4, -50, 3];

fn engine(g: &mut Game, at: Pos, n: usize) {
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; n], Vec::new()));
}

fn slots(g: &Game, at: Pos) -> Slots {
    g.world.borrow().engine[&(OW.to_string(), at)].0.clone()
}

fn put(g: &mut Game, at: Pos, i: usize, s: Option<Stack>) {
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = s;
}

fn count(s: &[Option<Stack>], id: &str) -> u32 {
    s.iter().flatten().filter(|x| x.id == id).map(|x| x.count).sum()
}

#[test]
fn the_excavator_digs_to_a_deposit_and_draws_raw_ore() {
    let mut g = game();
    let deposit = [CORE[0], -64, CORE[2]];
    g.world.borrow_mut().set(OW, deposit, "ntm:ore_bedrock");
    g.place(CORE, "ntm:machine_excavator[facing=north,honey_level=0]");
    engine(&mut g, CORE, slot::COUNT);
    g.run(2);
    put(&mut g, CORE, slot::DRILL, Some(Stack::new("ntm:drillbit__ferro", 1)));
    // What the deposit asks, in the tank.
    let dep = bedrock::deposit(deposit[0], deposit[2]);
    let power = |g: &mut Game| {
        let r = g.ready();
        if let Some(Data::Excavator(e)) = r.world.machines.get_mut(&(OW.to_string(), CORE)).map(|m| &mut m.data) {
            e.power = MAX_POWER;
            // On (its button, out of the test player's reach down here).
            e.enabled = true;
            if let Some((f, _)) = dep.fluid {
                e.tank.set_type(f);
                e.tank.mb = e.tank.cap;
            }
        }
    };
    power(&mut g);
    // 10 layers of deepslate (27 work ÷ 2.5 a layer), then 2 400 ticks an
    // extraction.
    for _ in 0..2_800 {
        power(&mut g);
        g.run(1);
    }
    let s = slots(&g, CORE);
    assert_eq!(count(&s, "minecraft:cobbled_deepslate"), 90, "o poço de 3×3 × 10: {s:?}");
    for y in -63..=-54 {
        assert_eq!(g.world.borrow().get(OW, [CORE[0] + 1, y, CORE[2] - 1]), "minecraft:air", "y {y}");
    }
    assert_eq!(g.world.borrow().get(OW, deposit), "ntm:ore_bedrock", "o depósito fica");
    let raw: Vec<_> = s.iter().flatten().filter(|x| x.id == slopper::RAW).collect();
    assert_eq!(raw.len(), 1, "{s:?}");
    // Its densities: the chunk's, times the ferro bit's fortune (1.1).
    let d = bedrock::read_ore_data(raw[0].custom.as_deref());
    for (a, b) in d.iter().zip(bedrock::densities(deposit[0], deposit[2])) {
        assert!((a - b * 1.1).abs() < 1e-5, "{d:?}");
    }
}

/// A deposit of a higher tier than the bit: nothing comes.
#[test]
fn a_bit_below_the_deposits_tier_draws_nothing() {
    let Some((x, z)) = (0..64).flat_map(|i| (0..64).map(move |j| (i * 16 + 4, j * 16 + 3))).find(|&(x, z)| bedrock::deposit(x, z).tier >= 2) else {
        panic!("sem depósito de nível 2+")
    };
    let mut g = game();
    let core = [x, -50, z];
    // The fake world is 64×64 around 0: this one stands on its own column.
    g.world.borrow_mut().set(OW, [x, -64, z], "ntm:ore_bedrock");
    g.place(core, "ntm:machine_excavator[facing=north,honey_level=0]");
    engine(&mut g, core, slot::COUNT);
    g.run(2);
    put(&mut g, core, slot::DRILL, Some(Stack::new("ntm:drillbit__steel", 1)));
    for _ in 0..6_200 {
        let r = g.ready();
        if let Some(Data::Excavator(e)) = r.world.machines.get_mut(&(OW.to_string(), core)).map(|m| &mut m.data) {
            e.power = MAX_POWER;
            e.enabled = true;
        }
        g.run(1);
    }
    assert_eq!(count(&slots(&g, core), slopper::RAW), 0);
}

/// The raw ore through the slopper: BASE ores by its densities.
#[test]
fn the_slopper_reads_the_raw_ores_densities() {
    let mut g = game();
    let at = [8, 70, 0];
    g.place(at, "ntm:machine_ore_slopper[facing=north,honey_level=0]");
    engine(&mut g, at, slopper::SLOTS);
    g.run(2);
    let dens = [2.5, 0.0, 1.0, 0.0, 0.5, 0.0];
    put(&mut g, at, slopper::INPUT, Some(Stack { custom: Some(bedrock::ore_data(&dens, 1.0)), ..Stack::new(slopper::RAW, 2) }));
    for _ in 0..(2 * slopper::TICKS + 4) {
        let r = g.ready();
        if let Some(Data::OreSlopper(m)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
            m.power = slopper::MAX_POWER;
            m.water.mb = m.water.cap;
            m.slop.mb = 0;
        }
        g.run(1);
    }
    let s = slots(&g, at);
    assert_eq!((count(&s, &slopper::base(0)), count(&s, &slopper::base(2)), count(&s, &slopper::base(4))), (5, 2, 1), "{s:?}");
    assert_eq!(count(&s, slopper::RAW), 0);
}

/// The centrifuge's bedrock rows (`CentrifugeRecipes.java:267-288`): a
/// BASE light ore into a PRIMARY and gravel, the PRIMARY into 9 iron and 9
/// copper fragments.
#[test]
fn the_centrifuge_takes_bedrock_ore_to_fragments() {
    let mut g = game();
    let at = [8, 70, 2];
    g.place(at, "ntm:machine_centrifuge[facing=north,honey_level=0]");
    engine(&mut g, at, 5);
    g.run(2);
    let run = |g: &mut Game, ticks: u32| {
        for _ in 0..ticks {
            let r = g.ready();
            if let Some(Data::Crafter(c)) = r.world.machines.get_mut(&(OW.to_string(), at)).map(|m| &mut m.data) {
                c.power = c.max_power();
            }
            g.run(1);
        }
    };
    put(&mut g, at, 0, Some(Stack::new(&slopper::base(0), 1)));
    run(&mut g, 210);
    let s = slots(&g, at);
    assert_eq!((count(&s, "ntm:bedrock_ore__primary_light"), count(&s, "minecraft:gravel")), (1, 1), "{s:?}");
    // The PRIMARY back in as the input.
    let i = s.iter().position(|x| x.as_ref().is_some_and(|x| x.id == "ntm:bedrock_ore__primary_light")).unwrap();
    put(&mut g, at, i, None);
    put(&mut g, at, 0, Some(Stack::new("ntm:bedrock_ore__primary_light", 1)));
    run(&mut g, 210);
    let s = slots(&g, at);
    assert_eq!((count(&s, "ntm:bedrock_ore_fragment__iron"), count(&s, "ntm:bedrock_ore_fragment__copper")), (9, 9), "{s:?}");
}

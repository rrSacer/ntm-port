//! The foundry's blocks by hand (`onBlockActivated` of the channel, the tank, the mold and the basin, the crucible): the shovel
//! takes the metal as exact scraps, a mold goes in by a click, the cast comes out by a click. One transfer each: a refusal moves
//! nothing.
use super::*;
use crate::foundry::flow::Melt;
use crate::server::container_bridge::Stack;

const AT: Pos = [0, 70, 0];

fn put_machine(g: &mut Game, state: &str, slots: usize) {
    g.place(AT, state);
    g.world.borrow_mut().engine.insert((OW.to_string(), AT), (vec![None; slots], Vec::new()));
    g.run(2);
}

fn hands(g: &mut Game, inventory: Vec<Option<Stack>>) {
    let inventory = inventory.into_iter().map(|s| s.map(|stack| HeldStack { encoded: format!("{stack:?}"), stack, plain: true })).collect();
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory, ..Default::default() });
}
fn inventory(g: &Game) -> Vec<Option<Stack>> {
    g.world.borrow().hands.as_ref().unwrap().inventory.iter().map(|s| s.as_ref().map(|h| h.stack.clone())).collect()
}
fn used(g: &mut Game) -> bool {
    g.server.native_foundry_use(&mut g.sh, OW, AT, AT, "p1")
}
fn edit(g: &mut Game, f: impl FnOnce(&mut Data)) {
    f(&mut g.ready().world.machines.get_mut(&(OW.to_string(), AT)).unwrap().data);
}
fn shovel() -> Option<Stack> {
    Some(Stack::new("minecraft:iron_shovel", 1))
}
fn slots(g: &Game) -> Vec<Option<Stack>> {
    g.world.borrow().engine[&(OW.to_string(), AT)].0.clone()
}

#[test]
fn a_shovel_takes_a_channels_metal_as_one_exact_scrap_and_the_click_is_taken_even_when_empty() {
    let mut g = game();
    put_machine(&mut g, "ntm:foundry_channel[facing=north,honey_level=0]", 0);
    edit(&mut g, |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = Melt { mat: Some("iron".into()), amount: 100 };
    });
    hands(&mut g, vec![shovel(), None]);
    g.world.borrow_mut().refuse_commits = true;
    assert!(used(&mut g));
    assert_eq!(inventory(&g)[1], None, "refused: nothing moves");
    g.world.borrow_mut().refuse_commits = false;
    assert!(used(&mut g));
    assert_eq!(inventory(&g)[1], Some(crate::foundry::scrap::stack("iron", 100)));
    let Data::Channel(c) = g.machine(AT).data else { panic!() };
    assert_eq!(c.melt, Melt::default());
    assert!(used(&mut g), "an empty channel: the click is taken, nothing happens");
    assert_eq!(inventory(&g)[1].as_ref().map(|s| s.count), Some(1));
    // Not a shovel: the click goes on.
    hands(&mut g, vec![Some(Stack::new("minecraft:stick", 1)), None]);
    assert!(!used(&mut g));
}

#[test]
fn a_shovel_takes_a_tanks_metal_the_same_way() {
    let mut g = game();
    put_machine(&mut g, "ntm:foundry_tank[facing=north,honey_level=0]", 0);
    edit(&mut g, |d| {
        let Data::FoundryTank(c) = d else { panic!() };
        c.melt = Melt { mat: Some("steel".into()), amount: 1_000 };
    });
    hands(&mut g, vec![shovel(), None]);
    assert!(used(&mut g));
    assert_eq!(inventory(&g)[1], Some(crate::foundry::scrap::stack("steel", 1_000)));
}

#[test]
fn a_mold_blocks_click_takes_the_cast_first_then_the_mold_of_its_size_then_the_shovel() {
    let mut g = game();
    put_machine(&mut g, "ntm:foundry_mold[facing=north,honey_level=0]", 2);
    // A cast in its slot comes out, whatever the hand holds (even nothing): the click is taken.
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), AT)).unwrap().0[1] = Some(Stack::new("ntm:ingot_steel", 1));
    hands(&mut g, vec![None, None]);
    assert!(used(&mut g));
    assert_eq!(slots(&g)[1], None);
    assert_eq!(inventory(&g)[0], Some(Stack::new("ntm:ingot_steel", 1)));
    // A mold of another size (a basin's) does not go into a mold block; one of its size does, one of the stack.
    hands(&mut g, vec![Some(Stack::new("ntm:mold__block", 2)), Some(Stack::new("ntm:mold__ingot", 3))]);
    assert!(!used(&mut g), "a large mold: the click goes on");
    g.world.borrow_mut().hands.as_mut().unwrap().hand = Some(PlayerSlot::Inventory(1));
    assert!(used(&mut g));
    assert_eq!(slots(&g)[0], Some(Stack::new("ntm:mold__ingot", 1)));
    assert_eq!(inventory(&g)[1], Some(Stack::new("ntm:mold__ingot", 2)));
    // The slot taken: a shovel takes the pool.
    edit(&mut g, |d| {
        let (Data::Mold(c) | Data::Basin(c)) = d else { panic!() };
        c.melt = Melt { mat: Some("steel".into()), amount: 20 };
    });
    hands(&mut g, vec![shovel(), None]);
    assert!(used(&mut g));
    assert_eq!(inventory(&g)[1], Some(crate::foundry::scrap::stack("steel", 20)));
}

#[test]
fn a_basin_takes_a_large_mold_only() {
    let mut g = game();
    put_machine(&mut g, "ntm:foundry_basin[facing=north,honey_level=0]", 2);
    hands(&mut g, vec![Some(Stack::new("ntm:mold__ingot", 1)), None]);
    assert!(!used(&mut g), "a small mold is no use for a basin");
    hands(&mut g, vec![Some(Stack::new("ntm:mold__block", 1)), None]);
    assert!(used(&mut g));
    assert_eq!(slots(&g)[0], Some(Stack::new("ntm:mold__block", 1)));
}

#[test]
fn a_shovel_takes_each_stack_of_a_crucible_as_its_own_exact_scrap() {
    let mut g = game();
    put_machine(&mut g, "ntm:machine_crucible[facing=north,honey_level=0]", 10);
    edit(&mut g, |d| {
        let Data::Crucible(c) = d else { panic!() };
        c.recipe_stack = vec![("steel".into(), 200)];
        c.waste_stack = vec![("iron".into(), 150), ("copper".into(), 30)];
    });
    hands(&mut g, vec![shovel(), None, None, None]);
    assert!(used(&mut g));
    let inv = inventory(&g);
    let scraps: Vec<_> = inv[1..].iter().flatten().cloned().collect();
    assert_eq!(
        scraps,
        vec![crate::foundry::scrap::stack("steel", 200), crate::foundry::scrap::stack("iron", 150), crate::foundry::scrap::stack("copper", 30)]
    );
    let Data::Crucible(c) = g.machine(AT).data else { panic!() };
    assert!(c.recipe_stack.is_empty() && c.waste_stack.is_empty());
    // Without a shovel the screen opens.
    hands(&mut g, vec![Some(Stack::new("minecraft:stick", 1)), None]);
    assert!(!used(&mut g));
}

/// The Java reads `getHeldItem` alone: a shovel in the off hand is no use (the click goes on).
#[test]
fn only_the_main_hand_acts() {
    let mut g = game();
    put_machine(&mut g, "ntm:foundry_channel[facing=north,honey_level=0]", 0);
    edit(&mut g, |d| {
        let Data::Channel(c) = d else { panic!() };
        c.melt = Melt { mat: Some("iron".into()), amount: 100 };
    });
    let off = HeldStack { encoded: "off".into(), stack: Stack::new("minecraft:iron_shovel", 1), plain: true };
    g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Offhand), inventory: vec![None, None], offhand: Some(off), ..Default::default() });
    assert!(!used(&mut g));
    let Data::Channel(c) = g.machine(AT).data else { panic!() };
    assert_eq!(c.melt.amount, 100);
}

#[test]
fn the_shovels_come_from_the_content() {
    use crate::server::foundry_hand::is_shovel;
    for yes in ["minecraft:wooden_shovel", "minecraft:netherite_shovel", "ntm:titanium_shovel"] {
        assert!(is_shovel(yes), "{yes}");
    }
    // A name that ends in `_shovel` is no proof of a class (a mod item with none); the pickaxe and the sword are not shovels.
    for no in ["ntm:titanium_pickaxe", "minecraft:iron_sword", "ntm:made_up_shovel", "minecraft:stick"] {
        assert!(!is_shovel(no), "{no}");
    }
}

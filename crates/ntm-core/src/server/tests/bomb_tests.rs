//! Bombs in the fake world (docs/bombas.md): parts in their slots,
//! redstone, fuses, the detonator, the explosions, their radiation and
//! settings.
use super::*;
use crate::bomb;
use crate::server::container_bridge::Stack;

/// A stone floor y 60..=69 over `r` blocks around the origin.
pub(super) fn floor(g: &mut Game, r: i32) {
    let mut w = g.world.borrow_mut();
    for x in -r..=r {
        for z in -r..=r {
            for y in 60..70 {
                w.set(OW, [x, y, z], "minecraft:stone");
            }
        }
    }
}

/// A game with a stone floor and nukes cut to radius `max_r`.
fn bomb_game(max_r: i32) -> Game {
    let mut g = game();
    floor(&mut g, 24);
    let who = player();
    let out = g.server.bomb_command(&mut g.sh, Some(&who), &format!("config mundo raiomax {max_r}"));
    assert!(out.is_ok(), "{out:?}");
    g.run(2);
    g
}

/// The bomb `id` at `at`, its block entity with a slot per part.
pub(super) fn place_bomb(g: &mut Game, at: Pos, id: &str) {
    g.place(at, &format!("{id}[lit=false]"));
    let n = bomb::bomb_type(id).map_or(0, |k| k.slots.len());
    g.world.borrow_mut().engine.insert((OW.to_string(), at), (vec![None; n], Vec::new()));
    g.run(2);
}

/// Redstone on the bomb at `at`, or none.
pub(super) fn power(g: &mut Game, at: Pos, on: bool) {
    g.world.borrow_mut().signal.insert((OW.to_string(), at), if on { 15 } else { 0 });
}

/// Puts components `range` of the bomb at `at` in its slots.
pub(super) fn assemble(g: &mut Game, at: Pos, id: &str, range: std::ops::Range<usize>) {
    for (i, s) in bomb::bomb_type(id).unwrap().slots.iter().enumerate().skip(range.start).take(range.len()) {
        g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = Some(Stack::new(s.item, 1));
    }
    g.run(2);
}

pub(super) fn set_creative(g: &mut Game, creative: bool) {
    let mut p = player();
    p.creative = creative;
    g.server.set_players(vec![p]);
}

pub(super) fn bomb_data(g: &mut Game, at: Pos) -> Option<bomb::Bomb> {
    match g.ready().world.machines.get(&(OW.to_string(), at)).map(|m| m.data.clone()) {
        Some(Data::Bomb(b)) => Some(b),
        _ => None,
    }
}

/// Runs until no explosion is active (or `limit` ticks).
fn until_quiet(g: &mut Game, limit: usize) {
    for _ in 0..limit {
        g.run(1);
        if g.server.blasts.nukes.is_empty() {
            return;
        }
    }
    panic!("explosão não terminou: {:?}", g.server.blasts.nukes.active().map(|p| p.line()).collect::<Vec<_>>());
}

#[test]
fn bomb_lit_matches_the_registry() {
    assert!(super::super::bomb_server::BOMB_LIT_FOR_TESTS.starts_with(registry::BOMB_POWERED));
}

#[test]
fn a_little_boy_assembled_in_its_slots_goes_off_by_redstone() {
    let mut g = bomb_game(10);
    set_creative(&mut g, false);
    let at = [3, 70, 3];
    let id = "ntm:nuke_boy";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..4);
    assert!(bomb_data(&mut g, at).unwrap().blast().is_none());
    // Not ready: redstone does nothing.
    power(&mut g, at, true);
    g.run(3);
    assert!(bomb_data(&mut g, at).is_some());
    power(&mut g, at, false);
    g.run(2);
    assemble(&mut g, at, id, 4..5);
    g.run(6);
    // Four already in: the fifth fills the last slot.
    let b = bomb_data(&mut g, at).unwrap();
    assert_eq!(b.slots.iter().flatten().count(), 5, "{b:?}");
    assert_eq!(b.blast().map(|b| b.radius()), Some(120));
    // Redstone: the rising edge sets it off.
    power(&mut g, at, true);
    g.run(2);
    assert!(bomb_data(&mut g, at).is_none(), "a bomba continua");
    assert_eq!(g.world.borrow().get(OW, at), "minecraft:air");
    until_quiet(&mut g, 2000);
    // A crater under it (stone removed), sellafield in it, damage and dose.
    let w = g.world.borrow();
    let dug = (60..70).filter(|&y| w.get(OW, [3, y, 3]) == "minecraft:air").count();
    // R=10 em pedra: o modelo do NTM cava pouco (custo 6^(0,5+3,5i/R)).
    assert!(dug >= 2, "cratera rasa: {dug}");
    assert!(w.blocks.values().any(|s| s == "ntm:sellafield_slaked"));
    assert!(w.hurts.iter().any(|h| h.0 == "p1" && h.2 == "minecraft:explosion"), "sem dano");
    drop(w);
    let dose = g.ready().world.radiation.get("p1").map_or(0, |d| d.millirad);
    assert!(dose > 0, "sem dose");
    assert_eq!(g.ready().world.bombs.zones.len(), 1);
    assert!(g.server.area_rate("p1") > 0);
    // A zona soma dose a cada segundo, sem depender da amostra do cliente.
    // (Passada a carência de entrada; o pulso já deu o teto de 2.500 RAD:
    // zera para medir.)
    g.run(crate::radiation::GRACE_TICKS as usize);
    g.ready().world.radiation.insert("p1".into(), Default::default());
    let before = 0;
    g.run(40);
    let after = g.ready().world.radiation.get("p1").map_or(0, |d| d.millirad);
    assert!(after > before, "a zona não somou dose: {before} → {after}");
    // Criativo: nenhuma radiação de área.
    set_creative(&mut g, true);
    g.run(1);
    assert_eq!(g.server.area_rate("p1"), 0);
    // The components are spent: nothing dropped.
    assert!(g.world.borrow().dropped.is_empty());
    // Nothing of the explosion left in the storage.
    assert!(g.world.borrow().kv.map.keys().all(|k| !k.starts_with("nx.")), "{:?}", g.world.borrow().kv.map.keys());
}

#[test]
fn a_charge_burns_its_fuse_then_digs_like_vanilla() {
    let mut g = bomb_game(10);
    let at = [0, 70, 0];
    let id = "ntm:dynamite";
    place_bomb(&mut g, at, id);
    power(&mut g, at, true);
    g.run(2);
    let fuse = bomb_data(&mut g, at).and_then(|b| b.fuse);
    assert!(fuse.is_some_and(|f| f > 60), "{fuse:?}");
    g.run(bomb::FUSE_TICKS as usize + 4);
    assert!(bomb_data(&mut g, at).is_none());
    let w = g.world.borrow();
    assert_eq!(w.get(OW, [0, 69, 0]), "minecraft:air");
    assert_eq!(w.get(OW, [0, 60, 0]), "minecraft:stone", "fundo da carga");
    assert!(g.server.blasts.nukes.is_empty());
}

#[test]
fn the_detonator_links_and_sets_off_a_ready_bomb() {
    let mut g = bomb_game(8);
    let at = [4, 70, 0];
    let id = "ntm:nuke_n2";
    place_bomb(&mut g, at, id);
    let p = player();
    // Unlinked: says so.
    let said = g.server.detonator(&mut g.sh, &p, None, false);
    assert!(said.contains("sem bomba"), "{said}");
    let said = g.server.detonator(&mut g.sh, &p, Some(at), true);
    assert!(said.contains("Detonador ligado"), "{said}");
    let said = g.server.detonator(&mut g.sh, &p, None, false);
    assert!(said.contains("não está pronta"), "{said}");
    assemble(&mut g, at, id, 0..12);
    let said = g.server.detonator(&mut g.sh, &p, None, false);
    assert!(said.contains("Detonada"), "{said}");
    g.run(3);
    assert!(bomb_data(&mut g, at).is_none());
    until_quiet(&mut g, 2000);
    // N2: no fallout, no slag, no radiation.
    let w = g.world.borrow();
    assert!(!w.blocks.values().any(|s| s.starts_with("ntm:sellafield") || s.starts_with("ntm:fallout")));
    drop(w);
    assert!(g.ready().world.bombs.zones.is_empty());
}

#[test]
fn breaking_a_bomb_leaves_its_parts_to_its_slots() {
    let mut g = bomb_game(8);
    let at = [2, 70, 0];
    let id = "ntm:nuke_man";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..2);
    assert_eq!(bomb_data(&mut g, at).unwrap().slots.iter().flatten().count(), 2);
    // The game spills the slots; the server drops nothing more.
    g.world.borrow_mut().engine.remove(&(OW.to_string(), at));
    g.break_block(at);
    g.run(40);
    assert!(g.world.borrow().dropped.is_empty(), "{:?}", g.world.borrow().dropped);
    assert!(bomb_data(&mut g, at).is_none());
}

#[test]
fn an_explosion_resumes_after_a_restart() {
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone());
    g.run(10);
    floor(&mut g, 24);
    let who = player();
    g.server.bomb_command(&mut g.sh, Some(&who), "config mundo raiomax 14").unwrap();
    g.server.bomb_command(&mut g.sh, Some(&who), "teste 14").unwrap();
    g.run(2);
    assert!(!g.server.blasts.nukes.is_empty());
    assert!(w.borrow().kv.map.keys().any(|k| k.starts_with("nx.")), "sem diário");
    drop(g);
    // Back in the same world: the explosion (in the fallout already) comes
    // back from the storage and finishes.
    let mut g = Game::new(w.clone());
    let mut resumed = false;
    for _ in 0..200 {
        g.run(1);
        resumed |= !g.server.blasts.nukes.is_empty();
        if resumed && g.server.blasts.nukes.is_empty() {
            break;
        }
    }
    assert!(resumed, "não retomou");
    until_quiet(&mut g, 3000);
    assert!((60..70).filter(|&y| w.borrow().get(OW, [0, y, 0]) == "minecraft:air").count() >= 3);
    assert!(w.borrow().kv.map.keys().all(|k| !k.starts_with("nx.")));
}

#[test]
fn settings_are_global_or_per_world() {
    let mut g = bomb_game(8);
    let who = player();
    let s = g.server.bomb_command(&mut g.sh, Some(&who), "config").unwrap();
    assert!(s.contains("raiomax 8") && s.contains("do mundo"), "{s}");
    let s = g.server.bomb_command(&mut g.sh, Some(&who), "config global modo aprimorado").unwrap();
    assert!(s.contains("modo aprimorado"), "{s}");
    // The world's own settings still win.
    assert!(g.server.bomb_config(&mut g.sh).mode == crate::blast::Mode::Otimizado);
    g.server.bomb_command(&mut g.sh, Some(&who), "config mundo limpar").unwrap();
    assert!(g.server.bomb_config(&mut g.sh).mode == crate::blast::Mode::Aprimorado);
    assert!(g.server.bomb_command(&mut g.sh, Some(&who), "config mundo efeitos talvez").is_err());
    g.server.bomb_command(&mut g.sh, Some(&who), "config global raiomax 6").unwrap();
    g.server.bomb_command(&mut g.sh, Some(&who), "teste 6").unwrap();
    g.run(3);
    until_quiet(&mut g, 2000);
    // The global settings live in the storage; a new server reads them.
    assert!(g.world.borrow().kv.map.contains_key(crate::blast::config::KEY));
}

/// The native detonator: used on a bomb it links, used anywhere else (the
/// air, another block) it sets the linked bomb off; a player not in the
/// world gets nothing.
#[test]
fn the_native_detonator_links_on_a_bomb_and_fires_elsewhere() {
    let mut g = bomb_game(8);
    let at = [4, 70, 0];
    let id = "ntm:nuke_n2";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..12);
    assert_eq!(g.server.native_detonator(&mut g.sh, "nobody", None), None);
    let said = g.server.native_detonator(&mut g.sh, "p1", Some(at)).unwrap();
    assert!(said.contains("Detonador ligado"), "{said}");
    assert!(bomb_data(&mut g, at).is_some());
    let said = g.server.native_detonator(&mut g.sh, "p1", Some([0, 69, 0])).unwrap();
    assert!(said.contains("Detonada"), "{said}");
    assert!(bomb_data(&mut g, at).is_none());
}
